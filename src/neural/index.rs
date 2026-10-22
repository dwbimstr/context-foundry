//! 009 derived semantic index: one validated generation per profile digest
//! under `<store>/semantic/<function digest>/`: an immutable F16 base, one bounded
//! F16 delta, and separate current locations. A v5 manifest atomically names
//! content-addressed artifacts; retired base keys are filtered during ANN selection.
//! The base is compacted only at bounded churn thresholds or explicit repair.
//! Earlier formats used one F16 index file,
//! its label map and a generation manifest naming both file hashes, the
//! profile digest, the recipe and the cache entries it covers. Since v2 the
//! label map also lists every unit location each label stood for, and the
//! manifest records the source revision those locations were read at and
//! whether they cover every eligible unit: serving expands a dense hit
//! through this mapping and never walks the partition table per request.
//!
//! Availability is decided by VALIDATION (parse + hashes + identity), never
//! by directory name: a missing, partial or mismatched set is unavailable and
//! is rebuilt from the f32 cache with zero document calls. Every read, write,
//! rename and removal is descriptor-anchored ([`crate::neural::anchor`]): the
//! store and `semantic/` directories are opened without following links and
//! all later work is descriptor-relative, so substituting an ancestor path
//! after a check cannot redirect it, and USearch never writes through a
//! pathname (the index is serialized to a buffer and written through the
//! descriptor). The USearch build is feature-gated (`semantic`); manifest and
//! label validation is not, so `status` never loads the tokenizer or the
//! model.
//!
//! 009 T004: generation v3 records the profile's geometry (its output
//! dimension, metric and scalar kind) and is validated against the serving
//! profile's dimension; a v2 generation is never served by a v2 profile and
//! is rebuilt from the cache under the new one (zero document calls).
use crate::control::Control;
#[cfg(feature = "semantic")]
use crate::error::FResult;
use crate::error::FoundryError;
use crate::neural::anchor::{Dir, conflict_or_io};
#[cfg(feature = "semantic")]
use crate::neural::cache::{self, CacheLookup};
use crate::neural::provider::SUPPORTED_DIMENSIONS;
#[cfg(feature = "semantic")]
use crate::store::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(feature = "semantic")]
use std::time::Instant;

#[cfg(feature = "semantic")]
pub(crate) mod mapping;
#[cfg(feature = "semantic")]
pub(crate) mod publication;
#[cfg(feature = "semantic")]
pub(crate) use mapping::MappingWalk;

pub(crate) mod locations;
pub(crate) mod verification;
pub use locations::Locations;

pub const SEMANTIC_DIR: &str = "semantic";
pub const INDEX_FILE: &str = "index.usearch";
pub const LABELS_FILE: &str = "labels.json";
pub const MANIFEST_FILE: &str = "generation.json";
/// Layout version of the manifest and label map below. v2 (009 T002) added
/// each label's unit locations and the coverage record serving needs; v3
/// (009 T004) records geometry; v4 (T008) separates immutable vector segments
/// from current locations under one atomic manifest; v5 adds independent bounded
/// location overlays, immutable membership and a transactional mapping revision.
/// An older generation is
/// unavailable by name until preparation republishes it from the cache
/// (zero document calls).
pub const GENERATION_VERSION: u32 = 5;
/// The scalar kind and metric of the derived index (D001 chosen values).
pub const SCALAR_KIND: &str = "f16";
pub const METRIC: &str = "cos";
/// Manifest and label-map files are bounded before they are parsed.
const MAX_META_BYTES: u64 = 256 * 1024 * 1024;
/// An index file larger than this is refused before it is read into memory.
const MAX_INDEX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Logical f32 payload retained while building a generation. This bounds
/// scratch payloads, not allocator overhead or total process RSS.
#[cfg(feature = "semantic")]
const MAX_VECTOR_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Approximate payload retained by the publication mapping view: each unit's
/// partition row, reverse reference and location together. This is memory, not
/// a file: the label-map file it publishes stays under [`MAX_META_BYTES`]. The
/// per-unit estimate (`mapping::row_bytes`) is a conservative proxy, not
/// measured allocation. A 62,000-file rust-lang/rust hierarchy (685,623 units)
/// estimates 1.03 GiB; 2026-10-21: raised from 1 GiB, which refused it.
#[cfg(feature = "semantic")]
const MAX_VIEW_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[cfg(feature = "semantic")]
fn within_publication_limit(bytes: u64, limit: u64, what: &str) -> FResult<()> {
    if bytes > limit {
        return Err(FoundryError::Semantic {
            code: "resource_limit",
            message: format!("publication {what} exceeds {limit} bytes; cached vectors stay"),
        });
    }
    Ok(())
}

/// The exact `serde_json` length of `text` as a JSON string, quotes included.
#[cfg(feature = "semantic")]
fn json_string_bytes(text: &str) -> u64 {
    2 + text
        .bytes()
        .map(|byte| match byte {
            b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 0x08 | 0x0c => 2,
            0..=0x1f => 6,
            _ => 1,
        })
        .sum::<u64>()
}

/// An upper bound on one serialized location: its escaped strings plus field
/// names, the kind and both offsets at full width. It sizes the locations
/// retained by the mapping walk; the published file is counted exactly by
/// [`label_map_bytes`].
#[cfg(feature = "semantic")]
fn location_bytes(path: &str, hash: &str) -> u64 {
    json_string_bytes(path) + json_string_bytes(hash) + 160
}

/// The exact compact `serde_json` length of `value`, without allocating it.
#[cfg(feature = "semantic")]
fn serialized_bytes(value: &impl Serialize) -> FResult<u64> {
    struct Count(u64);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len() as u64;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    serde_json::to_writer(&mut count, value)?;
    Ok(count.0)
}

/// The serialized [`LabelMap`] the reader caps at [`MAX_META_BYTES`]: exact
/// strings and locations, over-counting only each label ordinal (taken at full
/// u64 width) and one separator. A worst-case estimate refused a 182 MiB
/// rust-lang/rust hierarchy map as 277 MiB (2026-10-21), so publication counts
/// what it will write.
#[cfg(feature = "semantic")]
fn label_map_bytes(entries: &[GenerationEntry]) -> FResult<u64> {
    // `{"labels":[` … `]}`, then per entry `{"label":<20 digits>,"input_key":`
    // … `,"units":[` … `]}` and a separating comma.
    let mut bytes = 13u64;
    for entry in entries {
        bytes = bytes.saturating_add(55 + json_string_bytes(&entry.input_key));
        for unit in &entry.units {
            bytes = bytes.saturating_add(serialized_bytes(unit)? + 1);
        }
    }
    Ok(bytes)
}
/// The manifest's coverage words.
pub const COVERAGE_COMPLETE: &str = "complete";
pub const COVERAGE_PARTIAL: &str = "partial";

/// The generation manifest: the validated identity of one published set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationManifest {
    pub v: u32,
    /// The profile's document-function digest; also the directory name.
    pub function_digest: String,
    pub recipe_id: String,
    pub dimensions: usize,
    pub metric: String,
    pub scalar: String,
    pub count: usize,
    /// SHA-256 of the index file bytes.
    pub index_sha256: String,
    /// Ordinals in the immutable base, including retired keys.
    #[serde(skip)]
    pub base_entries: std::sync::Arc<Vec<String>>,
    #[serde(default)]
    pub base_count: usize,
    #[serde(default)]
    pub members_sha256: String,
    /// At most one bounded delta; ordinals are local to this segment.
    pub delta: Option<DeltaManifest>,
    /// SHA-256 of the label-map bytes this manifest was written with.
    pub labels_sha256: String,
    /// One bounded replacement/tombstone overlay over the immutable location base.
    pub locations_delta: Option<String>,
    /// Absent when the mapping walk observed concurrent commits.
    pub mapping_revision: Option<u64>,
    pub coverage_stats: Option<crate::neural::cache::CoverageSnapshot>,
    /// The store source revision the label map's unit locations were read at.
    pub source_revision: u64,
    /// [`COVERAGE_COMPLETE`] when every admitted source had a current
    /// partition AND every eligible unit's vector is in this index at
    /// `source_revision`; otherwise [`COVERAGE_PARTIAL`].
    pub coverage: String,
}

/// Immutable delta membership. Locations belong to the generation, not a segment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeltaManifest {
    pub sha256: String,
    pub entries: Vec<String>,
}

impl GenerationManifest {
    /// Content-named files are published before the single manifest switch.
    pub fn index_file(&self) -> String {
        format!("ann-{}.usearch", self.index_sha256)
    }
    pub fn members_file(&self) -> String {
        format!("members-{}.json", self.members_sha256)
    }
    pub fn location_delta_file(&self) -> Option<String> {
        self.locations_delta
            .as_ref()
            .map(|hash| format!("location-delta-{hash}.json"))
    }
    pub fn mapping_id(&self) -> String {
        format!(
            "{}:{}",
            self.labels_sha256,
            self.locations_delta.as_deref().unwrap_or("")
        )
    }
    pub fn metadata_files(&self) -> Vec<String> {
        let mut files = vec![self.labels_file(), self.members_file()];
        files.extend(self.location_delta_file());
        files
    }
    pub fn labels_file(&self) -> String {
        format!("locations-{}.json", self.labels_sha256)
    }
    pub fn delta_file(&self) -> Option<String> {
        self.delta
            .as_ref()
            .map(|delta| format!("ann-{}.usearch", delta.sha256))
    }
}

/// Hard bounds on both churn and delta build payload. A ratio threshold also
/// compacts small bases before their overlays dominate them.
pub const MAX_DELTA_ENTRIES: usize = 4096;
pub const MAX_DELTA_BYTES: u64 = 32 * 1024 * 1024;
pub fn churn_limit(base: usize) -> usize {
    (base / 4).clamp(64, MAX_DELTA_ENTRIES)
}

/// Verified ANN segments to restore or reuse; no subsequent pathname resolution.
pub struct IndexBuffers {
    pub base: Segment,
    pub delta: Option<Segment>,
}

/// Verified bytes, or a previously verified and restored immutable segment.
pub enum Segment {
    Bytes(Vec<u8>),
    #[cfg(feature = "semantic")]
    Resident(std::sync::Arc<usearch::Index>),
}

/// Label ↔ document-input key. Labels are ordinals over the SORTED unique key
/// set, so a generation is deterministic for one cache state and no unit
/// identity ever comes from storage ordinals: identity is the key itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelMap {
    pub labels: Vec<LabelEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelEntry {
    pub label: u64,
    pub input_key: String,
    /// Every current unit carrying `input_key` at publication: serving
    /// expands a dense hit through these, never through a partition walk.
    pub units: Vec<UnitLocation>,
}

/// One unit a label stood for at publication. The request's final read
/// revalidates it against the current source, like any lexical candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitLocation {
    #[serde(default)]
    pub kind: crate::neural::cache::CardKind,
    pub path: String,
    pub start: u64,
    pub end: u64,
    pub source_sha256: String,
}

/// One vector of a generation being built, with its unit locations.
#[cfg(feature = "semantic")]
pub struct GenerationEntry {
    pub input_key: String,
    pub vector: Vec<f32>,
    pub units: Vec<UnitLocation>,
}

/// What a published label map describes: the source revision its locations
/// were read at, and whether it covers every eligible unit at that revision.
#[cfg(feature = "semantic")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationScope {
    pub source_revision: u64,
    pub complete: bool,
}

#[cfg(feature = "semantic")]
impl GenerationScope {
    fn coverage(self) -> &'static str {
        if self.complete {
            COVERAGE_COMPLETE
        } else {
            COVERAGE_PARTIAL
        }
    }
}

/// The serving mapping preparation publishes: every input key with a
/// current cached vector, the unit locations carrying it, and the scope.
#[cfg(feature = "semantic")]
pub struct CurrentMapping {
    pub(crate) coverage: cache::CoverageSnapshot,
    pub units: Locations,
    pub(crate) mapping_revision: Option<u64>,
    pub scope: GenerationScope,
}

#[cfg(feature = "semantic")]
impl CurrentMapping {
    /// True when `generation` publishes exactly this mapping and scope.
    pub(crate) fn published_by(&self, generation: &Generation) -> bool {
        let manifest = &generation.manifest;
        manifest.mapping_revision == self.mapping_revision
            && manifest.source_revision == self.scope.source_revision
            && manifest.coverage == self.scope.coverage()
            && generation.labels.len() == self.units.len()
            && generation.labels == self.units
    }
}

/// A validated generation.
#[derive(Clone, Debug)]
pub struct Generation {
    pub manifest: GenerationManifest,
    pub labels: Locations,
    #[cfg(feature = "semantic")]
    pub(crate) location_base: Locations,
    #[cfg(feature = "semantic")]
    pub(crate) location_overlay: locations::Overlay,
    #[cfg(feature = "semantic")]
    pub(crate) receipt: std::sync::Arc<verification::ManifestReceipt>,
}

/// Why a generation did not validate: unavailable (a named reason) or the
/// caller's control interrupted the validation work.
#[derive(Debug)]
pub enum GenerationError {
    Unavailable(String),
    Interrupted(FoundryError),
}

impl GenerationError {
    pub fn reason(&self) -> String {
        match self {
            Self::Unavailable(reason) => reason.clone(),
            Self::Interrupted(error) => error.to_string(),
        }
    }
}

fn unavailable(reason: impl Into<String>) -> GenerationError {
    GenerationError::Unavailable(reason.into())
}

/// The pathname of one profile digest's generation directory (for display
/// and tests; no operation resolves it).
pub fn generation_dir(store_dir: &Path, function_digest: &str) -> PathBuf {
    store_dir.join(SEMANTIC_DIR).join(function_digest)
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Open the generation directory of `function_digest` through descriptors,
/// starting from the Engine's bound store descriptor (a duplicate passed in
/// by the caller): `semantic/`, then the digest directory. With `create`,
/// missing levels are made with `mkdirat`. A symlink or non-directory at any
/// level is a `repair_path_conflict`; `None` when a level is absent and
/// `create` is false.
pub(crate) fn open_generation_dir(
    store: &Dir,
    function_digest: &str,
    create: bool,
) -> Result<Option<Dir>, FoundryError> {
    if !is_hex64(function_digest) {
        return Err(FoundryError::RepairPathConflict(format!(
            "{function_digest:?} is not a generation directory name"
        )));
    }
    let root = if create {
        Some(
            store
                .open_or_create_dir(SEMANTIC_DIR)
                .map_err(|e| conflict_or_io(e, "semantic root"))?,
        )
    } else {
        store
            .open_dir(SEMANTIC_DIR)
            .map_err(|e| conflict_or_io(e, "semantic root"))?
    };
    let Some(root) = root else {
        return Ok(None);
    };
    if create {
        root.open_or_create_dir(function_digest)
            .map(Some)
            .map_err(|e| conflict_or_io(e, "generation directory"))
    } else {
        root.open_dir(function_digest)
            .map_err(|e| conflict_or_io(e, "generation directory"))
    }
}

/// Streamed SHA-256 of an open file with a control checkpoint per MiB.
fn hash_reader(
    file: &mut std::fs::File,
    control: &Control,
    limit: u64,
) -> Result<String, GenerationError> {
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        control.check().map_err(GenerationError::Interrupted)?;
        let read = file
            .read(&mut buffer)
            .map_err(|e| unavailable(format!("index file unreadable: {e}")))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > limit {
            return Err(unavailable("index file exceeds its byte bound"));
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crate::lower_hex(&hasher.finalize()))
}

/// Read a bounded metadata file completely.
fn read_bounded(file: std::fs::File, what: &str) -> Result<Vec<u8>, GenerationError> {
    let mut bytes = Vec::new();
    file.take(MAX_META_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| unavailable(format!("{what} unreadable: {e}")))?;
    if bytes.len() as u64 > MAX_META_BYTES {
        return Err(unavailable(format!(
            "{what} exceeds {MAX_META_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

/// Read the index file ONCE into memory, bounded, with a control
/// checkpoint per MiB: the caller hashes and restores these same bytes.
fn read_index(
    file: std::fs::File,
    control: &Control,
    limit: u64,
    hash: bool,
) -> Result<(Vec<u8>, Option<String>), GenerationError> {
    let len = file
        .metadata()
        .map_err(|e| unavailable(format!("index file unreadable: {e}")))?
        .len();
    if len > limit {
        return Err(unavailable(format!(
            "index file is {len} bytes, over the {limit}-byte load bound"
        )));
    }
    let mut hasher = hash.then(Sha256::new);
    let mut bytes = Vec::with_capacity(len as usize);
    let mut bounded = file.take(limit + 1);
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        control.check().map_err(GenerationError::Interrupted)?;
        let read = bounded
            .read(&mut chunk)
            .map_err(|e| unavailable(format!("index file unreadable: {e}")))?;
        if read == 0 {
            break;
        }
        if bytes.len() as u64 + read as u64 > limit {
            return Err(unavailable("index file exceeds its byte bound"));
        }
        if let Some(hasher) = &mut hasher {
            hasher.update(&chunk[..read]);
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    if bytes.len() as u64 > limit {
        return Err(unavailable(format!(
            "index file exceeds the {limit}-byte load bound"
        )));
    }
    control.check().map_err(GenerationError::Interrupted)?;
    Ok((bytes, hasher.map(|h| crate::lower_hex(&h.finalize()))))
}

/// The format version alone, read before the full manifest is decoded so an
/// older format is refused by name rather than as a decode failure.
#[derive(Deserialize)]
struct FormatVersion {
    v: u32,
}

/// Validate one generation by content, honoring `control` between bounded
/// units of work (file chunks). `store` is a duplicate of the Engine's bound
/// store-directory descriptor: the store pathname is not resolved here. A
/// mismatched function digest, recipe or geometry (the profile's
/// `dimensions`) is a refusal, so a foreign or stale directory can never
/// serve; a symlinked semantic root, generation directory or file never
/// validates. Every read goes through descriptors opened without following
/// links.
pub fn validate_generation_with(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    dimensions: usize,
    control: &Control,
) -> Result<Generation, GenerationError> {
    validate_inner(
        store,
        function_digest,
        recipe_id,
        dimensions,
        control,
        false,
        &mut verification::Cache::default(),
    )
    .map(|(generation, _)| generation)
}

/// [`validate_generation_with`] for serving: the index file is read ONCE
/// into memory and THAT buffer is hashed against the manifest, so the
/// caller restores exactly the bytes that validated (009 T002). A file
/// replaced after this read cannot reach the loaded index.
pub fn load_generation_with(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    dimensions: usize,
    control: &Control,
) -> Result<(Generation, IndexBuffers), GenerationError> {
    let (generation, bytes) = validate_inner(
        store,
        function_digest,
        recipe_id,
        dimensions,
        control,
        true,
        &mut verification::Cache::default(),
    )?;
    Ok((
        generation,
        bytes.expect("a kept index read returns its bytes"),
    ))
}

/// Owner-local fast path. Explicit validation above deliberately has no receipts.
#[cfg(feature = "semantic")]
pub(crate) fn validate_cached(
    store: &Dir,
    identity: &Prepared,
    control: &Control,
    keep_index: bool,
    cache: &std::sync::Mutex<verification::Cache>,
) -> Result<(Generation, Option<IndexBuffers>), GenerationError> {
    let mut cache = verification::lock(cache, control)?;
    validate_inner(
        store,
        &identity.digest,
        &identity.recipe,
        identity.dimensions,
        control,
        keep_index,
        &mut cache,
    )
}

fn validate_inner(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    dimensions: usize,
    control: &Control,
    keep_index: bool,
    cache: &mut verification::Cache,
) -> Result<(Generation, Option<IndexBuffers>), GenerationError> {
    control.check().map_err(GenerationError::Interrupted)?;
    let missing = || {
        unavailable(format!(
            "no generation manifest under {SEMANTIC_DIR}/{function_digest}"
        ))
    };
    if !is_hex64(function_digest) {
        return Err(missing());
    }
    let generation = match open_generation_dir(store, function_digest, false) {
        Ok(Some(dir)) => dir,
        Ok(None) => return Err(missing()),
        Err(error) => return Err(unavailable(error.to_string())),
    };
    let manifest_file = match generation.open_file(MANIFEST_FILE) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
        Err(e) => return Err(unavailable(format!("generation manifest: {e}"))),
    };
    let partial = |what: &str, e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::NotFound {
            unavailable("generation is partial (index, labels or manifest missing)")
        } else {
            unavailable(format!("{what}: {e}"))
        }
    };
    let receipt = verification::ManifestReceipt::capture(&manifest_file, &generation)?;
    let manifest_bytes = read_bounded(manifest_file, "generation manifest")?;
    if !receipt.unchanged() {
        return Err(unavailable("generation manifest changed during read"));
    }
    let version: FormatVersion = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| unavailable(format!("generation manifest cannot be decoded: {e}")))?;
    if version.v != GENERATION_VERSION {
        return Err(unavailable(format!(
            "generation format v{} is not v{GENERATION_VERSION}; `semantic prepare` \
             republishes it from the cache",
            version.v
        )));
    }
    let mut manifest: GenerationManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| unavailable(format!("generation manifest cannot be decoded: {e}")))?;
    if manifest.coverage != COVERAGE_COMPLETE && manifest.coverage != COVERAGE_PARTIAL {
        return Err(unavailable("generation coverage record is malformed"));
    }
    if manifest.function_digest != function_digest {
        return Err(unavailable(
            "generation belongs to a different document function",
        ));
    }
    if manifest.recipe_id != recipe_id {
        return Err(unavailable(
            "generation belongs to a different partition recipe",
        ));
    }
    if manifest.dimensions != dimensions
        || !SUPPORTED_DIMENSIONS.contains(&manifest.dimensions)
        || manifest.metric != METRIC
        || manifest.scalar != SCALAR_KIND
    {
        return Err(unavailable(format!(
            "generation geometry ({} dimensions, {} {}) differs from the profile's \
             ({dimensions} dimensions, {METRIC} {SCALAR_KIND})",
            manifest.dimensions, manifest.metric, manifest.scalar
        )));
    }
    validate_membership(&manifest)?;
    let index_file = generation
        .open_file(manifest.index_file())
        .map_err(|e| partial("index file", e))?;
    let delta_file = manifest
        .delta_file()
        .map(|name| {
            generation
                .open_file(name)
                .map_err(|e| partial("delta file", e))
        })
        .transpose()?;
    cache.prepare(&generation, &manifest)?;
    let (labels, _location_base, _location_overlay) =
        locations::read(&generation, &mut manifest, cache, control)?;
    validate_membership(&manifest)?;
    let base = cache.verify(
        index_file,
        &manifest.index_sha256,
        MAX_INDEX_BYTES,
        keep_index,
        control,
    )?;
    let delta = match (delta_file, &manifest.delta) {
        (Some(file), Some(delta)) => {
            cache.verify(file, &delta.sha256, MAX_DELTA_BYTES, keep_index, control)?
        }
        _ => None,
    };
    let index_bytes = base.map(|base| IndexBuffers { base, delta });
    // The manifest binds the expected identity and geometry (dimensions,
    // metric and scalar kind, checked above), the coverage record and source
    // revision, to the index and label-map bytes through their SHA-256
    // digests. This is pair integrity for the trusted writer, not
    // authentication. The library's own dense header is checked where the
    // index is loaded for a query (009 T002), from the kept bytes.
    Ok((
        Generation {
            manifest,
            labels,
            #[cfg(feature = "semantic")]
            location_base: _location_base,
            #[cfg(feature = "semantic")]
            location_overlay: _location_overlay,
            #[cfg(feature = "semantic")]
            receipt,
        },
        index_bytes,
    ))
}

fn validate_membership(manifest: &GenerationManifest) -> Result<(), GenerationError> {
    if !is_hex64(&manifest.index_sha256)
        || !is_hex64(&manifest.members_sha256)
        || !is_hex64(&manifest.labels_sha256)
        || manifest
            .locations_delta
            .as_ref()
            .is_some_and(|hash| !is_hex64(hash))
        || (manifest.base_count as u64).saturating_mul(67) > MAX_META_BYTES
    {
        return Err(unavailable(
            "generation membership or artifact hash is malformed",
        ));
    }
    if let Some(delta) = &manifest.delta
        && (!is_hex64(&delta.sha256)
            || delta.entries.is_empty()
            || !delta.entries.iter().all(|key| is_hex64(key))
            || !delta.entries.windows(2).all(|pair| pair[0] < pair[1])
            || delta.entries.len() > churn_limit(manifest.base_count)
            || delta.entries.len() as u64 * manifest.dimensions as u64 * 4 > MAX_DELTA_BYTES
            || delta
                .entries
                .iter()
                .any(|key| manifest.base_entries.binary_search(key).is_ok()))
    {
        return Err(unavailable(
            "delta membership is malformed or exceeds its bound",
        ));
    }
    if let Some(stats) = &manifest.coverage_stats
        && (stats.version != 1
            || stats.source_revision != manifest.source_revision
            || stats.function_digest != manifest.function_digest
            || stats.recipe_id != manifest.recipe_id
            || stats.index_entries != manifest.count as u64
            || stats.unpartitioned_sources > stats.sources
            || stats.cached_current_units > stats.eligible_units
            || stats.searchable_current_units != stats.cached_current_units
            || (manifest.coverage == COVERAGE_COMPLETE
                && (stats.unpartitioned_sources != 0
                    || stats.cached_current_units != stats.eligible_units)))
    {
        return Err(unavailable("generation coverage counters are malformed"));
    }
    Ok(())
}

/// [`validate_generation_with`] without a deadline; `Err` names why the
/// generation is unavailable.
pub fn validate_generation(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    dimensions: usize,
) -> Result<Generation, String> {
    validate_generation_with(
        store,
        function_digest,
        recipe_id,
        dimensions,
        &Control::unbounded(),
    )
    .map_err(|e| e.reason())
}

/// The prepared profile the state row records: its function digest,
/// partition recipe and output dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub digest: String,
    pub recipe: String,
    pub dimensions: usize,
}

/// Why a state row names no servable prepared profile: none was prepared,
/// or the recorded one is a descriptor v1 profile (its row has no
/// dimension), whose cache rows and generation are kept, never rebuilt.
pub fn unprepared_reason(state: Option<&crate::neural::cache::SemanticState>) -> &'static str {
    match state {
        Some(state) if state.function_digest.is_some() && state.dimensions.is_none() => {
            "the recorded profile is descriptor v1 (profile_unsupported); its cache rows and \
             generation are kept until `semantic purge`"
        }
        _ => "no semantic profile prepared",
    }
}

/// The [`Prepared`] identity of a state row, if it names one this build
/// serves.
pub fn prepared(state: Option<&crate::neural::cache::SemanticState>) -> Option<Prepared> {
    let state = state?;
    Some(Prepared {
        digest: state.function_digest.clone()?,
        recipe: state.recipe_id.clone()?,
        dimensions: state.dimensions? as usize,
    })
}

/// What a rebuild did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SemanticIndexReport {
    pub rebuilt: bool,
    pub entries: usize,
    pub reason: Option<String>,
}

/// What publishing pending committed coverage did.
#[cfg(feature = "semantic")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication {
    /// No current vector is cached: nothing to publish.
    Nothing,
    /// A validated generation already covers exactly the cached vectors.
    Current(usize),
    /// A new generation was built and published from the cache.
    Rebuilt(usize),
    /// The base is reused; only the bounded delta is built.
    Delta(usize),
    /// No ANN bytes are rebuilt or rewritten.
    Mapped(usize),
}

#[cfg(feature = "semantic")]
impl Publication {
    pub fn count(self) -> usize {
        match self {
            Self::Nothing => 0,
            Self::Current(n) | Self::Rebuilt(n) | Self::Delta(n) | Self::Mapped(n) => n,
        }
    }
}

#[cfg(feature = "semantic")]
impl Engine {
    /// The serving mapping of the current profile (metadata only): every
    /// input key whose cache row carries the active function's digest and a
    /// valid layout, with the location of every CURRENT unit carrying it
    /// (shared document inputs keep one key and list each unit), and its
    /// scope. The scope is complete only when every admitted source has a
    /// current partition and every unit has such a cached vector, all at
    /// one unchanged source revision.
    pub fn semantic_current_mapping(
        &self,
        control: &Control,
        function_digest: &str,
        recipe_id: &str,
    ) -> FResult<CurrentMapping> {
        let mut walk = MappingWalk::audit(self, function_digest, recipe_id)?;
        while !walk.step(self, control, None)? {}
        walk.finish(self, control)
    }

    /// The recorded profile identity, if a profile this build serves was
    /// prepared.
    pub(crate) fn semantic_identity(&self) -> FResult<Option<Prepared>> {
        Ok(prepared(self.semantic_state()?.as_ref()))
    }

    /// Rebuild the current profile's generation from the f32 cache: replay
    /// only, ZERO document calls. Shared document inputs contribute ONE label
    /// however many units map to them. A same-length nonfinite cache row
    /// fails the decode in the lookup, is skipped by name and is re-embedded
    /// by the next preparation. Called by `repair-index` and by preparation
    /// after new vectors were committed. A descriptor v1 state is never
    /// rebuilt: its generation stays as it is until `semantic purge`.
    pub fn semantic_rebuild_index(&self, control: &Control) -> FResult<SemanticIndexReport> {
        let state = self.semantic_state()?;
        let Some(identity) = prepared(state.as_ref()) else {
            return Ok(SemanticIndexReport {
                reason: Some(unprepared_reason(state.as_ref()).into()),
                ..SemanticIndexReport::default()
            });
        };
        let mapping = self.semantic_current_mapping(control, &identity.digest, &identity.recipe)?;
        let count = self.semantic_publish_mapping(mapping, &identity, control)?;
        Ok(SemanticIndexReport {
            rebuilt: true,
            entries: count,
            reason: None,
        })
    }

    /// Explicit repair always compacts from the authoritative cache.
    fn semantic_publish_mapping(
        &self,
        mapping: CurrentMapping,
        identity: &Prepared,
        control: &Control,
    ) -> FResult<usize> {
        self.publish_plan(publication::Plan::new(mapping, None), identity, control)
            .map(|result| result.count())
    }

    fn publish_plan(
        &self,
        mut plan: publication::Plan,
        identity: &Prepared,
        control: &Control,
    ) -> FResult<Publication> {
        let mut entries = Vec::new();
        let mut keys = plan.needed().into_iter();
        lookup_entries(
            self,
            &mut keys,
            identity,
            &mut plan.scope,
            &mut entries,
            control,
            None,
        )?;
        let store = self.semantic_anchor()?;
        let (staged, coverage, result) = plan.stage(&store, identity, &entries, control, &|| {})?;
        self.semantic_publish_staged(staged, control)?;
        self.semantic_record_coverage(coverage)?;
        Ok(result)
    }

    /// Clean restart/replay can use an independently validated publication. The
    /// revision includes cache/partition changes even without source changes.
    pub(crate) fn semantic_generation_current(&self, generation: &Generation) -> FResult<bool> {
        if generation.manifest.mapping_revision == Some(self.semantic_mapping_revision()?)
            && generation.manifest.source_revision == self.source_revision()?
            && let Some(coverage) = &generation.manifest.coverage_stats
        {
            self.semantic_record_coverage(coverage.clone())?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn semantic_remember_mapping(
        &self,
        id: String,
        units: Locations,
        directory: (u64, u64),
        control: &Control,
    ) -> FResult<()> {
        let mut cache =
            verification::lock(&self.semantic_segments, control).map_err(|e| match e {
                GenerationError::Interrupted(error) => error,
                GenerationError::Unavailable(message) => FoundryError::Semantic {
                    code: "index_unavailable",
                    message,
                },
            })?;
        cache.remember_locations(directory, id, units);
        Ok(())
    }
    pub(crate) fn semantic_publish_staged(
        &self,
        staged: Staged,
        control: &Control,
    ) -> FResult<usize> {
        let snapshot = staged.snapshot.clone();
        let directory = staged.directory_identity()?;
        let count = staged.publish(control)?;
        if let Some((id, units)) = snapshot {
            self.semantic_remember_mapping(id, units, directory, control)?;
        }
        Ok(count)
    }

    /// Reuse immutable vectors and publish bounded vector/location changes.
    pub fn semantic_publish_pending(&self, control: &Control) -> FResult<Publication> {
        let Some(identity) = self.semantic_identity()? else {
            return Ok(Publication::Nothing);
        };
        let store = self.semantic_anchor()?;
        let previous =
            match validate_cached(&store, &identity, control, false, &self.semantic_segments) {
                Ok((generation, _)) => Some(generation),
                Err(GenerationError::Interrupted(error)) => return Err(error),
                Err(_) => None,
            };
        if let Some(generation) = &previous
            && self.semantic_generation_current(generation)?
        {
            return Ok(Publication::Current(generation.manifest.count));
        }
        let mut walk =
            MappingWalk::resume(self, &identity.digest, &identity.recipe, previous.as_ref())?;
        while !walk.step(self, control, None)? {}
        let wanted = walk.finish(self, control)?;
        if let Some(generation) = &previous
            && wanted.published_by(generation)
        {
            let dir = open_generation_dir(&store, &identity.digest, false)?
                .expect("validated generation");
            self.semantic_remember_mapping(
                generation.manifest.mapping_id(),
                wanted.units,
                dir.identity()?,
                control,
            )?;
            self.semantic_record_coverage(wanted.coverage)?;
            return Ok(Publication::Current(generation.manifest.count));
        }
        if previous.is_none() && wanted.units.is_empty() {
            self.semantic_record_coverage(wanted.coverage)?;
            return Ok(Publication::Nothing);
        }
        self.publish_plan(publication::Plan::new(wanted, previous), &identity, control)
    }
}

/// Decode the cached vectors of the next mapping `keys` into `entries`,
/// until the keys end (`true`), or until `until` passed once a key was read
/// (`false`: call again). A key whose cached row no longer decodes is
/// excluded by name and leaves the generation partial: its units are not
/// searchable there.
#[cfg(feature = "semantic")]
pub(crate) fn lookup_entries(
    engine: &Engine,
    keys: &mut impl Iterator<Item = (String, Vec<UnitLocation>)>,
    identity: &Prepared,
    scope: &mut GenerationScope,
    entries: &mut Vec<GenerationEntry>,
    control: &Control,
    until: Option<Instant>,
) -> FResult<bool> {
    let mut read = 0usize;
    loop {
        if read > 0 && until.is_some_and(|until| Instant::now() >= until) {
            return Ok(false);
        }
        let Some((input_key, units)) = keys.next() else {
            return Ok(true);
        };
        if read.is_multiple_of(64) {
            control.check()?;
        }
        read += 1;
        let bytes = (entries.len() as u64 + 1)
            .saturating_mul(identity.dimensions as u64)
            .saturating_mul(4);
        within_publication_limit(bytes, MAX_VECTOR_BYTES, "vectors")?;
        match engine.semantic_cache_lookup(&input_key, &identity.digest, identity.dimensions)? {
            CacheLookup::Hit(vector) => entries.push(GenerationEntry {
                input_key,
                vector,
                units,
            }),
            CacheLookup::Corrupt(_) | CacheLookup::Miss => scope.complete = false,
        }
    }
}

/// Build one generation and publish it as a validated set, entirely through
/// descriptors: [`stage_generation`], then [`Staged::publish`].
#[cfg(feature = "semantic")]
pub fn build_generation(
    store: &Dir,
    identity: &Prepared,
    entries: &[GenerationEntry],
    scope: GenerationScope,
    control: &Control,
) -> FResult<usize> {
    stage_generation(store, identity, entries, scope, control, &|| {})?.publish(control)
}

#[cfg(feature = "semantic")]
fn io_error(what: &'static str) -> impl Fn(std::io::Error) -> FoundryError {
    move |e| conflict_or_io(e, what)
}

#[cfg(feature = "semantic")]
pub(crate) use publication::Staged;

/// Explicit full rebuild used by repair and boundary tests.
#[cfg(feature = "semantic")]
pub(crate) fn stage_generation(
    store: &Dir,
    identity: &Prepared,
    entries: &[GenerationEntry],
    scope: GenerationScope,
    control: &Control,
    built: &dyn Fn(),
) -> FResult<Staged> {
    publication::full(store, identity, entries, scope, control, built)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_dir_names_the_profile_digest() {
        let dir = generation_dir(Path::new("/store"), "abc");
        assert_eq!(dir, Path::new("/store/semantic/abc"));
    }

    #[test]
    fn a_missing_generation_names_its_reason() {
        let scratch = tempfile::tempdir().unwrap();
        let store = Dir::open_path(scratch.path()).unwrap();
        let err = validate_generation(&store, "abc", "recipe", 768).unwrap_err();
        assert!(err.contains("no generation manifest"), "{err}");
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn publication_location_bound_tracks_the_serialized_label_map() {
        // The bound must hold for escape-heavy paths and stay close to the
        // real size: a worst-case estimate refused ~112k-card corpora whose
        // label maps were far below the reader's limit.
        for path in ["library/core/src/num/f64.rs", "a\"b\\c\nd\u{1}é.rs"] {
            let unit = UnitLocation {
                kind: crate::neural::cache::CardKind::ContainerFragment,
                path: path.into(),
                start: u64::MAX - 1,
                end: u64::MAX,
                source_sha256: "ab".repeat(32),
            };
            let actual = serde_json::to_vec(&unit).unwrap().len() as u64;
            let bound = location_bytes(&unit.path, &unit.source_sha256);
            assert!(
                actual <= bound && bound <= actual + 64,
                "{actual} > {bound}"
            );
        }
        // The string length is exact for every ASCII byte, Unicode and mixes.
        let mut strings: Vec<String> = (0u8..=0x7f).map(|b| char::from(b).to_string()).collect();
        strings.extend(["", "é🦀/", "a\"b\\c\nd\u{1}\u{7f}é.rs"].map(String::from));
        strings.push("ab".repeat(32));
        for text in strings {
            assert_eq!(
                json_string_bytes(&text),
                serde_json::to_vec(&text).unwrap().len() as u64,
                "{text:?}"
            );
        }
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn publication_counts_the_label_map_it_writes() {
        use crate::neural::cache::CardKind;
        let location = |path: &str, start: u64, end: u64| UnitLocation {
            kind: if start.is_multiple_of(2) {
                CardKind::Body
            } else {
                CardKind::ContainerFragment
            },
            path: path.into(),
            start,
            end,
            source_sha256: "cd".repeat(32),
        };
        let written = |entries: &[GenerationEntry]| {
            serde_json::to_vec(&LabelMap {
                labels: entries
                    .iter()
                    .enumerate()
                    .map(|(label, entry)| LabelEntry {
                        label: label as u64,
                        input_key: entry.input_key.clone(),
                        units: entry.units.clone(),
                    })
                    .collect(),
            })
            .unwrap()
            .len() as u64
        };
        // An empty map is exact; otherwise the only excess is each ordinal's
        // missing digits plus one separator per entry, so any under-count of a
        // string, location or structural byte fails the equality.
        assert_eq!(label_map_bytes(&[]).unwrap(), written(&[]));
        let mut entries = entries(4);
        entries[0].units = vec![location("a\"b\\c\nd\u{1}é.rs", 7, u64::MAX)];
        entries[1].units = (0..9)
            .map(|i| {
                location(
                    &format!("library/core/src/num/f{i}.rs"),
                    i * 1_000_003,
                    u64::MAX - i,
                )
            })
            .collect();
        for size in [1, 3, 12] {
            let mut map: Vec<GenerationEntry> = (0..size)
                .map(|i| GenerationEntry {
                    input_key: format!("{i:064x}"),
                    vector: Vec::new(),
                    units: entries[i % entries.len()].units.clone(),
                })
                .collect();
            map.sort_by(|a, b| a.input_key.cmp(&b.input_key));
            let excess = 1
                + (0..size as u64)
                    .map(|i| 21 - i.to_string().len() as u64)
                    .sum::<u64>();
            assert_eq!(
                label_map_bytes(&map).unwrap(),
                written(&map) + excess,
                "{size} entries"
            );
        }
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn oversized_publication_preserves_the_previous_generation() {
        let scratch = tempfile::tempdir().unwrap();
        let store = Dir::open_path(scratch.path()).unwrap();
        let identity = identity(128);
        let scope = GenerationScope {
            source_revision: 1,
            complete: true,
        };
        let control = Control::unbounded();
        let mut entries = entries(128);
        build_generation(&store, &identity, &entries, scope, &control).unwrap();
        let original = validate_generation(&store, &identity.digest, "r1", 128).unwrap();
        // A direct builder caller can bypass admitted-path bounds. Refuse
        // before cloning labels, allocating ANN storage or replacing files.
        // Each control character serializes as six bytes.
        entries[0].units[0].path = "\u{1}".repeat((MAX_META_BYTES / 6 + 1) as usize);
        let error = build_generation(&store, &identity, &entries, scope, &control).unwrap_err();
        assert_eq!(error.code(), "resource_limit");
        let retained = validate_generation(&store, &identity.digest, "r1", 128).unwrap();
        assert_eq!(
            retained.manifest.index_sha256,
            original.manifest.index_sha256
        );
        assert_eq!(
            retained.manifest.labels_sha256,
            original.manifest.labels_sha256
        );
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn invalid_direct_publication_preserves_the_previous_readable_generation() {
        use crate::neural::cache::CardKind;
        let scratch = tempfile::tempdir().unwrap();
        let store = Dir::open_path(scratch.path()).unwrap();
        let mut identity = identity(128);
        identity.recipe = "cards-v1+kinds1+units5:512+test".into();
        let control = Control::unbounded();
        let scope = GenerationScope {
            source_revision: 1,
            complete: true,
        };
        let valid = entries(128);
        build_generation(&store, &identity, &valid, scope, &control).unwrap();
        let manifest = generation_dir(scratch.path(), &identity.digest).join(MANIFEST_FILE);
        let original = std::fs::read(&manifest).unwrap();
        let unit = valid[0].units[0].clone();
        let mut later = unit.clone();
        later.path = "z.txt".into();
        let mut empty_path = unit.clone();
        empty_path.path.clear();
        let mut empty_range = unit.clone();
        empty_range.end = empty_range.start;
        let mut invalid_hash = unit.clone();
        invalid_hash.source_sha256 = "invalid".into();
        let mut wrong_role = unit.clone();
        wrong_role.kind = CardKind::Body;
        let mut unknown_role = unit.clone();
        unknown_role.kind = CardKind::Unknown;
        for units in [
            vec![later.clone(), unit.clone()],
            vec![unit.clone(), unit.clone()],
            Vec::new(),
            vec![empty_path],
            vec![empty_range],
            vec![invalid_hash],
            vec![wrong_role],
            vec![unknown_role],
        ] {
            let mut invalid = entries(128);
            invalid[0].units = units;
            let error = build_generation(&store, &identity, &invalid, scope, &control).unwrap_err();
            assert_eq!(error.code(), "cache_corrupt");
            assert_eq!(std::fs::read(&manifest).unwrap(), original);
            validate_generation(&store, &identity.digest, &identity.recipe, 128).unwrap();
        }
        let mut invalid = entries(128);
        invalid[0].input_key = "invalid".into();
        assert_eq!(
            build_generation(&store, &identity, &invalid, scope, &control)
                .unwrap_err()
                .code(),
            "cache_corrupt"
        );
        assert_eq!(std::fs::read(&manifest).unwrap(), original);
        // A subsequent valid multi-location publication remains readable.
        let mut replacement = entries(128);
        replacement[0].units.push(later);
        let scope = GenerationScope {
            source_revision: 2,
            complete: true,
        };
        build_generation(&store, &identity, &replacement, scope, &control).unwrap();
        let published =
            validate_generation(&store, &identity.digest, &identity.recipe, 128).unwrap();
        assert_eq!(published.manifest.source_revision, 2);
        assert_eq!(published.manifest.count, replacement.len());
        assert_eq!(
            *published.labels[&replacement[0].input_key],
            replacement[0].units
        );
    }

    #[cfg(feature = "semantic")]
    fn identity(dimensions: usize) -> Prepared {
        Prepared {
            digest: "d1".repeat(32),
            recipe: "r1".into(),
            dimensions,
        }
    }

    #[cfg(feature = "semantic")]
    fn entries(dimensions: usize) -> Vec<GenerationEntry> {
        (0..3)
            .map(|i| GenerationEntry {
                input_key: format!("{:064x}", i),
                vector: vec![i as f32 / 10.0 + 0.1; dimensions],
                units: vec![UnitLocation {
                    kind: crate::neural::cache::CardKind::Address,
                    path: format!("f{i}.txt"),
                    start: 0,
                    end: 5,
                    source_sha256: "ab".repeat(32),
                }],
            })
            .collect()
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn a_built_generation_validates_and_a_tampered_one_refuses() {
        let scratch = tempfile::tempdir().unwrap();
        let store = Dir::open_path(scratch.path()).unwrap();
        let identity = identity(768);
        let digest = &identity.digest;
        let entries = entries(768);
        let scope = GenerationScope {
            source_revision: 7,
            complete: false,
        };
        let count =
            build_generation(&store, &identity, &entries, scope, &Control::unbounded()).unwrap();
        assert_eq!(count, 3);
        let ok = validate_generation(&store, digest, "r1", 768).unwrap();
        assert_eq!(ok.manifest.count, 3);
        assert_eq!(ok.manifest.v, GENERATION_VERSION);
        assert_eq!(ok.manifest.dimensions, 768);
        // The serving mapping and its scope are part of the validated set.
        assert_eq!(ok.manifest.source_revision, 7);
        assert_eq!(ok.manifest.coverage, COVERAGE_PARTIAL);
        assert_eq!(*ok.labels[&entries[1].input_key], entries[1].units);
        // Serving restores exactly the bytes that matched the manifest.
        let (_, bytes) =
            load_generation_with(&store, digest, "r1", 768, &Control::unbounded()).unwrap();
        let Segment::Bytes(base) = bytes.base else {
            panic!("audit returns bytes")
        };
        assert_eq!(crate::digest(&base), ok.manifest.index_sha256);
        // A foreign function digest never serves.
        let err = validate_generation(&store, &"d2".repeat(32), "r1", 768).unwrap_err();
        assert!(err.contains("no generation manifest"), "{err}");
        // Another geometry never serves.
        let err = validate_generation(&store, digest, "r1", 256).unwrap_err();
        assert!(err.contains("geometry"), "{err}");
        // Tampering with the index file breaks the hash pair.
        let dir = generation_dir(scratch.path(), digest);
        let original = std::fs::read(dir.join(ok.manifest.index_file())).unwrap();
        std::fs::write(dir.join(ok.manifest.index_file()), b"tampered").unwrap();
        let err = validate_generation(&store, digest, "r1", 768).unwrap_err();
        assert!(err.contains("does not match"), "{err}");
        std::fs::write(dir.join(ok.manifest.index_file()), original).unwrap();
        assert!(validate_generation(&store, digest, "r1", 768).is_ok());
        // A recipe change refuses the old generation.
        let err = validate_generation(&store, digest, "r2", 768).unwrap_err();
        assert!(err.contains("recipe"), "{err}");
        // An expired control interrupts validation instead of hashing on.
        let expired = Control::cancelled();
        assert!(matches!(
            validate_generation_with(&store, digest, "r1", 768, &expired),
            Err(GenerationError::Interrupted(_))
        ));
        // No temporary build directory survives a publication.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers.len(), 4, "{leftovers:?}");
    }

    /// 009 T004: every supported Matryoshka dimension builds, records its
    /// geometry and validates only under its own dimension; a vector of
    /// another length is refused before anything is written.
    #[cfg(feature = "semantic")]
    #[test]
    fn each_dimension_records_its_geometry_and_a_wrong_length_is_refused() {
        for dimensions in [768, 512, 256, 128] {
            let scratch = tempfile::tempdir().unwrap();
            let store = Dir::open_path(scratch.path()).unwrap();
            let identity = identity(dimensions);
            let scope = GenerationScope {
                source_revision: 1,
                complete: true,
            };
            let built = build_generation(
                &store,
                &identity,
                &entries(dimensions),
                scope,
                &Control::unbounded(),
            );
            assert_eq!(built.unwrap(), 3, "{dimensions}");
            let (generation, bytes) = load_generation_with(
                &store,
                &identity.digest,
                "r1",
                dimensions,
                &Control::unbounded(),
            )
            .unwrap();
            assert_eq!(generation.manifest.dimensions, dimensions);
            let Segment::Bytes(base) = bytes.base else {
                panic!("audit returns bytes")
            };
            let header = usearch::Index::metadata_from_buffer(&base).unwrap();
            assert_eq!(header.dimensions, dimensions as u64);
            let other = if dimensions == 768 { 512 } else { 768 };
            assert!(validate_generation(&store, &identity.digest, "r1", other).is_err());
            let mut wrong = entries(dimensions);
            wrong[1].vector.pop();
            let refused = build_generation(&store, &identity, &wrong, scope, &Control::unbounded());
            assert_eq!(refused.unwrap_err().code(), "provider_malformed");
        }
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn an_older_generation_format_is_unavailable_by_name() {
        let scratch = tempfile::tempdir().unwrap();
        let store = Dir::open_path(scratch.path()).unwrap();
        let identity = identity(256);
        let scope = GenerationScope {
            source_revision: 1,
            complete: true,
        };
        build_generation(
            &store,
            &identity,
            &entries(256),
            scope,
            &Control::unbounded(),
        )
        .unwrap();
        let manifest = generation_dir(scratch.path(), &identity.digest).join(MANIFEST_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        value["v"] = 2.into();
        std::fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
        let err = validate_generation(&store, &identity.digest, "r1", 256).unwrap_err();
        assert!(err.contains("format v2 is not v5"), "{err}");
    }
}
