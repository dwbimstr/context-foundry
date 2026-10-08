//! 009 T004 card partition, recipe `cards-v1` (spec 009 § T004 "Cards, not
//! bodies").
//!
//! One card per definition (001 T007's definitions: a named
//! programming-language unit, a Rust `impl` excluded) and per Markdown
//! section. A card is, line by line:
//!
//! 1. its address, `<path> <kind>[ <qualified name>]`;
//! 2. its signature (a unit with a body: from its head to the end of the
//!    line where the body opens, or to the start of that line when the body
//!    begins a line of its own) or its head lines (a section, or a unit
//!    without a body: from its head to its end);
//! 3. its leading documentation (the leading run of comments and attributes
//!    before its head).
//!
//! Blank lines are dropped and trailing whitespace is trimmed. The card is
//! the longest prefix of those lines whose input rendered with the
//! profile's document template fits the card limit (template and special
//! tokens included); an address that alone does not fit is cut at a UTF-8
//! boundary by halving. Nothing is truncated silently and no source byte is
//! rewritten: every kept line is verbatim.
//!
//! A card's cache key is its exact rendered input; its provenance is the
//! unit range recorded in the partition row with the source hash, so an
//! edit re-embeds only the cards whose text changed. The recipe pins the
//! 001 T005/T007 grammar versions this crate builds, the tokenizer identity
//! and the card limit; changing any of them is a recipe change, never a
//! tuning step.
//!
//! This module never loads a tokenizer: token counting goes through the
//! [`TokenCount`] seam so it stays pure and testable.
use crate::neural::provider::{self, ProviderError};
use crate::syntax::{self, Lang, Unit, UnitKind};

/// The pinned recipe grammar version.
pub const RECIPE_GRAMMAR: &str = "cards-v1";

/// The partition recipe id for one tokenizer identity and card limit. It
/// participates in mapping eligibility: a partition row under another
/// recipe is stale. It names the search index version too, which changes
/// whenever units can (a grammar, language or unit-kind change, 001's index
/// version gate): a source partitioned before such a change, even into no
/// cards, is partitioned again, while unchanged card inputs keep their
/// cached vectors.
pub fn recipe_id(tokenizer_identity: &str, card_tokens: u32) -> String {
    let units = crate::store::SEARCH_SCHEMA;
    format!("{RECIPE_GRAMMAR}+units{units}:{card_tokens}+{tokenizer_identity}")
}

/// One rendered card: the unit's half-open byte range, its exact
/// document-input key and the exact model-input token ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
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

/// What a card's rendering depends on besides the source: the document
/// template, the function digest its key is computed under and the limit.
#[derive(Clone, Copy, Debug)]
pub struct CardRecipe<'a> {
    pub template: &'a str,
    pub function_digest: &'a str,
    pub card_tokens: usize,
}

/// True when `unit` gets a card: a definition or a Markdown section.
fn carded(unit: &Unit) -> bool {
    unit.name_range.is_some() || unit.kind == UnitKind::Section
}

/// The units of `source` that get a card, in the unit forest's pre-order
/// (start ascending, the enclosing unit first): card `i` of the source is
/// rendered from unit `i`. `lang` is 001's mapping of the path; a language
/// without units, or a source over the parse bound, has none.
pub fn carded_units(source: &str, lang: Option<Lang>) -> Vec<Unit> {
    match lang.filter(|lang| lang.has_units()) {
        Some(mapped) => syntax::units(source, mapped)
            .into_iter()
            .filter(carded)
            .collect(),
        None => Vec::new(),
    }
}

/// The cards of `source` at workspace-relative `path`, in the unit forest's
/// pre-order ([`carded_units`], each rendered by [`card`]).
pub fn cards(
    source: &str,
    path: &str,
    lang: Option<Lang>,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Vec<Card>, ProviderError> {
    carded_units(source, lang)
        .iter()
        .map(|unit| card(source, path, unit, recipe, tokens))
        .collect()
}

/// The card of one carded `unit` of `source`: its range, the key of its
/// exact rendered input and that input's token ids.
pub fn card(
    source: &str,
    path: &str,
    unit: &Unit,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<Card, ProviderError> {
    let (rendered, ids) = render_card(source, path, unit, recipe, tokens)?;
    Ok(Card {
        start: unit.start,
        end: unit.end,
        input_key: provider::input_key(recipe.function_digest, &rendered),
        ids,
    })
}

/// The end of a signature whose body opens at `body`: the end of that line
/// when code precedes the body on it, else the start of that line.
fn signature_end(source: &str, head: usize, body: usize) -> usize {
    let line_start = source[..body].rfind('\n').map_or(0, |at| at + 1).max(head);
    if source[line_start..body].trim().is_empty() {
        return line_start;
    }
    source[body..]
        .find('\n')
        .map_or(source.len(), |at| body + at + 1)
}

/// The nonblank lines of `text`, trailing whitespace trimmed.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
}

/// The address line: `<path> <kind>[ <qualified name>]`, control characters
/// replaced by spaces.
fn address(path: &str, unit: &Unit) -> String {
    let mut line = format!("{path} {}", unit.kind.as_str());
    if let Some(qname) = &unit.qname {
        line.push(' ');
        line.push_str(qname);
    }
    line.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// One unit's rendered card input and its token ids (the module rule).
fn render_card(
    source: &str,
    path: &str,
    unit: &Unit,
    recipe: &CardRecipe<'_>,
    tokens: &dyn TokenCount,
) -> Result<(String, Vec<u32>), ProviderError> {
    let limit = recipe.card_tokens;
    let encode = |text: &str| -> Result<(String, Vec<u32>), ProviderError> {
        let rendered = provider::render(recipe.template, text);
        let ids = tokens.encode(&rendered)?.ids;
        Ok((rendered, ids))
    };
    // 1. The address, cut by halving at a UTF-8 boundary while it alone does
    //    not fit.
    let mut text = address(path, unit);
    let mut best = encode(&text)?;
    while best.1.len() > limit {
        let mut cut = text.len() / 2;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        if cut == 0 {
            return Err(ProviderError::InputTooLarge(format!(
                "{path}: no card fits the {limit}-token card limit with this template"
            )));
        }
        text.truncate(cut);
        best = encode(&text)?;
    }
    // 2–3. Signature or head lines, then the leading documentation, each
    //      line kept while the card still fits. At most `limit` lines are
    //      tried: every nonblank line costs at least one token.
    let shown = match unit.body {
        Some((body, _)) if unit.kind != UnitKind::Section => {
            &source[unit.head..signature_end(source, unit.head, body)]
        }
        _ => &source[unit.head..unit.end],
    };
    let documentation = &source[unit.start..unit.head];
    for line in lines(shown).chain(lines(documentation)).take(limit) {
        let candidate = format!("{text}\n{line}");
        let encoded = encode(&candidate)?;
        if encoded.1.len() > limit {
            break;
        }
        text = candidate;
        best = encoded;
    }
    if best.1.is_empty() {
        return Err(ProviderError::InputTooLarge(format!(
            "{path}: a card tokenized to zero ids"
        )));
    }
    Ok(best)
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
    const TEMPLATE: &str = "doc: {text}";

    fn recipe(card_tokens: usize) -> CardRecipe<'static> {
        CardRecipe {
            template: TEMPLATE,
            function_digest: DIGEST,
            card_tokens,
        }
    }

    /// Each card's text (the rendered input minus the template) by unit.
    fn texts(path: &str, source: &str, card_tokens: usize) -> Vec<String> {
        let cards = cards(
            source,
            path,
            Lang::from_path(path),
            &recipe(card_tokens),
            &Bytes,
        )
        .unwrap();
        cards
            .iter()
            .map(|card| {
                assert!(card.ids.len() <= card_tokens, "over the limit");
                let bytes: Vec<u8> = card.ids.iter().map(|&b| b as u8).collect();
                let rendered = String::from_utf8(bytes).expect("byte ids of UTF-8 text");
                assert_eq!(card.input_key, provider::input_key(DIGEST, &rendered));
                rendered.strip_prefix("doc: ").unwrap().to_owned()
            })
            .collect()
    }

    /// One fixture per language with units: the card of each definition
    /// (and each Markdown section) is its address, its signature or head
    /// lines, then its leading documentation as `syntax::units` attaches it
    /// (context-v2 § Unit forest: Rust outer doc comments and attributes,
    /// `/**` blocks in Java, JavaScript and TypeScript, Go comments; a
    /// Python, C or C++ comment, or a JavaScript `//` line, is not part of
    /// the unit, so those cards have none).
    #[test]
    fn every_language_renders_address_signature_and_documentation() {
        let cases: [(&str, &str, &[&str]); 9] = [
            (
                "src/lib.rs",
                "/// Adds one.\n#[inline]\npub fn add_one(x: u32) -> u32 {\n    x + 1\n}\n",
                &[
                    "src/lib.rs fn add_one\npub fn add_one(x: u32) -> u32 {\n/// Adds one.\n#[inline]",
                ],
            ),
            (
                "pkg/mod.py",
                "# Adds one.\ndef add_one(x):\n    return x + 1\n",
                &["pkg/mod.py fn add_one\ndef add_one(x):"],
            ),
            (
                "web/a.ts",
                "/** Adds one. */\nexport function addOne(x: number): number {\n  return x + 1;\n}\n",
                &[
                    "web/a.ts fn addOne\nexport function addOne(x: number): number {\n/** Adds one. */",
                ],
            ),
            (
                "web/b.js",
                "// Adds one.\nfunction addOne(x) {\n  return x + 1;\n}\n",
                &["web/b.js fn addOne\nfunction addOne(x) {"],
            ),
            (
                "cmd/main.go",
                "// AddOne adds one.\nfunc AddOne(x int) int {\n\treturn x + 1\n}\n",
                &["cmd/main.go fn AddOne\nfunc AddOne(x int) int {\n// AddOne adds one."],
            ),
            (
                "c/add.c",
                "/* Adds one. */\nint add_one(int x) {\n    return x + 1;\n}\n",
                &["c/add.c fn add_one\nint add_one(int x) {"],
            ),
            (
                "c/add.cpp",
                "// Adds one.\nint add_one(int x) {\n    return x + 1;\n}\n",
                &["c/add.cpp fn add_one\nint add_one(int x) {"],
            ),
            (
                "j/Add.java",
                "/** Adds. */\nclass Add {\n    int one(int x) { return x + 1; }\n}\n",
                &[
                    "j/Add.java class Add\nclass Add {\n/** Adds. */",
                    "j/Add.java method Add.one\nint one(int x) { return x + 1; }",
                ],
            ),
            (
                "docs/guide.md",
                "# Guide\n\nIntro text.\n\n## Install\n\nRun the script.\n",
                &[
                    "docs/guide.md section Guide\n# Guide\nIntro text.\n## Install\nRun the script.",
                    "docs/guide.md section Guide.Install\n## Install\nRun the script.",
                ],
            ),
        ];
        let wrong: Vec<String> = cases
            .iter()
            .filter_map(|(path, source, want)| {
                let got = texts(path, source, 2048);
                (got != *want).then(|| format!("{path}: {got:?}"))
            })
            .collect();
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn a_rust_impl_is_a_container_and_its_methods_carry_cards() {
        let source = "struct S;\nimpl S {\n    /// Reads.\n    fn read(&self) {}\n}\n";
        assert_eq!(
            texts("s.rs", source, 2048),
            [
                "s.rs struct S\nstruct S;",
                "s.rs fn S::read\nfn read(&self) {}\n/// Reads."
            ]
        );
    }

    #[test]
    fn the_card_is_the_longest_fitting_line_prefix_and_never_exceeds_the_limit() {
        let source = "/// One.\n/// Two.\npub fn f(a: u32) -> u32 {\n    a\n}\n";
        let full = "s.rs fn f\npub fn f(a: u32) -> u32 {\n/// One.\n/// Two.";
        assert_eq!(texts("s.rs", source, 2048), [full]);
        // `doc: ` is 5 bytes: a limit of exactly the full card keeps it all;
        // one token less drops the last documentation line, never a byte of it.
        let exact = 5 + full.len();
        assert_eq!(texts("s.rs", source, exact), [full]);
        assert_eq!(
            texts("s.rs", source, exact - 1),
            ["s.rs fn f\npub fn f(a: u32) -> u32 {\n/// One."]
        );
        // The address alone, then an address cut at a UTF-8 boundary.
        assert_eq!(texts("s.rs", source, 5 + 9), ["s.rs fn f"]);
        let cut = texts("é/s.rs", source, 5 + 3);
        assert_eq!(cut, ["é/"]);
    }

    #[test]
    fn languages_without_units_and_unmapped_files_have_no_cards() {
        for path in ["a.toml", "a.json", "notes.txt", "Makefile"] {
            assert!(texts(path, "x = 1\n", 2048).is_empty(), "{path}");
        }
        assert!(texts("e.rs", "", 2048).is_empty());
    }

    #[test]
    fn a_template_change_re_keys_every_card_and_an_edit_only_its_own() {
        // A signature runs to the end of the line its body opens on, so a
        // one-line body is card text; these bodies span their own lines.
        let source = "fn a() {\n}\nfn b() {\n}\n";
        let keyed = |template: &str, source: &str| {
            let recipe = CardRecipe {
                template,
                function_digest: DIGEST,
                card_tokens: 2048,
            };
            cards(source, "k.rs", Some(Lang::Rust), &recipe, &Bytes)
                .unwrap()
                .into_iter()
                .map(|card| card.input_key)
                .collect::<Vec<_>>()
        };
        let base = keyed("doc: {text}", source);
        let other = keyed("title: none | text: {text}", source);
        assert_eq!(base.len(), 2);
        assert!(base.iter().zip(&other).all(|(a, b)| a != b));
        // Editing b's body changes no card text: no key changes. Editing a's
        // signature changes a's key alone.
        assert_eq!(
            keyed("doc: {text}", "fn a() {\n}\nfn b() {\n    1;\n}\n"),
            base
        );
        let edited = keyed("doc: {text}", "fn a(x: u8) {\n}\nfn b() {\n}\n");
        assert_ne!(edited[0], base[0]);
        assert_eq!(edited[1], base[1]);
    }

    #[test]
    fn a_body_that_opens_on_its_own_line_is_not_part_of_the_signature() {
        let source = "def f(\n    a,\n):\n    return a\n";
        assert_eq!(
            texts("m.py", source, 2048),
            ["m.py fn f\ndef f(\n    a,\n):"]
        );
    }

    /// 001 T008 gave shell (and 14 more languages) units. A shell source
    /// partitioned before it holds a completed zero-card partition under
    /// the recipe of that time; it is not current under today's, so
    /// preparation partitions it again and its function gets a card.
    #[test]
    fn a_zero_card_partition_made_before_a_unit_change_is_not_current() {
        use crate::neural::cache::{PartitionRecord, partition_is_current};
        let source = "outer() {\n  echo hello\n}\n";
        assert_eq!(texts("tools.sh", source, 2048).len(), 1);
        let meta = crate::store::SourceMeta {
            hash: "h".into(),
            chunks: 1,
            bytes: source.len(),
            lines: 3,
        };
        let record = |recipe: String| PartitionRecord {
            source_hash: "h".into(),
            recipe_id: recipe,
            function_digest: DIGEST.into(),
            units: Vec::new(),
        };
        let current = recipe_id("tok", 128);
        let before_t008 = record(format!("{RECIPE_GRAMMAR}:128+tok"));
        assert!(!partition_is_current(&before_t008, &meta, &current, DIGEST));
        let under_index_4 = record(format!("{RECIPE_GRAMMAR}+units4:128+tok"));
        assert!(!partition_is_current(
            &under_index_4,
            &meta,
            &current,
            DIGEST
        ));
        // The same rows under today's recipe are current: the check is the
        // recipe, not the empty card list.
        assert!(partition_is_current(
            &record(current.clone()),
            &meta,
            &current,
            DIGEST
        ));
    }
}
