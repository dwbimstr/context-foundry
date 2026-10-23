//! 009 T002 query-side retrieval: embed one query under its ceiling, load
//! the derived dense index through the store descriptor (never a re-resolved
//! path) from the very bytes that matched the generation manifest, with the
//! USearch header and geometry check T001 deferred to this task, and return
//! the bounded dense window of nearest cards. A dense hit expands to unit
//! locations through the generation's own label map; no request walks the
//! partition table.
//!
//! 009 T004: only a query without an anchor reaches this module (the
//! request path decides that before any model call); the candidate assembly
//! normally places the window's units first, in similarity order, ahead of the
//! lexical candidates (`Engine::search_candidates_semantic`). Nothing here
//! rewrites the query or calls a second model. Hierarchy localization (experimental prose selection only)
//! exactly scores a bounded set of published body vectors from discovered files,
//! reusing that query vector and retaining only current point-joined locations.
//! A failure at any
//! step is a NAMED fallback ([`Fallback`]) for the baseline path; it never
//! fails the response.
use crate::control::Control;
use crate::error::FoundryError;
use crate::neural::index::{
    self, COVERAGE_COMPLETE, GenerationError, METRIC, SCALAR_KIND, UnitLocation,
};
use crate::neural::partition::TokenCount as _;
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::{self, EmbeddingProvider, LateCall, ProviderError, TokenizedInput};
use crate::neural::tokenize::DocumentTokenizer;
use crate::store::Engine;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

mod refinement;
mod runtime;
use super::area;
use super::cache;
pub(crate) use refinement::{OwnerProof, OwnerScope};
pub(crate) use runtime::KnowledgeCall;
pub use runtime::{DocumentCall, MakeProvider, QueryRuntime};

/// The dense candidate window (D001): the top 64 nearest cards.
pub const DENSE_WINDOW: usize = 64;
/// The per-request query-embedding ceiling (D001): the earlier of 1500 ms
/// and HALF the remaining read deadline, so a cut-off embedding always
/// leaves the baseline fallback at least as much time as it got; deadlines
/// never accumulate.
pub const QUERY_CEILING: Duration = Duration::from_millis(1500);

/// Why one request is served without dense candidates: a stable category
/// `code` (`provider_timeout`, `profile_mismatch`, `index_unavailable`,
/// `index_geometry`, `semantic_unprepared`, …) and a detail. The response
/// header names it as `fallback:<code>[: <detail>]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallback {
    pub code: &'static str,
    pub detail: String,
}

impl Fallback {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.detail.is_empty() {
            f.write_str(self.code)
        } else {
            write!(f, "{}: {}", self.code, self.detail)
        }
    }
}

impl From<ProviderError> for Fallback {
    fn from(error: ProviderError) -> Self {
        let text = error.to_string();
        let detail = text
            .strip_prefix(error.code())
            .map_or(text.as_str(), |rest| rest.trim_start_matches(": "));
        Self::new(error.code(), detail)
    }
}

impl From<GenerationError> for Fallback {
    fn from(error: GenerationError) -> Self {
        match error {
            GenerationError::Interrupted(error) => error.into(),
            GenerationError::Unavailable(reason) => Self::new("index_unavailable", reason),
        }
    }
}

impl From<FoundryError> for Fallback {
    fn from(error: FoundryError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

/// One dense hit: the label of a stored vector in the serving generation
/// and its distance to the query vector (ascending = better).
#[derive(Clone, Debug)]
pub struct DenseHit {
    pub label: usize,
    pub distance: f32,
}

/// The dense window of one request: the top hits of the validated index,
/// nearest first, read against the generation that produced them.
pub struct DenseWindow {
    pub hits: Vec<DenseHit>,
    generation: Arc<DenseIndex>,
    /// Reused only for bounded exact scoring of already published body
    /// vectors. Refinement never embeds source or starts preparation in a read.
    vector: Vec<f32>,
    pub(crate) areas: Vec<crate::store::RecordCandidate>,
}

impl std::fmt::Debug for DenseWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DenseWindow")
            .field("hits", &self.hits)
            .finish_non_exhaustive()
    }
}

/// A validated, loaded dense index. Availability came from VALIDATION
/// (manifest, file hashes, header and geometry), and the index was restored
/// from the very buffer whose hash matched the manifest, read through the
/// store's bound descriptor: no later file substitution can reach it.
pub struct DenseIndex {
    segments: Vec<DenseSegment>,
    identity: index::Prepared,
    owner: Option<Arc<crate::neural::work::Wake>>,
    receipt: Arc<index::verification::ManifestReceipt>,
    dimensions: usize,
    /// Shared current locations keyed by the segment ordinal's input key.
    labels: index::Locations,
    /// The source revision the label map was read at.
    source_revision: u64,
    /// True when the generation covers every eligible unit at that revision.
    complete: bool,
}

struct DenseSegment {
    index: Arc<usearch::Index>,
    keys: Arc<Vec<String>>,
}

impl DenseIndex {
    /// Load the serving generation: the state row names the function digest
    /// and recipe; the manifest and label map are validated by content (the
    /// manifest's geometry against the profile's `dimensions`) and the index
    /// file is read ONCE and hashed in memory; then the USearch header of
    /// THOSE bytes is checked against the manifest's geometry BEFORE they
    /// are restored, and the restored index's own geometry is checked
    /// again. `Err` is the named fallback the baseline path reports.
    pub fn load(
        engine: &Engine,
        expected_digest: &str,
        expected_recipe: &str,
        dimensions: usize,
        control: &Control,
    ) -> Result<Arc<Self>, Fallback> {
        let unprepared = || Fallback::new("semantic_unprepared", "no semantic profile prepared");
        let Some(state) = engine.semantic_state()? else {
            return Err(unprepared());
        };
        let (digest, recipe) = match (state.function_digest, state.recipe_id) {
            (Some(digest), Some(recipe)) => (digest, recipe),
            _ => return Err(unprepared()),
        };
        // A store prepared for ANOTHER profile or partition recipe never
        // serves this one: unavailable by name, baseline intact.
        if digest != expected_digest {
            return Err(Fallback::new(
                "profile_mismatch",
                "the store was prepared for a different document function; \
                 run `semantic prepare` for this profile",
            ));
        }
        if recipe != expected_recipe {
            return Err(Fallback::new(
                "profile_mismatch",
                "the store's partition recipe differs from this profile's",
            ));
        }
        let anchor = engine.semantic_anchor()?;
        let identity = index::Prepared {
            digest,
            recipe,
            dimensions,
        };
        let (generation, bytes) =
            index::validate_cached(&anchor, &identity, control, true, &engine.semantic_segments)?;
        neural_fault!(LOAD_AFTER_VERIFY, Some(control), &identity.digest)?;
        let hashes: Vec<_> = std::iter::once(generation.manifest.index_sha256.clone())
            .chain(generation.manifest.delta.as_ref().map(|d| d.sha256.clone()))
            .collect();
        let mut loaded = Self::from_buffers(generation, bytes.expect("kept segments"))?;
        Arc::get_mut(&mut loaded).expect("new generation").owner =
            Some(engine.semantic_wake.clone());
        let mut cache = index::verification::lock(&engine.semantic_segments, control)?;
        for (hash, segment) in hashes.iter().zip(&loaded.segments) {
            cache.remember(hash, segment.index.clone());
        }
        Ok(loaded)
    }

    pub(crate) fn from_buffers(
        generation: index::Generation,
        bytes: index::IndexBuffers,
    ) -> Result<Arc<Self>, Fallback> {
        let manifest = &generation.manifest;
        let mut segments = Vec::with_capacity(2);
        let mut load = |segment: index::Segment, keys: Arc<Vec<String>>| -> Result<(), Fallback> {
            let geometry = |detail: String| Fallback::new("index_geometry", detail);
            let loaded = match segment {
                index::Segment::Bytes(bytes) => {
                    let header = usearch::Index::metadata_from_buffer(&bytes).map_err(|e| {
                        Fallback::new("index_unavailable", format!("usearch header: {e}"))
                    })?;
                    if header.dimensions as usize != manifest.dimensions
                        || header.count_present as usize != keys.len()
                        || header.multi
                        || !format!("{:?}", header.metric).eq_ignore_ascii_case(METRIC)
                        || !format!("{:?}", header.quantization).eq_ignore_ascii_case(SCALAR_KIND)
                    {
                        return Err(geometry("usearch header disagrees with generation geometry or segment membership".into()));
                    }
                    neural_fault!(SEGMENT_RESTORE, None, "ANN segment")?;
                    Arc::new(usearch::Index::restore_from_buffer(&bytes).map_err(|e| {
                        Fallback::new("index_unavailable", format!("usearch load: {e}"))
                    })?)
                }
                index::Segment::Resident(index) => index,
            };
            if loaded.dimensions() != manifest.dimensions
                || loaded.size() != keys.len()
                || !format!("{:?}", loaded.metric_kind()).eq_ignore_ascii_case(METRIC)
                || !format!("{:?}", loaded.scalar_kind()).eq_ignore_ascii_case(SCALAR_KIND)
            {
                return Err(geometry(
                    "loaded index geometry disagrees with manifest".into(),
                ));
            }
            segments.push(DenseSegment {
                index: loaded,
                keys,
            });
            Ok(())
        };
        load(bytes.base, manifest.base_entries.clone())?;
        if let (Some(bytes), Some(delta)) = (bytes.delta, &manifest.delta) {
            load(bytes, Arc::new(delta.entries.clone()))?;
        }
        Ok(Arc::new(Self {
            segments,
            identity: index::Prepared {
                digest: manifest.function_digest.clone(),
                recipe: manifest.recipe_id.clone(),
                dimensions: manifest.dimensions,
            },
            owner: None,
            receipt: generation.receipt,
            dimensions: manifest.dimensions,
            labels: generation.labels,
            source_revision: manifest.source_revision,
            complete: manifest.coverage == COVERAGE_COMPLETE,
        }))
    }

    pub(crate) fn input_key(&self, label: usize) -> &str {
        let base = &self.segments[0].keys;
        if label < base.len() {
            &base[label]
        } else {
            &self.segments[1].keys[label - base.len()]
        }
    }

    fn label_for_key(&self, key: &str) -> Option<usize> {
        let mut offset = 0;
        for segment in &self.segments {
            if let Ok(ordinal) = segment
                .keys
                .binary_search_by(|stored| stored.as_str().cmp(key))
            {
                return Some(offset + ordinal);
            }
            offset += segment.keys.len();
        }
        None
    }

    fn score_label(
        &self,
        query: &[f32],
        label: usize,
        scratch: &mut [f32],
    ) -> Result<DenseHit, Fallback> {
        if query.len() != self.dimensions || scratch.len() != self.dimensions {
            return Err(Fallback::new(
                "index_geometry",
                "refinement vector dimensions disagree",
            ));
        }
        let mut ordinal = label;
        for segment in &self.segments {
            if ordinal >= segment.keys.len() {
                ordinal -= segment.keys.len();
                continue;
            }
            let count = segment.index.get(ordinal as u64, scratch).map_err(|e| {
                Fallback::new("index_unavailable", format!("usearch vector lookup: {e}"))
            })?;
            if count != 1 {
                return Err(Fallback::new(
                    "index_unavailable",
                    "published body vector is missing",
                ));
            }
            // USearch stores f16 vectors. Account for quantization's small norm
            // change instead of assuming the restored f32 values have norm 1.
            let (mut dot, mut query_norm, mut body_norm) = (0.0f64, 0.0f64, 0.0f64);
            for (&a, &b) in query.iter().zip(scratch.iter()) {
                let (a, b) = (f64::from(a), f64::from(b));
                dot += a * b;
                query_norm += a * a;
                body_norm += b * b;
            }
            if !dot.is_finite()
                || !query_norm.is_finite()
                || !body_norm.is_finite()
                || query_norm <= 0.0
                || body_norm <= 0.0
            {
                return Err(Fallback::new(
                    "index_geometry",
                    "refinement vector is nonfinite or zero",
                ));
            }
            let distance = (1.0 - dot / (query_norm * body_norm).sqrt()).clamp(0.0, 2.0) as f32;
            return Ok(DenseHit { label, distance });
        }
        Err(Fallback::new(
            "index_unavailable",
            "refinement label is outside generation membership",
        ))
    }

    /// At most two searches and 2*k candidates. Filtering happens INSIDE ANN
    /// selection, so retired base entries cannot consume the candidate window.
    pub fn search(&self, vector: &[f32], k: usize) -> Result<Vec<DenseHit>, Fallback> {
        if vector.len() != self.dimensions {
            return Err(Fallback::new(
                "index_unavailable",
                format!(
                    "query vector has {} values, index holds {}",
                    vector.len(),
                    self.dimensions
                ),
            ));
        }
        let k = k.min(DENSE_WINDOW);
        if k == 0 {
            return Ok(Vec::new());
        }
        let mut hits = Vec::with_capacity(k * self.segments.len());
        let mut offset = 0;
        for segment in &self.segments {
            let matches = segment
                .index
                .filtered_search(vector, k, |key| {
                    segment
                        .keys
                        .get(key as usize)
                        .is_some_and(|key| self.labels.contains_key(key))
                })
                .map_err(|e| Fallback::new("index_unavailable", format!("usearch search: {e}")))?;
            // usearch returns only keys the predicate admitted.
            for (&key, &distance) in matches.keys.iter().zip(&matches.distances) {
                hits.push(DenseHit {
                    label: offset + key as usize,
                    distance,
                });
            }
            offset += segment.keys.len();
        }
        hits.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| self.input_key(a.label).cmp(self.input_key(b.label)))
        });
        // Membership validation makes segments disjoint, so there are no duplicate keys.
        hits.truncate(k);
        Ok(hits)
    }
}

/// A named fallback word for the response header: `fallback:<reason>`, one
/// short line with no header separator, so a provider's message can never
/// forge a segment.
pub fn fallback_word(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|c| {
            if c.is_control() || c == '\u{b7}' {
                ' '
            } else {
                c
            }
        })
        .take(120)
        .collect();
    format!("fallback:{}", cleaned.trim())
}

/// Render one query with the profile's query template, tokenize it and
/// refuse the serving limit: the exact model input of a query.
pub fn tokenize_query(
    tokenizer: &DocumentTokenizer,
    template: &str,
    query: &str,
) -> Result<TokenizedInput, ProviderError> {
    tokenize_query_with_limit(tokenizer, template, query, provider::SERVING_LIMIT_TOKENS)
}

fn tokenize_query_with_limit(
    tokenizer: &DocumentTokenizer,
    template: &str,
    query: &str,
    limit: usize,
) -> Result<TokenizedInput, ProviderError> {
    let rendered = provider::render(template, query);
    let tokenized = tokenizer.encode(&rendered)?;
    let input = TokenizedInput { ids: tokenized.ids };
    provider::check_query_with_limit(&input, limit)?;
    Ok(input)
}
