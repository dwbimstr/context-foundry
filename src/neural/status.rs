//! 009 semantic readiness census (`foundry semantic status`): metadata-only
//! paged reads. Status never opens source bodies, never tokenizes, never
//! loads Python or the model, never decodes a cache vector, never starts
//! preparation, and never populates missing mappings — not even the profile
//! file: a broken profile is named, not parsed. Repeated status (and
//! context) therefore performs zero corpus tokenization and zero inference.
//!
//! Every phase honors the caller's `Control`: generation validation hashes in
//! checkpointed chunks and the cache census checks before each page of rows.
//!
//! Status trusts committed payloads. Its counts come from committed metadata
//! only: mappings, cache-row lengths and stored function digests. It does NOT
//! scan vector payloads, so a same-length row tampered with after commit (a
//! NaN/infinity component) still counts as cached here. The lookup that USES
//! a vector decodes it and names a bad one; a repair then EXCLUDES it from
//! the derived generation (zero inference — nothing is re-embedded), and only
//! an explicit preparation replaces the vector. Every vector is validated
//! (length, finite values) before it is committed.
//!
//! Semantics (spec 009 § Readiness and retrieval): counts describe the read
//! snapshot; `unpartitioned_sources` have UNKNOWN unit totals, so while any
//! exist the totals are unknown and never "empty"/"ready"; `empty` requires
//! every admitted source to carry a current completed zero-unit partition
//! (or no admitted sources at all). A persisted `running` state is a dead
//! owner's leftover and reads as `stopped` (interrupted).
use crate::control::Control;
use crate::error::FResult;
use crate::neural::cache::{self, CacheProbe, DEFAULT_CACHE_CAP_BYTES};
use crate::neural::index::{self, GenerationError};
use crate::store::Engine;
use serde::Serialize;
use std::collections::HashSet;

/// Named fault points of the status read (test-faults only).
#[cfg(feature = "test-faults")]
pub mod fault_names {
    pub const BEFORE_VALIDATION: &str = "ctxfoundry-fault/semantic_status.before_validation";
    pub const BEFORE_CENSUS: &str = "ctxfoundry-fault/semantic_status.before_census";
}

#[cfg(feature = "test-faults")]
macro_rules! status_fault {
    ($name:ident, $control:expr) => {
        crate::fault::hit(
            fault_names::$name,
            &crate::fault::Ctx {
                engine: None,
                control: Some($control),
                detail: "",
            },
        )
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! status_fault {
    ($name:ident, $control:expr) => {
        Ok::<(), crate::FoundryError>(())
    };
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticStatus {
    pub workspace_id: Option<String>,
    pub source_revision: u64,
    pub profile: Option<ProfileIdentity>,
    pub provider: ProviderStatus,
    /// `stopped`, `paused` or `running`.
    pub state: String,
    pub last_error: Option<cache::StateError>,
    pub sources: u64,
    /// Sources without a current completed partition; their unit totals are
    /// unknown.
    pub unpartitioned_sources: u64,
    /// `complete` when every source has a current partition, else `partial`.
    pub partition_coverage: &'static str,
    /// `unknown` while any source is unpartitioned, else `empty`/`nonempty`.
    pub corpus: &'static str,
    pub eligible_units: u64,
    pub cached_current_units: u64,
    pub searchable_current_units: u64,
    pub missing_units: u64,
    pub committed_units: u64,
    pub cache: CacheStatus,
    pub index: IndexStatus,
}

/// What the last preparation run observed of the provider. It is a record,
/// never a live probe: `unknown` until a run observed something, then
/// `ready`, `refused` or `failed` with the time and profile of that run.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderStatus {
    pub state: String,
    pub code: Option<String>,
    pub observed_at_unix: Option<u64>,
    pub profile: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileIdentity {
    pub name: String,
    pub function_digest: String,
    pub recipe_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheStatus {
    pub entries: u64,
    pub bytes: u64,
    /// Valid retained entries no current partition references (cache
    /// retention of any function, disclosed; explicit purge removes them).
    pub orphan_entries: u64,
    /// Rows whose OWN layout is invalid; valid rows of another function are
    /// retention, never corruption.
    pub corrupt_entries: u64,
    pub cap_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexStatus {
    pub available: bool,
    pub entries: u64,
    pub reason: Option<String>,
}

impl Engine {
    /// The metadata-only census, paged by 128 and bounded by `control`.
    pub fn semantic_status(&self, control: &Control) -> FResult<SemanticStatus> {
        control.check()?;
        let state = self.semantic_state()?;
        let prepared = state.as_ref().and_then(|state| {
            Some((
                state.function_digest.as_deref()?,
                state.recipe_id.as_deref()?,
            ))
        });
        let mut unpartitioned = 0u64;
        let mut sources = 0u64;
        let mut eligible = 0u64;
        let mut cached_current = 0u64;
        let mut searchable_current = 0u64;
        let mut referenced: HashSet<String> = HashSet::new();
        // The published generation, when one validates for the recorded
        // profile: validated by content (hash work checkpointed against the
        // deadline), never served by directory name.
        status_fault!(BEFORE_VALIDATION, control)?;
        let (generation, index_reason) = match prepared {
            None => (None, Some("no semantic profile prepared".to_owned())),
            Some((digest, recipe)) => {
                let store = self.semantic_anchor()?;
                match index::validate_generation_with(&store, digest, recipe, control) {
                    Ok(generation) => (Some(generation), None),
                    Err(GenerationError::Interrupted(error)) => return Err(error),
                    Err(GenerationError::Unavailable(reason)) => (None, Some(reason)),
                }
            }
        };
        let indexed: HashSet<&str> = generation
            .as_ref()
            .map(|generation| {
                generation
                    .manifest
                    .entries
                    .iter()
                    .map(String::as_str)
                    .collect()
            })
            .unwrap_or_default();
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page = cache::source_page(&self.db, after.as_deref())?;
            if page.is_empty() {
                break;
            }
            for (path, meta) in &page {
                sources += 1;
                let Some((digest, recipe)) = prepared else {
                    unpartitioned += 1;
                    continue;
                };
                let record = self.semantic_partition(path)?;
                let current = record.as_ref().is_some_and(|record| {
                    cache::partition_is_current(record, meta, recipe, digest)
                });
                if !current {
                    unpartitioned += 1;
                    continue;
                }
                let record = record.expect("current implies present");
                eligible += record.units.len() as u64;
                for unit in &record.units {
                    referenced.insert(unit.input_key.clone());
                    if self.semantic_cache_probe(&unit.input_key, digest)? == CacheProbe::Current {
                        cached_current += 1;
                        if indexed.contains(unit.input_key.as_str()) {
                            searchable_current += 1;
                        }
                    }
                }
            }
            after = page.last().map(|(path, _)| path.clone());
        }
        // Cache census: layout-only, paged, deadline-checked.
        status_fault!(BEFORE_CENSUS, control)?;
        let census = self.semantic_cache_census(control, &referenced)?;
        let partition_coverage = if unpartitioned == 0 {
            "complete"
        } else {
            "partial"
        };
        let corpus = if unpartitioned > 0 {
            "unknown"
        } else if eligible == 0 {
            "empty"
        } else {
            "nonempty"
        };
        Ok(SemanticStatus {
            workspace_id: self.workspace_id(),
            source_revision: self.source_revision()?,
            profile: state.as_ref().and_then(|state| {
                Some(ProfileIdentity {
                    name: state.profile_name.clone()?,
                    function_digest: state.function_digest.clone()?,
                    recipe_id: state.recipe_id.clone()?,
                })
            }),
            provider: match state.as_ref().and_then(|state| state.provider.as_ref()) {
                Some(observed) => ProviderStatus {
                    state: observed.state.clone(),
                    code: observed.code.clone(),
                    observed_at_unix: Some(observed.observed_at_unix),
                    profile: Some(observed.profile.clone()),
                },
                None => ProviderStatus {
                    state: "unknown".into(),
                    code: None,
                    observed_at_unix: None,
                    profile: None,
                },
            },
            state: state
                .as_ref()
                .map(|state| state.state.clone())
                .unwrap_or_else(|| "stopped".into()),
            last_error: state.as_ref().and_then(|state| state.last_error.clone()),
            sources,
            unpartitioned_sources: unpartitioned,
            partition_coverage,
            corpus,
            eligible_units: eligible,
            cached_current_units: cached_current,
            searchable_current_units: searchable_current,
            missing_units: eligible.saturating_sub(cached_current),
            committed_units: state.as_ref().map_or(0, |state| state.committed_units),
            cache: CacheStatus {
                entries: census.entries,
                bytes: census.bytes,
                orphan_entries: census.orphan,
                corrupt_entries: census.corrupt,
                cap_bytes: DEFAULT_CACHE_CAP_BYTES,
            },
            index: IndexStatus {
                available: generation.is_some(),
                entries: generation
                    .as_ref()
                    .map_or(0, |generation| generation.manifest.count as u64),
                reason: index_reason,
            },
        })
    }
}
