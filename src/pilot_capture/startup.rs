//! Startup-only revision 2: durable refusals, never generation admission.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{CaptureFault, CaptureSession, Journal, claim_run_dir, host, protocol, runtime};
use crate::neural::protocol::{FrameError, FrameHeader, read_frame_as, write_frame_as};
use protocol::{ChannelIdentity, FaultClass, ProtocolFault};

pub const MAX_ATTEMPTS: u32 = 20;
pub const OBSERVATION_SECONDS: u64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    ModelConnectionWarmup,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    HEAD,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    Https,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    AnthropicOrigin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
    Unclassified,
    Protocol,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChildMessage {
    StartupAttempt {
        attempt: u32,
        owner: Owner,
        method: Method,
        scheme: Scheme,
        hostname: String,
        port: u16,
        route: Route,
    },
    /// Only the SDK's original warm-up caller join has completed. Other owners
    /// remain unqualified; this is intentionally not full startup settlement.
    OwnerSettled {
        owner: Owner,
    },
    RestrictionsSettled {
        tool_names: Vec<String>,
        mcp_disabled: bool,
        memory_disabled: bool,
        credential_usage_joined: bool,
    },
    Fault {
        code: Fault,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParentMessage {
    StartupRefused {
        attempt: u32,
        journal_seq: u64,
        journal_sha256: String,
    },
    Close {
        reason: protocol::CloseReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildHeader {
    pub v: u32,
    pub session: String,
    pub host: String,
    pub seq: u32,
    pub msg: ChildMessage,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParentHeader {
    pub v: u32,
    pub session: String,
    pub host: String,
    pub seq: u32,
    pub msg: ParentMessage,
}
impl FrameHeader for ChildHeader {
    const PROTOCOL: u32 = 2;
    const MAX_PAYLOAD_BYTES: usize = 0;
    fn protocol(&self) -> u32 {
        self.v
    }
}
impl FrameHeader for ParentHeader {
    const PROTOCOL: u32 = 2;
    const MAX_PAYLOAD_BYTES: usize = 0;
    fn protocol(&self) -> u32 {
        self.v
    }
}

#[derive(Debug)]
pub struct Inbound {
    pub seq: u32,
    pub msg: ChildMessage,
}

/// One instance per direction, reusing the existing bounded frame codec.
pub(crate) struct ChannelState {
    identity: ChannelIdentity,
    next: u32,
}
impl ChannelState {
    pub(crate) fn new(identity: ChannelIdentity) -> Self {
        Self { identity, next: 1 }
    }
    pub(crate) fn read<R: Read>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<Inbound>, ProtocolFault> {
        let (h, _) = match read_frame_as::<ChildHeader, R>(reader) {
            Ok(value) => value,
            Err(FrameError::Eof) => return Ok(None),
            Err(_) => return Err(ProtocolFault("startup_frame_invalid")),
        };
        if h.session != self.identity.session || h.host != self.identity.host || h.seq != self.next
        {
            return Err(ProtocolFault("startup_frame_identity_or_sequence"));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ProtocolFault("startup_sequence_exhausted"))?;
        if let ChildMessage::StartupAttempt {
            attempt,
            hostname,
            port,
            ..
        } = &h.msg
            && (!(1..=MAX_ATTEMPTS).contains(attempt)
                || hostname != "api.anthropic.com"
                || *port != 443)
        {
            return Err(ProtocolFault("startup_route_unclassified"));
        }
        if let ChildMessage::RestrictionsSettled {
            tool_names,
            mcp_disabled,
            memory_disabled,
            credential_usage_joined,
        } = &h.msg
            && (!mcp_disabled
                || !memory_disabled
                || !credential_usage_joined
                || tool_names != &["edit", "read"])
        {
            return Err(ProtocolFault("startup_restrictions_unqualified"));
        }
        Ok(Some(Inbound {
            seq: h.seq,
            msg: h.msg,
        }))
    }
    pub(crate) fn write<W: Write>(
        &mut self,
        writer: &mut W,
        msg: ParentMessage,
    ) -> Result<(), ProtocolFault> {
        let seq = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ProtocolFault("startup_sequence_exhausted"))?;
        let header = ParentHeader {
            v: 2,
            session: self.identity.session.clone(),
            host: self.identity.host.clone(),
            seq,
            msg,
        };
        write_frame_as(writer, &header, &[])
            .map_err(|_| ProtocolFault("startup_channel_write_failed"))
    }
}

pub struct ObservationAdmission {
    pub run_dir: PathBuf,
    pub omp: PathBuf,
    pub workspace: PathBuf,
    pub faults: super::VerificationFaults,
}

/// This is an observed refusal receipt, never a compatibility or execution seal.
#[derive(Debug, Serialize)]
pub struct ObservationReceipt {
    pub attempts: u32,
    pub terminal_code: &'static str,
    pub journal_records: u64,
    pub journal_sha256: String,
    pub exit: ObservationExit,
}
#[derive(Debug, Serialize)]
pub struct ObservationExit {
    pub normal: bool,
    pub forced: bool,
    pub group_gone: bool,
}

fn fault(class: FaultClass, code: &'static str) -> CaptureFault {
    CaptureFault::new(class, code)
}

impl CaptureSession {
    /// Observe the owned startup seam without constructing a provider transport,
    /// counters or permits. An incomplete inventory always ends in durable HOLD.
    pub fn observe_startup(
        admission: ObservationAdmission,
    ) -> Result<ObservationReceipt, CaptureFault> {
        runtime()?.block_on(observe(admission))
    }
}

async fn observe(a: ObservationAdmission) -> Result<ObservationReceipt, CaptureFault> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(OBSERVATION_SECONDS);
    let (run_dir, hosts_dir) = claim_run_dir(&a.run_dir)?;
    let mut journal = Journal::create(&run_dir.join("journal.jsonl"), a.faults.journal_fail_after)?;
    let identity = ChannelIdentity::new(
        format!("s-{}", uuid::Uuid::new_v4().simple()),
        "startup-owner".into(),
    )
    .map_err(|e| fault(FaultClass::Protocol, e.0))?;
    journal.append("startup_observation", json!({"session":identity.session,"host":identity.host,"omp_sha256":host::STARTUP_OBSERVATION_OMP_SHA256,"inventory_qualified":false}))?;
    let spec = host::HostSpec {
        omp: a.omp,
        root: hosts_dir.join("startup-owner"),
        workspace: a.workspace,
        real_home: super::real_home()?,
        deny: vec![run_dir.clone()],
        deny_literals: vec![],
        identity,
        catalog: host::CatalogProfile::StartupObservation,
    };
    let (host, mut events) =
        host::Host::launch(&spec).map_err(|e| journal.admission_refusal("startup_launch", e))?;
    let mut attempts = 0;
    let mut warmup_done = false;
    let mut restrictions_done = false;
    let result: Result<(), CaptureFault> = tokio::time::timeout_at(deadline, async {
        journal.append("host_launched", json!({"pid":host.launch.pid,"bridge_sha256":host.launch.bridge_sha256,"policy_sha256":host.launch.policy_sha256}))?;
        loop {
            match events.recv().await {
                Some(host::HostEvent::Startup(inbound)) => match inbound.msg {
                    ChildMessage::StartupAttempt { attempt, .. } => {
                        if attempt != attempts + 1 || attempts == MAX_ATTEMPTS { return Err(fault(FaultClass::Protocol,"startup_attempt_order")); }
                        journal.append("startup_refused", json!({"child_seq":inbound.seq,"attempt":inbound.msg}))?;
                        // Sync happened in append; attest exactly the committed prefix
                        // before the response can reach the child.
                        let digest = journal.digest()?;
                        host.send_startup(ParentMessage::StartupRefused { attempt, journal_seq:journal.records,journal_sha256:digest }).await?;
                        attempts += 1;
                    }
                    ChildMessage::OwnerSettled { .. } => {
                        if attempts == 0 || warmup_done { return Err(fault(FaultClass::Protocol,"startup_owner_order")); }
                        journal.append("startup_owner_settled",json!({"child_seq":inbound.seq,"owner":Owner::ModelConnectionWarmup}))?;
                        warmup_done = true;
                    }
                    ChildMessage::RestrictionsSettled { .. } => {
                        if restrictions_done || !warmup_done { return Err(fault(FaultClass::Protocol,"startup_restrictions_order")); }
                        journal.append("startup_restrictions_settled",json!({"child_seq":inbound.seq,"restrictions":inbound.msg}))?;
                        restrictions_done = true;
                    }
                    ChildMessage::Fault { .. } => return Err(fault(FaultClass::Protocol,"startup_child_fault")),
                },
                Some(host::HostEvent::Rpc(host::RpcEvent::Ready | host::RpcEvent::Other)) => {},
                Some(host::HostEvent::BridgeFault(e)) => return Err(fault(FaultClass::Protocol,e.0)),
                _ => return Err(fault(FaultClass::Protocol,"startup_unexpected_event")),
            }
            if warmup_done && restrictions_done { return Err(fault(FaultClass::Authority,"startup_inventory_unqualified")); }
        }
    }).await.unwrap_or_else(|_|Err(fault(FaultClass::Deadline,"startup_observation_deadline")));
    let terminal = result
        .err()
        .unwrap_or_else(|| fault(FaultClass::Authority, "startup_admission_unavailable"));
    let terminal = journal.admission_refusal("startup_observation", terminal);
    // Always reap, including append/readback/write failures. Close grants no IO.
    let _ = host
        .send_startup(ParentMessage::Close {
            reason: protocol::CloseReason::Halted,
        })
        .await;
    let (drained, exit) = host.finish(events, Duration::from_secs(30)).await;
    if terminal.class == FaultClass::Persistence {
        return Err(terminal);
    }
    journal.append("startup_host_exit",json!({"normal":exit.normal,"forced":exit.forced,"group_gone":exit.group_gone,"stderr_bytes":exit.stderr_bytes,"stderr_sha256":exit.stderr_sha256}))?;
    if terminal.code != "startup_inventory_unqualified" {
        return Err(terminal);
    }
    if !exit.normal
        || exit.forced
        || !exit.group_gone
        || drained.iter().any(|e| {
            matches!(
                e,
                host::HostEvent::Startup(_)
                    | host::HostEvent::Bridge(_)
                    | host::HostEvent::BridgeFault(_)
                    | host::HostEvent::BridgeWriteFailed(_)
            )
        })
    {
        return Err(journal.admission_refusal(
            "startup_cleanup",
            fault(FaultClass::Cleanup, "startup_shutdown_incomplete"),
        ));
    }
    Ok(ObservationReceipt {
        attempts,
        terminal_code: terminal.code,
        journal_records: journal.records,
        journal_sha256: journal.digest()?,
        exit: ObservationExit {
            normal: exit.normal,
            forced: exit.forced,
            group_gone: exit.group_gone,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::protocol::write_frame_as;
    fn identity() -> ChannelIdentity {
        ChannelIdentity::new("startup-test".into(), "owner-test".into()).unwrap()
    }
    fn attempt() -> ChildMessage {
        ChildMessage::StartupAttempt {
            attempt: 1,
            owner: Owner::ModelConnectionWarmup,
            method: Method::HEAD,
            scheme: Scheme::Https,
            hostname: "api.anthropic.com".into(),
            port: 443,
            route: Route::AnthropicOrigin,
        }
    }
    fn frame(seq: u32, msg: ChildMessage) -> Vec<u8> {
        let id = identity();
        let h = ChildHeader {
            v: 2,
            session: id.session,
            host: id.host,
            seq,
            msg,
        };
        let mut out = Vec::new();
        write_frame_as(&mut out, &h, &[]).unwrap();
        out
    }
    #[test]
    fn startup_metadata_is_closed_and_cannot_decode_ready_or_generation() {
        for message in [
            json!({"type":"ready"}),
            json!({"type":"fetch_begin","fetch":1}),
            json!({"type":"owner_settled","owner":"other"}),
            json!({"type":"owner_settled","owner":"model_connection_warmup","extra":true}),
        ] {
            assert!(serde_json::from_value::<ChildMessage>(message).is_err());
        }
        let mut channel = ChannelState::new(identity());
        let bytes = frame(1, attempt());
        assert_eq!(channel.read(&mut bytes.as_slice()).unwrap().unwrap().seq, 1);
        assert!(channel.read(&mut bytes.as_slice()).is_err());
    }
    #[test]
    fn startup_framing_refuses_truncation_identity_version_payload_and_exhaustion() {
        let good = frame(1, attempt());
        for end in 1..good.len() {
            assert!(
                ChannelState::new(identity())
                    .read(&mut &good[..end])
                    .is_err(),
                "end={end}"
            );
        }
        let mut stale = identity();
        stale.host = "other".into();
        assert!(ChannelState::new(stale).read(&mut good.as_slice()).is_err());
        let mut exhausted = ChannelState::new(identity());
        exhausted.next = u32::MAX;
        assert!(
            exhausted
                .write(
                    &mut Vec::new(),
                    ParentMessage::Close {
                        reason: protocol::CloseReason::Halted
                    }
                )
                .is_err()
        );
        let mut wrong_version = good.clone();
        let at = wrong_version
            .windows(5)
            .position(|w| w == b"\"v\":2")
            .unwrap();
        wrong_version[at + 4] = b'1';
        assert!(
            ChannelState::new(identity())
                .read(&mut wrong_version.as_slice())
                .is_err()
        );
        let mut body = good.clone();
        let n = body.len();
        body[n - 4..].copy_from_slice(&1u32.to_le_bytes());
        body.push(0);
        assert!(
            ChannelState::new(identity())
                .read(&mut body.as_slice())
                .is_err()
        );
    }
    #[test]
    fn startup_restrictions_require_all_owner_facts_and_the_exact_tools() {
        for (mcp_disabled, memory_disabled, credential_usage_joined, tool_names) in [
            (false, true, true, vec!["edit".into(), "read".into()]),
            (true, false, true, vec!["edit".into(), "read".into()]),
            (true, true, false, vec!["edit".into(), "read".into()]),
            (true, true, true, vec!["read".into()]),
        ] {
            let message = ChildMessage::RestrictionsSettled {
                mcp_disabled,
                memory_disabled,
                credential_usage_joined,
                tool_names,
            };
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, message).as_slice())
                    .is_err()
            );
        }
        let message = ChildMessage::RestrictionsSettled {
            mcp_disabled: true,
            memory_disabled: true,
            credential_usage_joined: true,
            tool_names: vec!["edit".into(), "read".into()],
        };
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, message).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_routes_and_attempt_ids_fail_closed() {
        for (id, hostname, port) in [
            (0, "api.anthropic.com", 443),
            (21, "api.anthropic.com", 443),
            (1, "api.anthropic.com?secret", 443),
            (1, "other", 443),
            (1, "api.anthropic.com", 80),
        ] {
            let mut message = attempt();
            if let ChildMessage::StartupAttempt {
                attempt,
                hostname: host,
                port: p,
                ..
            } = &mut message
            {
                *attempt = id;
                *host = hostname.into();
                *p = port;
            }
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, message).as_slice())
                    .is_err()
            );
        }
    }
}
