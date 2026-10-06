//! 009 derived semantic index: ONE validated USearch generation per profile
//! digest under `<store>/semantic/<function digest>/` — the F16 index file,
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
//! label validation is not, so `status` never loads the tokenizer, Python or
//! the model.
use crate::control::Control;
#[cfg(feature = "semantic")]
use crate::error::FResult;
use crate::error::FoundryError;
use crate::neural::anchor::{Dir, conflict_or_io};
#[cfg(feature = "semantic")]
use crate::neural::cache::{self, CacheLookup, CacheProbe};
use crate::neural::provider::DIMENSIONS;
#[cfg(feature = "semantic")]
use crate::store::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
#[cfg(feature = "semantic")]
use std::time::Instant;

pub const SEMANTIC_DIR: &str = "semantic";
pub const INDEX_FILE: &str = "index.usearch";
pub const LABELS_FILE: &str = "labels.json";
pub const MANIFEST_FILE: &str = "generation.json";
/// Layout version of the manifest and label map below. v2 (009 T002) adds
/// each label's unit locations and the coverage record serving needs; a v1
/// generation is unavailable by name until preparation republishes it from
/// the cache (zero document calls).
pub const GENERATION_VERSION: u32 = 2;
/// The scalar kind and metric of the derived index (D001 chosen values).
pub const SCALAR_KIND: &str = "f16";
pub const METRIC: &str = "cos";
/// Manifest and label-map files are bounded before they are parsed.
const MAX_META_BYTES: u64 = 256 * 1024 * 1024;
/// An index file larger than this is refused before it is read into memory.
const MAX_INDEX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
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
    /// SHA-256 of the label-map bytes this manifest was written with.
    pub labels_sha256: String,
    /// The UNIQUE cache entries (document input keys) this generation covers.
    pub entries: Vec<String>,
    /// The store source revision the label map's unit locations were read at.
    pub source_revision: u64,
    /// [`COVERAGE_COMPLETE`] when every admitted source had a current
    /// partition AND every eligible unit's vector is in this index at
    /// `source_revision`; otherwise [`COVERAGE_PARTIAL`].
    pub coverage: String,
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
    pub units: std::collections::BTreeMap<String, Vec<UnitLocation>>,
    pub scope: GenerationScope,
}

#[cfg(feature = "semantic")]
impl CurrentMapping {
    /// True when `generation` publishes exactly this mapping and scope.
    pub(crate) fn published_by(&self, generation: &Generation) -> bool {
        let manifest = &generation.manifest;
        manifest.source_revision == self.scope.source_revision
            && manifest.coverage == self.scope.coverage()
            && generation.labels.labels.len() == self.units.len()
            && generation
                .labels
                .labels
                .iter()
                .zip(&self.units)
                .all(|(entry, (key, units))| &entry.input_key == key && &entry.units == units)
    }
}

/// A validated generation.
#[derive(Clone, Debug)]
pub struct Generation {
    pub manifest: GenerationManifest,
    pub labels: LabelMap,
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
fn hash_reader(file: &mut std::fs::File, control: &Control) -> Result<String, GenerationError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        control.check().map_err(GenerationError::Interrupted)?;
        let read = file
            .read(&mut buffer)
            .map_err(|e| unavailable(format!("index file unreadable: {e}")))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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
fn read_index(file: std::fs::File, control: &Control) -> Result<Vec<u8>, GenerationError> {
    let len = file
        .metadata()
        .map_err(|e| unavailable(format!("index file unreadable: {e}")))?
        .len();
    if len > MAX_INDEX_BYTES {
        return Err(unavailable(format!(
            "index file is {len} bytes, over the {MAX_INDEX_BYTES}-byte load bound"
        )));
    }
    let mut bytes = Vec::with_capacity(len as usize);
    let mut bounded = file.take(MAX_INDEX_BYTES + 1);
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        control.check().map_err(GenerationError::Interrupted)?;
        let read = bounded
            .read(&mut chunk)
            .map_err(|e| unavailable(format!("index file unreadable: {e}")))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Err(unavailable(format!(
            "index file exceeds the {MAX_INDEX_BYTES}-byte load bound"
        )));
    }
    Ok(bytes)
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
/// mismatched function digest or recipe is a refusal, so a foreign or stale
/// directory can never serve; a symlinked semantic root, generation
/// directory or file never validates. Every read goes through descriptors
/// opened without following links.
pub fn validate_generation_with(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    control: &Control,
) -> Result<Generation, GenerationError> {
    validate_inner(store, function_digest, recipe_id, control, false)
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
    control: &Control,
) -> Result<(Generation, Vec<u8>), GenerationError> {
    let (generation, bytes) = validate_inner(store, function_digest, recipe_id, control, true)?;
    Ok((
        generation,
        bytes.expect("a kept index read returns its bytes"),
    ))
}

fn validate_inner(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    control: &Control,
    keep_index: bool,
) -> Result<(Generation, Option<Vec<u8>>), GenerationError> {
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
    let labels_file = generation
        .open_file(LABELS_FILE)
        .map_err(|e| partial("label map", e))?;
    let index_file = generation
        .open_file(INDEX_FILE)
        .map_err(|e| partial("index file", e))?;
    let manifest_bytes = read_bounded(manifest_file, "generation manifest")?;
    let version: FormatVersion = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| unavailable(format!("generation manifest cannot be decoded: {e}")))?;
    if version.v != GENERATION_VERSION {
        return Err(unavailable(format!(
            "generation format v{} is not v{GENERATION_VERSION}; `semantic prepare` \
             republishes it from the cache",
            version.v
        )));
    }
    let manifest: GenerationManifest = serde_json::from_slice(&manifest_bytes)
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
    if manifest.dimensions != DIMENSIONS
        || manifest.metric != METRIC
        || manifest.scalar != SCALAR_KIND
    {
        return Err(unavailable(
            "generation geometry differs from the selected profile",
        ));
    }
    let labels_bytes = read_bounded(labels_file, "label map")?;
    if crate::digest(&labels_bytes) != manifest.labels_sha256 {
        return Err(unavailable("label map does not match the manifest hash"));
    }
    let index_bytes = if keep_index {
        let bytes = read_index(index_file, control)?;
        if crate::digest(&bytes) != manifest.index_sha256 {
            return Err(unavailable("index file does not match the manifest hash"));
        }
        Some(bytes)
    } else {
        let mut index_file = index_file;
        if hash_reader(&mut index_file, control)? != manifest.index_sha256 {
            return Err(unavailable("index file does not match the manifest hash"));
        }
        None
    };
    let labels: LabelMap = serde_json::from_slice(&labels_bytes)
        .map_err(|e| unavailable(format!("label map cannot be decoded: {e}")))?;
    if labels.labels.len() != manifest.count || manifest.entries.len() != manifest.count {
        return Err(unavailable("generation counts disagree with the label map"));
    }
    for (ordinal, entry) in labels.labels.iter().enumerate() {
        if entry.label != ordinal as u64 {
            return Err(unavailable("label map is not the sorted ordinal sequence"));
        }
        if manifest.entries.get(ordinal) != Some(&entry.input_key) {
            return Err(unavailable("label map and manifest entries disagree"));
        }
        if entry.units.is_empty()
            || entry.units.iter().any(|unit| {
                unit.path.is_empty() || unit.start >= unit.end || !is_hex64(&unit.source_sha256)
            })
        {
            return Err(unavailable("label map unit locations are malformed"));
        }
    }
    // The manifest binds the expected identity and geometry (dimensions,
    // metric and scalar kind, checked above), the coverage record and source
    // revision, to the index and label-map bytes through their SHA-256
    // digests. This is pair integrity for the trusted writer, not
    // authentication. The library's own dense header is checked where the
    // index is loaded for a query (009 T002), from the kept bytes.
    Ok((Generation { manifest, labels }, index_bytes))
}

/// [`validate_generation_with`] without a deadline; `Err` names why the
/// generation is unavailable.
pub fn validate_generation(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
) -> Result<Generation, String> {
    validate_generation_with(store, function_digest, recipe_id, &Control::unbounded())
        .map_err(|e| e.reason())
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
        let mut walk = MappingWalk::start(self, function_digest, recipe_id)?;
        while !walk.step(self, control, None)? {}
        walk.finish(self)
    }

    /// The recorded profile digest and recipe, if a profile was prepared.
    pub(crate) fn semantic_identity(&self) -> FResult<Option<(String, String)>> {
        Ok(self
            .semantic_state()?
            .and_then(|state| Some((state.function_digest?, state.recipe_id?))))
    }

    /// Rebuild the current profile's generation from the f32 cache: replay
    /// only, ZERO document calls. Shared document inputs contribute ONE label
    /// however many units map to them. A same-length nonfinite cache row
    /// fails the decode in the lookup, is skipped by name and is re-embedded
    /// by the next preparation. Called by `repair-index` and by preparation
    /// after new vectors were committed.
    pub fn semantic_rebuild_index(&self, control: &Control) -> FResult<SemanticIndexReport> {
        let Some((digest, recipe)) = self.semantic_identity()? else {
            return Ok(SemanticIndexReport {
                reason: Some("no semantic profile prepared".into()),
                ..SemanticIndexReport::default()
            });
        };
        let mapping = self.semantic_current_mapping(control, &digest, &recipe)?;
        let count = self.semantic_publish_mapping(mapping, &digest, &recipe, control)?;
        Ok(SemanticIndexReport {
            rebuilt: true,
            entries: count,
            reason: None,
        })
    }

    /// Build and publish `mapping` from the f32 cache (zero document calls).
    /// A key whose cached row no longer decodes is excluded by name and
    /// leaves the generation partial: its units are not searchable here.
    fn semantic_publish_mapping(
        &self,
        mapping: CurrentMapping,
        digest: &str,
        recipe: &str,
        control: &Control,
    ) -> FResult<usize> {
        let mut scope = mapping.scope;
        let mut keys = mapping.units.into_iter();
        let mut entries: Vec<GenerationEntry> = Vec::with_capacity(keys.len());
        lookup_entries(
            self,
            &mut keys,
            digest,
            &mut scope,
            &mut entries,
            control,
            None,
        )?;
        let store = self.semantic_anchor()?;
        build_generation(&store, digest, recipe, &entries, scope, control)
    }

    /// Publish pending committed coverage: when a validated generation
    /// already covers exactly the cached current vectors nothing is rebuilt;
    /// otherwise the generation is rebuilt from the cache. Zero inference,
    /// bounded by `control` (the hash work and the rebuild checkpoint it).
    pub fn semantic_publish_pending(&self, control: &Control) -> FResult<Publication> {
        let Some((digest, recipe)) = self.semantic_identity()? else {
            return Ok(Publication::Nothing);
        };
        let wanted = self.semantic_current_mapping(control, &digest, &recipe)?;
        if wanted.units.is_empty() {
            return Ok(Publication::Nothing);
        }
        // Current means the same keys, unit locations, coverage AND source
        // revision: an older format, a moved unit or a changed revision is
        // republished from the cache.
        match validate_generation_with(&self.semantic_anchor()?, &digest, &recipe, control) {
            Ok(generation) if wanted.published_by(&generation) => {
                return Ok(Publication::Current(generation.manifest.count));
            }
            Err(GenerationError::Interrupted(error)) => return Err(error),
            _ => {}
        }
        let count = self.semantic_publish_mapping(wanted, &digest, &recipe, control)?;
        Ok(Publication::Rebuilt(count))
    }
}

/// [`Engine::semantic_current_mapping`] read a page of sources at a time, so
/// the MCP owner's preparation driver reads it in short engine-slot steps
/// (009 T003, captain decision 2026-10-06). A source change between the
/// steps leaves the mapping incomplete, exactly as one during a single walk
/// does.
#[cfg(feature = "semantic")]
pub(crate) struct MappingWalk {
    digest: String,
    recipe: String,
    source_revision: u64,
    complete: bool,
    units: std::collections::BTreeMap<String, Vec<UnitLocation>>,
    /// One cache probe per distinct key: true when its vector is current.
    probed: std::collections::HashMap<String, bool>,
    after: Option<String>,
}

#[cfg(feature = "semantic")]
impl MappingWalk {
    pub(crate) fn start(engine: &Engine, function_digest: &str, recipe_id: &str) -> FResult<Self> {
        Ok(Self {
            digest: function_digest.to_owned(),
            recipe: recipe_id.to_owned(),
            source_revision: engine.source_revision()?,
            complete: true,
            units: std::collections::BTreeMap::new(),
            probed: std::collections::HashMap::new(),
            after: None,
        })
    }

    /// Walk pages of sources until the walk ends (`true`), or until `until`
    /// passed once a page was read (`false`: call again).
    pub(crate) fn step(
        &mut self,
        engine: &Engine,
        control: &Control,
        until: Option<Instant>,
    ) -> FResult<bool> {
        loop {
            control.check()?;
            let page = cache::source_page(&engine.db, self.after.as_deref())?;
            if page.is_empty() {
                return Ok(true);
            }
            for (path, meta) in &page {
                let Some(record) = engine.semantic_partition(path)?.filter(|record| {
                    cache::partition_is_current(record, meta, &self.recipe, &self.digest)
                }) else {
                    self.complete = false;
                    continue;
                };
                for unit in &record.units {
                    let cached = match self.probed.get(&unit.input_key) {
                        Some(&cached) => cached,
                        None => {
                            let cached = engine
                                .semantic_cache_probe(&unit.input_key, &self.digest)?
                                == CacheProbe::Current;
                            self.probed.insert(unit.input_key.clone(), cached);
                            cached
                        }
                    };
                    if !cached {
                        self.complete = false;
                        continue;
                    }
                    self.units
                        .entry(unit.input_key.clone())
                        .or_default()
                        .push(UnitLocation {
                            path: path.clone(),
                            start: unit.start as u64,
                            end: unit.end as u64,
                            source_sha256: record.source_hash.clone(),
                        });
                }
            }
            self.after = page.last().map(|(path, _)| path.clone());
            if until.is_some_and(|until| Instant::now() >= until) {
                return Ok(false);
            }
        }
    }

    /// The mapping and its scope. A source change during the paged walk
    /// leaves a mixed mapping: its locations still revalidate per request,
    /// but it is never complete.
    pub(crate) fn finish(self, engine: &Engine) -> FResult<CurrentMapping> {
        let complete = self.complete && engine.source_revision()? == self.source_revision;
        Ok(CurrentMapping {
            units: self.units,
            scope: GenerationScope {
                source_revision: self.source_revision,
                complete,
            },
        })
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
    function_digest: &str,
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
        match engine.semantic_cache_lookup(&input_key, function_digest)? {
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
    function_digest: &str,
    recipe_id: &str,
    entries: &[GenerationEntry],
    scope: GenerationScope,
    control: &Control,
) -> FResult<usize> {
    stage_generation(
        store,
        function_digest,
        recipe_id,
        entries,
        scope,
        control,
        &|| {},
    )?
    .publish(control)
}

#[cfg(feature = "semantic")]
fn io_error(what: &'static str) -> impl Fn(std::io::Error) -> FoundryError {
    move |e| conflict_or_io(e, what)
}

/// A generation built and written into its private build directory beside
/// the published set, not yet published. Dropping it unpublished removes the
/// build directory.
#[cfg(feature = "semantic")]
pub(crate) struct Staged {
    generation: Dir,
    build: Dir,
    build_name: String,
    count: usize,
    pending: bool,
}

#[cfg(feature = "semantic")]
impl Staged {
    /// Publish the staged set: index file and label map first, the manifest
    /// LAST — a crash mid-publication leaves a mismatched pair, which
    /// validation refuses and the next rebuild repairs from cache.
    /// Cancellation is honored up to the first rename; the three renames
    /// are the publication itself and run to completion. The entry count.
    pub(crate) fn publish(mut self, control: &Control) -> FResult<usize> {
        control.check()?;
        for (file, what) in [
            (INDEX_FILE, "index file"),
            (LABELS_FILE, "label map"),
            (MANIFEST_FILE, "generation manifest"),
        ] {
            self.build
                .rename_into(file, &self.generation, file)
                .map_err(io_error(what))?;
        }
        self.pending = false;
        self.generation
            .remove_tree(&self.build_name)
            .map_err(io_error("build directory"))?;
        Ok(self.count)
    }
}

#[cfg(feature = "semantic")]
impl Drop for Staged {
    fn drop(&mut self) {
        if self.pending {
            let _ = self.generation.remove_tree(&self.build_name);
        }
    }
}

/// Build one generation and write it, unpublished, into a private build
/// directory beside the published set. `store` is a duplicate of the
/// Engine's bound store-directory descriptor; `semantic/` and the digest
/// directory are opened (or made with `mkdirat`) under it without following
/// links; USearch serializes to a buffer and the bytes are written through
/// the descriptor, so no library call resolves a pathname. Needs no engine:
/// the MCP owner's driver runs it with the engine slot free and takes the
/// slot only for [`Staged::publish`]. `built` runs once the index is built
/// in memory, before it is serialized (the driver's test seam).
#[cfg(feature = "semantic")]
pub(crate) fn stage_generation(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    entries: &[GenerationEntry],
    scope: GenerationScope,
    control: &Control,
    built: &dyn Fn(),
) -> FResult<Staged> {
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

    let io = io_error;
    // Confine the destination before any work or write; keep the descriptors.
    let generation = open_generation_dir(store, function_digest, true)?
        .ok_or_else(|| FoundryError::RepairPathConflict("generation directory vanished".into()))?;
    neural_fault!(PUBLISH_AFTER_CHECK, Some(control), function_digest)?;

    // Deterministic generation: sorted unique keys, ordinal labels.
    let mut sorted: Vec<&GenerationEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.input_key.cmp(&b.input_key));
    let mut seen = std::collections::HashSet::new();
    for entry in &sorted {
        let key = &entry.input_key;
        if !seen.insert(key.as_str()) {
            return Err(FoundryError::Semantic {
                code: "cache_corrupt",
                message: format!("duplicate input key {key} in the generation input"),
            });
        }
    }
    let labels = LabelMap {
        labels: sorted
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| LabelEntry {
                label: ordinal as u64,
                input_key: entry.input_key.clone(),
                units: entry.units.clone(),
            })
            .collect(),
    };
    let options = IndexOptions {
        dimensions: DIMENSIONS,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F16,
        multi: false,
        ..IndexOptions::default()
    };
    let unavailable_index = |what: &str, e: &dyn std::fmt::Display| FoundryError::Semantic {
        code: "index_unavailable",
        message: format!("usearch {what} failed: {e}"),
    };
    let index = Index::new(&options).map_err(|e| unavailable_index("index creation", &e))?;
    index
        .reserve(sorted.len().max(1))
        .map_err(|e| unavailable_index("reserve", &e))?;
    for (ordinal, entry) in sorted.iter().enumerate() {
        let vector = &entry.vector;
        if ordinal % 256 == 0 {
            control.check()?;
        }
        if vector.len() != DIMENSIONS {
            return Err(FoundryError::Semantic {
                code: "provider_malformed",
                message: format!("vector for entry {ordinal} has {} values", vector.len()),
            });
        }
        index
            .add(ordinal as u64, vector.as_slice())
            .map_err(|e| unavailable_index("add", &e))?;
    }
    built();
    // Serialize to memory: no library call touches a pathname.
    let mut buffer = vec![0u8; index.serialized_length()];
    index
        .save_to_buffer(&mut buffer)
        .map_err(|e| unavailable_index("serialization", &e))?;
    let index_sha256 = crate::digest(&buffer);
    let labels_bytes = serde_json::to_vec(&labels).map_err(FoundryError::from)?;
    let manifest = GenerationManifest {
        v: GENERATION_VERSION,
        function_digest: function_digest.to_owned(),
        recipe_id: recipe_id.to_owned(),
        dimensions: DIMENSIONS,
        metric: METRIC.into(),
        scalar: SCALAR_KIND.into(),
        count: sorted.len(),
        index_sha256,
        labels_sha256: crate::digest(&labels_bytes),
        entries: sorted.iter().map(|entry| entry.input_key.clone()).collect(),
        source_revision: scope.source_revision,
        coverage: scope.coverage().to_owned(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(FoundryError::from)?;
    control.check()?;

    let build_name = format!(".building-{}", std::process::id());
    if generation
        .kind_of(&build_name)
        .map_err(io("build directory"))?
        .is_some()
    {
        generation
            .remove_tree(&build_name)
            .map_err(io("build directory"))?;
    }
    generation
        .create_dir(&build_name)
        .map_err(io("build directory"))?;
    let build = generation
        .open_dir(&build_name)
        .map_err(io("build directory"))?
        .ok_or_else(|| FoundryError::RepairPathConflict("build directory vanished".into()))?;
    let staged = Staged {
        generation,
        build,
        build_name,
        count: sorted.len(),
        pending: true,
    };
    staged
        .build
        .write_new(INDEX_FILE, &buffer)
        .map_err(io("index file"))?;
    staged
        .build
        .write_new(LABELS_FILE, &labels_bytes)
        .map_err(io("label map"))?;
    staged
        .build
        .write_new(MANIFEST_FILE, &manifest_bytes)
        .map_err(io("generation manifest"))?;
    Ok(staged)
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
        let err = validate_generation(&store, "abc", "recipe").unwrap_err();
        assert!(err.contains("no generation manifest"), "{err}");
    }

    #[cfg(feature = "semantic")]
    #[test]
    fn a_built_generation_validates_and_a_tampered_one_refuses() {
        let scratch = tempfile::tempdir().unwrap();
        let store = Dir::open_path(scratch.path()).unwrap();
        let entries: Vec<GenerationEntry> = (0..3)
            .map(|i| GenerationEntry {
                input_key: format!("{:064x}", i),
                vector: vec![i as f32 / 10.0; DIMENSIONS],
                units: vec![UnitLocation {
                    path: format!("f{i}.txt"),
                    start: 0,
                    end: 5,
                    source_sha256: "ab".repeat(32),
                }],
            })
            .collect();
        let digest = "d1".repeat(32);
        let scope = GenerationScope {
            source_revision: 7,
            complete: false,
        };
        let count = build_generation(
            &store,
            &digest,
            "r1",
            &entries,
            scope,
            &Control::unbounded(),
        )
        .unwrap();
        assert_eq!(count, 3);
        let ok = validate_generation(&store, &digest, "r1").unwrap();
        assert_eq!(ok.manifest.count, 3);
        // The serving mapping and its scope are part of the validated set.
        assert_eq!(ok.manifest.source_revision, 7);
        assert_eq!(ok.manifest.coverage, COVERAGE_PARTIAL);
        assert_eq!(ok.labels.labels[1].units, entries[1].units);
        // Serving restores exactly the bytes that matched the manifest.
        let (_, bytes) =
            load_generation_with(&store, &digest, "r1", &Control::unbounded()).unwrap();
        assert_eq!(crate::digest(&bytes), ok.manifest.index_sha256);
        // A foreign function digest never serves.
        let err = validate_generation(&store, &"d2".repeat(32), "r1").unwrap_err();
        assert!(err.contains("no generation manifest"), "{err}");
        // Tampering with the index file breaks the hash pair.
        let dir = generation_dir(scratch.path(), &digest);
        let original = std::fs::read(dir.join(INDEX_FILE)).unwrap();
        std::fs::write(dir.join(INDEX_FILE), b"tampered").unwrap();
        let err = validate_generation(&store, &digest, "r1").unwrap_err();
        assert!(err.contains("does not match"), "{err}");
        std::fs::write(dir.join(INDEX_FILE), original).unwrap();
        assert!(validate_generation(&store, &digest, "r1").is_ok());
        // A recipe change refuses the old generation.
        let err = validate_generation(&store, &digest, "r2").unwrap_err();
        assert!(err.contains("recipe"), "{err}");
        // An expired control interrupts validation instead of hashing on.
        let expired = Control::cancelled();
        assert!(matches!(
            validate_generation_with(&store, &digest, "r1", &expired),
            Err(GenerationError::Interrupted(_))
        ));
        // No temporary build directory survives a publication.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers.len(), 3, "{leftovers:?}");
    }
}
