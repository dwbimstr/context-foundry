//! Explicit task snapshots at the gateway's final request boundary.
//! Live resolution uses the existing MCP owner; the gateway never opens its store.
use crate::{FResult, FoundryError, digest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(crate) mod live;

pub const MAX_PACKET_BYTES: usize = 1024 * 1024;
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
fn invalid() -> FoundryError {
    FoundryError::InvalidArgument("invalid or mismatched exported task view binding".into())
}
fn id(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub v: u8,
    pub entry_id: String,
    pub tool_call_id: String,
    pub original_sha256: String,
    pub view_sha256: String,
    pub max_request_bytes: usize,
    pub view: Value,
}
impl Packet {
    pub(crate) fn new(
        view: Value,
        entry_id: String,
        tool_call_id: String,
        original_sha256: String,
        max_request_bytes: usize,
    ) -> FResult<Self> {
        let packet = Self {
            v: 1,
            entry_id,
            tool_call_id,
            original_sha256,
            view_sha256: digest(&serde_json::to_vec(&view)?),
            max_request_bytes,
            view,
        };
        packet.validate()?;
        Ok(packet)
    }
    pub fn validate(&self) -> FResult<()> {
        let view = &self.view;
        if self.v != 1
            || !id(&self.entry_id)
            || !id(&self.tool_call_id)
            || !hash(&self.original_sha256)
            || !hash(&self.view_sha256)
            || !(256..=MAX_BODY_BYTES).contains(&self.max_request_bytes)
            || crate::json_len(self)? > MAX_PACKET_BYTES
            || digest(&serde_json::to_vec(view)?) != self.view_sha256
            || !view["workspace_id"].as_str().is_some_and(id)
            || !view["task_id"].as_str().is_some_and(id)
            || view["revision"].as_u64().is_none_or(|n| n == 0)
            || view["boundary"] != "foundry_task_context"
            || view["host_application"] != "unknown"
        {
            return Err(invalid());
        }
        let entries = view["entries"].as_array().ok_or_else(invalid)?;
        let matching: Vec<_> = entries
            .iter()
            .filter(|e| e["id"] == self.entry_id)
            .collect();
        if entries.len() > 128
            || entries.iter().any(|e| {
                matches!(e["kind"].as_str(), Some("requirement" | "constraint"))
                    && (e["representation"] != "original"
                        || !e["text"]
                            .as_str()
                            .is_some_and(|t| e["sha256"] == digest(t.as_bytes())))
            })
            || matching.len() != 1
            || matching[0]["kind"] != "observation"
            || matching[0]["representation"] != "caller_summary"
            || matching[0]["sha256"] != self.original_sha256
        {
            return Err(invalid());
        }
        Ok(())
    }
    fn content(&self, live: bool) -> FResult<String> {
        let content = if live {
            json!({"scope":"live_task_snapshot","source_freshness":"indexed_at_resolution","view_sha256":self.view_sha256,"view_at_resolution":self.view})
        } else {
            json!({"scope":"exported_task_snapshot","source_freshness":"unchecked_at_send","view_sha256":self.view_sha256,"view_at_export":self.view})
        };
        Ok(serde_json::to_string(&content)?)
    }
    /// Caller has strictly parsed and validated the host profile. No mutation
    /// occurs until uniqueness, linkage and original content have been checked.
    pub(crate) fn apply(
        &self,
        incoming: &[u8],
        request: Value,
    ) -> FResult<(Vec<u8>, Option<AppliedView>)> {
        self.apply_resolved(incoming, request, false)
    }
    pub(crate) fn apply_resolved(
        &self,
        incoming: &[u8],
        mut request: Value,
        live: bool,
    ) -> FResult<(Vec<u8>, Option<AppliedView>)> {
        let messages = request["messages"].as_array_mut().ok_or_else(invalid)?;
        let results: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m["role"] == "tool" && m["tool_call_id"] == self.tool_call_id)
            .map(|(i, _)| i)
            .collect();
        if results.is_empty() {
            if incoming.len() > self.max_request_bytes {
                return Err(invalid());
            }
            return Ok((incoming.to_vec(), None));
        }
        if results.len() != 1 {
            return Err(invalid());
        }
        let index = results[0];
        let calls: Vec<usize> = messages
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                m.get("tool_calls")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|c| c["id"] == self.tool_call_id)
                    .map(move |_| i)
            })
            .collect();
        if calls.len() != 1 || calls[0] >= index || messages[calls[0]]["role"] != "assistant" {
            return Err(invalid());
        }
        let original = messages[index]["content"].as_str().ok_or_else(invalid)?;
        let content = self.content(live)?;
        if digest(original.as_bytes()) != self.original_sha256 && original != content {
            return Err(invalid());
        }
        messages[index]["content"] = Value::String(content);
        let body = serde_json::to_vec(&request)?;
        if body.len() > self.max_request_bytes {
            return Err(invalid());
        }
        let receipt = AppliedView {
            boundary: "gateway_upstream_body".into(),
            workspace_id: self.view["workspace_id"].as_str().unwrap().into(),
            task_id: self.view["task_id"].as_str().unwrap().into(),
            revision: self.view["revision"].as_u64().unwrap(),
            entry_id: self.entry_id.clone(),
            tool_call_id: self.tool_call_id.clone(),
            view_sha256: self.view_sha256.clone(),
            incoming_sha256: digest(incoming),
            request_sha256: digest(&body),
            incoming_bytes: incoming.len() as u64,
            request_bytes: body.len() as u64,
            synchronization: live.then(|| "owner_snapshot".into()),
        };
        Ok((body, Some(receipt)))
    }
}

/// Serialization evidence for one attempted request, not a provider-consumption
/// assertion. Source text and summaries are deliberately absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synchronization: Option<String>,
    pub boundary: String,
    pub workspace_id: String,
    pub task_id: String,
    pub revision: u64,
    pub entry_id: String,
    pub tool_call_id: String,
    pub view_sha256: String,
    pub incoming_sha256: String,
    pub request_sha256: String,
    pub incoming_bytes: u64,
    pub request_bytes: u64,
}
impl AppliedView {
    pub(crate) fn validate(&self) -> FResult<()> {
        if self.boundary != "gateway_upstream_body"
            || self
                .synchronization
                .as_deref()
                .is_some_and(|s| s != "owner_snapshot")
            || self.revision == 0
            || [
                &self.workspace_id,
                &self.task_id,
                &self.entry_id,
                &self.tool_call_id,
            ]
            .iter()
            .any(|s| !id(s))
            || [
                &self.view_sha256,
                &self.incoming_sha256,
                &self.request_sha256,
            ]
            .iter()
            .any(|s| !hash(s))
            || !(1..=MAX_BODY_BYTES as u64).contains(&self.incoming_bytes)
            || !(1..=MAX_BODY_BYTES as u64).contains(&self.request_bytes)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(cap: usize) -> Packet {
        let original_sha256 = digest(b"original");
        Packet::new(json!({"workspace_id":"workspace","task_id":"task","revision":2,"boundary":"foundry_task_context","host_application":"unknown","entries":[{"id":"entry","kind":"observation","representation":"caller_summary","text":"folded","sha256":original_sha256},{"id":"keep","kind":"constraint","representation":"original","text":"keep","sha256":digest(b"keep")}]}),"entry".into(),"call".into(),original_sha256,cap).unwrap()
    }
    #[test]
    fn ceiling_applies_to_the_final_serialized_request_and_absent_carriers_stay_byte_exact() {
        let packet = packet(512);
        let request = json!({"messages":[{"role":"assistant","tool_calls":[{"id":"call"}]},{"role":"tool","tool_call_id":"call","content":"original"}]});
        let input = serde_json::to_vec(&request).unwrap();
        assert!(input.len() < 512);
        assert!(
            packet.apply(&input, request).is_err(),
            "the replacement and JSON escaping count toward the ceiling"
        );
        let input = b"{ \"messages\": [] }\n";
        let (sent, applied) = packet
            .apply(input, serde_json::from_slice(input).unwrap())
            .unwrap();
        assert_eq!(sent, input);
        assert!(applied.is_none());
        let request = json!({"messages":[{"role":"user","content":"x".repeat(513)}]});
        assert!(
            packet
                .apply(&serde_json::to_vec(&request).unwrap(), request)
                .is_err()
        );
    }
    #[test]
    fn altered_views_and_folded_protected_entries_are_invalid() {
        let mut p = packet(4096);
        p.view["revision"] = json!(3);
        assert!(p.validate().is_err());
        p.view_sha256 = digest(&serde_json::to_vec(&p.view).unwrap());
        p.validate().unwrap();
        p.view["entries"][1]["representation"] = json!("caller_summary");
        p.view_sha256 = digest(&serde_json::to_vec(&p.view).unwrap());
        assert!(p.validate().is_err());
    }
}
