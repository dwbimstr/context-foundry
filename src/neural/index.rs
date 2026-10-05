//! 009 derived semantic index: ONE validated USearch generation per profile
//! digest under `<store>/semantic/<function digest>/` — the F16 index file,
//! its label map and a generation manifest naming both file hashes, the
//! profile digest, the recipe and the cache entries it covers.
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

pub const SEMANTIC_DIR: &str = "semantic";
pub const INDEX_FILE: &str = "index.usearch";
pub const LABELS_FILE: &str = "labels.json";
pub const MANIFEST_FILE: &str = "generation.json";
/// Layout version of the manifest and label map below.
pub const GENERATION_VERSION: u32 = 1;
/// The scalar kind and metric of the derived index (D001 chosen values).
pub const SCALAR_KIND: &str = "f16";
pub const METRIC: &str = "cos";
/// Manifest and label-map files are bounded before they are parsed.
const MAX_META_BYTES: u64 = 256 * 1024 * 1024;

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
    let mut index_file = generation
        .open_file(INDEX_FILE)
        .map_err(|e| partial("index file", e))?;
    let manifest_bytes = read_bounded(manifest_file, "generation manifest")?;
    let manifest: GenerationManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| unavailable(format!("generation manifest cannot be decoded: {e}")))?;
    if manifest.v != GENERATION_VERSION {
        return Err(unavailable(format!(
            "generation manifest version {} is not {GENERATION_VERSION}",
            manifest.v
        )));
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
    if hash_reader(&mut index_file, control)? != manifest.index_sha256 {
        return Err(unavailable("index file does not match the manifest hash"));
    }
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
    }
    // The manifest binds the expected identity and geometry (dimensions,
    // metric and scalar kind, checked above) to the index and label-map
    // bytes through their SHA-256 digests. This is pair integrity for the
    // trusted writer, not authentication. The library's own dense header is NOT parsed
    // here; loading the index for a query — with a USearch header and
    // geometry check at load time — is a 009 T002 obligation.
    Ok(Generation { manifest, labels })
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
    /// The sorted, UNIQUE input keys of every CURRENT unit whose cache row
    /// carries the active function's digest and a valid layout (metadata
    /// only). Shared document inputs appear once however many units map to
    /// them; mappings stay per unit in the partition table.
    pub fn semantic_current_keys(
        &self,
        control: &Control,
        function_digest: &str,
        recipe_id: &str,
    ) -> FResult<Vec<String>> {
        let mut keys = std::collections::BTreeSet::new();
        let mut seen = std::collections::HashSet::new();
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page = cache::source_page(&self.db, after.as_deref())?;
            if page.is_empty() {
                break;
            }
            for (path, meta) in &page {
                let Some(record) = self.semantic_partition(path)? else {
                    continue;
                };
                if !cache::partition_is_current(&record, meta, recipe_id, function_digest) {
                    continue;
                }
                for unit in &record.units {
                    if !seen.insert(unit.input_key.clone()) {
                        continue;
                    }
                    if self.semantic_cache_probe(&unit.input_key, function_digest)?
                        == CacheProbe::Current
                    {
                        keys.insert(unit.input_key.clone());
                    }
                }
            }
            after = page.last().map(|(path, _)| path.clone());
        }
        Ok(keys.into_iter().collect())
    }

    /// The recorded profile digest and recipe, if a profile was prepared.
    fn semantic_identity(&self) -> FResult<Option<(String, String)>> {
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
        let keys = self.semantic_current_keys(control, &digest, &recipe)?;
        let mut entries: Vec<(String, Vec<f32>)> = Vec::with_capacity(keys.len());
        for (ordinal, key) in keys.into_iter().enumerate() {
            if ordinal % 64 == 0 {
                control.check()?;
            }
            match self.semantic_cache_lookup(&key, &digest)? {
                CacheLookup::Hit(vector) => entries.push((key, vector)),
                CacheLookup::Corrupt(_) | CacheLookup::Miss => {}
            }
        }
        let store = self.semantic_anchor()?;
        let count = build_generation(&store, &digest, &recipe, &entries, control)?;
        Ok(SemanticIndexReport {
            rebuilt: true,
            entries: count,
            reason: None,
        })
    }

    /// Publish pending committed coverage: when a validated generation
    /// already covers exactly the cached current vectors nothing is rebuilt;
    /// otherwise the generation is rebuilt from the cache. Zero inference,
    /// bounded by `control` (the hash work and the rebuild checkpoint it).
    pub fn semantic_publish_pending(&self, control: &Control) -> FResult<Publication> {
        let Some((digest, recipe)) = self.semantic_identity()? else {
            return Ok(Publication::Nothing);
        };
        let wanted = self.semantic_current_keys(control, &digest, &recipe)?;
        if wanted.is_empty() {
            return Ok(Publication::Nothing);
        }
        match validate_generation_with(&self.semantic_anchor()?, &digest, &recipe, control) {
            Ok(generation) if generation.manifest.entries == wanted => {
                return Ok(Publication::Current(generation.manifest.count));
            }
            Err(GenerationError::Interrupted(error)) => return Err(error),
            _ => {}
        }
        let report = self.semantic_rebuild_index(control)?;
        Ok(Publication::Rebuilt(report.entries))
    }
}

/// Build one generation and publish it as a validated set, entirely through
/// descriptors. `store` is a duplicate of the Engine's bound store-directory
/// descriptor; `semantic/` and the digest directory are opened (or made with
/// `mkdirat`) under it without following links; USearch serializes to a
/// buffer and the bytes are written through the descriptor, so no library
/// call resolves a pathname. Publication order: index file and label map
/// first (from a private temporary directory, by `renameat` on the same
/// descriptors), the manifest LAST — a crash mid-publication leaves a
/// mismatched pair, which validation refuses and the next rebuild repairs
/// from cache.
#[cfg(feature = "semantic")]
pub fn build_generation(
    store: &Dir,
    function_digest: &str,
    recipe_id: &str,
    entries: &[(String, Vec<f32>)],
    control: &Control,
) -> FResult<usize> {
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

    let io = |what: &'static str| move |e: std::io::Error| conflict_or_io(e, what);
    // Confine the destination before any work or write; keep the descriptors.
    let generation = open_generation_dir(store, function_digest, true)?
        .ok_or_else(|| FoundryError::RepairPathConflict("generation directory vanished".into()))?;
    neural_fault!(PUBLISH_AFTER_CHECK, Some(control), function_digest)?;

    // Deterministic generation: sorted unique keys, ordinal labels.
    let mut sorted: Vec<&(String, Vec<f32>)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut seen = std::collections::HashSet::new();
    for (key, _) in &sorted {
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
            .map(|(ordinal, (key, _))| LabelEntry {
                label: ordinal as u64,
                input_key: (*key).clone(),
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
    for (ordinal, (_, vector)) in sorted.iter().enumerate() {
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
        entries: sorted.iter().map(|(key, _)| (*key).clone()).collect(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(FoundryError::from)?;
    control.check()?;

    let tmp_name = format!(".building-{}", std::process::id());
    let tmp_name = tmp_name.as_str();
    if generation
        .kind_of(tmp_name)
        .map_err(io("build directory"))?
        .is_some()
    {
        generation
            .remove_tree(tmp_name)
            .map_err(io("build directory"))?;
    }
    generation
        .create_dir(tmp_name)
        .map_err(io("build directory"))?;
    let tmp = generation
        .open_dir(tmp_name)
        .map_err(io("build directory"))?
        .ok_or_else(|| FoundryError::RepairPathConflict("build directory vanished".into()))?;
    tmp.write_new(INDEX_FILE, &buffer)
        .map_err(io("index file"))?;
    tmp.write_new(LABELS_FILE, &labels_bytes)
        .map_err(io("label map"))?;
    tmp.write_new(MANIFEST_FILE, &manifest_bytes)
        .map_err(io("generation manifest"))?;
    // Cancellation is honored up to here; the three renames below are the
    // publication itself and run to completion.
    control.check()?;
    tmp.rename_into(INDEX_FILE, &generation, INDEX_FILE)
        .map_err(io("index file"))?;
    tmp.rename_into(LABELS_FILE, &generation, LABELS_FILE)
        .map_err(io("label map"))?;
    tmp.rename_into(MANIFEST_FILE, &generation, MANIFEST_FILE)
        .map_err(io("generation manifest"))?;
    drop(tmp);
    generation
        .remove_tree(tmp_name)
        .map_err(io("build directory"))?;
    Ok(sorted.len())
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
        let entries: Vec<(String, Vec<f32>)> = (0..3)
            .map(|i| (format!("{:064x}", i), vec![i as f32 / 10.0; DIMENSIONS]))
            .collect();
        let digest = "d1".repeat(32);
        let count =
            build_generation(&store, &digest, "r1", &entries, &Control::unbounded()).unwrap();
        assert_eq!(count, 3);
        let ok = validate_generation(&store, &digest, "r1").unwrap();
        assert_eq!(ok.manifest.count, 3);
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
