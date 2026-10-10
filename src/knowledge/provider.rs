//! Explicit OpenAI-compatible Chat Completions boundary. No ambient endpoint,
//! proxy, redirects, retries or process launch. Provider usage is kept separate
//! from Foundry's o200k request admission units.
use super::*;
use futures_util::StreamExt;
use std::future::Future;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}
pub struct Reply {
    pub content: String,
    pub usage: Usage,
}
fn transport_error() -> FoundryError {
    fail(
        "knowledge_provider_failed",
        "generation request failed; provider details are not echoed",
    )
}
async fn controlled<T>(control: &Control, future: impl Future<Output = FResult<T>>) -> FResult<T> {
    tokio::pin!(future);
    loop {
        control.check()?;
        tokio::select! {
            result = &mut future => { control.check()?; return result; }
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
    }
}
pub struct HttpProvider {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    bearer: Option<String>,
}
impl HttpProvider {
    pub fn new(profile: &Profile) -> FResult<Self> {
        let endpoint = profile.endpoint()?;
        let bearer = profile
            .token_env
            .as_ref()
            .map(|key| {
                std::env::var(key).map_err(|_| {
                    fail(
                        "knowledge_credentials_missing",
                        "configured token environment variable is absent",
                    )
                })
            })
            .transpose()?;
        if bearer.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > 8192 || value.contains(['\r', '\n'])
        }) {
            return Err(fail(
                "knowledge_credentials_invalid",
                "configured token is invalid",
            ));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(profile.call_seconds))
            .build()
            .map_err(|_| transport_error())?;
        Ok(Self {
            client,
            endpoint,
            bearer,
        })
    }
    pub async fn generate(&self, body: Vec<u8>, model: &str, control: &Control) -> FResult<Reply> {
        controlled(control, async {
            let mut request = self
                .client
                .post(self.endpoint.clone())
                .header("content-type", "application/json")
                .body(body);
            if let Some(bearer) = &self.bearer {
                request = request.bearer_auth(bearer);
            }
            let response = request.send().await.map_err(|_| transport_error())?;
            if response.status().as_u16() == 429 || response.status().is_server_error() {
                return Err(fail(
                    "knowledge_provider_busy",
                    "generation provider refused the call; retry backoff recorded",
                ));
            }
            if !response.status().is_success() {
                return Err(transport_error());
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
            {
                return Err(fail(
                    "knowledge_output_limit",
                    "provider response exceeds 256 KiB",
                ));
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| transport_error())?;
                if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return Err(fail(
                        "knowledge_output_limit",
                        "provider response exceeds 256 KiB",
                    ));
                }
                bytes.extend_from_slice(&chunk);
            }
            let reply: Value = serde_json::from_slice(&bytes).map_err(|_| {
                fail(
                    "knowledge_provider_malformed",
                    "expected a non-streaming Chat Completions response",
                )
            })?;
            let choices = reply["choices"]
                .as_array()
                .filter(|c| c.len() == 1)
                .ok_or_else(|| {
                    fail(
                        "knowledge_provider_malformed",
                        "expected exactly one completion",
                    )
                })?;
            if reply["model"].as_str() != Some(model)
                || choices[0]["finish_reason"] != "stop"
                || !choices[0]["message"]["tool_calls"].is_null()
            {
                return Err(fail(
                    "knowledge_provider_malformed",
                    "model mismatch, incomplete answer or tool call",
                ));
            }
            let content = choices[0]["message"]["content"]
                .as_str()
                .ok_or_else(|| fail("knowledge_provider_malformed", "completion has no text"))?
                .to_owned();
            let usage = Usage {
                prompt_tokens: reply["usage"]["prompt_tokens"].as_u64(),
                completion_tokens: reply["usage"]["completion_tokens"].as_u64(),
            };
            Ok(Reply { content, usage })
        })
        .await
    }
}
