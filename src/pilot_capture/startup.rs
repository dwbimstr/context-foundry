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
    CatalogRetired {
        discovery_retired: bool,
        originals_joined: bool,
        callbacks_joined: bool,
        timers_retired: bool,
        operations: u32,
        rejections: u32,
    },
    SettingsRetired {
        auth_listener_retired: bool,
        settings_retired: bool,
        originals_joined: bool,
        callbacks_joined: bool,
        timers_retired: bool,
        operations: u32,
        rejections: u32,
    },
    AuthRetired {
        empty_auth: bool,
        auth_retired: bool,
        originals_joined: bool,
        callbacks_joined: bool,
        store_closed: bool,
        operations: u32,
        rejections: u32,
    },
    PresenceRetired {
        presence_retired: bool,
        registration_joined: bool,
        callbacks_joined: bool,
        cleanup_joined: bool,
        presence_removed: bool,
    },
    TelemetryRetired {
        telemetry_disabled: bool,
        original_joined: bool,
        callbacks_joined: bool,
    },
    MarketplaceRetired {
        marketplace_disabled: bool,
        schedule_completed: bool,
    },
    LoggingRetired {
        logging_closed: bool,
        transport_closed: bool,
        retention_joined: bool,
    },
    PluginRootsRetired {
        plugin_roots_closed: bool,
        originals_joined: bool,
        callbacks_joined: bool,
        operations: u32,
    },
    ExtensionRetired {
        handlers_joined: bool,
        callbacks_joined: bool,
        starts_closed: bool,
        handlers: u32,
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
        if let ChildMessage::CatalogRetired {
            discovery_retired,
            originals_joined,
            callbacks_joined,
            timers_retired,
            operations,
            rejections,
        } = &h.msg
            && (!discovery_retired
                || !originals_joined
                || !callbacks_joined
                || !timers_retired
                || *operations == 0
                || *operations > 4096
                || rejections > operations)
        {
            return Err(ProtocolFault("startup_catalog_unqualified"));
        }
        if let ChildMessage::SettingsRetired {
            auth_listener_retired,
            settings_retired,
            originals_joined,
            callbacks_joined,
            timers_retired,
            operations,
            rejections,
        } = &h.msg
            && (!auth_listener_retired
                || !settings_retired
                || !originals_joined
                || !callbacks_joined
                || !timers_retired
                || *operations == 0
                || *operations > 4096
                || rejections > operations)
        {
            return Err(ProtocolFault("startup_settings_unqualified"));
        }
        if let ChildMessage::AuthRetired {
            empty_auth,
            auth_retired,
            originals_joined,
            callbacks_joined,
            store_closed,
            operations,
            rejections,
        } = &h.msg
            && (!empty_auth
                || !auth_retired
                || !originals_joined
                || !callbacks_joined
                || !store_closed
                || *operations == 0
                || *operations > 4096
                || rejections > operations)
        {
            return Err(ProtocolFault("startup_auth_unqualified"));
        }
        if let ChildMessage::PresenceRetired {
            presence_retired,
            registration_joined,
            callbacks_joined,
            cleanup_joined,
            presence_removed,
        } = &h.msg
            && (!presence_retired
                || !registration_joined
                || !callbacks_joined
                || !cleanup_joined
                || !presence_removed)
        {
            return Err(ProtocolFault("startup_presence_unqualified"));
        }
        if let ChildMessage::TelemetryRetired {
            telemetry_disabled,
            original_joined,
            callbacks_joined,
        } = &h.msg
            && (!telemetry_disabled || !original_joined || !callbacks_joined)
        {
            return Err(ProtocolFault("startup_telemetry_unqualified"));
        }
        if let ChildMessage::MarketplaceRetired {
            marketplace_disabled,
            schedule_completed,
        } = &h.msg
            && (!marketplace_disabled || !schedule_completed)
        {
            return Err(ProtocolFault("startup_marketplace_unqualified"));
        }
        if let ChildMessage::LoggingRetired {
            logging_closed,
            transport_closed,
            retention_joined,
        } = &h.msg
            && (!logging_closed || !transport_closed || !retention_joined)
        {
            return Err(ProtocolFault("startup_logging_unqualified"));
        }
        if let ChildMessage::PluginRootsRetired {
            plugin_roots_closed,
            originals_joined,
            callbacks_joined,
            operations,
        } = &h.msg
            && (!plugin_roots_closed
                || !originals_joined
                || !callbacks_joined
                || *operations == 0
                || *operations > 4096)
        {
            return Err(ProtocolFault("startup_plugin_roots_unqualified"));
        }
        if let ChildMessage::ExtensionRetired {
            handlers_joined,
            callbacks_joined,
            starts_closed,
            handlers,
        } = &h.msg
            && (!handlers_joined
                || !callbacks_joined
                || !starts_closed
                || *handlers == 0
                || *handlers > 64)
        {
            return Err(ProtocolFault("startup_extension_unqualified"));
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

#[derive(Default)]
struct ObservedOwners {
    attempts: u32,
    warmup: bool,
    restrictions: bool,
    catalog: bool,
    settings: bool,
    auth: bool,
    presence: bool,
    telemetry: bool,
    marketplace: bool,
    logging: bool,
    plugin_roots: bool,
    extension: bool,
}
impl ObservedOwners {
    fn accept(&mut self, message: &ChildMessage) -> Result<(), ProtocolFault> {
        match message {
            ChildMessage::StartupAttempt { attempt, .. } => {
                if self.warmup || self.restrictions || self.catalog {
                    return Err(ProtocolFault("startup_attempt_after_settlement"));
                }
                if *attempt != self.attempts + 1 || self.attempts == MAX_ATTEMPTS {
                    return Err(ProtocolFault("startup_attempt_order"));
                }
                self.attempts += 1;
            }
            ChildMessage::OwnerSettled { .. } => {
                if self.attempts == 0 || self.warmup {
                    return Err(ProtocolFault("startup_owner_order"));
                }
                self.warmup = true;
            }
            ChildMessage::RestrictionsSettled { .. } => {
                if !self.warmup || self.restrictions {
                    return Err(ProtocolFault("startup_restrictions_order"));
                }
                self.restrictions = true;
            }
            ChildMessage::CatalogRetired { .. } => {
                if !self.restrictions || self.catalog {
                    return Err(ProtocolFault("startup_catalog_order"));
                }
                self.catalog = true;
            }
            ChildMessage::SettingsRetired { .. } => {
                if !self.catalog || self.settings {
                    return Err(ProtocolFault("startup_settings_order"));
                }
                self.settings = true;
            }
            ChildMessage::AuthRetired { .. } => {
                if !self.settings || self.auth {
                    return Err(ProtocolFault("startup_auth_order"));
                }
                self.auth = true;
            }
            ChildMessage::PresenceRetired { .. } => {
                if !self.auth || self.presence {
                    return Err(ProtocolFault("startup_presence_order"));
                }
                self.presence = true;
            }
            ChildMessage::TelemetryRetired { .. } => {
                if !self.presence || self.telemetry {
                    return Err(ProtocolFault("startup_telemetry_order"));
                }
                self.telemetry = true;
            }
            ChildMessage::MarketplaceRetired { .. } => {
                if !self.telemetry || self.marketplace {
                    return Err(ProtocolFault("startup_marketplace_order"));
                }
                self.marketplace = true;
            }
            ChildMessage::LoggingRetired { .. } => {
                if !self.marketplace || self.logging {
                    return Err(ProtocolFault("startup_logging_order"));
                }
                self.logging = true;
            }
            ChildMessage::PluginRootsRetired { .. } => {
                if !self.logging || self.plugin_roots {
                    return Err(ProtocolFault("startup_plugin_roots_order"));
                }
                self.plugin_roots = true;
            }
            ChildMessage::ExtensionRetired { .. } => {
                if !self.plugin_roots || self.extension {
                    return Err(ProtocolFault("startup_extension_order"));
                }
                self.extension = true;
            }
            ChildMessage::Fault { .. } => return Err(ProtocolFault("startup_child_fault")),
        }
        Ok(())
    }
    fn complete(&self) -> bool {
        self.warmup
            && self.restrictions
            && self.catalog
            && self.settings
            && self.auth
            && self.presence
            && self.telemetry
            && self.marketplace
            && self.logging
            && self.plugin_roots
            && self.extension
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
    let mut owners = ObservedOwners::default();
    let result: Result<(), CaptureFault> = tokio::time::timeout_at(deadline, async {
        journal.append("host_launched", json!({"pid":host.launch.pid,"bridge_sha256":host.launch.bridge_sha256,"policy_sha256":host.launch.policy_sha256}))?;
        loop {
            match events.recv().await {
                Some(host::HostEvent::Startup(inbound)) => {
                    owners.accept(&inbound.msg).map_err(|e|fault(FaultClass::Protocol,e.0))?;
                    match inbound.msg {
                    ChildMessage::StartupAttempt { attempt, .. } => {
                        journal.append("startup_refused", json!({"child_seq":inbound.seq,"attempt":inbound.msg}))?;
                        // Sync happened in append; attest exactly the committed prefix
                        // before the response can reach the child.
                        let digest = journal.digest()?;
                        host.send_startup(ParentMessage::StartupRefused { attempt, journal_seq:journal.records,journal_sha256:digest }).await?;
                    }
                    ChildMessage::OwnerSettled { .. } => {
                        journal.append("startup_owner_settled",json!({"child_seq":inbound.seq,"owner":Owner::ModelConnectionWarmup}))?;
                    }
                    ChildMessage::RestrictionsSettled { .. } => {
                        journal.append("startup_restrictions_settled",json!({"child_seq":inbound.seq,"restrictions":inbound.msg}))?;
                    }
                    ChildMessage::CatalogRetired { .. } => {
                        journal.append("startup_catalog_retired",json!({"child_seq":inbound.seq,"catalog":inbound.msg}))?;
                    }
                    ChildMessage::SettingsRetired { .. } => {
                        journal.append("startup_settings_retired",json!({"child_seq":inbound.seq,"settings":inbound.msg}))?;
                    }
                    ChildMessage::AuthRetired { .. } => {
                        journal.append("startup_auth_retired",json!({"child_seq":inbound.seq,"auth":inbound.msg}))?;
                    }
                    ChildMessage::PresenceRetired { .. } => {
                        journal.append("startup_presence_retired",json!({"child_seq":inbound.seq,"presence":inbound.msg}))?;
                    }
                    ChildMessage::TelemetryRetired { .. } => {
                        journal.append("startup_telemetry_retired",json!({"child_seq":inbound.seq,"telemetry":inbound.msg}))?;
                    }
                    ChildMessage::MarketplaceRetired { .. } => {
                        journal.append("startup_marketplace_retired",json!({"child_seq":inbound.seq,"marketplace":inbound.msg}))?;
                    }
                    ChildMessage::LoggingRetired { .. } => {
                        journal.append("startup_logging_retired",json!({"child_seq":inbound.seq,"logging":inbound.msg}))?;
                    }
                    ChildMessage::PluginRootsRetired { .. } => {
                        journal.append("startup_plugin_roots_retired",json!({"child_seq":inbound.seq,"plugin_roots":inbound.msg}))?;
                    }
                    ChildMessage::ExtensionRetired { .. } => {
                        journal.append("startup_extension_retired",json!({"child_seq":inbound.seq,"extension":inbound.msg}))?;
                    }
                    ChildMessage::Fault { .. } => return Err(fault(FaultClass::Protocol,"startup_child_fault")),
                    }
                },
                Some(host::HostEvent::Rpc(host::RpcEvent::Ready | host::RpcEvent::Other)) => {},
                Some(host::HostEvent::BridgeFault(e)) => return Err(fault(FaultClass::Protocol,e.0)),
                _ => return Err(fault(FaultClass::Protocol,"startup_unexpected_event")),
            }
            if owners.complete() { return Err(fault(FaultClass::Authority,"startup_inventory_unqualified")); }
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
        attempts: owners.attempts,
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
    fn startup_catalog_requires_each_join_and_bounded_source_operations() {
        let valid = json!({"type":"catalog_retired","discovery_retired":true,"originals_joined":true,"callbacks_joined":true,"timers_retired":true,"operations":1,"rejections":0});
        for field in [
            "discovery_retired",
            "originals_joined",
            "callbacks_joined",
            "timers_retired",
        ] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        for (operations, rejections) in [(0, 0), (4097, 0), (1, 2)] {
            let mut bad = valid.clone();
            bad["operations"] = json!(operations);
            bad["rejections"] = json!(rejections);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_settings_requires_original_joins_and_bounded_source_operations() {
        let valid = json!({"type":"settings_retired","auth_listener_retired":true,"settings_retired":true,"originals_joined":true,"callbacks_joined":true,"timers_retired":true,"operations":1,"rejections":0});
        for field in [
            "auth_listener_retired",
            "settings_retired",
            "originals_joined",
            "callbacks_joined",
            "timers_retired",
        ] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        for (operations, rejections) in [(0, 0), (4097, 0), (1, 2)] {
            let mut bad = valid.clone();
            bad["operations"] = json!(operations);
            bad["rejections"] = json!(rejections);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_auth_requires_original_joins_and_bounded_source_operations() {
        let valid = json!({"type":"auth_retired","empty_auth":true,"auth_retired":true,"originals_joined":true,"callbacks_joined":true,"store_closed":true,"operations":1,"rejections":0});
        for field in [
            "empty_auth",
            "auth_retired",
            "originals_joined",
            "callbacks_joined",
            "store_closed",
        ] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        for (operations, rejections) in [(0, 0), (4097, 0), (1, 2)] {
            let mut bad = valid.clone();
            bad["operations"] = json!(operations);
            bad["rejections"] = json!(rejections);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_presence_requires_registration_observer_cleanup_and_removal() {
        let valid = json!({"type":"presence_retired","presence_retired":true,"registration_joined":true,"callbacks_joined":true,"cleanup_joined":true,"presence_removed":true});
        for field in [
            "presence_retired",
            "registration_joined",
            "callbacks_joined",
            "cleanup_joined",
            "presence_removed",
        ] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        let mut extra = valid.clone();
        extra["path"] = json!("must-not-export");
        assert!(serde_json::from_value::<ChildMessage>(extra).is_err());
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_extension_requires_joined_closed_bounded_and_exact_fields() {
        let valid = json!({"type":"extension_retired","handlers_joined":true,"callbacks_joined":true,"starts_closed":true,"handlers":1});
        for field in ["handlers_joined", "callbacks_joined", "starts_closed"] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        for count in [0, 65] {
            let mut bad = valid.clone();
            bad["handlers"] = json!(count);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        for field in [
            "handlers_joined",
            "callbacks_joined",
            "starts_closed",
            "handlers",
        ] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        let mut extra = valid.clone();
        extra["path"] = json!("must-not-export");
        assert!(serde_json::from_value::<ChildMessage>(extra).is_err());
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_plugin_roots_require_closed_joined_bounded_and_exact_fields() {
        let valid = json!({"type":"plugin_roots_retired","plugin_roots_closed":true,"originals_joined":true,"callbacks_joined":true,"operations":1});
        for field in [
            "plugin_roots_closed",
            "originals_joined",
            "callbacks_joined",
        ] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        for count in [0, 4097] {
            let mut bad = valid.clone();
            bad["operations"] = json!(count);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
        }
        for field in [
            "plugin_roots_closed",
            "originals_joined",
            "callbacks_joined",
            "operations",
        ] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        let mut extra = valid.clone();
        extra["path"] = json!("must-not-export");
        assert!(serde_json::from_value::<ChildMessage>(extra).is_err());
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_logging_requires_closed_transport_retention_and_exact_fields() {
        let valid = json!({"type":"logging_retired","logging_closed":true,"transport_closed":true,"retention_joined":true});
        for field in ["logging_closed", "transport_closed", "retention_joined"] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        let mut extra = valid.clone();
        extra["path"] = json!("must-not-export");
        assert!(serde_json::from_value::<ChildMessage>(extra).is_err());
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_marketplace_requires_disabled_completed_and_exact_fields() {
        let valid = json!({"type":"marketplace_retired","marketplace_disabled":true,"schedule_completed":true});
        for field in ["marketplace_disabled", "schedule_completed"] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        let mut extra = valid.clone();
        extra["path"] = json!("must-not-export");
        assert!(serde_json::from_value::<ChildMessage>(extra).is_err());
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_telemetry_requires_disabled_original_and_callback_join() {
        let valid = json!({"type":"telemetry_retired","telemetry_disabled":true,"original_joined":true,"callbacks_joined":true});
        for field in ["telemetry_disabled", "original_joined", "callbacks_joined"] {
            let mut bad = valid.clone();
            bad[field] = json!(false);
            let msg = serde_json::from_value::<ChildMessage>(bad).unwrap();
            assert!(
                ChannelState::new(identity())
                    .read(&mut frame(1, msg).as_slice())
                    .is_err()
            );
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ChildMessage>(missing).is_err());
        }
        let mut extra = valid.clone();
        extra["endpoint"] = json!("must-not-export");
        assert!(serde_json::from_value::<ChildMessage>(extra).is_err());
        let msg = serde_json::from_value::<ChildMessage>(valid).unwrap();
        assert!(
            ChannelState::new(identity())
                .read(&mut frame(1, msg).as_slice())
                .is_ok()
        );
    }

    #[test]
    fn startup_owner_order_rejects_duplicates_and_attempts_after_settlement() {
        let warmup = ChildMessage::OwnerSettled {
            owner: Owner::ModelConnectionWarmup,
        };
        let restrictions = ChildMessage::RestrictionsSettled {
            tool_names: vec!["edit".into(), "read".into()],
            mcp_disabled: true,
            memory_disabled: true,
            credential_usage_joined: true,
        };
        let catalog = ChildMessage::CatalogRetired {
            discovery_retired: true,
            originals_joined: true,
            callbacks_joined: true,
            timers_retired: true,
            operations: 1,
            rejections: 0,
        };
        let settings = ChildMessage::SettingsRetired {
            auth_listener_retired: true,
            settings_retired: true,
            originals_joined: true,
            callbacks_joined: true,
            timers_retired: true,
            operations: 1,
            rejections: 0,
        };
        let auth = ChildMessage::AuthRetired {
            empty_auth: true,
            auth_retired: true,
            originals_joined: true,
            callbacks_joined: true,
            store_closed: true,
            operations: 1,
            rejections: 0,
        };
        let presence = ChildMessage::PresenceRetired {
            presence_retired: true,
            registration_joined: true,
            callbacks_joined: true,
            cleanup_joined: true,
            presence_removed: true,
        };
        let telemetry = ChildMessage::TelemetryRetired {
            telemetry_disabled: true,
            original_joined: true,
            callbacks_joined: true,
        };
        let marketplace = ChildMessage::MarketplaceRetired {
            marketplace_disabled: true,
            schedule_completed: true,
        };
        let logging = ChildMessage::LoggingRetired {
            logging_closed: true,
            transport_closed: true,
            retention_joined: true,
        };
        let plugin_roots = ChildMessage::PluginRootsRetired {
            plugin_roots_closed: true,
            originals_joined: true,
            callbacks_joined: true,
            operations: 1,
        };
        let extension = ChildMessage::ExtensionRetired {
            handlers_joined: true,
            callbacks_joined: true,
            starts_closed: true,
            handlers: 1,
        };
        for early in [
            &extension,
            &plugin_roots,
            &logging,
            &marketplace,
            &telemetry,
            &warmup,
            &restrictions,
            &catalog,
            &settings,
            &auth,
            &presence,
        ] {
            assert!(ObservedOwners::default().accept(early).is_err());
        }
        let mut state = ObservedOwners::default();
        state.accept(&attempt()).unwrap();
        assert!(state.accept(&attempt()).is_err());
        assert!(state.accept(&restrictions).is_err());
        assert!(state.accept(&catalog).is_err());
        state.accept(&warmup).unwrap();
        assert!(state.accept(&warmup).is_err());
        assert!(state.accept(&attempt()).is_err());
        state.accept(&restrictions).unwrap();
        assert!(state.accept(&restrictions).is_err());
        assert!(!state.complete());
        assert!(state.accept(&settings).is_err());
        state.accept(&catalog).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&auth).is_err());
        state.accept(&settings).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&presence).is_err());
        state.accept(&auth).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&telemetry).is_err());
        state.accept(&presence).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&marketplace).is_err());
        state.accept(&telemetry).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&logging).is_err());
        state.accept(&marketplace).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&plugin_roots).is_err());
        state.accept(&logging).unwrap();
        assert!(!state.complete());
        assert!(state.accept(&extension).is_err());
        state.accept(&plugin_roots).unwrap();
        assert!(!state.complete());
        state.accept(&extension).unwrap();
        assert!(state.complete());
        assert!(state.accept(&extension).is_err());
        assert!(state.accept(&plugin_roots).is_err());
        assert!(state.accept(&logging).is_err());
        assert!(state.accept(&marketplace).is_err());
        assert!(state.accept(&telemetry).is_err());
        assert!(state.accept(&presence).is_err());
        assert!(state.accept(&auth).is_err());
        assert!(state.accept(&settings).is_err());
        assert!(state.accept(&catalog).is_err());
        assert!(state.accept(&attempt()).is_err());
        let mut bounded = ObservedOwners::default();
        for id in 1..=MAX_ATTEMPTS {
            let mut msg = attempt();
            if let ChildMessage::StartupAttempt { attempt, .. } = &mut msg {
                *attempt = id;
            }
            bounded.accept(&msg).unwrap();
        }
        let mut extra = attempt();
        if let ChildMessage::StartupAttempt { attempt, .. } = &mut extra {
            *attempt = MAX_ATTEMPTS + 1;
        }
        assert!(bounded.accept(&extra).is_err());
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
