//! Body-v1: non-overlapping evidence, complete short units, bounded long windows.
//! Context is a source-local parent/header prefix, never an extra ANN candidate.
//! Paths and byte offsets belong to provenance, not the rendered vector input.
use super::*;
use std::sync::Arc;

const MAX_CHUNKS: usize = 16_384;

#[derive(Clone, Debug)]
pub struct Span {
    kind: crate::neural::cache::CardKind,
    pub start: usize,
    pub end: usize,
    context: Arc<str>,
}

impl Span {
    /// Conservatively count shared context per reference when retaining a plan.
    #[cfg(feature = "semantic")]
    pub(super) fn retained_heap_bytes(&self) -> usize {
        let alignment = std::mem::align_of::<usize>();
        self.context
            .len()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(alignment - 1))
            .map_or(usize::MAX, |bytes| bytes & !(alignment - 1))
    }
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
        kind: span.kind,
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

/// Append `source[start..end]` and a newline, bounded so `text` stays within
/// `cap` bytes before tokenization.
fn append(text: &mut String, source: &str, start: usize, end: usize, cap: usize) {
    let end = boundary(
        source,
        end.min(start.saturating_add(cap.saturating_sub(text.len()))),
    );
    if start < end {
        text.push_str(&source[start..end]);
        text.push('\n');
    }
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
    let header_end = |unit: &Unit| unit.body.map_or(unit.head, |(body, _)| body.min(unit.end));
    let own = owner.map(|index| &units[index]);
    let mut text = String::new();
    match own.and_then(|unit| unit.parent) {
        Some(parent) => {
            let parent = &units[parent];
            append(&mut text, source, parent.start, header_end(parent), cap);
        }
        None => append(&mut text, source, 0, preamble, cap),
    }
    if let Some(unit) = own {
        append(&mut text, source, unit.start, header_end(unit), cap);
    }
    if text.is_empty() || tokens.count(&text)? <= limit {
        return Ok(Arc::from(text));
    }
    // Over budget: the region's own declaration outranks the enclosing header
    // or file preamble, and its signature outranks long leading documentation.
    // Truncating the combined text from its end would keep only outer context.
    if let Some(unit) = own {
        let signature = (unit.head > unit.start).then_some(unit.head);
        for from in std::iter::once(unit.start).chain(signature) {
            let mut header = String::new();
            append(&mut header, source, from, header_end(unit), cap);
            if header.is_empty() {
                continue;
            }
            text = header;
            if tokens.count(&text)? <= limit {
                return Ok(Arc::from(text));
            }
        }
    }
    while !text.is_empty() && tokens.count(&text)? > limit {
        text.truncate(boundary(&text, text.len() / 2));
    }
    Ok(Arc::from(text))
}

/// Only parsed nonleaf containers can have structural residuals. The syntax
/// body boundary proves which delimiter belongs to this container; the residual
/// must otherwise be whitespace. Fields, initializers, comments, leaf declarations
/// and prototypes retain Body, as do unknown-language/uncertain residuals. Leading
/// documentation is retained as Body rather than being discarded as a header.
fn region_kind(
    source: &str,
    units: &[Unit],
    owner: Option<usize>,
    start: usize,
    end: usize,
) -> crate::neural::cache::CardKind {
    use crate::neural::cache::CardKind;
    let Some(unit) = owner.map(|index| &units[index]) else {
        return CardKind::Body;
    };
    if unit.children.is_empty()
        || matches!(
            unit.kind,
            UnitKind::Fn | UnitKind::Method | UnitKind::Macro | UnitKind::Section | UnitKind::Block
        )
        || (start == unit.start && end == unit.end)
    {
        return CardKind::Body;
    }
    let Some((body_start, body_end)) = unit.body else {
        return CardKind::Body;
    };
    let opening = start <= unit.head
        && source[start..unit.head].trim().is_empty()
        && ((end <= body_start)
            || (start <= body_start && body_start < end && source[body_start..end].trim() == "{"));
    let closing = body_end > body_start
        && source.as_bytes().get(body_end - 1) == Some(&b'}')
        && start < body_end
        && body_end <= end
        && source[start..body_end - 1].trim().is_empty()
        && source[body_end..end].trim().is_empty();
    if opening || closing {
        CardKind::ContainerFragment
    } else {
        CardKind::Body
    }
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
    let (units, clean_tree) = lang
        .map(|lang| syntax::units_with_certainty(source, lang))
        .unwrap_or_default();
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
            kind: if clean_tree {
                region_kind(source, &units, owner, start, end)
            } else {
                crate::neural::cache::CardKind::Body
            },
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
            let kind = span.kind;
            emit(
                span,
                Card {
                    kind,
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
                        kind: span.kind,
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
    fn over_budget_context_keeps_the_regions_own_declaration() {
        let context_of = |source: &str, needle: &str| -> String {
            let at = source.find(needle).unwrap();
            let cards = collect(source, Some(Lang::Rust), 512, 96);
            let (span, _) = cards
                .iter()
                .find(|(span, _)| span.start <= at && at < span.end)
                .unwrap();
            span.context.to_string()
        };
        let unit = "/// Reconcile accounts.\nfn reconcile(ledger: &Ledger) {\n    settle_the_ledger();\n}\n";
        // Within budget, the preamble and the unit's header both remain.
        let fits = context_of(&format!("use std::fmt;\n{unit}"), "settle_the_ledger");
        assert!(
            fits.contains("use std::fmt;") && fits.contains("fn reconcile(ledger: &Ledger)"),
            "{fits}"
        );
        // A long module preamble must not displace the unit's own declaration.
        let narrative = "Module narrative. ".repeat(20);
        let long_preamble = format!("//! {narrative}\nuse std::fmt;\n{unit}");
        let kept = context_of(&long_preamble, "settle_the_ledger");
        assert!(
            kept.contains("/// Reconcile accounts.\nfn reconcile(ledger: &Ledger)"),
            "{kept}"
        );
        assert!(!kept.contains("Module narrative"), "{kept}");
        // So must a long enclosing header, for a nested method.
        let nested = format!(
            "/// {narrative}\nimpl Ledger {{\n    fn settle(&self) {{\n        credit();\n    }}\n}}\n"
        );
        let kept = context_of(&nested, "credit");
        assert!(kept.contains("fn settle(&self)"), "{kept}");
        // When its own documentation is too long, the signature is kept.
        let documented = format!(
            "/// {narrative}\nfn reconcile(ledger: &Ledger) {{\n    settle_the_ledger();\n}}\n"
        );
        let kept = context_of(&documented, "settle_the_ledger");
        assert!(kept.starts_with("fn reconcile(ledger: &Ledger)"), "{kept}");
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
    fn container_fragments_keep_coverage_keys_and_useful_residuals() {
        use crate::neural::cache::CardKind;
        let source = "impl Billing {\n    fn apply() { settle(); }\n    // Keep the operational rationale.\n    const LIMIT: usize = 4;\n}\ntrait Service {\n    fn declaration(&self);\n}\nstruct Empty;\n";
        let cards = collect(source, Some(Lang::Rust), 512, 64);
        let fragments: Vec<_> = cards
            .iter()
            .filter(|(_, card)| card.kind == CardKind::ContainerFragment)
            .collect();
        assert!(
            fragments
                .iter()
                .any(|(span, _)| source[span.start..span.end].trim() == "impl Billing {")
        );
        assert!(
            fragments
                .iter()
                .any(|(span, _)| source[span.start..span.end].trim() == "}")
        );
        for needle in [
            "fn apply()",
            "const LIMIT",
            "fn declaration",
            "struct Empty",
        ] {
            assert!(
                cards.iter().any(|(span, card)| card.kind == CardKind::Body
                    && source[span.start..span.end].contains(needle)),
                "{needle}"
            );
        }
        let mut covered = vec![false; source.len()];
        for (span, card) in &cards {
            let mut former = span.clone();
            former.kind = CardKind::Body;
            let before = super::card(source, &former, &recipe(512), &Bytes).unwrap();
            assert_eq!(card.input_key, before.input_key);
            assert_eq!(
                card.ids, before.ids,
                "classification changes no model input"
            );
            assert_eq!(
                *card,
                super::card(source, span, &recipe(512), &Bytes).unwrap()
            );
            for seen in &mut covered[span.start..span.end] {
                assert!(!*seen);
                *seen = true;
            }
        }
        for (index, byte) in source.bytes().enumerate() {
            assert!(covered[index] || byte.is_ascii_whitespace());
        }
        // A class residual containing a real assignment must not become a shell.
        let python =
            "class Settings:\n    retries = 4\n    def apply(self):\n        return self.retries\n";
        assert!(
            collect(python, Some(Lang::Python), 512, 64)
                .iter()
                .any(|(span, card)| card.kind == CardKind::Body
                    && python[span.start..span.end].contains("retries = 4"))
        );
        assert!(
            collect(source, None, 128, 16)
                .iter()
                .all(|(_, card)| card.kind == CardKind::Body)
        );
    }

    #[test]
    fn recovered_syntax_keeps_all_residuals_as_body() {
        use crate::neural::cache::CardKind;
        // A valid nonleaf container still exists before the malformed item.
        // A locally plausible shell must not acquire a stronger role from an
        // error-recovered tree elsewhere in the same source.
        let source = "impl Billing {\n    fn apply() { settle(); }\n}\nfn broken( {\n";
        let (units, clean_tree) = syntax::units_with_certainty(source, Lang::Rust);
        assert!(!clean_tree);
        assert!(units.iter().any(|unit| !unit.children.is_empty()));
        let cards = collect(source, Some(Lang::Rust), 512, 64);
        assert!(!cards.is_empty());
        assert!(cards.iter().all(|(_, card)| card.kind == CardKind::Body));
        let mut covered = vec![false; source.len()];
        for (span, _) in cards {
            for seen in &mut covered[span.start..span.end] {
                assert!(!*seen);
                *seen = true;
            }
        }
        for (index, byte) in source.bytes().enumerate() {
            assert!(covered[index] || byte.is_ascii_whitespace());
        }
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
