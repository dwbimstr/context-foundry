//! Owned generation. The core supervises a separately linked Rust/llama.cpp
//! worker; neither source acceptance nor the HTTP provider loads model code.
use super::{Profile, fail};
use crate::{Control, FResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod protocol;
pub mod resources;
pub mod runtime;
mod supervisor;
pub use supervisor::Provider;

pub const ADAPTER: &str = "foundry-generate-v1";
pub const LLAMA_CPP: &str = crate::neural::provider::LLAMA_CPP_COMMIT;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Device {
    Cpu,
    /// Metal exposes monitored allocations, not a hard per-process GPU quota.
    Metal {
        layers: u32,
        allocation_bytes: u64,
        reserve_bytes: u64,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEnforcement {
    HardAddressSpace,
    SupervisedFootprint,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Explicit experimental native isolation; never a production package claim.
    pub development: bool,
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub gguf: PathBuf,
    pub llama_cpp: String,
    /// Explicit llama.cpp built-in template name, part of input identity.
    pub chat_template: String,
    pub device: Device,
    pub context_tokens: u32,
    pub batch_tokens: u32,
    pub threads: u32,
    pub memory_bytes: u64,
    pub memory_enforcement: MemoryEnforcement,
    /// Worker-lifetime CPU seconds (all threads), a hard OS ceiling.
    pub cpu_seconds: u64,
    pub load_seconds: u64,
    /// Release model/device residency after this much idle time; keep verified inputs.
    pub idle_seconds: u64,
}
impl Config {
    pub fn validate(&self) -> FResult<()> {
        let absolute =
            |path: &std::path::Path| path.is_absolute() && path.to_string_lossy().len() <= 4096;
        if !self.development
            || !absolute(&self.executable)
            || !absolute(&self.gguf)
            || !super::digest(&self.executable_sha256)
            || self.llama_cpp != LLAMA_CPP
            || self.chat_template.is_empty()
            || self.chat_template.len() > 64
            || !self
                .chat_template
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !(512..=131072).contains(&self.context_tokens)
            || !(32..=2048).contains(&self.batch_tokens)
            || self.batch_tokens > self.context_tokens
            || !(1..=64).contains(&self.threads)
            || !(128 * 1024 * 1024..=128 * 1024 * 1024 * 1024).contains(&self.memory_bytes)
            || !(1..=86400).contains(&self.cpu_seconds)
            || !(1..=300).contains(&self.load_seconds)
            || !(1..=300).contains(&self.idle_seconds)
        {
            return Err(fail(
                "knowledge_profile_invalid",
                "invalid local worker identity, development admission or limits",
            ));
        }
        if let Device::Metal {
            layers,
            allocation_bytes,
            reserve_bytes,
        } = self.device
            && (!(1..=1024).contains(&layers)
                || !(16 * 1024 * 1024..=self.memory_bytes).contains(&allocation_bytes)
                || reserve_bytes == 0
                || reserve_bytes > 128 * 1024 * 1024 * 1024)
        {
            return Err(fail(
                "knowledge_profile_invalid",
                "invalid Metal layer/allocation/reserve limits",
            ));
        }
        Ok(())
    }
    pub fn function_id(&self) -> FResult<String> {
        // Resource ceilings, paths and idle/load timeouts do not change model inputs.
        // Geometry, execution backend, exact worker and template do.
        let device = match self.device {
            Device::Cpu => ("cpu", 0),
            Device::Metal { layers, .. } => ("metal", layers),
        };
        super::hash(&(
            ADAPTER,
            &self.executable_sha256,
            &self.llama_cpp,
            &self.chat_template,
            device,
            self.context_tokens,
            self.batch_tokens,
            self.threads,
        ))
    }
    pub fn platform(&self) -> FResult<()> {
        self.validate()?;
        #[cfg(target_os = "linux")]
        if self.device == Device::Cpu
            && self.memory_enforcement == MemoryEnforcement::HardAddressSpace
        {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        if self.memory_enforcement == MemoryEnforcement::SupervisedFootprint {
            return Ok(());
        }
        Err(fail(
            "knowledge_isolation_unavailable",
            "this platform cannot enforce the selected device/memory profile",
        ))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub pid: Option<u32>,
    pub launches: u64,
    pub calls: u64,
    pub verified_bytes: u64,
    pub peak_memory_bytes: u64,
    pub peak_gpu_bytes: u64,
    pub isolation: Option<String>,
    pub last_stop: Option<String>,
}

#[derive(Clone, Default)]
pub struct Telemetry(std::sync::Arc<std::sync::Mutex<Observation>>);
impl Telemetry {
    pub fn snapshot(&self) -> Observation {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub(super) fn publish(&self, observation: &Observation) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = observation.clone();
    }
}

/// The CLI and continuous owner keep the same session across selected nodes.
/// Idle unload drops the process, never the verified artifact snapshot.
#[derive(Default)]
pub struct Session {
    provider: Option<Provider>,
    refused: Option<(String, &'static str)>,
    pub observation: Observation,
    pub telemetry: Telemetry,
    pub(crate) resources: Option<std::sync::Arc<crate::model_resources::Resources>>,
    reservation: Option<crate::model_resources::Reservation>,
}
impl Session {
    pub(crate) fn admit_resources(
        &mut self,
        profile: &Profile,
    ) -> Result<(), crate::model_resources::Deferred> {
        let Some(resources) = &self.resources else {
            return Ok(());
        };
        if let Some(config) = &profile.local
            && self.reservation.is_none()
        {
            self.reservation = Some(resources.reserve(
                crate::model_resources::Lane::Generation,
                config.memory_bytes,
            )?);
        } else {
            resources.background()?;
        }
        Ok(())
    }
    pub fn prepare(&mut self, profile: &Profile, control: &Control) -> FResult<()> {
        control.check()?;
        let identity = super::hash(&(&profile.local, &profile.model, &profile.artifact_sha256))?;
        if let Some((previous, code)) = &self.refused
            && previous == &identity
        {
            return Err(fail(
                code,
                "local acquisition already refused in this run; completed nodes remain reusable",
            ));
        }
        if self
            .provider
            .as_ref()
            .is_some_and(|provider| !provider.matches(profile))
        {
            self.unload("profile_changed");
            self.provider = None;
        }
        if self.provider.is_none() {
            self.provider = match Provider::new(
                profile,
                control,
                &mut self.observation,
                self.telemetry.clone(),
            ) {
                Ok(provider) => {
                    self.refused = None;
                    Some(provider)
                }
                Err(error) => {
                    self.refused = Some((identity, error.code()));
                    return Err(error);
                }
            };
        }
        Ok(())
    }
    pub async fn generate(
        &mut self,
        profile: &Profile,
        body: Vec<u8>,
        control: &Control,
    ) -> FResult<super::provider::Reply> {
        self.prepare(profile, control)?;
        let result = self
            .provider
            .as_mut()
            .expect("initialized provider")
            .generate(profile, body, control, &mut self.observation)
            .await;
        if self.observation.pid.is_none() {
            self.reservation = None;
        }
        result
    }
    pub fn idle(&mut self) {
        if let Some(Err(reason)) = self
            .resources
            .as_ref()
            .map(|resources| resources.background())
        {
            self.unload(reason.code());
            return;
        }
        if let Some(provider) = &mut self.provider {
            provider.idle(&mut self.observation);
        }
        if self.observation.pid.is_none() {
            self.reservation = None;
        }
    }
    pub fn unload(&mut self, reason: &str) {
        if let Some(provider) = &mut self.provider {
            provider.unload(reason, &mut self.observation);
        }
        self.reservation = None;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.unload("stopped");
    }
}
