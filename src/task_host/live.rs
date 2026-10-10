//! One bounded read from the existing HTTP MCP owner before each model request.
//! No stale fallback, transcript ingestion, additional owner or database handle.
use super::{Packet, invalid};
use crate::FResult;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    pub url: String,
    pub token_env: String,
}
impl Config {
    pub fn validate(&self) -> FResult<()> {
        let url = reqwest::Url::parse(&self.url).map_err(|_| invalid())?;
        let mut name = self.token_env.bytes();
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none_or(|p| p == 0)
            || url.path() != "/mcp"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || self.url.len() > 256
            || self.token_env.len() > 256
            || !name
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            || !name.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(invalid());
        }
        Ok(())
    }
}
struct State {
    client: Option<RunningService<rmcp::RoleClient, ()>>,
    revision: u64,
}
pub(crate) struct Owner {
    config: Config,
    token: String,
    http: reqwest::Client,
    state: Mutex<State>,
}
impl Owner {
    pub fn new(config: Config, anchor: &Packet) -> FResult<Self> {
        config.validate()?;
        let token = std::env::var(&config.token_env).map_err(|_| invalid())?;
        if token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(invalid());
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(|_| invalid())?;
        Ok(Self {
            config,
            token,
            http,
            state: Mutex::new(State {
                client: None,
                revision: anchor.view["revision"].as_u64().ok_or_else(invalid)?,
            }),
        })
    }
    pub async fn resolve(&self, anchor: &Packet) -> FResult<Packet> {
        let mut state = self.state.lock().await;
        // Own the SDK service for the duration of this read. Cancellation of
        // this future drops it too, so a shorter gateway deadline cannot leave
        // an outstanding read attached to the next request's session.
        let mut client = state.client.take();
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            if client.is_none() {
                let mut config =
                    StreamableHttpClientTransportConfig::with_uri(self.config.url.clone())
                        .auth_header(self.token.clone());
                config.max_concurrent_requests = 1;
                config.control_request_timeout = Duration::from_secs(2);
                config.session_recovery_timeout = Duration::from_secs(2);
                // Our owner sends bounded SSE tool replies. JSON initialization
                // replies are trusted owner protocol metadata, never provider data.
                config.max_sse_event_size = 2 * super::MAX_PACKET_BYTES;
                let transport =
                    StreamableHttpClientTransport::with_client(self.http.clone(), config);
                client = Some(().serve(transport).await.map_err(|_| invalid())?);
            }
            let args = json!({"workspace_id":anchor.view["workspace_id"],
                "task_id":anchor.view["task_id"],"command":{"op":"render","max_bytes":262144}});
            let reply = client
                .as_ref()
                .unwrap()
                .call_tool(
                    CallToolRequestParams::new("task_context")
                        .with_arguments(args.as_object().unwrap().clone()),
                )
                .await
                .map_err(|_| invalid())?;
            if reply.is_error == Some(true) || reply.content.len() != 1 {
                return Err(invalid());
            }
            let text = reply.content[0].as_text().ok_or_else(invalid)?;
            if text.text.len() > 262144 {
                return Err(invalid());
            }
            let view: serde_json::Value =
                serde_json::from_str(&text.text).map_err(|_| invalid())?;
            if view["workspace_id"] != anchor.view["workspace_id"]
                || view["task_id"] != anchor.view["task_id"]
                || view["revision"].as_u64().is_none_or(|r| r < state.revision)
            {
                return Err(invalid());
            }
            let packet = Packet::new(
                view,
                anchor.entry_id.clone(),
                anchor.tool_call_id.clone(),
                anchor.original_sha256.clone(),
                anchor.max_request_bytes,
            )?;
            state.revision = packet.view["revision"].as_u64().unwrap();
            Ok(packet)
        })
        .await
        .unwrap_or_else(|_| Err(invalid()));
        if result.is_ok() {
            state.client = client;
        } else {
            // Drop cancels the SDK service. Retry establishes a new owner session;
            // a failed/expired owner never authorizes the frozen startup snapshot.
            drop(client);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_admission_requires_an_exact_local_endpoint_and_named_credential() {
        let valid = Config {
            url: "http://127.0.0.1:9633/mcp".into(),
            token_env: "FOUNDRY_MCP_TOKEN".into(),
        };
        valid.validate().unwrap();
        for url in [
            "https://example.com/mcp",
            "http://localhost:9633/mcp",
            "http://127.0.0.1:9633/mcp?secret=x",
            "http://user@127.0.0.1:9633/mcp",
            "http://127.0.0.1:0/mcp",
            "http://127.0.0.1:9633/other",
        ] {
            assert!(
                Config {
                    url: url.into(),
                    ..valid.clone()
                }
                .validate()
                .is_err(),
                "{url}"
            );
        }
        assert!(
            Config {
                token_env: "not a name".into(),
                ..valid
            }
            .validate()
            .is_err()
        );
    }
}
