//! The bounded parse (context-v2 § Languages): the work budget and its test
//! hooks, the tree-sitter parse and its reference and boundary entry points,
//! and the scanner limits checked before a grammar's external scanner runs.
//! The languages and unit extraction stay in the parent module.
use super::*;

/// The work budget of one parse: [`PARSE_WORK_BUDGET`], or a test's own.
fn work_budget() -> u64 {
    #[cfg(feature = "test-faults")]
    if let Some(budget) = parse_hooks::budget() {
        return budget;
    }
    PARSE_WORK_BUDGET
}

/// Test hooks for the parse (built with the `test-faults` feature only).
#[cfg(feature = "test-faults")]
pub mod parse_hooks {
    use std::cell::Cell;

    thread_local! {
        static BUDGET: Cell<Option<u64>> = const { Cell::new(None) };
    }

    /// Sets this thread's parse work budget; `None` restores
    /// [`super::PARSE_WORK_BUDGET`].
    pub fn set_budget(budget: Option<u64>) {
        BUDGET.with(|cell| cell.set(budget));
    }

    pub(super) fn budget() -> Option<u64> {
        BUDGET.with(Cell::get)
    }

    /// Each scanner limit that applies to `lang`, with `source`'s measure
    /// against it (the corpus calibration of 001 T008 review M1).
    pub fn scanner_measures(source: &str, lang: super::Lang) -> Vec<(super::ScannerLimit, usize)> {
        super::ScannerLimit::ALL
            .iter()
            .filter(|limit| limit.applies(lang))
            .map(|&limit| (limit, limit.measure(source.as_bytes()).value))
            .collect()
    }
}

/// One parse under the work budget ([`PARSE_WORK_BUDGET`]): the source is
/// handed to the parser one UTF-8 character per read, and each read and
/// each progress check spends one unit. Once the budget is spent, the
/// parser reads a line break at every offset up to the one it stopped at and
/// end of input after it, and the next progress check stops the parse; its
/// tree, if any, is dropped. A line break rather than end of input, so that
/// a scanner loop only a line break ends (Kotlin's after `@`) cannot run
/// forever at a forced end of input.
/// `Ok(None)`: the grammar did not load or the parser returned no tree.
pub(crate) fn parse_boundary_python(
    source: &str,
) -> Result<Option<tree_sitter::Tree>, ParseStopped> {
    if let Some(stopped) = over_scanner_limit(Lang::Python, source.as_bytes()) {
        return Err(stopped);
    }
    parse(source, &tree_sitter_python::LANGUAGE.into())
}

/// Bounded syntax evidence for occurrences/call candidates, not name resolution.
pub(crate) fn parse_reference_source(path: &str, source: &str) -> Option<tree_sitter::Tree> {
    if source.len() > MAX_PARSE_BYTES {
        return None;
    }
    let lang = Lang::from_path(path)?;
    if over_scanner_limit(lang, source.as_bytes()).is_some() {
        return None;
    }
    let grammar = match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => return None,
    };
    let tree = parse(source, &grammar).ok()??;
    (!tree.root_node().has_error()).then_some(tree)
}

pub(super) fn parse(
    source: &str,
    grammar: &tree_sitter::Language,
) -> Result<Option<tree_sitter::Tree>, ParseStopped> {
    use std::cell::Cell;
    use std::ops::ControlFlow;
    let mut parser = tree_sitter::Parser::new();
    // Every grammar is a locked dependency whose ABI the tests load; a
    // failure here would be a build defect, surfaced by those tests.
    if parser.set_language(grammar).is_err() {
        return Ok(None);
    }
    let bytes = source.as_bytes();
    let budget = work_budget();
    let used = Cell::new(0u64);
    let stopped: Cell<Option<usize>> = Cell::new(None);
    // Spends one unit at byte `at`; false once the budget is spent, the
    // first refusal recording where.
    let spend = |at: usize| {
        if stopped.get().is_some() {
            return false;
        }
        if used.get() >= budget {
            stopped.set(Some(at));
            return false;
        }
        used.set(used.get() + 1);
        true
    };
    let mut read = |at: usize, _: tree_sitter::Point| -> &[u8] {
        if at >= bytes.len() {
            return &[];
        }
        if !spend(at) {
            return match stopped.get() {
                Some(stop) if at <= stop => b"\n",
                _ => &[],
            };
        }
        // One character: the lead byte and its continuation bytes.
        let mut end = at + 1;
        while end < bytes.len() && bytes[end] & 0xC0 == 0x80 {
            end += 1;
        }
        &bytes[at..end]
    };
    let mut progress = |state: &tree_sitter::ParseState| {
        if spend(state.current_byte_offset()) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    };
    let tree = parser.parse_with_options(
        &mut read,
        None,
        Some(tree_sitter::ParseOptions::new().progress_callback(&mut progress)),
    );
    match stopped.get() {
        Some(at) => Err(ParseStopped {
            at,
            stop: Stop::Budget(budget),
        }),
        None => Ok(tree),
    }
}

/// A limit a grammar's external scanner needs that the work budget cannot
/// enforce, because the scanner breaks inside one call, where no read or
/// progress check can stop it (context-v2 § Languages; the audit of every
/// grammar's scanner is 001 T008 review M1). A source over a limit is a
/// stopped parse ([`Stop::Scanner`]) before the parser runs: a linear pass
/// over its bytes that over-approximates what the scanner would do, so a
/// source under every limit cannot reach the breakage. Each limit lies above
/// the largest measure in the T008 corpus and well below the breakage (the
/// constants' docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScannerLimit {
    /// F#'s scanner reads a nested `(* … *)` comment by native recursion,
    /// one frame per level: an unbounded nesting overflows the thread's
    /// stack. Measure: the deepest `(*` nesting, counted as the scanner does
    /// inside a comment wherever it appears ([`FSHARP_COMMENT_DEPTH`]).
    FSharpCommentDepth,
    /// Perl's scanner reads bracket delimiters nested inside a quote-like
    /// body (`q{ { … } }`) by native recursion. Measure: the deepest nesting
    /// of each bracket pair over the whole source, escapes skipped as the
    /// scanner skips them ([`PERL_BRACKET_DEPTH`]).
    PerlBracketDepth,
    /// Perl's heredoc scanner copies a heredoc's identifier, and the first
    /// word of each heredoc line, into 1,000-byte stack buffers without a
    /// bound. Measure: the longest such word ([`PERL_HEREDOC_WORD`]).
    PerlHeredocWord,
    /// Python's scanner serializes two bytes per indentation level after
    /// its open string delimiters into tree-sitter's 1,024-byte state
    /// buffer and writes one byte past it at 383 levels or more. Measure:
    /// the distinct indentation widths the scanner can compute
    /// ([`PYTHON_INDENT_WIDTHS`]).
    PythonIndentWidths,
    /// Kotlin's scanner skips from an `@` to the next whitespace (to the
    /// next line break after a `(`) and never stops at end of input.
    /// Measure: 1 when an `@` has no line break after it (limit 0).
    KotlinTrailingAt,
}

/// [`ScannerLimit::FSharpCommentDepth`] and [`ScannerLimit::PerlBracketDepth`]:
/// either scanner's recursion overflows a 2 MiB stack between 32,768 and
/// 49,152 levels (about 50 bytes a frame, debug and release builds), so
/// 8,192 levels take about 400 KiB. The T008 corpus's deepest measures are
/// 0 (8 F# sources) and 1,855 (5,182 Perl sources; a binary blob after
/// `__DATA__`, which the measure counts like code).
const FSHARP_COMMENT_DEPTH: usize = 8192;
const PERL_BRACKET_DEPTH: usize = 8192;
/// [`ScannerLimit::PerlHeredocWord`]: about half the 1,000-byte buffers;
/// the corpus's longest such word is 135 bytes.
const PERL_HEREDOC_WORD: usize = 512;
/// [`ScannerLimit::PythonIndentWidths`]: 256 widths keep the serialized
/// state under 770 bytes whatever the open string delimiters; the corpus
/// (17,247 Python sources) has at most 56.
const PYTHON_INDENT_WIDTHS: usize = 256;

impl ScannerLimit {
    const ALL: [Self; 5] = [
        Self::FSharpCommentDepth,
        Self::PerlBracketDepth,
        Self::PerlHeredocWord,
        Self::PythonIndentWidths,
        Self::KotlinTrailingAt,
    ];

    fn applies(self, lang: Lang) -> bool {
        match self {
            Self::FSharpCommentDepth => matches!(lang, Lang::FSharp | Lang::FSharpSignature),
            Self::PerlBracketDepth | Self::PerlHeredocWord => lang == Lang::Perl,
            Self::PythonIndentWidths => lang == Lang::Python,
            Self::KotlinTrailingAt => lang == Lang::Kotlin,
        }
    }

    /// The largest measure a source may have.
    pub fn limit(self) -> usize {
        match self {
            Self::FSharpCommentDepth => FSHARP_COMMENT_DEPTH,
            Self::PerlBracketDepth => PERL_BRACKET_DEPTH,
            Self::PerlHeredocWord => PERL_HEREDOC_WORD,
            Self::PythonIndentWidths => PYTHON_INDENT_WIDTHS,
            Self::KotlinTrailingAt => 0,
        }
    }

    fn measure(self, source: &[u8]) -> Peak {
        let peak = Peak::new(self.limit());
        match self {
            Self::FSharpCommentDepth => fsharp_comment_depth(source, peak),
            Self::PerlBracketDepth => perl_bracket_depth(source, peak),
            Self::PerlHeredocWord => perl_heredoc_word(source, peak),
            Self::PythonIndentWidths => python_indent_widths(source, peak),
            Self::KotlinTrailingAt => kotlin_trailing_at(source, peak),
        }
    }
}

impl std::fmt::Display for ScannerLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let limit = self.limit();
        match self {
            Self::FSharpCommentDepth => write!(f, "F# comments nest deeper than {limit}"),
            Self::PerlBracketDepth => write!(f, "Perl brackets nest deeper than {limit}"),
            Self::PerlHeredocWord => write!(f, "a Perl heredoc word is longer than {limit} bytes"),
            Self::PythonIndentWidths => {
                write!(f, "Python indentation has more than {limit} widths")
            }
            Self::KotlinTrailingAt => write!(f, "a Kotlin `@` has no line break after it"),
        }
    }
}

/// The stop of a source over a scanner limit of its language, at the first
/// byte where its measure exceeds the limit.
pub(super) fn over_scanner_limit(lang: Lang, source: &[u8]) -> Option<ParseStopped> {
    ScannerLimit::ALL
        .into_iter()
        .filter(|limit| limit.applies(lang))
        .find_map(|limit| {
            limit.measure(source).over.map(|at| ParseStopped {
                at,
                stop: Stop::Scanner(limit),
            })
        })
}

/// A measure's largest value, and the first byte where it exceeds `limit`.
#[derive(Clone, Copy)]
struct Peak {
    limit: usize,
    value: usize,
    over: Option<usize>,
}

impl Peak {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            value: 0,
            over: None,
        }
    }

    fn raise(&mut self, value: usize, at: usize) {
        self.value = self.value.max(value);
        if value > self.limit && self.over.is_none() {
            self.over = Some(at);
        }
    }
}

/// The deepest F# comment nesting. Outside a comment a `(*` opens one
/// unless `)` follows it (`(*)` is an operator); inside one, every `(*`
/// opens a level (the scanner recurses on it, `(*)` included) and every
/// `*)` closes one. Strings are not skipped: a `(*` in a string can only
/// raise the measure.
fn fsharp_comment_depth(source: &[u8], mut peak: Peak) -> Peak {
    let mut depth = 0usize;
    let mut at = 0;
    while at + 1 < source.len() {
        match (source[at], source[at + 1]) {
            (b'(', b'*') if depth > 0 || source.get(at + 2) != Some(&b')') => {
                depth += 1;
                peak.raise(depth, at);
                at += 2;
            }
            (b'*', b')') if depth > 0 => {
                depth -= 1;
                at += 2;
            }
            _ => at += 1,
        }
    }
    peak
}

/// The deepest nesting of any one bracket pair (`()`, `[]`, `{}`, `<>`)
/// over the whole source, a close below zero ignored, and a backslash
/// skipping the byte after it, as the scanner skips an escaped character
/// inside a quote-like body. Inside such a body the scanner's recursion
/// depth is at most this nesting.
fn perl_bracket_depth(source: &[u8], mut peak: Peak) -> Peak {
    let mut depth = [0usize; 4];
    let mut at = 0;
    while at < source.len() {
        let byte = source[at];
        if byte == b'\\' {
            at += 2;
            continue;
        }
        if let Some(pair) = b"([{<".iter().position(|&open| open == byte) {
            depth[pair] += 1;
            peak.raise(depth[pair], at);
        } else if let Some(pair) = b")]}>".iter().position(|&close| close == byte) {
            depth[pair] = depth[pair].saturating_sub(1);
        }
        at += 1;
    }
    peak
}

/// The longest word Perl's heredoc scanner can copy, in bytes: an
/// identifier after `<<` (past `~`, `\`, whitespace and an opening quote);
/// and, after the first such identifier, each whitespace-delimited word
/// that starts a line (an indented heredoc skips the indentation), and the
/// rest of a word after its first `\` or `$` (a non-interpolating heredoc
/// reads on from there).
fn perl_heredoc_word(source: &[u8], mut peak: Peak) -> Peak {
    let identifier = |byte: &u8| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte >= 0x80;
    let mut first = None;
    let mut at = 0;
    while let Some(found) = source[at..].windows(2).position(|pair| pair == b"<<") {
        let opener = at + found;
        let mut word = opener + 2;
        if source.get(word) == Some(&b'~') {
            word += 1;
        }
        if source.get(word) == Some(&b'\\') {
            word += 1;
        }
        while source.get(word).is_some_and(u8::is_ascii_whitespace) {
            word += 1;
        }
        if matches!(source.get(word), Some(b'"' | b'\'' | b'`')) {
            word += 1;
        }
        let length = source[word..]
            .iter()
            .take_while(|byte| identifier(byte))
            .count();
        if length > 0 {
            first.get_or_insert(opener);
            peak.raise(length, word);
        }
        at = opener + 2;
    }
    let Some(first) = first else {
        return peak;
    };
    // Whitespace-delimited words after the first heredoc opener.
    let mut line_start = false;
    let mut at = first;
    while at < source.len() {
        if source[at].is_ascii_whitespace() {
            line_start |= source[at] == b'\n';
            at += 1;
            continue;
        }
        let end = at
            + source[at..]
                .iter()
                .take_while(|byte| !byte.is_ascii_whitespace())
                .count();
        if line_start {
            peak.raise(end - at, at);
        }
        if let Some(escape) = source[at..end]
            .iter()
            .position(|&byte| matches!(byte, b'\\' | b'$'))
        {
            peak.raise(end - at - escape - 1, at + escape);
        }
        line_start = false;
        at = end;
    }
    peak
}

/// How many distinct non-zero indentation widths Python's scanner can
/// compute, each where its first new width appears. After each line break
/// the scanner counts a space as 1 and a tab as 8, restarts at a line
/// break, carriage return or form feed, skips comment lines and continues
/// across a backslash line continuation; every width it pushes is one of
/// these (also computed as if restarted at each continuation, and before
/// each comment), and its indentation stack holds distinct widths.
fn python_indent_widths(source: &[u8], mut peak: Peak) -> Peak {
    let mut widths = std::collections::HashSet::new();
    let mut record = |width: u16, at: usize| {
        if width != 0 && widths.insert(width) {
            peak.raise(widths.len(), at);
        }
    };
    let mut at = 0;
    while let Some(found) = source[at..].iter().position(|&byte| byte == b'\n') {
        let mut next = at + found;
        // `width` as the scanner counts from this line break; `fresh` as if
        // it had started at the latest one, continuations included.
        let (mut width, mut fresh) = (0u16, 0u16);
        loop {
            match source.get(next) {
                Some(b'\n' | b'\r' | b'\x0c') => {
                    width = 0;
                    fresh = 0;
                    next += 1;
                }
                Some(b' ') => {
                    width = width.wrapping_add(1);
                    fresh = fresh.wrapping_add(1);
                    next += 1;
                }
                Some(b'\t') => {
                    width = width.wrapping_add(8);
                    fresh = fresh.wrapping_add(8);
                    next += 1;
                }
                Some(b'#') => {
                    record(width, next);
                    record(fresh, next);
                    next += source[next..]
                        .iter()
                        .position(|&byte| byte == b'\n')
                        .unwrap_or(source.len() - next);
                }
                Some(b'\\') => {
                    let mut after = next + 1;
                    if source.get(after) == Some(&b'\r') {
                        after += 1;
                    }
                    match source.get(after) {
                        Some(b'\n') => {
                            fresh = 0;
                            next = after + 1;
                        }
                        None => next = after,
                        Some(_) => break,
                    }
                }
                _ => break,
            }
        }
        record(width, next);
        record(fresh, next);
        at = next.max(at + found + 1);
    }
    peak
}

/// 1, at the first `@` with no line break after it; else 0.
fn kotlin_trailing_at(source: &[u8], mut peak: Peak) -> Peak {
    let last_line = source
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    if let Some(at) = source[last_line..].iter().position(|&byte| byte == b'@') {
        peak.raise(1, last_line + at);
    }
    peak
}
