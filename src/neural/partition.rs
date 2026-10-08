//! 009 retrieval partition, recipe `nemotron-units-v1` (spec 009 § Documents,
//! embedding units and returned evidence).
//!
//! The recipe pins: the 001 T005 grammar versions this crate builds
//! (tree-sitter/pulldown-cmark as pinned by `Cargo.toml`), the tokenizer
//! identity string, the 1024-token document-unit limit including the
//! `passage: ` rendered prefix and special tokens, and the tie-breaks below.
//! Changing any of them is a recipe-version bump, never a tuning step.
//!
//! Tie-breaks and ordering rules: pieces are emitted in source order;
//! whitespace between pieces joins the FOLLOWING piece and a trailing tail
//! joins the last (decided before any limit check or greedy combination, at
//! the top level and inside oversized descents alike); a delivery unit that
//! alone exceeds the limit is replaced by its direct children's ranges plus
//! the residual regions between them, recursively; every other oversized
//! piece splits at line boundaries (each line keeps its own terminating
//! newline), then into token-bounded UTF-8 spans; the greedy combine walks
//! pieces in source order and closes a unit before the next piece would
//! overflow it. Pieces tile `[0, len)` exactly, so every nonempty source byte
//! belongs to exactly one embedding unit and no byte is encoded twice. An
//! empty source partitions to zero units. Counts are the exact final model
//! input; nothing is ever truncated silently — an input that cannot fit is
//! refused.
//!
//! This module never loads a tokenizer: token counting goes through the
//! [`TokenCount`] seam so it stays pure and testable.
use crate::neural::provider::{self, DOCUMENT_PREFIX, DOCUMENT_UNIT_TOKENS, ProviderError};
use crate::syntax::{self, Lang, Unit};
use std::collections::HashMap;

/// The pinned recipe grammar version. `-v2` (001 T008): 15 more languages
/// have units and every grammar parses on the `tree-sitter` 0.26 runtime,
/// so partitions made under `-v1` are stale.
pub const RECIPE_GRAMMAR: &str = "nemotron-units-v2";

/// The partition recipe id for one tokenizer identity. It participates in
/// mapping eligibility: a partition row under another recipe is stale.
pub fn recipe_id(tokenizer_identity: &str) -> String {
    format!("{RECIPE_GRAMMAR}+{tokenizer_identity}")
}

/// One tokenized embedding unit of a partitioned source: a half-open byte
/// range, its exact document-input key and the exact model-input token ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitPartition {
    pub start: usize,
    pub end: usize,
    pub input_key: String,
    pub ids: Vec<u32>,
}

/// The exact tokenization of one rendered input: ids and, for each id, its
/// half-open byte offsets into the rendered string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenizedText {
    pub ids: Vec<u32>,
    pub offsets: Vec<(usize, usize)>,
}

/// Exact model-input tokenization seam. The production implementation is the
/// Rust tokenizer over the profile's verified `tokenizer.json`
/// ([`crate::neural::tokenize`]); tests use deterministic byte counters.
/// Implementations never truncate.
pub trait TokenCount {
    fn encode(&self, rendered: &str) -> Result<TokenizedText, ProviderError>;

    fn count(&self, rendered: &str) -> Result<usize, ProviderError> {
        Ok(self.encode(rendered)?.ids.len())
    }
}

/// The unit forest of one source, indexed by range for the descent rule.
struct Forest<'a> {
    units: &'a [Unit],
    by_range: HashMap<(usize, usize), usize>,
}

impl<'a> Forest<'a> {
    fn new(units: &'a [Unit]) -> Self {
        let by_range = units
            .iter()
            .enumerate()
            .map(|(i, u)| ((u.start, u.end), i))
            .collect();
        Self { units, by_range }
    }

    fn index_of(&self, start: usize, end: usize) -> Option<usize> {
        self.by_range.get(&(start, end)).copied()
    }
}

/// One piece of a region: the bytes `lead..tail_end` are its embedding range,
/// where `lead` includes the whitespace that precedes the piece and
/// `tail_end` the trailing tail that joined it. A unit piece keeps its
/// natural `start..end` for forest identity.
struct Piece {
    lead: usize,
    start: usize,
    end: usize,
    tail_end: usize,
    unit: bool,
}

/// Lay a region `lo..hi` out as pieces around the ordered, strictly inside
/// `children` ranges. Whitespace-only separators join the following piece; a
/// separator holding text becomes a block whose leading whitespace is its own
/// and whose trailing whitespace joins the following piece; a whitespace-only
/// tail joins the last piece and a textual tail is a block. `None` when the
/// children are not ordered inside the region.
fn layout(source: &str, lo: usize, hi: usize, children: &[(usize, usize)]) -> Option<Vec<Piece>> {
    let mut pieces: Vec<Piece> = Vec::with_capacity(children.len() * 2 + 1);
    // End of the previously emitted piece: whitespace after it belongs to the
    // next piece.
    let mut pending = lo;
    for &(start, end) in children {
        if start < pending || end > hi || start >= end {
            return None;
        }
        if pending < start {
            let gap = &source[pending..start];
            if !gap.trim().is_empty() {
                let block_end = pending + gap.trim_end().len();
                pieces.push(Piece {
                    lead: pending,
                    start: pending,
                    end: block_end,
                    tail_end: block_end,
                    unit: false,
                });
                pending = block_end;
            }
        }
        pieces.push(Piece {
            lead: pending,
            start,
            end,
            tail_end: end,
            unit: true,
        });
        pending = end;
    }
    if pending < hi {
        if source[pending..hi].trim().is_empty() {
            match pieces.last_mut() {
                Some(last) => last.tail_end = hi,
                None => pieces.push(Piece {
                    lead: pending,
                    start: pending,
                    end: hi,
                    tail_end: hi,
                    unit: false,
                }),
            }
        } else {
            pieces.push(Piece {
                lead: pending,
                start: pending,
                end: hi,
                tail_end: hi,
                unit: false,
            });
        }
    }
    Some(pieces)
}

/// Partition `source` into embedding units under `function_digest`. The
/// source must be valid UTF-8 (admitted sources are); `lang` is 001's mapping
/// of the path. Empty sources return an empty partition, which the caller
/// records as a completed zero-unit partition.
pub fn partition(
    source: &str,
    lang: Option<Lang>,
    function_digest: &str,
    tokens: &dyn TokenCount,
) -> Result<Vec<UnitPartition>, ProviderError> {
    if source.is_empty() {
        return Ok(Vec::new());
    }
    // `Lang::from_path` decides mapping: only languages with delivery units
    // (and sources under the parse bound, which yield no units) use the
    // unit forest; everything else partitions by blank-line paragraphs.
    let units = match lang.filter(|l| l.has_units()) {
        Some(mapped) => syntax::units(source, mapped),
        None => Vec::new(),
    };
    let forest = Forest::new(&units);
    let roots: Vec<(usize, usize)> = units
        .iter()
        .filter(|u| u.parent.is_none())
        .map(|u| (u.start, u.end))
        .collect();
    let top = if roots.is_empty() {
        None
    } else {
        layout(source, 0, source.len(), &roots)
    };
    let mut resolved: Vec<(usize, usize)> = Vec::new();
    match top {
        Some(pieces) => {
            resolve_pieces(source, &forest, pieces, tokens, &mut resolved)?;
        }
        // A forest defect must never double-encode bytes; paragraphs tile by
        // construction.
        None => {
            for (start, end) in paragraphs(source) {
                resolve_generic(source, start, end, tokens, &mut resolved)?;
            }
        }
    }
    let combined = combine(source, &resolved, tokens)?;
    let mut out = Vec::with_capacity(combined.len());
    for (start, end) in combined {
        let rendered = provider::render_document(&source[start..end]);
        let ids = tokens.encode(&rendered)?.ids;
        // Every emitted range was verified against the limit while it was
        // formed; this guard makes silent truncation impossible even if a
        // tokenizer behaved pathologically across boundaries.
        if ids.len() > DOCUMENT_UNIT_TOKENS {
            return Err(ProviderError::InputTooLarge(format!(
                "unit [{start},{end}) needs {} tokens; limit {DOCUMENT_UNIT_TOKENS}",
                ids.len()
            )));
        }
        out.push(UnitPartition {
            start,
            end,
            input_key: provider::input_key(function_digest, &rendered),
            ids,
        });
    }
    Ok(out)
}

/// Resolve pieces in source order with an EXPLICIT work stack (a delivery
/// forest of any depth costs heap, never call stack, and no depth cutoff can
/// change the recipe). A piece is kept whole when it fits; an oversized
/// delivery unit is replaced, in place, by its children and residual regions
/// (whitespace joining the following piece, the unit's own leading whitespace
/// and trailing tail carried to the first and last sub-piece), recursively;
/// anything else splits at lines then token-bounded spans.
fn resolve_pieces(
    source: &str,
    forest: &Forest<'_>,
    pieces: Vec<Piece>,
    tokens: &dyn TokenCount,
    out: &mut Vec<(usize, usize)>,
) -> Result<(), ProviderError> {
    let mut stack: Vec<Piece> = pieces.into_iter().rev().collect();
    while let Some(piece) = stack.pop() {
        if fits(source, piece.lead, piece.tail_end, tokens)? {
            out.push((piece.lead, piece.tail_end));
            continue;
        }
        if piece.unit
            && let Some(index) = forest.index_of(piece.start, piece.end)
        {
            let children: Vec<(usize, usize)> = forest.units[index]
                .children
                .iter()
                .map(|&child| (forest.units[child].start, forest.units[child].end))
                .collect();
            if !children.is_empty()
                && let Some(mut sub) = layout(source, piece.start, piece.end, &children)
            {
                if let Some(first) = sub.first_mut() {
                    first.lead = piece.lead;
                }
                if let Some(last) = sub.last_mut() {
                    last.tail_end = piece.tail_end;
                }
                // Reversed, so the first sub-piece is popped (and emitted) first.
                stack.extend(sub.into_iter().rev());
                continue;
            }
        }
        resolve_generic(source, piece.lead, piece.tail_end, tokens, out)?;
    }
    Ok(())
}

/// Greedy line accumulation; a line that alone exceeds the limit splits into
/// token-bounded UTF-8 spans.
fn resolve_generic(
    source: &str,
    start: usize,
    end: usize,
    tokens: &dyn TokenCount,
    out: &mut Vec<(usize, usize)>,
) -> Result<(), ProviderError> {
    let mut acc: Option<(usize, usize)> = None;
    let mut pos = start;
    while pos < end {
        let line_end = line_end(source, pos, end);
        match acc {
            None => {
                if fits(source, pos, line_end, tokens)? {
                    acc = Some((pos, line_end));
                } else {
                    span_split(source, pos, line_end, tokens, out)?;
                }
            }
            Some((acc_start, _)) => {
                if fits(source, acc_start, line_end, tokens)? {
                    acc = Some((acc_start, line_end));
                } else {
                    out.push(acc.take().expect("accumulator is set"));
                    if fits(source, pos, line_end, tokens)? {
                        acc = Some((pos, line_end));
                    } else {
                        span_split(source, pos, line_end, tokens, out)?;
                    }
                }
            }
        }
        pos = line_end;
    }
    if let Some(piece) = acc {
        out.push(piece);
    }
    Ok(())
}

/// Split one oversized line into spans that each fit the unit limit. Cut
/// points come from the line's own tokenization (never mid-token) and are
/// verified by re-tokenizing the exact candidate input, so the exact final
/// model input never exceeds the limit. A span that provably cannot fit is a
/// named refusal, never a truncation.
fn span_split(
    source: &str,
    start: usize,
    end: usize,
    tokens: &dyn TokenCount,
    out: &mut Vec<(usize, usize)>,
) -> Result<(), ProviderError> {
    let rendered = provider::render_document(&source[start..end]);
    let encoded = tokens.encode(&rendered)?;
    let prefix_len = DOCUMENT_PREFIX.len();
    let ids = &encoded.ids;
    // Source byte offset of a rendered-text offset (token boundaries are
    // char boundaries in the rendered string; stay defensive anyway).
    let to_source = |rendered_offset: usize| -> usize {
        let relative = rendered_offset.saturating_sub(prefix_len);
        let mut target = start + relative;
        while target > start && !source.is_char_boundary(target) {
            target -= 1;
        }
        target.min(end)
    };
    let mut chunk_first = 0usize;
    let mut chunk_source_start = start;
    while chunk_first < ids.len() && chunk_source_start < end {
        let remaining = ids.len() - chunk_first;
        let mut take = remaining.min(DOCUMENT_UNIT_TOKENS);
        let chunk_source_end;
        loop {
            let last = chunk_first + take - 1;
            // Never cut inside a character: the smallest legal end is the
            // next char boundary after the chunk start.
            let mut min_end = chunk_source_start + 1;
            while min_end < end && !source.is_char_boundary(min_end) {
                min_end += 1;
            }
            let candidate_end = to_source(encoded.offsets[last].1).max(min_end);
            let fits = tokens.count(&provider::render_document(
                &source[chunk_source_start..candidate_end],
            ))?;
            if fits <= DOCUMENT_UNIT_TOKENS {
                chunk_source_end = candidate_end;
                break;
            }
            if take == 1 {
                return Err(ProviderError::InputTooLarge(format!(
                    "one token of the line at byte {chunk_source_start} cannot fit \
                     {DOCUMENT_UNIT_TOKENS} model tokens"
                )));
            }
            take -= 1;
        }
        out.push((chunk_source_start, chunk_source_end));
        chunk_first += take;
        chunk_source_start = chunk_source_end;
    }
    Ok(())
}

/// Greedy combine in source order: close the accumulated unit before the
/// next piece would overflow it.
fn combine(
    source: &str,
    pieces: &[(usize, usize)],
    tokens: &dyn TokenCount,
) -> Result<Vec<(usize, usize)>, ProviderError> {
    let mut result = Vec::new();
    let mut acc: Option<(usize, usize)> = None;
    for &(start, end) in pieces {
        match acc {
            None => acc = Some((start, end)),
            Some((acc_start, _)) => {
                if fits(source, acc_start, end, tokens)? {
                    acc = Some((acc_start, end));
                } else {
                    result.push(acc.replace((start, end)).expect("set above"));
                }
            }
        }
    }
    if let Some((start, end)) = acc {
        result.push((start, end));
    }
    Ok(result)
}

fn fits(
    source: &str,
    start: usize,
    end: usize,
    tokens: &dyn TokenCount,
) -> Result<bool, ProviderError> {
    Ok(tokens.count(&provider::render_document(&source[start..end]))? <= DOCUMENT_UNIT_TOKENS)
}

/// Blank-line paragraphs: consecutive non-blank lines form one piece, a
/// blank-line run joins the following piece, and a trailing run joins the
/// last. Whitespace-only sources are one piece.
fn paragraphs(source: &str) -> Vec<(usize, usize)> {
    let len = source.len();
    let mut pieces = Vec::new();
    let mut blank_run_start: Option<usize> = None;
    let mut para: Option<(usize, usize)> = None;
    let mut pos = 0usize;
    while pos < len {
        let end = line_end(source, pos, len);
        let blank = source[pos..end].trim().is_empty();
        match (&mut para, blank) {
            (None, true) => {
                blank_run_start.get_or_insert(pos);
            }
            (None, false) => {
                let start = blank_run_start.take().unwrap_or(pos);
                para = Some((start, end));
            }
            (Some(piece), false) => {
                piece.1 = end;
            }
            (Some(_), true) => {
                pieces.push(para.take().expect("paragraph is set"));
                blank_run_start = Some(pos);
            }
        }
        pos = end;
    }
    match (para, blank_run_start) {
        (Some((start, _)), _) => pieces.push((start, len)),
        // A trailing blank run joins the LAST piece; a whitespace-only
        // source is one piece.
        (None, Some(_)) => match pieces.last_mut() {
            Some(last) => last.1 = len,
            None => pieces.push((0, len)),
        },
        (None, None) => {}
    }
    pieces.retain(|&(start, end)| start < end);
    pieces
}

/// The exclusive end of the line starting at `pos`, including its `\n`.
fn line_end(source: &str, pos: usize, end: usize) -> usize {
    let line = &source.as_bytes()[pos..end];
    match line.iter().position(|&b| b == b'\n') {
        Some(newline) => pos + newline + 1,
        None => end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One token per UTF-8 byte, with offsets spanning the whole character
    /// as a byte-level tokenizer reports them: exact, never truncating.
    struct Bytes;

    impl TokenCount for Bytes {
        fn encode(&self, rendered: &str) -> Result<TokenizedText, ProviderError> {
            let mut offsets = Vec::with_capacity(rendered.len());
            for (index, ch) in rendered.char_indices() {
                for _ in 0..ch.len_utf8() {
                    offsets.push((index, index + ch.len_utf8()));
                }
            }
            Ok(TokenizedText {
                ids: rendered.bytes().map(|b| b as u32).collect(),
                offsets,
            })
        }
    }

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn units_of(source: &str, lang: Option<Lang>) -> Vec<UnitPartition> {
        partition(source, lang, DIGEST, &Bytes).unwrap()
    }

    fn cover(source: &str, units: &[UnitPartition]) {
        let mut cursor = 0usize;
        for unit in units {
            assert!(unit.start < unit.end, "empty unit");
            assert_eq!(unit.start, cursor, "gap or overlap at {}", unit.start);
            assert!(unit.ids.len() <= DOCUMENT_UNIT_TOKENS);
            cursor = unit.end;
        }
        assert_eq!(cursor, source.len());
    }

    #[test]
    fn empty_source_is_zero_units() {
        assert!(units_of("", Some(Lang::Rust)).is_empty());
    }

    /// 001 T008 gave Ruby (and 14 more languages) units: a partition made
    /// before it, under the `-v1` recipe, is not current under this one, so
    /// preparation partitions the source again.
    #[test]
    fn a_partition_made_before_t008_languages_is_not_current() {
        use crate::neural::cache::{PartitionRecord, PartitionUnit, partition_is_current};
        let source = "class Store\n  def put(a)\n    a\n  end\nend\n";
        assert!(!syntax::units(source, Lang::Ruby).is_empty());
        let units = units_of(source, Some(Lang::Ruby));
        let meta = crate::store::SourceMeta {
            hash: "h".into(),
            chunks: 1,
            bytes: source.len(),
            lines: 5,
        };
        let record = |recipe: String| PartitionRecord {
            source_hash: "h".into(),
            recipe_id: recipe,
            function_digest: DIGEST.into(),
            units: units
                .iter()
                .map(|unit| PartitionUnit {
                    start: unit.start,
                    end: unit.end,
                    input_key: unit.input_key.clone(),
                })
                .collect(),
        };
        let current = recipe_id("tok");
        assert!(partition_is_current(
            &record(current.clone()),
            &meta,
            &current,
            DIGEST
        ));
        let before = record("nemotron-units-v1+tok".into());
        assert!(!partition_is_current(&before, &meta, &current, DIGEST));
    }

    #[test]
    fn small_source_is_one_unit_with_exact_prefix() {
        let source = "pub fn a() -> u32 {\n    7\n}\n";
        let units = units_of(source, Some(Lang::Rust));
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].start, 0);
        assert_eq!(units[0].end, source.len());
        // `passage: ` is 9 bytes: the exact model input counts the prefix.
        assert_eq!(units[0].ids.len(), source.len() + 9);
        cover(source, &units);
    }

    #[test]
    fn whitespace_only_source_is_one_unit() {
        let source = "\n\n   \n\t\n";
        let units = units_of(source, None);
        assert_eq!(units.len(), 1);
        cover(source, &units);
    }

    #[test]
    fn many_tiny_functions_combine_into_one_unit() {
        let mut source = String::new();
        for i in 0..25 {
            source.push_str(&format!("pub fn f{i}() -> u32 {{ {i} }}\n"));
        }
        // 25 fns of ~27 bytes: below the 1015-byte body budget (1024 - 9 prefix).
        let units = units_of(&source, Some(Lang::Rust));
        assert_eq!(units.len(), 1);
        cover(&source, &units);
    }

    #[test]
    fn unit_limit_boundaries_are_exact() {
        // With the byte counter the prefix is 9 tokens: a 1015-byte body is
        // exactly 1024 model tokens; 1016 bytes must split, never truncate.
        let body = "x".repeat(1015);
        let units = units_of(&body, None);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].ids.len(), DOCUMENT_UNIT_TOKENS);
        let over = "x".repeat(1016);
        let units = units_of(&over, None);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].ids.len(), DOCUMENT_UNIT_TOKENS);
        assert_eq!(units[1].ids.len(), 10); // 9 prefix + 1 body byte
        cover(&over, &units);
    }

    #[test]
    fn oversized_line_splits_into_token_bounded_spans_on_char_boundaries() {
        // One huge unbroken line of multibyte characters.
        let body = "é".repeat(2000);
        let units = units_of(&body, None);
        assert!(units.len() >= 3);
        cover(&body, &units);
        for unit in &units {
            assert!(body.is_char_boundary(unit.start));
            assert!(body.is_char_boundary(unit.end));
        }
    }

    #[test]
    fn oversized_unit_descends_to_children_and_residuals() {
        // One outer fn over the limit whose interior holds two small inner
        // fns (its delivery-unit children) and a large residual block
        // comment between them.
        let pad = "c".repeat(1000);
        let mut source = String::new();
        source.push_str("pub fn outer() {\n");
        source.push_str("    fn inner_a() -> u32 { 1 }\n");
        source.push_str(&format!("    /* {pad} */\n"));
        source.push_str("    fn inner_b() -> u32 { 2 }\n");
        source.push_str("}\n");
        let units = units_of(&source, Some(Lang::Rust));
        cover(&source, &units);
        // Children and residuals cover the parent exactly once and combine
        // greedily; nothing is left as one oversized blob.
        assert!(units.len() >= 2, "{:?}", units.len());
    }

    #[test]
    fn mapped_separator_whitespace_joins_the_following_piece() {
        let shell = |name: &str, total: usize| {
            let head = format!("pub fn {name}() {{ /*");
            let tail = "*/ }";
            format!(
                "{head}{}{tail}",
                "x".repeat(total - head.len() - tail.len())
            )
        };
        let (a, b) = (shell("a", 1000), shell("b", 100));
        assert_eq!((a.len(), b.len()), (1000, 100));
        // Each fits alone (1009 / 109 tokens); together they do not. The two
        // separator newlines belong to the SECOND function, not the first.
        let source = format!("{a}\n\n{b}");
        let units = units_of(&source, Some(Lang::Rust));
        cover(&source, &units);
        let spans: Vec<(usize, usize)> = units.iter().map(|u| (u.start, u.end)).collect();
        assert_eq!(spans, [(0, 1000), (1000, 1102)]);
        assert!(source[1000..1102].starts_with("\n\npub fn b"));
    }

    #[test]
    fn mapped_trailing_tail_joins_the_last_piece() {
        let shell = |name: &str, total: usize| {
            let head = format!("pub fn {name}() {{ /*");
            let tail = "*/ }";
            format!(
                "{head}{}{tail}",
                "x".repeat(total - head.len() - tail.len())
            )
        };
        let (a, b) = (shell("a", 1000), shell("b", 100));
        let source = format!("{a}\n\n{b}\n\n\n");
        let units = units_of(&source, Some(Lang::Rust));
        cover(&source, &units);
        let last = units.last().unwrap();
        assert_eq!(last.end, source.len(), "the tail joins the last piece");
        assert!(source[last.start..last.end].ends_with("\n\n\n"));
    }

    #[test]
    fn a_deep_but_small_forest_keeps_child_aware_boundaries() {
        // 70 nested modules (deeper than any fixed cutoff), a deepest module
        // holding two ~600-byte functions on ONE line. Every enclosing module
        // exceeds the limit, so each descends to its child; the two functions
        // fit alone but not together, so a unit must end exactly where the
        // first function ends — never inside it.
        let shell = |name: &str, total: usize| {
            let head = format!("pub fn {name}() {{ /*");
            let tail = "*/ }";
            format!(
                "{head}{}{tail}",
                "x".repeat(total - head.len() - tail.len())
            )
        };
        let (a, b) = (shell("a", 600), shell("b", 600));
        let mut source = String::new();
        for i in 0..70 {
            source.push_str(&format!("mod m{i} {{ "));
        }
        let a_start = source.len();
        source.push_str(&a);
        let a_end = source.len();
        source.push(' ');
        source.push_str(&b);
        source.push_str(&" }".repeat(70));
        source.push('\n');
        let units = units_of(&source, Some(Lang::Rust));
        cover(&source, &units);
        assert!(
            units.iter().any(|u| u.end == a_end),
            "no unit boundary at the end of the first function: {:?}",
            units.iter().map(|u| (u.start, u.end)).collect::<Vec<_>>()
        );
        assert!(
            units.iter().all(|u| {
                let inside = |at: usize| at > a_start && at < a_end;
                !inside(u.start) && !inside(u.end)
            }),
            "a unit boundary falls inside the first function"
        );
        let next = units
            .iter()
            .find(|u| u.start == a_end)
            .expect("a unit starts at the cut");
        assert!(
            source[next.start..next.end].starts_with(" pub fn b"),
            "the separator joins the FOLLOWING piece"
        );
    }

    #[test]
    fn oversized_markdown_section_splits_at_lines() {
        let mut source = String::from("# heading\n\n");
        for i in 0..60 {
            source.push_str(&format!("line {i:03} {}\n", "d".repeat(20)));
        }
        let units = units_of(&source, Some(Lang::Markdown));
        cover(&source, &units);
        assert!(units.len() >= 2);
    }

    #[test]
    fn fenced_heading_is_not_a_section_boundary() {
        let fenced = "# real section\n\nbody one\n\n```text\n# not a heading\n```\nmore body\n";
        let units = units_of(fenced, Some(Lang::Markdown));
        // One section: the fence content does not split it.
        assert_eq!(units.len(), 1);
        cover(fenced, &units);
    }

    #[test]
    fn crlf_and_unicode_cover_exactly() {
        let source = "alpha\r\nbeta\r\n\r\ngamma γamma 🦀 delta\r\n";
        let units = units_of(source, None);
        cover(source, &units);
        let crlf = "a\r\nb\r\nc\r\n";
        let units = units_of(crlf, Some(Lang::Rust));
        cover(crlf, &units);
    }

    #[test]
    fn paragraphs_join_separators_forward_and_tail_backward() {
        let source = "\n\none\n\ntwo\n\n\n  \nthree\n\n\n";
        let pieces = paragraphs(source);
        let mut cursor = 0;
        for &(start, end) in &pieces {
            assert_eq!(start, cursor);
            cursor = end;
        }
        assert_eq!(cursor, source.len());
        let text: Vec<&str> = pieces.iter().map(|&(a, b)| &source[a..b]).collect();
        // Leading blanks join the first paragraph; each separator joins the
        // following paragraph; the trailing tail joins the last.
        assert_eq!(text.len(), 3, "{text:?}");
        assert!(text[0].starts_with("\n\none"));
        assert!(text[1].starts_with("\ntwo"));
        assert!(text[2].starts_with("\n\n\n  \nthree") || text[2].starts_with("\n\n  \nthree"));
        assert!(text[2].ends_with("\n\n\n"));
        // Tiny paragraphs then combine greedily into one embedding unit.
        let units = units_of(source, None);
        cover(source, &units);
        assert_eq!(units.len(), 1);
    }

    #[test]
    fn identical_inputs_share_keys_independent_of_placement() {
        let body = "shared body\n";
        let left = units_of(body, None);
        let right = units_of(body, None);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].input_key, right[0].input_key);
    }
}
