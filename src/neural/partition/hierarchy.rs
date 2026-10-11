//! Independent extractive overviews, sharing the body recipe's exact inputs.
//! Overviews are separately embedded inputs, never averages of child vectors.
//! They point back to current source; omitted bytes are not claimed as represented.
use super::*;

const MAX_PLAN_BYTES: usize = 16 * 1024 * 1024;
const MAX_CARDS: usize = 16_384;

/// Fit a distributed file extract with logarithmically many tokenizations.
/// The previous greedy loop re-encoded a nearly full input for every remaining
/// header. This bounded prefix search always verifies the retained text; token
/// counts need not be monotonic and no maximal-fill claim is made.
fn fit_file_extract(
    source: &str,
    prefix: &str,
    excerpts: &[(usize, usize)],
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<String, ProviderError> {
    let mut text = prefix.to_owned();
    let limit = prefix
        .len()
        .saturating_add(recipe.card_tokens.saturating_mul(16));
    for &(start, end) in excerpts {
        let part = &source[start..end];
        let mut length = part.len().min(limit.saturating_sub(text.len()));
        while !part.is_char_boundary(length) {
            length -= 1;
        }
        text.push_str(&part[..length]);
        if text.len() >= limit || length < part.len() {
            break;
        }
        text.push('\n');
    }
    let fits = |end| {
        tokens
            .count(&provider::render(recipe.template, &text[..end]))
            .map(|count| count <= recipe.card_tokens)
    };
    if fits(text.len())? {
        if text[prefix.len()..].trim().is_empty() {
            return Err(ProviderError::InputTooLarge(
                "file overview has no source content".into(),
            ));
        }
        return Ok(text);
    }
    let (mut low, mut high) = (prefix.len(), text.len().saturating_sub(1));
    while low < high {
        let mut middle = low + (high - low).div_ceil(2);
        while !text.is_char_boundary(middle) {
            middle -= 1;
        }
        if middle <= low {
            middle = low + text[low..].chars().next().map_or(0, char::len_utf8);
            if middle > high || middle <= low {
                break;
            }
        }
        if fits(middle)? {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    if text[prefix.len()..low].trim().is_empty() {
        return Err(ProviderError::InputTooLarge(
            "file overview cannot fit source content".into(),
        ));
    }
    text.truncate(low);
    Ok(text)
}

#[derive(Clone, Debug)]
enum Content {
    Body(body::Span),
    Overview(String),
}

#[derive(Clone, Debug)]
pub struct Span {
    start: usize,
    end: usize,
    key: String,
    content: Content,
}

pub fn card(
    source: &str,
    span: &Span,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Card, ProviderError> {
    let card = match &span.content {
        Content::Body(body) => body::card(source, body, recipe, tokens)?,
        Content::Overview(text) => {
            let rendered = provider::render(recipe.template, text);
            let ids = tokens.encode(&rendered)?.ids;
            if ids.is_empty() || ids.len() > recipe.card_tokens {
                return Err(ProviderError::InputTooLarge(
                    "hierarchy overview no longer fits".into(),
                ));
            }
            Card {
                start: span.start,
                end: span.end,
                input_key: provider::input_key(recipe.function_digest, &rendered),
                ids,
            }
        }
    };
    if card.input_key != span.key {
        return Err(ProviderError::InputTooLarge(
            "hierarchy input changed while rendering".into(),
        ));
    }
    Ok(card)
}

fn overview(
    source: &str,
    headers: &[(usize, usize)],
    range: std::ops::Range<usize>,
    kind: &str,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Span, ProviderError> {
    let (start, end) = (range.start, range.end);
    let fitting = tokens.fitting();
    let tokens = fitting.as_deref().unwrap_or(tokens);
    let prefix = format!("hierarchy-v1 {kind}\nextractive overview (may omit source):\n");
    // A complete short scope is more useful than just signatures. Long scopes
    // use their opening documentation and distributed declaration headers.
    let text = if let Some(full) = complete_text(&source[start..end], &prefix, recipe, tokens)? {
        full
    } else {
        let mut excerpts = Vec::new();
        let first = source[start..end]
            .find('\n')
            .map_or(end, |at| start + at + 1);
        excerpts.push((start, first));
        // Seek this scope's headers, then inspect at most 128 distributed
        // declarations. Deep forests do not cause repeated whole-file scans.
        let lo = headers.partition_point(|&(head, _)| head < start);
        let hi = headers.partition_point(|&(head, _)| head < end);
        if kind == "file" && lo == hi {
            // Unmapped/plain text and parse fallbacks have no declarations.
            // Their first line may be blank or a license. Use bounded source
            // samples instead of embedding a generic prefix for every file.
            excerpts = text_excerpts(source, start..end);
        }
        let mut queue = std::collections::VecDeque::from([(lo, hi)]);
        while let Some((lo, hi)) = queue.pop_front() {
            if lo >= hi {
                continue;
            }
            let header = headers[lo];
            if header.1 <= end && !excerpts.contains(&header) {
                excerpts.push(header);
            }
            if excerpts.len() == 129 {
                break;
            }
            let mid = lo + 1 + (hi - lo - 1) / 2;
            queue.push_back((mid, hi));
            queue.push_back((lo + 1, mid));
        }
        if kind == "file" {
            fit_file_extract(source, &prefix, &excerpts, recipe, tokens)?
        } else {
            let mut text = prefix.clone();
            for (lo, hi) in excerpts {
                let mut part = &source[lo..hi];
                // Bounds pathological single-line signatures before tokenization.
                let mut cap = part.len().min(recipe.card_tokens.saturating_mul(16));
                while !part.is_char_boundary(cap) {
                    cap -= 1;
                }
                part = &part[..cap];
                let candidate = format!("{text}{part}\n");
                if tokens.count(&provider::render(recipe.template, &candidate))?
                    <= recipe.card_tokens
                {
                    text = candidate;
                }
            }
            if text == prefix {
                let mut length = (end - start).min(recipe.card_tokens.saturating_mul(16));
                loop {
                    while !source.is_char_boundary(start + length) {
                        length -= 1;
                    }
                    if length == 0 {
                        return Err(ProviderError::InputTooLarge(
                            "overview cannot fit any source content".into(),
                        ));
                    }
                    let candidate = format!("{prefix}{}", &source[start..start + length]);
                    if tokens.count(&provider::render(recipe.template, &candidate))?
                        <= recipe.card_tokens
                    {
                        text = candidate;
                        break;
                    }
                    length /= 2;
                }
            }
            text
        }
    };
    let rendered = provider::render(recipe.template, &text);
    if tokens.count(&rendered)? > recipe.card_tokens {
        return Err(ProviderError::InputTooLarge(
            "hierarchy overview prefix does not fit".into(),
        ));
    }
    Ok(Span {
        start,
        end,
        key: provider::input_key(recipe.function_digest, &rendered),
        content: Content::Overview(text),
    })
}

fn complete_text(
    source: &str,
    prefix: &str,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Option<String>, ProviderError> {
    if source.len() > recipe.card_tokens.saturating_mul(16) {
        return Ok(None);
    }
    let text = format!("{prefix}{source}");
    Ok(
        (tokens.count(&provider::render(recipe.template, &text))? <= recipe.card_tokens)
            .then_some(text),
    )
}

/// Sample independent regions in a stable breadth-first order. Each sample
/// contributes at most 256 bytes, so a single long opening line cannot consume
/// the entire extraction allowance. Source is never rewritten or decoded lossy.
fn text_excerpts(source: &str, range: std::ops::Range<usize>) -> Vec<(usize, usize)> {
    let mut excerpts = Vec::new();
    for slot in [0, 8, 4, 12, 2, 6, 10, 14, 1, 3, 5, 7, 9, 11, 13, 15] {
        let mut start = range.start + range.len() * slot / 16;
        while !source.is_char_boundary(start) {
            start -= 1;
        }
        let line = source[range.start..start]
            .rfind('\n')
            .map_or(range.start, |at| range.start + at + 1);
        if start - line <= 128 {
            start = line;
        }
        let mut end = (start + 256).min(range.end);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        start += source[start..end].len() - source[start..end].trim_start().len();
        if !source[start..end].trim().is_empty() && !excerpts.contains(&(start, end)) {
            excerpts.push((start, end));
        }
    }
    // Sparse files can have source between every sampled region. Retain at
    // least some actual nonblank content instead of publishing a prefix alone.
    if excerpts.is_empty()
        && let Some((at, _)) = source[range.clone()]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
    {
        let start = range.start + at;
        let mut end = (start + 256).min(range.end);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        excerpts.push((start, end));
    }
    excerpts
}

fn headers(source: &str, units: &[Unit]) -> Vec<(usize, usize)> {
    let mut headers: Vec<_> = units
        .iter()
        .filter_map(|unit| {
            let end = unit.body.map_or_else(
                || {
                    source[unit.head..unit.end]
                        .find('\n')
                        .map_or(unit.end, |i| unit.head + i + 1)
                },
                |(body, _)| signature_end(source, unit.head, body).min(unit.end),
            );
            (unit.head < end).then_some((unit.head, end))
        })
        .collect();
    headers.sort_unstable();
    headers.dedup();
    headers
}

/// Broad, bounded startup: at most one input per nonblank source. This never
/// fits body windows merely to discard them. Matching file inputs retain
/// hierarchy-v1's exact keys; source freshness still binds their locations.
pub fn file_plan(
    source: &str,
    lang: Option<Lang>,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Vec<Span>, ProviderError> {
    if source.trim().is_empty() {
        return Ok(Vec::new());
    }
    // Most repository files are small. If all their bytes fit, parsing a
    // syntax forest cannot improve this input. The exact input/cache key is
    // identical to hierarchy's full-file representation.
    let fitting = tokens.fitting();
    let counter = fitting.as_deref().unwrap_or(tokens);
    let prefix = "hierarchy-v1 file\nextractive overview (may omit source):\n";
    if let Some(text) = complete_text(source, prefix, recipe, counter)? {
        return Ok(vec![Span {
            start: 0,
            end: source.len(),
            key: provider::input_key(
                recipe.function_digest,
                &provider::render(recipe.template, &text),
            ),
            content: Content::Overview(text),
        }]);
    }
    let units = lang.map_or_else(Vec::new, |lang| syntax::units(source, lang));
    Ok(vec![overview(
        source,
        &headers(source, &units),
        0..source.len(),
        "file",
        recipe,
        tokens,
    )?])
}

/// One bounded source plan; token arrays are not retained. Source coordinates
/// sort in preorder, then by input key when distinct representations share a range.
pub fn plan(
    source: &str,
    lang: Option<Lang>,
    recipe: &CardRecipe<'_>,
    parent_tokens: usize,
    tokens: &dyn TokenCount,
) -> Result<Vec<Span>, ProviderError> {
    let mut spans = Vec::new();
    body::visit(source, lang, recipe, parent_tokens, tokens, |body, card| {
        spans.push(Span {
            start: card.start,
            end: card.end,
            key: card.input_key,
            content: Content::Body(body),
        });
    })?;
    if source.trim().is_empty() {
        return Ok(spans);
    }
    let units = lang.map_or_else(Vec::new, |lang| syntax::units(source, lang));
    let headers = headers(source, &units);
    let mut bytes = 0usize;
    let mut add = |span: Span| -> Result<(), ProviderError> {
        if let Content::Overview(text) = &span.content {
            bytes = bytes.saturating_add(text.len());
        }
        if bytes > MAX_PLAN_BYTES || spans.len() >= MAX_CARDS {
            return Err(ProviderError::InputTooLarge(
                "hierarchy plan exceeds its bound".into(),
            ));
        }
        spans.push(span);
        Ok(())
    };
    add(overview(
        source,
        &headers,
        0..source.len(),
        "file",
        recipe,
        tokens,
    )?)?;
    for unit in units.iter().filter(|u| !u.children.is_empty()) {
        add(overview(
            source,
            &headers,
            unit.start..unit.end,
            "parent",
            recipe,
            tokens,
        )?)?;
    }
    spans.sort_by(|a, b| {
        (a.start, std::cmp::Reverse(a.end), &a.key).cmp(&(
            b.start,
            std::cmp::Reverse(b.end),
            &b.key,
        ))
    });
    spans.dedup_by(|a, b| (a.start, a.end, &a.key) == (b.start, b.end, &b.key));
    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Bytes(AtomicUsize);
    impl TokenCount for Bytes {
        fn encode(&self, text: &str) -> Result<TokenizedText, ProviderError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(TokenizedText {
                ids: text.as_bytes().iter().map(|&b| u32::from(b)).collect(),
                offsets: text
                    .char_indices()
                    .flat_map(|(i, c)| std::iter::repeat_n((i, i + c.len_utf8()), c.len_utf8()))
                    .collect(),
            })
        }
    }
    #[test]
    fn file_overview_fitting_is_bounded_and_preserves_utf8() {
        let tokens = Bytes(AtomicUsize::new(0));
        let recipe = CardRecipe {
            template: "{text}",
            function_digest: "fixture",
            card_tokens: 256,
        };
        let source: String = (0..200)
            .map(|n| format!("/// Item {n}.\nfn item_{n}() {{ save({n}); }}\n"))
            .collect();
        let plan = file_plan(&source, Some(Lang::Rust), &recipe, &tokens).unwrap();
        let card = card(&source, &plan[0], &recipe, &tokens).unwrap();
        assert!(card.ids.len() <= 256);
        assert!(
            tokens.0.load(Ordering::Relaxed) <= 24,
            "bounded prefix fitting, not one encoding per declaration"
        );
        let recipe = CardRecipe {
            card_tokens: 4,
            ..recipe
        };
        assert_eq!(
            fit_file_extract("🦀a", "", &[(0, 5)], &recipe, &tokens).unwrap(),
            "🦀"
        );
    }

    #[test]
    fn unparsed_overviews_represent_content_after_blank_and_long_openings() {
        let tokens = Bytes(AtomicUsize::new(0));
        let recipe = CardRecipe {
            template: "{text}",
            function_digest: "fixture",
            card_tokens: 800,
        };
        let first = format!(
            "\n{}\nmidpoint revenue recognition\n{}",
            "🦀".repeat(1000),
            "x".repeat(4000)
        );
        let second = first.replace("revenue recognition", "inventory tracking");
        let a = file_plan(&first, None, &recipe, &tokens).unwrap();
        let b = file_plan(&second, None, &recipe, &tokens).unwrap();
        assert_ne!(
            a[0].key, b[0].key,
            "middle samples affect the independent vector input"
        );
        let input = card(&first, &a[0], &recipe, &tokens).unwrap();
        let text = String::from_utf8(input.ids.iter().map(|&id| id as u8).collect()).unwrap();
        assert!(text.contains("midpoint revenue recognition"), "{text}");
        assert!(input.ids.len() <= 800);
        let sparse = format!("{}unique content{}", "\n".repeat(17000), "\n".repeat(31000));
        let plan = file_plan(&sparse, None, &recipe, &tokens).unwrap();
        let input = card(&sparse, &plan[0], &recipe, &tokens).unwrap();
        assert!(
            String::from_utf8(input.ids.iter().map(|&id| id as u8).collect())
                .unwrap()
                .contains("unique content")
        );
        assert!(fit_file_extract("\n", "prefix", &[(0, 1)], &recipe, &tokens).is_err());
    }

    #[test]
    fn short_file_fast_path_has_the_same_input_as_full_hierarchy() {
        let tokens = Bytes(AtomicUsize::new(0));
        let recipe = CardRecipe {
            template: "document: {text}",
            function_digest: "fixture",
            card_tokens: 256,
        };
        let source = "// settle an account\nfn settle() { credit(); }\n";
        let file = file_plan(source, Some(Lang::Rust), &recipe, &tokens).unwrap();
        assert_eq!(
            tokens.0.load(Ordering::Relaxed),
            1,
            "one complete-input fit"
        );
        let hierarchy = plan(source, Some(Lang::Rust), &recipe, 32, &tokens).unwrap();
        assert!(hierarchy.iter().any(|span| span.key == file[0].key));
        assert!(
            file_plan(" \n\t", None, &recipe, &tokens)
                .unwrap()
                .is_empty()
        );
    }
}
