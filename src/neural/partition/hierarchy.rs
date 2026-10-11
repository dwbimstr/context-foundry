//! Independent extractive overviews, sharing the body recipe's exact inputs.
//! Overviews are separately embedded inputs, never averages of child vectors.
//! They point back to current source; omitted bytes are not claimed as represented.
use super::*;

const MAX_PLAN_BYTES: usize = 16 * 1024 * 1024;
const MAX_CARDS: usize = 16_384;

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
    let full = (end - start <= recipe.card_tokens.saturating_mul(16))
        .then(|| format!("{prefix}{}", &source[start..end]));
    let fits_full = full
        .as_ref()
        .map(|full| tokens.count(&provider::render(recipe.template, full)))
        .transpose()?
        .is_some_and(|count| count <= recipe.card_tokens);
    let text = if fits_full {
        full.expect("counted complete scope")
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
            if tokens.count(&provider::render(recipe.template, &candidate))? <= recipe.card_tokens {
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
