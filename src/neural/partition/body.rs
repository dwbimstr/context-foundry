//! Body-v1: non-overlapping evidence, complete short units, bounded long windows.
//! Context is a source-local parent/header prefix, never an extra ANN candidate.
//! Paths and byte offsets belong to provenance, not the rendered vector input.
use super::*;
use std::sync::Arc;

const MAX_CHUNKS: usize = 16_384;

#[derive(Clone, Debug)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    context: Arc<str>,
}

fn prefix(context: &str) -> String {
    format!("body-v1\ncontext:\n{context}\nsource:\n")
}

fn encode(
    source: &str,
    span: &Span,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<(String, TokenizedText), ProviderError> {
    let evidence = source.get(span.start..span.end).ok_or_else(|| {
        ProviderError::InputTooLarge("body window has invalid source coordinates".into())
    })?;
    let text = format!("{}{}", prefix(&span.context), evidence);
    let rendered = provider::render(recipe.template, &text);
    let encoded = tokens.encode(&rendered)?;
    Ok((rendered, encoded))
}

pub fn card(
    source: &str,
    span: &Span,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Card, ProviderError> {
    let (rendered, encoded) = encode(source, span, recipe, tokens)?;
    if encoded.ids.is_empty() || encoded.ids.len() > recipe.card_tokens {
        return Err(ProviderError::InputTooLarge(
            "body window no longer fits its recipe".into(),
        ));
    }
    Ok(Card {
        start: span.start,
        end: span.end,
        input_key: provider::input_key(recipe.function_digest, &rendered),
        ids: encoded.ids,
    })
}

fn boundary(source: &str, mut at: usize) -> usize {
    while !source.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn context(
    source: &str,
    units: &[Unit],
    owner: Option<usize>,
    preamble: usize,
    limit: usize,
    tokens: &dyn TokenCount,
) -> Result<Arc<str>, ProviderError> {
    if limit == 0 {
        return Ok(Arc::from(""));
    }
    // Bound context construction before tokenizing pathological headers. Only
    // bytes actually included affect reuse; omitted context is not represented.
    let cap = limit.saturating_mul(32);
    let mut text = String::new();
    let mut append = |start: usize, end: usize| {
        let end = boundary(
            source,
            end.min(start.saturating_add(cap.saturating_sub(text.len()))),
        );
        if start < end {
            text.push_str(&source[start..end]);
            text.push('\n');
        }
    };
    if let Some(index) = owner {
        let unit = &units[index];
        if let Some(parent) = unit.parent {
            let parent = &units[parent];
            let end = parent
                .body
                .map_or(parent.head, |(body, _)| body.min(parent.end));
            append(parent.start, end);
        } else {
            append(0, preamble);
        }
        let end = unit.body.map_or(unit.head, |(body, _)| body.min(unit.end));
        append(unit.start, end);
    } else {
        append(0, preamble);
    }
    while !text.is_empty() && tokens.count(&text)? > limit {
        text.truncate(boundary(&text, text.len() / 2));
    }
    Ok(Arc::from(text))
}

/// Compute/stream cards without retaining corpus-wide token arrays. The fallback
/// render path retains just spans and shared context strings for one source.
pub fn visit(
    source: &str,
    lang: Option<Lang>,
    recipe: &CardRecipe<'_>,
    parent_tokens: usize,
    tokens: &dyn TokenCount,
    mut emit: impl FnMut(Span, Card),
) -> Result<(), ProviderError> {
    if recipe.card_tokens == 0
        || recipe.card_tokens > provider::MAX_CONTEXT_TOKENS
        || parent_tokens > recipe.card_tokens / 4
        || source.len() > 2 * 1024 * 1024
    {
        return Err(ProviderError::InputTooLarge(
            "body recipe/source exceeds its bounds".into(),
        ));
    }
    let units = lang.map_or_else(Vec::new, |lang| syntax::units(source, lang));
    let preamble = units.first().map_or(0, |unit| unit.start);
    // Unit ownership is determined in one ordered traversal. Functions/methods
    // stay whole even with nested definitions; containers contribute residuals
    // and children. Whitespace-only gaps need no representation.
    let mut regions = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut through = 0;
    for (index, unit) in units.iter().enumerate() {
        if unit.start < through {
            continue;
        }
        while stack.last().is_some_and(|&i| units[i].end <= unit.start) {
            let i = stack.pop().expect("enclosing unit");
            if through < units[i].end {
                regions.push((through, units[i].end, Some(i)));
                through = units[i].end;
            }
        }
        if through < unit.start {
            regions.push((through, unit.start, stack.last().copied()));
        }
        if unit.children.is_empty()
            || matches!(unit.kind, UnitKind::Fn | UnitKind::Method | UnitKind::Macro)
        {
            regions.push((unit.start, unit.end, Some(index)));
            through = unit.end;
        } else {
            stack.push(index);
            through = unit.start;
        }
    }
    while let Some(i) = stack.pop() {
        if through < units[i].end {
            regions.push((through, units[i].end, Some(i)));
            through = units[i].end;
        }
    }
    if through < source.len() {
        regions.push((through, source.len(), None));
    }
    let mut emitted = 0;
    let mut plan_bytes = 0usize;
    for (start, end, owner) in regions {
        if source[start..end].trim().is_empty() {
            continue;
        }
        let context = context(source, &units, owner, preamble, parent_tokens, tokens)?;
        plan_bytes = plan_bytes.saturating_add(context.len());
        if plan_bytes > 16 * 1024 * 1024 {
            return Err(ProviderError::InputTooLarge(
                "body context plan exceeds 16 MiB".into(),
            ));
        }
        let mut span = Span {
            start,
            end,
            context,
        };
        let (rendered, encoded) = encode(source, &span, recipe, tokens)?;
        if !encoded.ids.is_empty() && encoded.ids.len() <= recipe.card_tokens {
            emitted += 1;
            if emitted > MAX_CHUNKS {
                return Err(ProviderError::InputTooLarge(
                    "body source exceeds 16384 windows".into(),
                ));
            }
            emit(
                span,
                Card {
                    start,
                    end,
                    input_key: provider::input_key(recipe.function_digest, &rendered),
                    ids: encoded.ids,
                },
            );
            continue;
        }
        let pre = prefix(&span.context);
        let base = recipe
            .template
            .split_once(provider::TEXT_SLOT)
            .map_or(0, |(before, _)| before.len())
            + pre.len();
        let overhead = tokens.count(&provider::render(recipe.template, &pre))?;
        let room = recipe.card_tokens.saturating_sub(overhead).max(1);
        // Offsets suggest boundaries only. Re-tokenize each complete candidate;
        // merges, normalizers and special tokens need not be prefix-monotonic.
        let offsets: Vec<usize> = encoded
            .offsets
            .into_iter()
            .filter_map(|(_, at)| {
                (at > base && at <= base + end - start).then_some(start + at.saturating_sub(base))
            })
            .collect();
        drop(encoded.ids);
        let monotonic = offsets.windows(2).all(|w| w[0] <= w[1]);
        while span.start < end {
            let next = if monotonic {
                let index = offsets.partition_point(|at| *at <= span.start);
                offsets
                    .get(index.saturating_add(room - 1))
                    .copied()
                    .unwrap_or(end)
            } else {
                span.start.saturating_add(room).min(end)
            };
            span.end = boundary(source, next.min(end));
            if span.end <= span.start {
                span.end = span.start
                    + source[span.start..]
                        .chars()
                        .next()
                        .expect("nonempty tail")
                        .len_utf8();
            }
            if span.end < end
                && let Some(line) = source[span.start..span.end].rfind('\n')
                && line + 1 >= (span.end - span.start) / 2
            {
                span.end = span.start + line + 1;
            }
            let card = loop {
                let (rendered, encoded) = encode(source, &span, recipe, tokens)?;
                if !encoded.ids.is_empty() && encoded.ids.len() <= recipe.card_tokens {
                    break Card {
                        start: span.start,
                        end: span.end,
                        input_key: provider::input_key(recipe.function_digest, &rendered),
                        ids: encoded.ids,
                    };
                }
                let cut = boundary(source, span.start + (span.end - span.start) / 2);
                if cut <= span.start {
                    return Err(ProviderError::InputTooLarge(
                        "body template/context leaves no room for a source character".into(),
                    ));
                }
                span.end = cut;
            };
            emitted += 1;
            if emitted > MAX_CHUNKS {
                return Err(ProviderError::InputTooLarge(
                    "body source exceeds 16384 windows".into(),
                ));
            }
            emit(span.clone(), card);
            span.start = span.end;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Bytes;
    impl TokenCount for Bytes {
        fn encode(&self, text: &str) -> Result<TokenizedText, ProviderError> {
            Ok(TokenizedText {
                ids: text.bytes().map(u32::from).collect(),
                offsets: (0..text.len()).map(|i| (i, i + 1)).collect(),
            })
        }
    }
    fn recipe(limit: usize) -> CardRecipe<'static> {
        CardRecipe {
            template: "doc: {text}",
            function_digest: "fixture",
            card_tokens: limit,
        }
    }
    fn collect(source: &str, lang: Option<Lang>, limit: usize, parent: usize) -> Vec<(Span, Card)> {
        let mut out = Vec::new();
        visit(source, lang, &recipe(limit), parent, &Bytes, |s, c| {
            out.push((s, c))
        })
        .unwrap();
        out
    }
    fn text(card: &Card) -> String {
        String::from_utf8(card.ids.iter().map(|&i| i as u8).collect()).unwrap()
    }

    #[test]
    fn body_edits_change_keys_while_complete_units_and_parent_context_stay_exact() {
        let before = "impl A {\n    fn opaque() {\n        retry_with_backoff();\n    }\n}\n";
        let after = before.replace("retry_with_backoff", "refund_the_charge");
        let cards = collect(before, Some(Lang::Rust), 512, 64);
        let changed = collect(&after, Some(Lang::Rust), 512, 64);
        let target = cards
            .iter()
            .find(|(_, c)| text(c).contains("retry_with_backoff"))
            .unwrap();
        assert_eq!(
            &before[target.0.start..target.0.end],
            "fn opaque() {\n        retry_with_backoff();\n    }"
        );
        assert!(text(&target.1).contains("impl A"));
        assert_ne!(target.1.input_key, changed[1].1.input_key);
        let parent = collect(
            &before.replace("impl A", "impl B"),
            Some(Lang::Rust),
            512,
            64,
        );
        assert_ne!(target.1.input_key, parent[1].1.input_key);
        let addr = |source| {
            super::super::cards(source, "a.rs", Some(Lang::Rust), &recipe(512), &Bytes).unwrap()
        };
        assert_eq!(
            addr(before)[0].input_key,
            addr(&after)[0].input_key,
            "legacy address omits body-only edits"
        );
    }

    #[test]
    fn oversized_unicode_crlf_and_unparsed_sources_tile_without_duplicate_or_lost_evidence() {
        for lang in [Some(Lang::Rust), None] {
            let source = format!("fn opaque() {{\r\n{}\r\n}}", "let é = \"💡\"; ".repeat(60));
            let chunks = collect(&source, lang, 128, 16);
            assert!(chunks.len() > 1);
            let mut seen = vec![false; source.len()];
            for (span, card) in &chunks {
                assert!(card.ids.len() <= 128);
                assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
                assert!(text(card).ends_with(&source[span.start..span.end]));
                assert_eq!(
                    *card,
                    super::card(&source, span, &recipe(128), &Bytes).unwrap()
                );
                for byte in &mut seen[span.start..span.end] {
                    assert!(!*byte);
                    *byte = true;
                }
            }
            for (i, b) in source.bytes().enumerate() {
                assert!(seen[i] || b.is_ascii_whitespace());
            }
        }
        let config = "[retry]\nbackoff_ms = 400\n";
        let chunks = collect(config, None, 128, 16);
        assert_eq!(chunks.len(), 1);
        assert!(text(&chunks[0].1).contains(config));
    }

    #[test]
    fn pathological_template_refuses_instead_of_dropping_source_and_blank_sources_have_no_inputs() {
        assert!(collect(" \r\n\t", None, 128, 16).is_empty());
        let mut calls = 0;
        assert!(
            visit(
                "important bytes",
                None,
                &CardRecipe {
                    template: "an impossibly long template {text}",
                    ..recipe(16)
                },
                0,
                &Bytes,
                |_, _| calls += 1
            )
            .is_err()
        );
        assert_eq!(calls, 0);
    }
}
