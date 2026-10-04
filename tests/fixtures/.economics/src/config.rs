//! Adapter budget policy configuration (003 adapter-economics contract v1).
//!
//! Host/project config v1: `max_context_tokens` (1..32768, default 2048),
//! optional `session_context_tokens` (positive), `tokenizer_id` (the core's
//! named o200k contract), and `scope` (`delivery` or `host_request`).
//! Host-request mode additionally requires `max_input_tokens`,
//! `max_output_tokens`, optional `session_provider_tokens` (positive checked
//! u64) and a pinned model/counting recipe. Delivery mode rejects those
//! host-only fields. Unknown or null fields refuse. An unknown tokenizer is
//! never replaced with an estimated fallback.

use crate::FoundryError;
use serde_json::{Map, Value};

pub const CONFIG_MAX_BYTES: usize = 64 * 1024;
pub const CORE_TOKENIZER: &str = "o200k_base";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Delivery,
    HostRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRequestConfig {
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub session_provider_tokens: Option<u64>,
    /// Pinned provider model and its counting recipe. Names only, never
    /// secrets; `provider_tokenizer_id` must be explicitly pinned because an
    /// unknown provider tokenizer is not replaced with chars/4.
    pub model_id: String,
    pub provider_tokenizer_id: String,
    pub counting_recipe: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetConfig {
    pub max_context_tokens: u64,
    pub session_context_tokens: Option<u64>,
    pub tokenizer_id: String,
    pub scope: Scope,
    /// Present only for `scope: host_request` configurations.
    pub host_request: Option<HostRequestConfig>,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 2048,
            session_context_tokens: None,
            tokenizer_id: CORE_TOKENIZER.to_owned(),
            scope: Scope::Delivery,
            host_request: None,
        }
    }
}

fn invalid(detail: &str) -> FoundryError {
    FoundryError::InvalidArgument(format!("budget config: {detail}"))
}

fn id_ok(v: &str) -> bool {
    !v.is_empty() && v.len() <= 256
}

fn positive(value: &Value, field: &str) -> Result<u64, FoundryError> {
    let Some(n) = value.as_u64() else {
        return Err(invalid(&format!("{field} must be a positive integer")));
    };
    if n == 0 {
        return Err(invalid(&format!("{field} must be a positive integer")));
    }
    Ok(n)
}

fn bounded(value: &Value, field: &str, max: u64) -> Result<u64, FoundryError> {
    let n = positive(value, field)?;
    if n > max {
        return Err(invalid(&format!("{field} must be 1..={max}")));
    }
    Ok(n)
}

impl BudgetConfig {
    /// Parse a strict config-v1 JSON object. Null, unknown fields, wrong
    /// types and out-of-range integers refuse the whole configuration.
    pub fn parse(bytes: &[u8]) -> Result<Self, FoundryError> {
        if bytes.len() > CONFIG_MAX_BYTES {
            return Err(invalid("config exceeds 64 KiB"));
        }
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| invalid(&format!("invalid JSON: {e}")))?;
        let Some(object) = value.as_object() else {
            return Err(invalid("config must be a JSON object"));
        };
        if object.len() != 1 || !object.contains_key("foundry_budget") {
            return Err(invalid(
                "config must contain exactly one `foundry_budget` object",
            ));
        }
        Self::from_object(&object["foundry_budget"])
    }

    pub fn from_object(value: &Value) -> Result<Self, FoundryError> {
        let Some(object) = value.as_object() else {
            return Err(invalid("`foundry_budget` must be an object"));
        };
        let allowed: [&str; 6] = [
            "v",
            "max_context_tokens",
            "session_context_tokens",
            "tokenizer_id",
            "scope",
            "host_request",
        ];
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(invalid(&format!("unknown field `{key}`")));
            }
        }
        if !object.contains_key("v") || object["v"].as_u64() != Some(1) {
            return Err(invalid("`v` must be 1"));
        }
        let max_context_tokens = match object.get("max_context_tokens") {
            None => 2048,
            Some(v) => bounded(v, "max_context_tokens", 32768)?,
        };
        let session_context_tokens = match object.get("session_context_tokens") {
            None => None,
            Some(v) => Some(positive(v, "session_context_tokens")?),
        };
        let tokenizer_id = match object.get("tokenizer_id") {
            None => CORE_TOKENIZER.to_owned(),
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err(invalid("tokenizer_id must be a string")),
        };
        if tokenizer_id != CORE_TOKENIZER {
            return Err(invalid(&format!(
                "unknown tokenizer `{tokenizer_id}`; the core contract is `{CORE_TOKENIZER}` and no estimated fallback exists"
            )));
        }
        let scope = match object.get("scope") {
            None => Scope::Delivery,
            Some(Value::String(s)) if s == "delivery" => Scope::Delivery,
            Some(Value::String(s)) if s == "host_request" => Scope::HostRequest,
            Some(_) => return Err(invalid("scope must be `delivery` or `host_request`")),
        };
        let host_request = match (object.get("host_request"), &scope) {
            (None, Scope::Delivery) => None,
            (Some(_), Scope::Delivery) => {
                // Delivery mode rejects host-only fields, including an
                // explicit null: optional means omitted, not null.
                return Err(invalid("host_request is rejected in delivery scope"));
            }
            (Some(Value::Null), Scope::HostRequest) => {
                return Err(invalid(
                    "host_request fields are required in host_request scope",
                ));
            }
            (None, Scope::HostRequest) => {
                return Err(invalid(
                    "host_request fields are required in host_request scope",
                ));
            }
            (Some(Value::Object(fields)), _) => {
                if scope == Scope::Delivery {
                    return Err(invalid(
                        "host_request fields are rejected in delivery scope",
                    ));
                }
                let field_allowed: [&str; 6] = [
                    "max_input_tokens",
                    "max_output_tokens",
                    "session_provider_tokens",
                    "model_id",
                    "provider_tokenizer_id",
                    "counting_recipe",
                ];
                for key in fields.keys() {
                    if !field_allowed.contains(&key.as_str()) {
                        return Err(invalid(&format!("unknown host_request field `{key}`")));
                    }
                }
                let max_input_tokens = fields
                    .get("max_input_tokens")
                    .ok_or_else(|| invalid("host_request.max_input_tokens is required"))?;
                let max_input_tokens = positive(max_input_tokens, "max_input_tokens")?;
                let max_output_tokens = fields
                    .get("max_output_tokens")
                    .ok_or_else(|| invalid("host_request.max_output_tokens is required"))?;
                let max_output_tokens = positive(max_output_tokens, "max_output_tokens")?;
                let session_provider_tokens = match fields.get("session_provider_tokens") {
                    None => None,
                    Some(v) => Some(positive(v, "session_provider_tokens")?),
                };
                let string_field = |name: &str| -> Result<String, FoundryError> {
                    match fields.get(name) {
                        Some(Value::String(s)) if id_ok(s) && !s.trim().is_empty() => Ok(s.clone()),
                        _ => Err(invalid(&format!(
                            "host_request.{name} must be a nonblank string of at most 256 bytes"
                        ))),
                    }
                };
                Some(HostRequestConfig {
                    max_input_tokens,
                    max_output_tokens,
                    session_provider_tokens,
                    model_id: string_field("model_id")?,
                    provider_tokenizer_id: string_field("provider_tokenizer_id")?,
                    counting_recipe: string_field("counting_recipe")?,
                })
            }
            (Some(_), _) => {
                return Err(invalid("host_request must be an object when present"));
            }
        };
        Ok(Self {
            max_context_tokens,
            session_context_tokens,
            tokenizer_id,
            scope,
            host_request,
        })
    }

    /// An MCP-only host has no complete-request hooks: a configured
    /// host-request scope is the named refusal `budget_scope_unsupported`,
    /// never a silent downgrade to a payload cap. Both startup paths call
    /// this before any store is opened.
    pub fn require_delivery(&self) -> Result<(), crate::adapter_error::AdapterError> {
        if self.scope != Scope::Delivery {
            return Err(crate::adapter_error::AdapterError::named(
                "budget_scope_unsupported",
                "full-request caps require actual host hooks; this MCP adapter enforces delivery scope only",
            ));
        }
        Ok(())
    }

    /// Effective context allowance is the minimum of the caller request, the
    /// configured context ceiling and the remaining connection/session
    /// allowance, all in the core's named tokenizer.
    pub fn effective_allowance(&self, caller_tokens: u64, remaining_session: Option<u64>) -> u64 {
        let mut allowance = caller_tokens.min(self.max_context_tokens);
        if let Some(remaining) = remaining_session {
            allowance = allowance.min(remaining);
        }
        allowance.max(1)
    }

    pub fn to_json(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("v".into(), Value::from(1));
        map.insert(
            "max_context_tokens".into(),
            Value::from(self.max_context_tokens),
        );
        if let Some(session) = self.session_context_tokens {
            map.insert("session_context_tokens".into(), Value::from(session));
        }
        map.insert(
            "tokenizer_id".into(),
            Value::String(self.tokenizer_id.clone()),
        );
        map.insert(
            "scope".into(),
            Value::String(
                match self.scope {
                    Scope::Delivery => "delivery",
                    Scope::HostRequest => "host_request",
                }
                .to_owned(),
            ),
        );
        map
    }
}
