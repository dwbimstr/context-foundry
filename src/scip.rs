//! 005 T001/T002: import one completed SCIP artifact into scoped compiler
//! facts.
//!
//! **Schema versions.** The producer (rust-analyzer 2026-08-31) builds its
//! artifact with scip 0.7.1, whose `Occurrence` carries only the deprecated
//! `range`/`enclosing_range` fields. This decoder is built on scip 0.10.0, a
//! wire-compatible superset that adds `typed_range`/`typed_enclosing_range`
//! (and wins when both are present). Old artifacts therefore decode with the
//! typed fields absent.
//!
//! **Pipeline.** The importer never runs a compiler, opens no manifest-listed
//! source and downloads nothing. It:
//!
//! 1. copies the caller's manifest and artifact into a private run directory
//!    under `<store>/import-scratch/` (at most 1 MiB buffers, cancellation
//!    checked per buffer) and hashes the COPIES; both parsing passes read only
//!    those copies, never the caller's pathnames again;
//! 2. streams the manifest into a disposable redb lookup (`inputs`), checks it
//!    against the bound store (workspace, source revision, artifact digest,
//!    every input hash) and rejects duplicate manifest paths;
//! 3. pass 1 streams `Index.documents` one message at a time (never the whole
//!    `Index` in memory), binds every document path to the manifest, rejects
//!    duplicate documents and records definition counts per symbol in the
//!    lookup;
//! 4. selects the snapshot tuple, then pass 2 re-reads the same copy and
//!    publishes one document per scope transaction, resolving references
//!    against the lookup. Absent profile-listed `.rs` sources are published
//!    accepted-empty; scopes of sources the completed manifest proves absent
//!    are retired.
//!
//! The lookup is disposable scratch, not a second truth store. Publication is
//! source-atomic, not whole-run atomic: an interrupted import leaves a named
//! partial snapshot whose unpublished scopes are ineligible.
use crate::control::Control;
use crate::error::{FResult, FoundryError};
use crate::graph::{
    NewOccurrence, OccurrenceKind, ProducerIdentity, ScopeRow, ScopeStatus, ScopeWrite,
    SnapshotState, SnapshotTuple, symbol_id,
};
use crate::store::{CHUNKS, Engine, META, SOURCES, SourceMeta, decode, validate_path};
use protobuf::CodedInputStream;
use protobuf::Enum as _;
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use scip::types::PositionEncoding;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The named failures of an import or a references query.
pub mod code {
    pub const UNBOUND_ARTIFACT: &str = "unbound_artifact";
    pub const STALE_ARTIFACT: &str = "stale_artifact";
    pub const INVALID_RANGE: &str = "invalid_range";
    pub const UNSUPPORTED_ENCODING: &str = "unsupported_encoding";
    pub const ARTIFACT_TOO_LARGE: &str = "artifact_too_large";
    pub const MANIFEST_TOO_LARGE: &str = "manifest_too_large";
    pub const DOCUMENT_TOO_LARGE: &str = "document_too_large";
    pub const PRODUCER_INCOMPLETE: &str = "producer_incomplete";
    pub const SCRATCH_FULL: &str = "scratch_full";
    pub const DUPLICATE_DOCUMENT: &str = "duplicate_document";
    pub const DUPLICATE_INPUT: &str = "duplicate_input";
    pub const SYMBOL_NOT_FOUND: &str = "symbol_not_found";
    pub const AMBIGUOUS_SYMBOL: &str = "ambiguous_symbol";
    pub const IMPORT_INCOMPLETE: &str = "import_incomplete";
}

/// A named 005 failure as a [`FoundryError`].
pub(crate) fn fail(code: &'static str, message: impl Into<String>) -> FoundryError {
    FoundryError::Scip {
        code,
        message: message.into(),
    }
}

/// The spec's selected engineering bounds. Tests scale the size caps down
/// through [`Engine::import_scip_with`]; the shipped CLI always uses
/// [`ImportLimits::default`].
#[derive(Clone, Debug)]
pub struct ImportLimits {
    /// Whole artifact: 1 GiB.
    pub artifact_bytes: u64,
    /// Whole manifest: 64 MiB.
    pub manifest_bytes: u64,
    /// One serialized document message: 8 MiB.
    pub document_bytes: u64,
    /// Occurrences in one decoded document (before deduplication): 16,384.
    pub document_occurrences: usize,
    /// One SCIP symbol string: 1024 bytes.
    pub symbol_bytes: usize,
    /// Scratch filesystem usage: 4 GiB.
    pub scratch_bytes: u64,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            artifact_bytes: 1 << 30,
            manifest_bytes: 64 << 20,
            document_bytes: 8 << 20,
            document_occurrences: 16_384,
            symbol_bytes: 1024,
            scratch_bytes: 4 << 30,
        }
    }
}

/// Producer namespace and revision (the manifest's `producer.name` and
/// `producer.release_tag`) each fit this many bytes.
const IDENTITY_BYTES: usize = 128;
/// `commit` and `version_output` are persisted with the selected snapshot.
const IDENTITY_TEXT_BYTES: usize = 1024;
/// Manifest input rows buffered at a time.
const INPUT_BATCH: usize = 128;
/// Documents per scratch lookup transaction.
const DOCUMENT_BATCH: usize = 128;
/// Copy buffer: at most 1 MiB.
const COPY_BUFFER: usize = 1 << 20;
/// Error samples kept in a report.
const ERROR_SAMPLES: usize = 20;
const SAMPLE_BYTES: usize = 256;
/// `Index.documents` is field 2, length-delimited.
const DOCUMENTS_TAG: u32 = (2 << 3) | 2;
/// `Document.relative_path` is field 1, length-delimited.
const PATH_TAG: u32 = (1 << 3) | 2;
const SCRATCH_DIR: &str = "import-scratch";
const OWNER_FILE: &str = "OWNER";
const OWNER_MAGIC: &str = "context-foundry/import-scratch/v1";

/// One failed document (or refused run) in an import report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportFailureSample {
    pub path: String,
    pub code: String,
    pub message: String,
}

/// Wall-clock costs, recorded with the import so the copy and scratch work is
/// visible. `scratch_peak_bytes` is the observed peak of the run directory,
/// not an OS quota.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportTimings {
    pub copy_ms: u64,
    pub lookup_ms: u64,
    pub publish_ms: u64,
    pub total_ms: u64,
}

/// The aggregate report of one import run, also persisted as the producer's
/// latest report.
///
/// `complete` means the manifest and artifact were fully consumed, every
/// document committed and nothing failed or was interrupted. It is NOT a
/// claim of completeness about all possible references: a source with
/// unresolved references is counted in `unresolved` (partial source
/// coverage), and per-query coverage is decided at read time.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportReport {
    pub producer: String,
    pub snapshot_id: String,
    pub source_revision: u64,
    pub artifact_sha256: String,
    pub manifest_sha256: String,
    /// The tuple became the producer's selected snapshot.
    pub selected: bool,
    pub complete: bool,
    /// Aggregate import coverage: `complete` only when every consumed source
    /// is complete - nothing unresolved, unknown, failed or interrupted.
    /// Paths outside the producer's scope (`outside_scope`, non-`.rs` for
    /// rust-analyzer) are informational and leave it unaffected. Distinct
    /// from `complete`, which is about consumption: a fully imported run
    /// with unresolved references is `complete: true, coverage: "partial"`
    /// and never certifies absence.
    pub coverage: String,
    /// `cancelled`, `deadline_exceeded` or the failure code that stopped a
    /// selected run before it published every document.
    pub interrupted: Option<String>,
    /// The preflight refusal or the interruption cause.
    pub failure: Option<ImportFailureSample>,
    /// Documents found in the artifact.
    pub documents: u64,
    /// Sources committed with every reference resolved.
    pub completed: u64,
    /// Sources committed with no occurrences (including profile-listed
    /// absent `.rs` sources).
    pub accepted_empty: u64,
    /// Sources committed with unresolved references (partial coverage).
    pub unresolved: u64,
    /// Documents that failed by path and left their scope unchanged.
    pub failed: u64,
    /// Manifest paths outside the producer's scope (non-`.rs` for
    /// rust-analyzer).
    pub outside_scope: u64,
    /// Manifest paths absent from the artifact whose coverage is unknown.
    pub unknown: u64,
    /// Scopes retired because the completed manifest proves their sources
    /// absent.
    pub retired: u64,
    pub occurrences: u64,
    pub definitions: u64,
    pub references: u64,
    /// Stored reference occurrences whose target was external, unknown or
    /// ambiguous at import time.
    pub unresolved_references: u64,
    pub failure_samples: Vec<ImportFailureSample>,
    pub copied_bytes: u64,
    pub scratch_peak_bytes: u64,
    pub timings: ImportTimings,
}

fn clip(text: &str, max: usize) -> String {
    let mut cut = text.len().min(max);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_owned()
}

impl ImportReport {
    fn note_failure(&mut self, path: &str, code: &str, message: &str) {
        self.failed += 1;
        if self.failure_samples.len() < ERROR_SAMPLES {
            self.failure_samples.push(ImportFailureSample {
                path: clip(path, 512),
                code: code.to_owned(),
                message: clip(message, SAMPLE_BYTES),
            });
        }
    }

    fn count_scope(&mut self, row: &ScopeRow) {
        match row.status {
            ScopeStatus::Complete => self.completed += 1,
            ScopeStatus::AcceptedEmpty => self.accepted_empty += 1,
            ScopeStatus::Partial => self.unresolved += 1,
        }
        self.definitions += u64::from(row.definitions);
        self.references += u64::from(row.references);
        self.occurrences += u64::from(row.definitions) + u64::from(row.references);
        self.unresolved_references += u64::from(row.unresolved);
    }
}

// ---------------------------------------------------------------------------
// Manifest v1 (strict JSON, at most 64 MiB).
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputRow {
    path: String,
    sha256: String,
}

struct ManifestHeader {
    v: u64,
    workspace_id: String,
    source_revision: u64,
    producer: ProducerIdentity,
    invocation: String,
    config: String,
    artifact_sha256: String,
}

const MANIFEST_FIELDS: &[&str] = &[
    "v",
    "workspace_id",
    "source_revision",
    "producer",
    "invocation",
    "config",
    "artifact_sha256",
    "inputs",
];

type InputSink<'a> = &'a mut dyn FnMut(InputRow) -> FResult<()>;

struct ManifestVisitor<'a> {
    sink: InputSink<'a>,
    /// The typed error a rejected input row raised inside the sink.
    error: &'a mut Option<FoundryError>,
}

struct InputsSeed<'a> {
    sink: InputSink<'a>,
    error: &'a mut Option<FoundryError>,
}

fn once<T, E: serde::de::Error>(
    slot: &mut Option<T>,
    value: T,
    field: &'static str,
) -> Result<(), E> {
    if slot.is_some() {
        return Err(E::duplicate_field(field));
    }
    *slot = Some(value);
    Ok(())
}

impl<'de> Visitor<'de> for ManifestVisitor<'_> {
    type Value = ManifestHeader;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a snapshot manifest v1 object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ManifestHeader, A::Error> {
        use serde::de::Error;
        let (mut v, mut workspace_id, mut source_revision, mut producer) = (None, None, None, None);
        let (mut invocation, mut config, mut artifact_sha256) = (None, None, None);
        let mut inputs_seen = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "v" => once(&mut v, map.next_value::<u64>()?, "v")?,
                "workspace_id" => once(
                    &mut workspace_id,
                    map.next_value::<String>()?,
                    "workspace_id",
                )?,
                "source_revision" => once(
                    &mut source_revision,
                    map.next_value::<u64>()?,
                    "source_revision",
                )?,
                "producer" => once(
                    &mut producer,
                    map.next_value::<ProducerIdentity>()?,
                    "producer",
                )?,
                "invocation" => once(&mut invocation, map.next_value::<String>()?, "invocation")?,
                "config" => once(&mut config, map.next_value::<String>()?, "config")?,
                "artifact_sha256" => once(
                    &mut artifact_sha256,
                    map.next_value::<String>()?,
                    "artifact_sha256",
                )?,
                "inputs" => {
                    if inputs_seen {
                        return Err(A::Error::duplicate_field("inputs"));
                    }
                    inputs_seen = true;
                    map.next_value_seed(InputsSeed {
                        sink: &mut *self.sink,
                        error: &mut *self.error,
                    })?;
                }
                other => return Err(A::Error::unknown_field(other, MANIFEST_FIELDS)),
            }
        }
        if !inputs_seen {
            return Err(A::Error::missing_field("inputs"));
        }
        Ok(ManifestHeader {
            v: v.ok_or_else(|| A::Error::missing_field("v"))?,
            workspace_id: workspace_id.ok_or_else(|| A::Error::missing_field("workspace_id"))?,
            source_revision: source_revision
                .ok_or_else(|| A::Error::missing_field("source_revision"))?,
            producer: producer.ok_or_else(|| A::Error::missing_field("producer"))?,
            invocation: invocation.ok_or_else(|| A::Error::missing_field("invocation"))?,
            config: config.ok_or_else(|| A::Error::missing_field("config"))?,
            artifact_sha256: artifact_sha256
                .ok_or_else(|| A::Error::missing_field("artifact_sha256"))?,
        })
    }
}

impl<'de> DeserializeSeed<'de> for InputsSeed<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for InputsSeed<'_> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an array of {path, sha256} inputs")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(row) = seq.next_element::<InputRow>()? {
            if let Err(error) = (self.sink)(row) {
                *self.error = Some(error);
                return Err(serde::de::Error::custom("an input row was rejected"));
            }
        }
        Ok(())
    }
}

/// Stream the frozen manifest copy: `inputs` rows go to `sink` one at a time
/// (never a whole `Vec`); everything else is strict and bounded by the
/// manifest cap.
fn parse_manifest(path: &Path, sink: InputSink<'_>) -> FResult<ManifestHeader> {
    let file = File::open(path)?;
    let mut deserializer =
        serde_json::Deserializer::from_reader(BufReader::with_capacity(COPY_BUFFER, file));
    let mut error = None;
    let parsed = serde::Deserializer::deserialize_map(
        &mut deserializer,
        ManifestVisitor {
            sink,
            error: &mut error,
        },
    )
    .and_then(|header| deserializer.end().map(|()| header));
    match parsed {
        Ok(header) => Ok(header),
        Err(parse_error) => Err(error.take().unwrap_or_else(|| {
            FoundryError::InvalidArgument(format!(
                "the snapshot manifest is not valid manifest v1 JSON: {parse_error}"
            ))
        })),
    }
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn invalid(message: impl Into<String>) -> FoundryError {
    FoundryError::InvalidArgument(message.into())
}

fn check_identity_text(label: &str, text: &str, max: usize, non_empty: bool) -> FResult<()> {
    if (non_empty && text.is_empty()) || text.len() > max {
        return Err(invalid(format!(
            "manifest {label} must be {}..{max} bytes",
            u8::from(non_empty)
        )));
    }
    if text.chars().any(|c| c.is_control()) {
        return Err(invalid(format!(
            "manifest {label} contains control characters"
        )));
    }
    Ok(())
}

impl ManifestHeader {
    /// Shape and size rules that need no store.
    fn validate(&self) -> FResult<()> {
        if self.v != 1 {
            return Err(invalid("manifest v must be 1"));
        }
        if !is_hex64(&self.workspace_id) {
            return Err(invalid("manifest workspace_id must be 64 lowercase hex"));
        }
        if !is_hex64(&self.artifact_sha256) {
            return Err(invalid("manifest artifact_sha256 must be 64 lowercase hex"));
        }
        if !is_hex64(&self.producer.binary_sha256) {
            return Err(invalid(
                "manifest producer.binary_sha256 must be 64 lowercase hex",
            ));
        }
        check_identity_text("producer.name", &self.producer.name, IDENTITY_BYTES, true)?;
        check_identity_text(
            "producer.release_tag",
            &self.producer.release_tag,
            IDENTITY_BYTES,
            true,
        )?;
        check_identity_text(
            "producer.commit",
            &self.producer.commit,
            IDENTITY_TEXT_BYTES,
            false,
        )?;
        check_identity_text(
            "producer.version_output",
            &self.producer.version_output,
            IDENTITY_TEXT_BYTES,
            false,
        )?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Private scratch: the run directory, its meter and the frozen copies.
// ---------------------------------------------------------------------------

/// The owned run directory `<store>/import-scratch/run-<32 hex>/`. It is
/// removed when the run ends, whatever the outcome. A directory left by an
/// aborted process is removed by the next import only when it is positively
/// ours: the run-name shape plus an `OWNER` marker naming this magic and this
/// workspace. Anything else under the area is never touched.
struct ScratchDir {
    path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct OwnerMarker {
    magic: String,
    workspace_id: String,
    run: String,
}

fn is_run_name(name: &str) -> bool {
    name.strip_prefix("run-")
        .is_some_and(|rest| rest.len() == 32 && rest.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Remove run directories under `area` that carry our marker for this
/// workspace. Returns how many were removed.
fn remove_owned_leftovers(area: &Path, workspace_id: &str) -> FResult<usize> {
    let mut removed = 0;
    for entry in std::fs::read_dir(area)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let is_directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if !is_run_name(name) || !is_directory {
            continue;
        }
        let marker = std::fs::read_to_string(entry.path().join(OWNER_FILE))
            .ok()
            .and_then(|raw| serde_json::from_str::<OwnerMarker>(&raw).ok());
        if marker.is_some_and(|m| {
            m.magic == OWNER_MAGIC && m.workspace_id == workspace_id && m.run == name
        }) {
            std::fs::remove_dir_all(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

impl ScratchDir {
    fn acquire(store: &Path, workspace_id: &str) -> FResult<Self> {
        let area = store.join(SCRATCH_DIR);
        match std::fs::symlink_metadata(&area) {
            Ok(meta) if !meta.is_dir() => {
                return Err(FoundryError::Internal(anyhow::anyhow!(
                    "the import scratch area is not a directory: {}",
                    area.display()
                )));
            }
            Ok(_) => {}
            Err(_) => std::fs::create_dir_all(&area)?,
        }
        remove_owned_leftovers(&area, workspace_id)?;
        let name = format!("run-{}", uuid::Uuid::new_v4().simple());
        let path = area.join(&name);
        std::fs::create_dir(&path)?;
        let scratch = Self { path };
        let marker = OwnerMarker {
            magic: OWNER_MAGIC.to_owned(),
            workspace_id: workspace_id.to_owned(),
            run: name,
        };
        std::fs::write(
            scratch.path.join(OWNER_FILE),
            serde_json::to_string(&marker)?,
        )?;
        Ok(scratch)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Scratch filesystem usage of the run directory, checked between copy and
/// lookup batches. Library writes can overshoot within a batch, so the
/// observed peak is recorded rather than an OS quota promised.
struct ScratchMeter {
    dir: PathBuf,
    budget: u64,
    peak: u64,
}

impl ScratchMeter {
    fn used(&self) -> u64 {
        std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.metadata().ok())
            .filter(|meta| meta.is_file())
            .map(|meta| meta.len())
            .sum()
    }

    fn check(&mut self) -> FResult<()> {
        let used = self.used();
        self.peak = self.peak.max(used);
        if used > self.budget {
            return Err(fail(
                code::SCRATCH_FULL,
                format!(
                    "import scratch uses {used} bytes, over the {} byte budget",
                    self.budget
                ),
            ));
        }
        Ok(())
    }
}

struct Frozen {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

/// One import input. The CLI names a path and keeps its explicit-path
/// semantics; the MCP staging path hands over a descriptor it has already
/// opened without following links and fstat'ed as a regular file, so the
/// importer copies the very object that was checked and never re-resolves a
/// name.
pub enum ImportInput<'a> {
    Path(&'a Path),
    Open(File),
}

impl ImportInput<'_> {
    fn into_file(self, what: &str) -> FResult<File> {
        match self {
            Self::Open(file) => Ok(file),
            Self::Path(path) => open_explicit(path, what),
        }
    }
}

/// Open a named input without blocking - a FIFO without a writer must
/// refuse, not hang.
fn open_explicit(source: &Path, what: &str) -> FResult<File> {
    let unavailable = |e: std::io::Error| FoundryError::ArtifactUnavailable(format!("{what}: {e}"));
    use std::os::fd::FromRawFd;
    let name = std::ffi::CString::new(source.as_os_str().as_encoded_bytes())
        .map_err(|_| unavailable(std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL")))?;
    let fd = unsafe {
        libc::open(
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(unavailable(std::io::Error::last_os_error()));
    }
    // SAFETY: a fresh descriptor from open(2), owned by this File.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Copy `input` into the run directory in at most 1 MiB buffers, hashing the
/// COPY as it is written. A cap overrun fails by name without a retry.
fn freeze(
    input: ImportInput<'_>,
    target: PathBuf,
    cap: u64,
    over: &'static str,
    what: &str,
    control: &Control,
    meter: &mut ScratchMeter,
) -> FResult<Frozen> {
    let unavailable = |e: std::io::Error| FoundryError::ArtifactUnavailable(format!("{what}: {e}"));
    // Stat the descriptor itself: the object actually read is the object
    // that was checked.
    let mut input = input.into_file(what)?;
    let metadata = input.metadata().map_err(unavailable)?;
    if !metadata.is_file() {
        return Err(FoundryError::ArtifactUnavailable(format!(
            "{what} is not a regular file"
        )));
    }
    if metadata.len() > cap {
        return Err(fail(
            over,
            format!("{what} is {} bytes; the limit is {cap}", metadata.len()),
        ));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUFFER];
    let mut total = 0u64;
    loop {
        control.check()?;
        let read = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(unavailable(e)),
        };
        total += read as u64;
        if total > cap {
            return Err(fail(
                over,
                format!("{what} grew past the {cap} byte limit while it was copied"),
            ));
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
        meter.check()?;
        fault!(SCIP_COPY_BUFFER, None, Some(control), what)?;
    }
    output.flush()?;
    Ok(Frozen {
        path: target,
        bytes: total,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

// ---------------------------------------------------------------------------
// The disposable lookup (redb in the run directory).
// ---------------------------------------------------------------------------

/// Manifest inputs: path -> sha256.
const INPUTS: TableDefinition<&str, &str> = TableDefinition::new("inputs");
/// Artifact documents found in pass 1: path -> "" (usable) or the failure code.
const DOCUMENTS: TableDefinition<&str, &str> = TableDefinition::new("documents");
/// Definition occurrences per symbol id (after in-document deduplication).
const DEFINITIONS: TableDefinition<&str, u32> = TableDefinition::new("definitions");

struct Lookup {
    db: Database,
}

impl Lookup {
    fn create(path: &Path) -> FResult<Self> {
        let db = redb::Builder::new().set_cache_size(16 << 20).create(path)?;
        // Every table exists before the first read of any of them.
        let tx = db.begin_write()?;
        tx.open_table(INPUTS)?;
        tx.open_table(DOCUMENTS)?;
        tx.open_table(DEFINITIONS)?;
        tx.commit()?;
        Ok(Self { db })
    }

    /// A scratch transaction: nothing here needs to survive a crash.
    fn write(&self) -> FResult<redb::WriteTransaction> {
        let mut tx = self.db.begin_write()?;
        tx.set_durability(Durability::None)
            .map_err(|e| FoundryError::Internal(anyhow::anyhow!("scratch durability: {e}")))?;
        Ok(tx)
    }

    fn input_hash(&self, path: &str) -> FResult<Option<String>> {
        let tx = self.db.begin_read()?;
        let inputs = tx.open_table(INPUTS)?;
        Ok(inputs.get(path)?.map(|v| v.value().to_owned()))
    }

    /// `Some(code)` when pass 1 already failed this document.
    fn document_failure(&self, path: &str) -> FResult<Option<String>> {
        let tx = self.db.begin_read()?;
        let documents = tx.open_table(DOCUMENTS)?;
        Ok(documents
            .get(path)?
            .map(|v| v.value().to_owned())
            .filter(|code| !code.is_empty()))
    }

    fn has_document(&self, path: &str) -> FResult<bool> {
        let tx = self.db.begin_read()?;
        let documents = tx.open_table(DOCUMENTS)?;
        Ok(documents.get(path)?.is_some())
    }

    /// A page of manifest inputs strictly after `after`, in path order.
    fn input_page(&self, after: Option<&str>, size: usize) -> FResult<Vec<(String, String)>> {
        let tx = self.db.begin_read()?;
        let inputs = tx.open_table(INPUTS)?;
        let lower = after.map_or(Bound::Unbounded, Bound::Excluded);
        let mut page = Vec::new();
        for row in inputs.range::<&str>((lower, Bound::Unbounded))? {
            let (path, hash) = row?;
            page.push((path.value().to_owned(), hash.value().to_owned()));
            if page.len() == size {
                break;
            }
        }
        Ok(page)
    }
}

// ---------------------------------------------------------------------------
// Streaming the artifact: `Index.documents`, one message at a time.
// ---------------------------------------------------------------------------

fn wire(error: protobuf::Error) -> FoundryError {
    fail(
        code::PRODUCER_INCOMPLETE,
        format!("the SCIP artifact cannot be decoded: {error}"),
    )
}

enum DocumentEvent {
    /// One document message within the size cap, still undecoded.
    Message { ordinal: u64, bytes: Vec<u8> },
    /// A document message over the cap; its path is read only by streaming
    /// the leading fields.
    Oversized {
        ordinal: u64,
        path: Option<String>,
        bytes: u64,
    },
}

/// Read the last `relative_path` of an oversized document by streaming its
/// whole message, skipping every other field undecoded: singular protobuf
/// field semantics keep the LAST value, so this must agree with normal
/// decoding or a document could be attributed to a path it does not have.
/// The path is field 1, but the generated Rust writers (rust-analyzer's)
/// emit `language` first, and a path over the string bound is skipped whole;
/// a message that never names a usable path is reported by ordinal.
fn peek_path(input: &mut CodedInputStream<'_>, length: u32) -> FResult<Option<String>> {
    let old = input.push_limit(u64::from(length)).map_err(wire)?;
    let mut path = None;
    while let Some(tag) = input.read_raw_tag_or_eof().map_err(wire)? {
        if tag == PATH_TAG {
            let n = input.read_raw_varint32().map_err(wire)?;
            if n <= 8192 {
                path = String::from_utf8(input.read_raw_bytes(n).map_err(wire)?).ok();
            } else {
                input.skip_raw_bytes(n).map_err(wire)?;
            }
        } else {
            skip_field(input, tag)?;
        }
    }
    if input.bytes_until_limit() != 0 {
        // The artifact physically ended inside the message: its declared
        // length runs past the end of the file. That is a truncated
        // artifact (`producer_incomplete`, before any selection), not a
        // document that merely failed its cap.
        return Err(malformed_field(
            "a document message ends before its declared length",
        ));
    }
    input.pop_limit(old);
    Ok(path)
}

/// Stream the frozen artifact copy. Everything but `Index.documents` is
/// skipped undecoded; a truncated or malformed message is
/// `producer_incomplete`.
fn for_each_document(
    artifact: &Path,
    control: &Control,
    limits: &ImportLimits,
    each: &mut dyn FnMut(DocumentEvent) -> FResult<()>,
) -> FResult<()> {
    let mut file = BufReader::with_capacity(COPY_BUFFER, File::open(artifact)?);
    let mut input = CodedInputStream::from_buf_read(&mut file);
    let mut ordinal = 0u64;
    while let Some(tag) = input.read_raw_tag_or_eof().map_err(wire)? {
        control.check()?;
        if tag == DOCUMENTS_TAG {
            ordinal += 1;
            let length = input.read_raw_varint32().map_err(wire)?;
            if u64::from(length) > limits.document_bytes {
                let path = peek_path(&mut input, length)?;
                each(DocumentEvent::Oversized {
                    ordinal,
                    path,
                    bytes: u64::from(length),
                })?;
                continue;
            }
            // The message is bounded by the document cap, so buffering it
            // keeps allocation bounded while its fields are streamed below.
            let bytes = input.read_raw_bytes(length).map_err(wire)?;
            each(DocumentEvent::Message { ordinal, bytes })?;
        } else {
            skip_field(&mut input, tag)?;
        }
    }
    Ok(())
}

/// Stream one buffered document message's fields: count the occurrence
/// submessages and capture the `relative_path`. Every other field is skipped
/// undecoded, so an oversized occurrence count is refused before the
/// occurrences are materialized.
fn scan_message_fields(bytes: &[u8]) -> FResult<(usize, Option<String>)> {
    let mut input = CodedInputStream::from_bytes(bytes);
    // Bound the stream to the message, so every length below is checked
    // against what the message actually holds (not only against end of
    // input) and `bytes_until_limit` is meaningful.
    input.push_limit(bytes.len() as u64).map_err(wire)?;
    let (mut occurrences, mut path) = (0usize, None);
    while let Some(tag) = input.read_raw_tag_or_eof().map_err(wire)? {
        match tag {
            PATH_TAG => {
                let n = input.read_raw_varint32().map_err(wire)?;
                if n <= 8192 {
                    path = String::from_utf8(input.read_raw_bytes(n).map_err(wire)?).ok();
                } else {
                    input.skip_raw_bytes(n).map_err(wire)?;
                }
            }
            tag if tag >> 3 == 2 && tag & 7 == 2 => {
                let n = input.read_raw_varint32().map_err(wire)?;
                occurrences += 1;
                input.skip_raw_bytes(n).map_err(wire)?;
            }
            _ => {
                skip_field(&mut input, tag)?;
            }
        }
    }
    Ok((occurrences, path))
}

/// What the importer keeps of one `Occurrence`: its ranges (deprecated and
/// typed), symbol text (only within the symbol cap) and roles. Everything
/// else - documentation overrides, syntax kinds, diagnostics - is skipped
/// undecoded, never allocated.
#[derive(Default)]
struct SlimOccurrence {
    range: Vec<i32>,
    enclosing_range: Vec<i32>,
    typed_range: Option<RawRange>,
    typed_enclosing_range: Option<RawRange>,
    symbol: String,
    /// The symbol's byte length, kept even when an over-cap symbol was
    /// skipped (then `symbol` is empty and the document fails by name).
    symbol_len: usize,
    symbol_roles: i32,
}

/// What the importer keeps of one `Document`: the declared position encoding
/// and its slim occurrences. `symbols`, `text`, `language` and every unknown
/// field are stream-skipped.
#[derive(Default)]
struct SlimDocument {
    position_encoding: i32,
    occurrences: Vec<SlimOccurrence>,
}

/// Nested groups an unknown field may open (protobuf's own limit).
const GROUP_DEPTH_LIMIT: usize = 100;

fn malformed_field(what: &str) -> FoundryError {
    fail(
        code::PRODUCER_INCOMPLETE,
        format!("the SCIP artifact cannot be decoded: {what}"),
    )
}

/// Skip one field (and, for a group, everything through its matching end)
/// whose `tag` was just read, refusing exactly what the generated decoder
/// refused: field number 0, wire types 6 and 7, an end group that does not
/// close the group opened (or closes none), a group left open at the end of
/// its parent, and a length that overruns its parent. Nothing is
/// materialized.
fn skip_field(input: &mut CodedInputStream<'_>, tag: u32) -> FResult<()> {
    let mut open: Vec<u32> = Vec::new();
    let mut tag = tag;
    loop {
        let number = tag >> 3;
        if number == 0 {
            return Err(malformed_field("field number 0"));
        }
        match tag & 7 {
            0 => {
                input.read_raw_varint64().map_err(wire)?;
            }
            1 => {
                input.read_fixed64().map_err(wire)?;
            }
            2 => {
                let length = input.read_raw_varint32().map_err(wire)?;
                if u64::from(length) > input.bytes_until_limit() {
                    return Err(malformed_field("a length overruns its message"));
                }
                input.skip_raw_bytes(length).map_err(wire)?;
            }
            3 => {
                if open.len() >= GROUP_DEPTH_LIMIT {
                    return Err(malformed_field("groups nest too deeply"));
                }
                open.push(number);
            }
            4 => {
                if open.pop() != Some(number) {
                    return Err(malformed_field("an end group closes no matching group"));
                }
            }
            5 => {
                input.read_fixed32().map_err(wire)?;
            }
            _ => return Err(malformed_field("an invalid wire type")),
        }
        if open.is_empty() {
            return Ok(());
        }
        tag = input
            .read_raw_tag_or_eof()
            .map_err(wire)?
            .ok_or_else(|| malformed_field("a group is not terminated"))?;
    }
}

/// `SingleLineRange` (`multi` false: line, start, end) or `MultiLineRange`
/// (`multi` true: start line, start, end line, end), length-delimited.
fn read_range_message(input: &mut CodedInputStream<'_>, multi: bool) -> FResult<RawRange> {
    let length = input.read_raw_varint32().map_err(wire)?;
    let old = input.push_limit(u64::from(length)).map_err(wire)?;
    let mut range = RawRange {
        start_line: 0,
        start_character: 0,
        end_line: 0,
        end_character: 0,
    };
    while let Some(tag) = input.read_raw_tag_or_eof().map_err(wire)? {
        match (tag, multi) {
            (8, _) => range.start_line = input.read_int32().map_err(wire)?,
            (16, _) => range.start_character = input.read_int32().map_err(wire)?,
            (24, false) => range.end_character = input.read_int32().map_err(wire)?,
            (24, true) => range.end_line = input.read_int32().map_err(wire)?,
            (32, true) => range.end_character = input.read_int32().map_err(wire)?,
            _ => skip_field(input, tag)?,
        }
    }
    input.pop_limit(old);
    if !multi {
        range.end_line = range.start_line;
    }
    Ok(range)
}

fn read_occurrence(
    input: &mut CodedInputStream<'_>,
    limits: &ImportLimits,
) -> FResult<SlimOccurrence> {
    let length = input.read_raw_varint32().map_err(wire)?;
    let old = input.push_limit(u64::from(length)).map_err(wire)?;
    let mut occurrence = SlimOccurrence::default();
    while let Some(tag) = input.read_raw_tag_or_eof().map_err(wire)? {
        match tag {
            // range (packed, or the unpacked element form)
            10 => input
                .read_repeated_packed_int32_into(&mut occurrence.range)
                .map_err(wire)?,
            8 => occurrence.range.push(input.read_int32().map_err(wire)?),
            18 => {
                let n = input.read_raw_varint32().map_err(wire)?;
                occurrence.symbol_len = n as usize;
                if n as usize <= limits.symbol_bytes {
                    occurrence.symbol = String::from_utf8(input.read_raw_bytes(n).map_err(wire)?)
                        .map_err(|_| {
                        fail(
                            code::PRODUCER_INCOMPLETE,
                            "the SCIP artifact has a symbol that is not UTF-8",
                        )
                    })?;
                } else {
                    input.skip_raw_bytes(n).map_err(wire)?;
                }
            }
            24 => occurrence.symbol_roles = input.read_int32().map_err(wire)?,
            // enclosing_range (packed, or the unpacked element form)
            58 => input
                .read_repeated_packed_int32_into(&mut occurrence.enclosing_range)
                .map_err(wire)?,
            56 => occurrence
                .enclosing_range
                .push(input.read_int32().map_err(wire)?),
            66 => occurrence.typed_range = Some(read_range_message(input, false)?),
            74 => occurrence.typed_range = Some(read_range_message(input, true)?),
            82 => occurrence.typed_enclosing_range = Some(read_range_message(input, false)?),
            90 => occurrence.typed_enclosing_range = Some(read_range_message(input, true)?),
            // override_documentation, syntax_kind, diagnostics and every
            // unknown field.
            _ => skip_field(input, tag)?,
        }
    }
    input.pop_limit(old);
    Ok(occurrence)
}

/// Decode the importer-relevant fields of one buffered document message.
/// Returns the raw `relative_path` text.
fn read_slim_document(bytes: &[u8], limits: &ImportLimits) -> FResult<(String, SlimDocument)> {
    let mut input = CodedInputStream::from_bytes(bytes);
    // Bound the stream to the message, as `scan_message_fields` does: this
    // decoder must not depend on the scan having run first.
    input.push_limit(bytes.len() as u64).map_err(wire)?;
    let (mut path, mut document) = (String::new(), SlimDocument::default());
    while let Some(tag) = input.read_raw_tag_or_eof().map_err(wire)? {
        match tag {
            PATH_TAG => path = input.read_string().map_err(wire)?,
            // occurrences
            18 => document
                .occurrences
                .push(read_occurrence(&mut input, limits)?),
            // position_encoding
            48 => document.position_encoding = input.read_int32().map_err(wire)?,
            // symbols, language, text and every unknown field
            _ => skip_field(&mut input, tag)?,
        }
    }
    Ok((path, document))
}

enum DecodedDocument {
    /// A decodable, in-cap document with its normalized path.
    Valid {
        path: String,
        document: Box<SlimDocument>,
    },
    /// A per-document failure with the best path label available. `path` is
    /// `None` when no real path was decoded: the ordinal is only a failure
    /// label, never a source path.
    Failed {
        path: Option<String>,
        failure: DocFailure,
    },
}

/// Decode one buffered document message. The occurrence count is checked
/// against the input cap FIRST - 16,384, before deduplication and before the
/// occurrences are materialized - then only the importer-relevant fields are
/// decoded.
fn decode_document_message(bytes: &[u8], limits: &ImportLimits) -> FResult<DecodedDocument> {
    let (occurrences, leading) = scan_message_fields(bytes)?;
    if occurrences > limits.document_occurrences {
        let path = match leading {
            Some(raw) => Some(normalize_document_path(&raw)?),
            None => None,
        };
        return Ok(DecodedDocument::Failed {
            path,
            failure: doc_failure(
                code::DOCUMENT_TOO_LARGE,
                format!(
                    "{occurrences} occurrences; the limit is {}",
                    limits.document_occurrences
                ),
            ),
        });
    }
    let (raw_path, document) = read_slim_document(bytes, limits)?;
    let path = normalize_document_path(&raw_path)?;
    Ok(DecodedDocument::Valid {
        path,
        document: Box::new(document),
    })
}

/// Strip one leading `./`, then the shared path rules: the normalized form
/// that duplicate detection compares.
fn normalize_document_path(raw: &str) -> FResult<String> {
    let path = raw.strip_prefix("./").unwrap_or(raw);
    validate_path(path).map_err(|_| {
        invalid(format!(
            "the artifact names a document path that is not a normalized relative path: {}",
            clip(raw, 200)
        ))
    })?;
    Ok(path.to_owned())
}

// ---------------------------------------------------------------------------
// Positions: SCIP ranges to UTF-8 byte ranges.
// ---------------------------------------------------------------------------

/// A failed document: it names its path and leaves its scope unchanged.
struct DocFailure {
    code: &'static str,
    message: String,
}

fn doc_failure(code: &'static str, message: impl Into<String>) -> DocFailure {
    DocFailure {
        code,
        message: message.into(),
    }
}

/// A SCIP range in the document's declared coordinates: zero-based lines and
/// UTF-8 code-unit offsets from the line start, half-open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RawRange {
    start_line: i32,
    start_character: i32,
    end_line: i32,
    end_character: i32,
}

fn raw_from_slice(range: &[i32]) -> Result<RawRange, String> {
    match *range {
        [line, start, end] => Ok(RawRange {
            start_line: line,
            start_character: start,
            end_line: line,
            end_character: end,
        }),
        [start_line, start_character, end_line, end_character] => Ok(RawRange {
            start_line,
            start_character,
            end_line,
            end_character,
        }),
        _ => Err(format!(
            "range has {} elements; expected 3 or 4",
            range.len()
        )),
    }
}

/// `typed_range` wins over the deprecated `range` when present.
fn occurrence_range(occurrence: &SlimOccurrence) -> Result<RawRange, String> {
    match occurrence.typed_range {
        Some(range) => Ok(range),
        None => raw_from_slice(&occurrence.range),
    }
}

/// `typed_enclosing_range` wins over the deprecated `enclosing_range`.
fn enclosing_range(occurrence: &SlimOccurrence) -> Result<Option<RawRange>, String> {
    match occurrence.typed_enclosing_range {
        Some(range) => Ok(Some(range)),
        None if occurrence.enclosing_range.is_empty() => Ok(None),
        None => raw_from_slice(&occurrence.enclosing_range).map(Some),
    }
}

/// Byte offset of each line start. Lines end at LF; a CR before it belongs to
/// the line. A source ending in LF has a final empty line.
struct LineIndex<'a> {
    body: &'a str,
    starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    fn new(body: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(body.match_indices('\n').map(|(at, _)| at + 1));
        Self { body, starts }
    }

    fn position(&self, line: i32, character: i32) -> Result<usize, String> {
        let (Ok(line), Ok(character)) = (usize::try_from(line), usize::try_from(character)) else {
            return Err("a position is negative".into());
        };
        let Some(&line_start) = self.starts.get(line) else {
            return Err(format!("line {line} is outside the source"));
        };
        let line_end = self
            .starts
            .get(line + 1)
            .map_or(self.body.len(), |next| next - 1);
        match line_start.checked_add(character) {
            Some(byte) if byte <= line_end => Ok(byte),
            _ => Err(format!(
                "character {character} is past the end of line {line}"
            )),
        }
    }

    /// UTF-8 byte range of `range`; both ends on codepoint boundaries. An
    /// occurrence must be non-empty; an enclosing range may be empty.
    fn bytes(&self, range: RawRange, allow_empty: bool) -> Result<(usize, usize), String> {
        let start = self.position(range.start_line, range.start_character)?;
        let end = self.position(range.end_line, range.end_character)?;
        if start > end || (start == end && !allow_empty) {
            return Err("the range is empty or inverted".into());
        }
        if !self.body.is_char_boundary(start) || !self.body.is_char_boundary(end) {
            return Err("a range end splits a UTF-8 codepoint".into());
        }
        Ok((start, end))
    }
}

/// The document-level checks that need no source text: the declared
/// encoding and every symbol's size.
fn check_document(
    path: &str,
    document: &SlimDocument,
    limits: &ImportLimits,
) -> Result<(), DocFailure> {
    match PositionEncoding::from_i32(document.position_encoding) {
        Some(PositionEncoding::UTF8CodeUnitOffsetFromLineStart) => {}
        other => {
            return Err(doc_failure(
                code::UNSUPPORTED_ENCODING,
                format!(
                    "{path}: only UTF8CodeUnitOffsetFromLineStart is accepted, found {other:?} ({})",
                    document.position_encoding
                ),
            ));
        }
    }
    for occurrence in &document.occurrences {
        if occurrence.symbol_len > limits.symbol_bytes {
            return Err(doc_failure(
                code::DOCUMENT_TOO_LARGE,
                format!(
                    "{path}: a symbol is {} bytes; the limit is {}",
                    occurrence.symbol_len, limits.symbol_bytes
                ),
            ));
        }
    }
    Ok(())
}

/// Validate one document against its verified indexed bytes and return its
/// definition symbol ids (deduplicated by the conversion). A document that
/// fails never establishes resolution: its definitions do not reach the
/// lookup.
fn validate_document(
    engine: &Engine,
    namespace: &str,
    revision: u64,
    limits: &ImportLimits,
    path: &str,
    hash: &str,
    document: &SlimDocument,
) -> FResult<Result<Vec<String>, DocFailure>> {
    if let Err(failure) = check_document(path, document, limits) {
        return Ok(Err(failure));
    }
    let body = match read_source(engine, revision, path, hash)? {
        Ok(body) => body,
        Err(failure) => return Ok(Err(failure)),
    };
    let occurrences = match convert_document(namespace, path, document, &body) {
        Ok(occurrences) => occurrences,
        Err(failure) => return Ok(Err(failure)),
    };
    Ok(Ok(occurrences
        .into_iter()
        .filter(|occurrence| occurrence.kind == OccurrenceKind::Definition)
        .map(|occurrence| occurrence.symbol_id)
        .collect()))
}

/// Validate and convert every occurrence of a document against its source
/// bytes: typed ranges first, UTF-8 byte ranges on codepoint boundaries,
/// exact duplicates deduplicated. Sorted by position.
fn convert_document(
    namespace: &str,
    path: &str,
    document: &SlimDocument,
    body: &str,
) -> Result<Vec<NewOccurrence>, DocFailure> {
    let index = LineIndex::new(body);
    let mut unique: BTreeMap<(u64, u64, OccurrenceKind, String), NewOccurrence> = BTreeMap::new();
    for (ordinal, occurrence) in document.occurrences.iter().enumerate() {
        let malformed = |what: String| {
            doc_failure(
                code::INVALID_RANGE,
                format!("{path}: occurrence {ordinal}: {what}"),
            )
        };
        let range = occurrence_range(occurrence).map_err(malformed)?;
        let (start, end) = index.bytes(range, false).map_err(malformed)?;
        if let Some(enclosing) = enclosing_range(occurrence).map_err(malformed)? {
            index.bytes(enclosing, true).map_err(malformed)?;
        }
        if occurrence.symbol.is_empty() {
            continue;
        }
        let kind = if occurrence.symbol_roles & 1 != 0 {
            OccurrenceKind::Definition
        } else {
            OccurrenceKind::Reference
        };
        let id = symbol_id(namespace, path, &occurrence.symbol);
        let entry = unique
            .entry((start as u64, end as u64, kind, id.clone()))
            .or_insert_with(|| NewOccurrence {
                kind,
                start: start as u64,
                end: end as u64,
                symbol_id: id,
                symbol: occurrence.symbol.clone(),
                roles: 0,
            });
        entry.roles |= occurrence.symbol_roles;
    }
    Ok(unique.into_values().collect())
}

// ---------------------------------------------------------------------------
// The import run.
// ---------------------------------------------------------------------------

/// What a producer's profile says about a manifest path the artifact lacks.
enum Absent {
    /// rust-analyzer skips documents with zero occurrences: a listed `.rs`
    /// source absent from the artifact is accepted-empty.
    AcceptedEmpty,
    /// Not a source the producer indexes (non-`.rs` for rust-analyzer):
    /// coverage is unaffected.
    OutsideScope,
    /// No profile: an absent document is unknown, never empty.
    Unknown,
}

fn absent_policy(producer: &str, path: &str) -> Absent {
    if producer == "rust-analyzer" {
        if path.ends_with(".rs") {
            Absent::AcceptedEmpty
        } else {
            Absent::OutsideScope
        }
    } else {
        Absent::Unknown
    }
}

struct Run<'a> {
    engine: &'a Engine,
    control: &'a Control,
    limits: &'a ImportLimits,
    dir: ScratchDir,
    meter: ScratchMeter,
    report: ImportReport,
}

/// Everything the publication phase needs from the preflight.
struct Prepared {
    tuple: SnapshotTuple,
    lookup: Lookup,
    artifact: PathBuf,
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

impl Run<'_> {
    /// Copy, parse and bind everything without changing any graph state.
    /// Any failure here leaves the prior selection untouched.
    fn prepare(
        &mut self,
        index: ImportInput<'_>,
        snapshot: ImportInput<'_>,
        workspace_id: &str,
    ) -> FResult<Prepared> {
        let copy_started = Instant::now();
        let control = self.control;
        let manifest = freeze(
            snapshot,
            self.dir.path.join("manifest.json"),
            self.limits.manifest_bytes,
            code::MANIFEST_TOO_LARGE,
            "snapshot manifest",
            control,
            &mut self.meter,
        )?;
        let artifact = freeze(
            index,
            self.dir.path.join("artifact.scip"),
            self.limits.artifact_bytes,
            code::ARTIFACT_TOO_LARGE,
            "SCIP artifact",
            control,
            &mut self.meter,
        )?;
        self.report.copied_bytes = manifest.bytes + artifact.bytes;
        self.report.manifest_sha256 = manifest.sha256.clone();
        self.report.artifact_sha256 = artifact.sha256.clone();
        self.report.timings.copy_ms = millis(copy_started);
        // Producers' files are frozen: from here on only the copies are read.
        fault!(SCIP_AFTER_COPY, Some(self.engine), Some(control), "")?;
        control.check()?;

        let lookup_started = Instant::now();
        let lookup = Lookup::create(&self.dir.path.join("lookup.redb"))?;
        let header = self.load_manifest(&lookup, &manifest.path)?;
        header.validate()?;
        self.report.producer = header.producer.name.clone();
        self.report.source_revision = header.source_revision;
        if header.workspace_id != workspace_id {
            return Err(fail(
                code::UNBOUND_ARTIFACT,
                "the manifest names a different workspace than this store is bound to",
            ));
        }
        let current = self.engine.source_revision()?;
        if header.source_revision != current {
            return Err(fail(
                code::STALE_ARTIFACT,
                format!(
                    "the manifest was produced at source revision {}, the store is at {current}",
                    header.source_revision
                ),
            ));
        }
        if artifact.sha256 != header.artifact_sha256 {
            return Err(fail(
                code::STALE_ARTIFACT,
                "the frozen artifact copy does not match the manifest's artifact_sha256",
            ));
        }
        self.bind_inputs(&lookup)?;
        let tuple = SnapshotTuple::new(
            header.producer.clone(),
            &header.invocation,
            &header.config,
            artifact.sha256.clone(),
            manifest.sha256.clone(),
            header.source_revision,
        );
        self.report.snapshot_id = tuple.snapshot_id.clone();
        self.lookup_pass(
            &lookup,
            &artifact.path,
            &header.producer.name,
            header.source_revision,
        )?;
        self.report.timings.lookup_ms = millis(lookup_started);
        fault!(SCIP_AFTER_LOOKUP, Some(self.engine), Some(control), "")?;
        control.check()?;
        Ok(Prepared {
            tuple,
            lookup,
            artifact: artifact.path,
        })
    }

    /// Stream the frozen manifest into the lookup in batches of 128 rows.
    /// Duplicate paths are `duplicate_input`, found by the lookup itself.
    fn load_manifest(&mut self, lookup: &Lookup, manifest: &Path) -> FResult<ManifestHeader> {
        let control = self.control;
        let meter = &mut self.meter;
        let mut batch: Vec<InputRow> = Vec::new();
        let mut previous: Option<String> = None;
        let flush = |batch: &mut Vec<InputRow>, meter: &mut ScratchMeter| -> FResult<()> {
            if batch.is_empty() {
                return Ok(());
            }
            let tx = lookup.write()?;
            {
                let mut inputs = tx.open_table(INPUTS)?;
                for row in batch.drain(..) {
                    if inputs
                        .insert(row.path.as_str(), row.sha256.as_str())?
                        .is_some()
                    {
                        return Err(duplicate_input(&row.path));
                    }
                }
            }
            tx.commit()?;
            meter.check()?;
            control.check()
        };
        let header = parse_manifest(manifest, &mut |row: InputRow| {
            validate_path(&row.path).map_err(|_| {
                invalid(format!(
                    "the manifest names an input path that is not a normalized relative path: {}",
                    clip(&row.path, 200)
                ))
            })?;
            if !is_hex64(&row.sha256) {
                return Err(invalid(format!(
                    "the manifest input hash is not 64 lowercase hex: {}",
                    clip(&row.path, 200)
                )));
            }
            if let Some(previous) = previous.as_deref() {
                match row.path.as_str().cmp(previous) {
                    std::cmp::Ordering::Equal => return Err(duplicate_input(&row.path)),
                    std::cmp::Ordering::Less => {
                        // Either an out-of-order duplicate or an unsorted list.
                        let seen = batch.iter().any(|b| b.path == row.path)
                            || lookup.input_hash(&row.path)?.is_some();
                        return Err(if seen {
                            duplicate_input(&row.path)
                        } else {
                            invalid("manifest inputs must be sorted by path")
                        });
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            previous = Some(row.path.clone());
            batch.push(row);
            if batch.len() == INPUT_BATCH {
                flush(&mut batch, meter)?;
            }
            Ok(())
        })?;
        flush(&mut batch, meter)?;
        Ok(header)
    }

    /// Every manifest input must be an indexed source with the manifest's
    /// hash; paged by 128 rows.
    fn bind_inputs(&self, lookup: &Lookup) -> FResult<()> {
        let mut after: Option<String> = None;
        loop {
            self.control.check()?;
            let page = lookup.input_page(after.as_deref(), INPUT_BATCH)?;
            let Some((last, _)) = page.last() else {
                return Ok(());
            };
            after = Some(last.clone());
            let tx = self.engine.db.begin_read()?;
            let sources = tx.open_table(SOURCES)?;
            for (path, hash) in &page {
                let Some(raw) = sources.get(path.as_str())? else {
                    return Err(fail(
                        code::STALE_ARTIFACT,
                        format!(
                            "a manifest input is not an indexed source: {}",
                            clip(path, 200)
                        ),
                    ));
                };
                let meta: SourceMeta = decode(raw.value(), "source")?;
                if meta.hash != *hash {
                    return Err(fail(
                        code::STALE_ARTIFACT,
                        format!(
                            "a manifest input hash differs from the indexed source: {}",
                            clip(path, 200)
                        ),
                    ));
                }
            }
        }
    }

    /// Pass 1: stream the documents, bind each to the manifest, reject
    /// duplicates and count definitions per symbol. Per-document failures
    /// that need no source text are recorded by path; the rest are global.
    fn lookup_pass(
        &mut self,
        lookup: &Lookup,
        artifact: &Path,
        namespace: &str,
        revision: u64,
    ) -> FResult<()> {
        let (engine, control, limits) = (self.engine, self.control, self.limits);
        let (report, meter) = (&mut self.report, &mut self.meter);
        let mut transaction = Some(lookup.write()?);
        let mut in_batch = 0usize;
        for_each_document(artifact, control, limits, &mut |event| {
            let tx = transaction
                .as_ref()
                .ok_or_else(|| FoundryError::Internal(anyhow::anyhow!("scratch transaction")))?;
            report.documents += 1;
            // `named` is false only for a document whose path could not be
            // read at all: it cannot be bound, deduplicated or looked up, so
            // it fails by ordinal alone.
            let (path, named, outcome) = match event {
                DocumentEvent::Oversized {
                    ordinal,
                    path,
                    bytes,
                } => {
                    let (label, named) = match path {
                        Some(raw) => (normalize_document_path(&raw)?, true),
                        None => (format!("document #{ordinal}"), false),
                    };
                    let failure = doc_failure(
                        code::DOCUMENT_TOO_LARGE,
                        format!(
                            "{label}: the document message is {bytes} bytes; the limit is {}",
                            limits.document_bytes
                        ),
                    );
                    (label, named, Err(failure))
                }
                DocumentEvent::Message { ordinal, bytes } => {
                    match decode_document_message(&bytes, limits)? {
                        DecodedDocument::Failed { path, failure } => {
                            let named = path.is_some();
                            let label = path.unwrap_or_else(|| format!("document #{ordinal}"));
                            (label, named, Err(failure))
                        }
                        DecodedDocument::Valid { path, document } => {
                            let hash = tx
                                .open_table(INPUTS)?
                                .get(path.as_str())?
                                .map(|value| value.value().to_owned());
                            let Some(hash) = hash else {
                                return Err(fail(
                                    code::UNBOUND_ARTIFACT,
                                    format!(
                                        "the artifact has a document the manifest does not list: {}",
                                        clip(&path, 200)
                                    ),
                                ));
                            };
                            let outcome = validate_document(
                                engine, namespace, revision, limits, &path, &hash, &document,
                            )?;
                            (path, true, outcome)
                        }
                    }
                }
            };
            if named {
                let bound = tx.open_table(INPUTS)?.get(path.as_str())?.is_some();
                if !bound {
                    return Err(fail(
                        code::UNBOUND_ARTIFACT,
                        format!(
                            "the artifact has a document the manifest does not list: {}",
                            clip(&path, 200)
                        ),
                    ));
                }
                let status = outcome.as_ref().err().map_or("", |failure| failure.code);
                let mut documents = tx.open_table(DOCUMENTS)?;
                let previous = documents.insert(path.as_str(), status)?;
                if previous.is_some() {
                    return Err(fail(
                        code::DUPLICATE_DOCUMENT,
                        format!("the artifact has two documents for {}", clip(&path, 200)),
                    ));
                }
            }
            match outcome {
                Ok(definitions) => {
                    let mut counts = tx.open_table(DEFINITIONS)?;
                    for id in definitions {
                        let seen = counts.get(id.as_str())?.map_or(0, |v| v.value());
                        counts.insert(id.as_str(), seen.saturating_add(1))?;
                    }
                }
                Err(failure) => report.note_failure(&path, failure.code, &failure.message),
            }
            in_batch += 1;
            if in_batch == DOCUMENT_BATCH {
                if let Some(tx) = transaction.take() {
                    tx.commit()?;
                }
                meter.check()?;
                transaction = Some(lookup.write()?);
                in_batch = 0;
            }
            Ok(())
        })?;
        if let Some(tx) = transaction.take() {
            tx.commit()?;
        }
        meter.check()
    }

    /// Select the snapshot tuple after the preflight and lookup succeeded,
    /// before the first document is published.
    fn select(&mut self, prepared: &Prepared) -> FResult<()> {
        self.engine.select_snapshot(&prepared.tuple)?;
        self.report.selected = true;
        Ok(())
    }

    /// Pass 2 and the rest of the publication. An error is an interruption:
    /// the report keeps what was committed, and every committed scope
    /// survives.
    fn publish(&mut self, prepared: &Prepared) -> FResult<()> {
        let publish_started = Instant::now();
        let outcome = self.publish_documents(prepared);
        self.report.timings.publish_ms = millis(publish_started);
        outcome
    }

    fn publish_documents(&mut self, prepared: &Prepared) -> FResult<()> {
        let (engine, control, limits) = (self.engine, self.control, self.limits);
        fault!(SCIP_AFTER_SELECTION, Some(engine), Some(control), "")?;
        control.check()?;
        let tuple = &prepared.tuple;
        let namespace = tuple.producer.name.as_str();
        let lookup = &prepared.lookup;
        let report = &mut self.report;

        // Pass 2: the same frozen copy, one document per scope transaction.
        for_each_document(&prepared.artifact, control, limits, &mut |event| {
            let DocumentEvent::Message { bytes, .. } = event else {
                // Oversized messages failed in pass 1.
                return Ok(());
            };
            let DecodedDocument::Valid { path, document } =
                decode_document_message(&bytes, limits)?
            else {
                // Failed in pass 1 (for example the occurrence cap).
                return Ok(());
            };
            // Cancellation occurs between documents; a committed scope stays.
            fault!(SCIP_BETWEEN_DOCUMENTS, Some(engine), Some(control), &path)?;
            control.check()?;
            if lookup.document_failure(&path)?.is_some() {
                // Pass 1 already counted this document as failed.
                return Ok(());
            }
            let hash = lookup.input_hash(&path)?.ok_or_else(|| {
                FoundryError::Internal(anyhow::anyhow!("a bound document lost its manifest row"))
            })?;
            let published =
                publish_document(engine, control, lookup, tuple, &path, &hash, &document)?;
            match published {
                Published::Done(row) => {
                    report.count_scope(&row);
                    fault!(SCIP_SCOPE_AFTER_COMMIT, Some(engine), Some(control), &path)?;
                }
                Published::Failed(failure) => {
                    report.note_failure(&path, failure.code, &failure.message);
                }
            }
            Ok(())
        })?;

        // Manifest sources the artifact lacks, by the producer's profile.
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page = lookup.input_page(after.as_deref(), INPUT_BATCH)?;
            let Some((last, _)) = page.last() else { break };
            after = Some(last.clone());
            for (path, hash) in page {
                if lookup.has_document(&path)? {
                    continue;
                }
                match absent_policy(namespace, &path) {
                    Absent::AcceptedEmpty => {
                        fault!(SCIP_BETWEEN_DOCUMENTS, Some(engine), Some(control), &path)?;
                        control.check()?;
                        match commit_scope(engine, control, tuple, &path, &hash, &[], 0)? {
                            Published::Done(row) => {
                                report.count_scope(&row);
                                fault!(
                                    SCIP_SCOPE_AFTER_COMMIT,
                                    Some(engine),
                                    Some(control),
                                    &path
                                )?;
                            }
                            Published::Failed(failure) => {
                                report.note_failure(&path, failure.code, &failure.message);
                            }
                        }
                    }
                    Absent::OutsideScope => report.outside_scope += 1,
                    Absent::Unknown => report.unknown += 1,
                }
            }
        }

        Ok(())
    }

    /// Record a refusal as the producer's latest report so it is named; the
    /// prior selection is left alone. Best effort: the refusal itself is the
    /// result the caller sees.
    fn record_refusal(&mut self, error: &FoundryError) {
        let named = matches!(
            error,
            FoundryError::Scip { .. } | FoundryError::InvalidArgument(_)
        );
        if self.report.producer.is_empty() || !named {
            return;
        }
        let mut report = self.report.clone();
        report.coverage = "partial".into();
        report.scratch_peak_bytes = self.meter.peak;
        report.failure = Some(ImportFailureSample {
            path: String::new(),
            code: error.code().to_owned(),
            message: clip(&error.to_string(), SAMPLE_BYTES),
        });
        let namespace = report.producer.clone();
        let _ = self.engine.update_producer(&namespace, move |row, _| {
            row.latest = Some(report);
            Ok(())
        });
    }
}

enum Published {
    Done(ScopeRow),
    Failed(DocFailure),
}

/// The origin source as the manifest bound it: current revision, the
/// manifest's hash, verified bytes. Anything else fails the document.
fn read_source(
    engine: &Engine,
    revision: u64,
    path: &str,
    hash: &str,
) -> FResult<Result<String, DocFailure>> {
    let tx = engine.db.begin_read()?;
    let current = crate::store::read_counter(&tx.open_table(META)?, "source_revision")?;
    if current != revision {
        return Ok(Err(doc_failure(
            code::STALE_ARTIFACT,
            format!("{path}: the source revision changed during the import"),
        )));
    }
    let sources = tx.open_table(SOURCES)?;
    let Some(raw) = sources.get(path)? else {
        return Ok(Err(doc_failure(
            code::STALE_ARTIFACT,
            format!("{path}: the source is no longer indexed"),
        )));
    };
    let meta: SourceMeta = decode(raw.value(), "source")?;
    if meta.hash != hash {
        return Ok(Err(doc_failure(
            code::STALE_ARTIFACT,
            format!("{path}: the source changed during the import"),
        )));
    }
    let chunks = tx.open_table(CHUNKS)?;
    Ok(Ok(crate::store::reconstruct_verified(
        &chunks, path, &meta,
    )?
    .body))
}

/// Reference occurrences whose symbol has no unique definition in the
/// lookup: external, unknown or ambiguous.
fn count_unresolved(lookup: &Lookup, occurrences: &[NewOccurrence]) -> FResult<u32> {
    let tx = lookup.db.begin_read()?;
    let counts = tx.open_table(DEFINITIONS)?;
    let mut known: HashMap<&str, u32> = HashMap::new();
    let mut unresolved = 0u32;
    for occurrence in occurrences
        .iter()
        .filter(|o| o.kind == OccurrenceKind::Reference)
    {
        let definitions = match known.get(occurrence.symbol_id.as_str()) {
            Some(&n) => n,
            None => {
                let n = counts
                    .get(occurrence.symbol_id.as_str())?
                    .map_or(0, |v| v.value());
                known.insert(&occurrence.symbol_id, n);
                n
            }
        };
        if definitions != 1 {
            unresolved += 1;
        }
    }
    Ok(unresolved)
}

/// One scope transaction; a stale source or snapshot fails the document.
fn commit_scope(
    engine: &Engine,
    control: &Control,
    tuple: &SnapshotTuple,
    path: &str,
    hash: &str,
    occurrences: &[NewOccurrence],
    unresolved: u32,
) -> FResult<Published> {
    let write = ScopeWrite {
        namespace: &tuple.producer.name,
        path,
        snapshot: tuple,
        source_hash: hash,
        occurrences,
        unresolved,
    };
    match engine.publish_scope(&write, control) {
        Ok(row) => Ok(Published::Done(row)),
        Err(FoundryError::Scip { code: c, message }) if c == code::STALE_ARTIFACT => Ok(
            Published::Failed(doc_failure(code::STALE_ARTIFACT, message)),
        ),
        Err(error) => Err(error),
    }
}

fn publish_document(
    engine: &Engine,
    control: &Control,
    lookup: &Lookup,
    tuple: &SnapshotTuple,
    path: &str,
    hash: &str,
    document: &SlimDocument,
) -> FResult<Published> {
    let body = match read_source(engine, tuple.source_revision, path, hash)? {
        Ok(body) => body,
        Err(failure) => return Ok(Published::Failed(failure)),
    };
    let occurrences = match convert_document(&tuple.producer.name, path, document, &body) {
        Ok(occurrences) => occurrences,
        Err(failure) => return Ok(Published::Failed(failure)),
    };
    let unresolved = count_unresolved(lookup, &occurrences)?;
    commit_scope(engine, control, tuple, path, hash, &occurrences, unresolved)
}

impl Engine {
    /// Import one completed SCIP artifact bound by a snapshot manifest, with
    /// the spec's limits. See [`Engine::import_scip_with`].
    pub fn import_scip(
        &self,
        index: &Path,
        snapshot: &Path,
        control: &Control,
    ) -> FResult<ImportReport> {
        self.import_scip_with(index, snapshot, control, &ImportLimits::default())
    }

    /// The importer with explicit bounds. A refused preflight (a named
    /// error, nothing selected or published, the prior selection intact) is
    /// `Err`; once the snapshot is selected every outcome - committed,
    /// failed, interrupted - is an `Ok` report so committed counts are never
    /// lost. Cancellation before selection is `Err(cancelled)`.
    pub fn import_scip_with(
        &self,
        index: &Path,
        snapshot: &Path,
        control: &Control,
        limits: &ImportLimits,
    ) -> FResult<ImportReport> {
        self.import_scip_inputs(
            ImportInput::Path(index),
            ImportInput::Path(snapshot),
            control,
            limits,
        )
    }

    /// [`Engine::import_scip_with`] over [`ImportInput`]s: the same
    /// importer, but an input may be an already-open, already-checked
    /// descriptor (the MCP staging area), which is copied as it is and never
    /// reopened by name.
    pub fn import_scip_inputs(
        &self,
        index: ImportInput<'_>,
        snapshot: ImportInput<'_>,
        control: &Control,
        limits: &ImportLimits,
    ) -> FResult<ImportReport> {
        let workspace_id = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        control.check()?;
        let started = Instant::now();
        let dir = ScratchDir::acquire(&self.directory, &workspace_id)?;
        let meter = ScratchMeter {
            dir: dir.path.clone(),
            budget: limits.scratch_bytes,
            peak: 0,
        };
        let mut run = Run {
            engine: self,
            control,
            limits,
            dir,
            meter,
            report: ImportReport::default(),
        };
        let prepared = match run.prepare(index, snapshot, &workspace_id) {
            Ok(prepared) => prepared,
            Err(error) => {
                run.record_refusal(&error);
                return Err(error);
            }
        };
        if let Err(error) = run.select(&prepared) {
            run.record_refusal(&error);
            return Err(error);
        }
        let outcome = run.publish(&prepared);
        run.report.scratch_peak_bytes = run.meter.peak;
        run.report.timings.total_ms = millis(started);
        if let Err(error) = outcome {
            run.report.interrupted = Some(error.code().to_owned());
            run.report.failure = Some(ImportFailureSample {
                path: String::new(),
                code: error.code().to_owned(),
                message: clip(&error.to_string(), SAMPLE_BYTES),
            });
        }
        run.report.complete = run.report.interrupted.is_none() && run.report.failed == 0;
        run.report.coverage = coverage_word(&run.report).to_owned();
        let namespace = prepared.tuple.producer.name.clone();
        let snapshot_id = prepared.tuple.snapshot_id.clone();
        // Only a completed manifest proves a source absent, and only a run
        // without a failed document or interruption may draw that
        // conclusion. Retirement, the final report and the snapshot state
        // commit together or not at all.
        if run.report.complete {
            let lookup = &prepared.lookup;
            let mut absent =
                |path: &str| -> FResult<bool> { Ok(lookup.input_hash(path)?.is_none()) };
            match self.finalize_import(
                &namespace,
                &snapshot_id,
                SnapshotState::Complete,
                &mut run.report,
                Some(&mut absent as &mut dyn FnMut(&str) -> FResult<bool>),
                control,
            ) {
                Ok(()) => return Ok(run.report),
                Err(error) => {
                    // Nothing was retired or published: name the interruption.
                    run.report.interrupted = Some(error.code().to_owned());
                    run.report.failure = Some(ImportFailureSample {
                        path: String::new(),
                        code: error.code().to_owned(),
                        message: clip(&error.to_string(), SAMPLE_BYTES),
                    });
                    run.report.complete = false;
                    run.report.coverage = coverage_word(&run.report).to_owned();
                }
            }
        }
        self.finalize_import(
            &namespace,
            &snapshot_id,
            SnapshotState::Partial,
            &mut run.report,
            None,
            control,
        )?;
        Ok(run.report)
    }
}

fn duplicate_input(path: &str) -> FoundryError {
    fail(
        code::DUPLICATE_INPUT,
        format!("the manifest lists {} twice", clip(path, 200)),
    )
}

/// The aggregate coverage word of a report: `complete` only when nothing is
/// unresolved, unknown, failed or interrupted. Paths outside the producer's
/// scope (non-`.rs` for rust-analyzer) are an informational count and leave
/// coverage unaffected.
fn coverage_word(report: &ImportReport) -> &'static str {
    if report.complete && report.unresolved == 0 && report.unknown == 0 {
        "complete"
    } else {
        "partial"
    }
}
