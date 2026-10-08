//! Syntax units and search documents (context-v2 § Syntax units and search
//! documents; 001 T005; 001 T008's languages, context-v2 § City map ›
//! Languages). The language is chosen by file extension (and the `Rakefile`
//! and `Gemfile` basenames); tree-sitter grammars (Markdown: pulldown-cmark
//! headings) yield a source-ordered interval forest of units, and every
//! source is tiled into search documents that each name their delivery
//! unit. Parsing is deterministic and error-tolerant; it is bounded by a
//! fixed work budget, never by time. Zero units, an unmapped language or a
//! source over 1 MiB falls back to blocks; so does a parse stopped by its
//! budget, which indexing names as a failure ([`index`]).

/// Sources above this size are not parsed: their outline equals their text
/// and their documents are blocks.
pub const MAX_PARSE_BYTES: usize = 1024 * 1024;
/// The work one parse may do (context-v2 § Languages): the parser reads its
/// source one UTF-8 character per input callback and calls its progress
/// callback every 100 parser operations; each read and each progress check
/// is one unit. Lexer lookahead and backtracking re-read characters, so
/// reads measure the external scanners' work, which progress checks alone
/// do not (a 4,000-deep Haskell `let` needs under 900 checks but about 296
/// million reads). A parse that would exceed the budget stops at the same
/// unit on every run and thread count. Calibration (001 T008): 22,492 real
/// files need at most 3,539,528 units (at most 18.5 per byte), so a 1 MiB
/// source at that density fits as well.
pub const PARSE_WORK_BUDGET: u64 = 1 << 25;
/// Blocks merge consecutive blank-line pieces up to this many bytes.
const BLOCK_BYTES: usize = 2048;
/// Regions above this size split into parts.
const SPLIT_OVER_BYTES: usize = 8192;
/// The largest part of a split region.
const PART_BYTES: usize = 4096;
/// Markdown heading names are cut at a UTF-8 boundary to this many bytes.
const NAME_BYTES: usize = 120;
/// Qualified names keep at most their last this-many bytes, from a UTF-8
/// boundary (owner decision, 001 T005 review M3): the unit's own name and
/// its nearest ancestors survive, and deep named nesting stays linear.
const QNAME_BYTES: usize = 256;

/// A source language, chosen by file extension (or basename).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Python,
    TypeScript,
    Tsx,
    JavaScript,
    Go,
    C,
    Cpp,
    Java,
    CSharp,
    /// F# implementation files and scripts.
    FSharp,
    /// F# signature files (`.fsi`): the crate's second grammar.
    FSharpSignature,
    VbNet,
    Php,
    Perl,
    PowerShell,
    Ruby,
    Kotlin,
    Swift,
    Scala,
    Lua,
    Dart,
    Elixir,
    Haskell,
    Markdown,
    Toml,
    Json,
    Yaml,
    Bash,
    Sql,
    Html,
    Css,
}

impl Lang {
    /// The extension map, plus the `Rakefile` and `Gemfile` basenames; every
    /// other extension (and a dotfile without a stem) is unmapped: no tag,
    /// no units, blocks only.
    pub fn from_path(path: &str) -> Option<Self> {
        let name = path.rsplit('/').next().unwrap_or(path);
        if matches!(name, "Rakefile" | "Gemfile") {
            return Some(Self::Ruby);
        }
        let (stem, extension) = name.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        Some(match extension {
            "rs" => Self::Rust,
            "py" | "pyi" => Self::Python,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "go" => Self::Go,
            "c" => Self::C,
            "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Self::Cpp,
            "java" => Self::Java,
            "cs" => Self::CSharp,
            "fs" | "fsx" => Self::FSharp,
            "fsi" => Self::FSharpSignature,
            "vb" => Self::VbNet,
            "php" | "phtml" => Self::Php,
            "pl" | "pm" | "t" | "psgi" => Self::Perl,
            "ps1" | "psm1" | "psd1" => Self::PowerShell,
            "rb" | "rake" => Self::Ruby,
            "kt" | "kts" => Self::Kotlin,
            "swift" => Self::Swift,
            "scala" | "sc" => Self::Scala,
            "lua" => Self::Lua,
            "dart" => Self::Dart,
            "ex" | "exs" => Self::Elixir,
            "hs" => Self::Haskell,
            "md" | "markdown" => Self::Markdown,
            "toml" => Self::Toml,
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "sh" | "bash" | "zsh" => Self::Bash,
            "sql" => Self::Sql,
            "html" => Self::Html,
            "css" => Self::Css,
            _ => return None,
        })
    }

    /// The fence language tag.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::FSharp | Self::FSharpSignature => "fsharp",
            Self::VbNet => "vbnet",
            Self::Php => "php",
            Self::Perl => "perl",
            Self::PowerShell => "powershell",
            Self::Ruby => "ruby",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Scala => "scala",
            Self::Lua => "lua",
            Self::Dart => "dart",
            Self::Elixir => "elixir",
            Self::Haskell => "haskell",
            Self::Markdown => "markdown",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Bash => "bash",
            Self::Sql => "sql",
            Self::Html => "html",
            Self::Css => "css",
        }
    }

    /// Languages mapped for a fence tag only have no units.
    pub fn has_units(self) -> bool {
        !matches!(
            self,
            Self::Toml | Self::Json | Self::Yaml | Self::Sql | Self::Html | Self::Css
        )
    }

    fn grammar(self) -> Option<tree_sitter::Language> {
        Some(match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::FSharp => tree_sitter_fsharp::LANGUAGE_FSHARP.into(),
            Self::FSharpSignature => tree_sitter_fsharp::LANGUAGE_SIGNATURE.into(),
            Self::VbNet => tree_sitter_vb_dotnet::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Perl => tree_sitter_perl::LANGUAGE.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
            Self::PowerShell => tree_sitter_powershell::LANGUAGE.into(),
            Self::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Self::Swift => tree_sitter_swift::LANGUAGE.into(),
            Self::Scala => tree_sitter_scala::LANGUAGE.into(),
            Self::Lua => tree_sitter_lua::LANGUAGE.into(),
            Self::Dart => tree_sitter_dart::LANGUAGE.into(),
            Self::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            Self::Haskell => tree_sitter_haskell::LANGUAGE.into(),
            _ => return None,
        })
    }

    /// Qualified names join with `::` where the languages' own qualified
    /// names do (Rust, C++, Perl, Ruby), else `.`.
    fn qname_separator(self) -> &'static str {
        match self {
            Self::Rust | Self::Cpp | Self::Perl | Self::Ruby => "::",
            _ => ".",
        }
    }

    /// A definition name ending in `?`, `!` or `'` (Ruby, Elixir, Haskell,
    /// F#) is stored and matched without that suffix (context-v2
    /// § Languages); a name made only of them is kept.
    fn strips_name_suffix(self) -> bool {
        matches!(
            self,
            Self::Ruby | Self::Elixir | Self::Haskell | Self::FSharp | Self::FSharpSignature
        )
    }
}

/// The rendered unit kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Fn,
    Struct,
    Enum,
    Union,
    Trait,
    Impl,
    Mod,
    Macro,
    Const,
    Static,
    Type,
    Class,
    Method,
    Interface,
    Section,
    Block,
    Variant,
}

impl UnitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Mod => "mod",
            Self::Macro => "macro",
            Self::Const => "const",
            Self::Static => "static",
            Self::Type => "type",
            Self::Class => "class",
            Self::Method => "method",
            Self::Interface => "interface",
            Self::Section => "section",
            Self::Block => "block",
            Self::Variant => "variant",
        }
    }
}

/// One unit of the forest. `start..end` is its byte range (a wrapper such as a
/// decorator, `export` or `template` supplies it, extended backward over its
/// leading run of documentation comments and attributes); `decl` is its
/// declaration start, where an outline's signature lines begin: the first
/// attribute of that run, else `head`. `head` is the node's (or wrapper's)
/// own start, after the leading run. `body` is the `body` field (else the
/// language's body child, or for a keyword-closed body the lines between its
/// header and its closing line), `None` when the unit has no elidable
/// interior. `name_range` is the byte range of the unit's name when the unit
/// is a definition (context-v2 § Definitions and addresses): a
/// programming-language unit with a name, except a Rust `impl` and the other
/// containers that extend a type defined elsewhere (a Swift or Dart
/// `extension`, an F# type extension, an Elixir `defimpl`, a Haskell
/// `instance`); a quoted name's range is the text inside its quotes.
/// `qualifiers` are its qualified name's address segments minus its own
/// name, taken from the syntax tree ([`address_segments`]). Units are stored
/// in source (pre-)order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub kind: UnitKind,
    pub name: Option<String>,
    pub qname: Option<String>,
    pub name_range: Option<(usize, usize)>,
    pub qualifiers: Vec<String>,
    pub start: usize,
    pub decl: usize,
    pub head: usize,
    pub end: usize,
    pub body: Option<(usize, usize)>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

/// The unit a search document is delivered as: a leaf is its own, a residual
/// its container's, a block its own; parts keep their region's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryUnit {
    pub start: usize,
    /// The unit's own start (see [`Unit::head`]): a tier-1 hit's best line;
    /// `start` for a block.
    pub head: usize,
    pub end: usize,
    pub kind: UnitKind,
    pub name: Option<String>,
    pub qname: Option<String>,
    /// The definition's name (see [`Unit::name_range`]); `None` for a
    /// block, a section and an `impl`.
    pub name_range: Option<(usize, usize)>,
    /// See [`Unit::qualifiers`].
    pub qualifiers: Vec<String>,
}

/// One search document: a byte range of the source and its delivery unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub start: usize,
    pub end: usize,
    pub unit: DeliveryUnit,
}

struct Candidate {
    start: usize,
    decl: usize,
    head: usize,
    end: usize,
    kind: UnitKind,
    name: Option<String>,
    /// The qualifier its qualified name puts before the name ([`Resolved`]):
    /// `Outer::Inner` of Ruby's `class Outer::Inner::Store`.
    qualifier: Option<String>,
    name_range: Option<(usize, usize)>,
    /// The name's own address segments ([`name_parts`]), lowercased; none
    /// for an outline-only analysis.
    address: Vec<String>,
    body: Option<(usize, usize)>,
    /// A statement-form container's open range (see [`Open`]).
    open: Option<Open>,
}

/// A namespace or package container that statement-form siblings divide
/// (C# `namespace X;`, PHP `namespace X;`, Perl `package X;`, Scala
/// `package x`): a statement form's members are its following siblings.
#[derive(Clone, Copy)]
struct Open {
    parent: usize,
    parent_end: usize,
    form: Form,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    /// Runs to the next [`Form`] in the same parent, or to the parent's end.
    Statement,
    /// Runs to the parent's end: Scala's successive clauses nest.
    Chained,
    /// A braced block (Perl `package X { … }`, PHP `namespace X { … }`):
    /// keeps its range and ends the statement form before it.
    Block,
}

/// The unit forest of `source`; empty for a language without units, a
/// source over [`MAX_PARSE_BYTES`] or a parse stopped by
/// [`PARSE_WORK_BUDGET`].
pub fn units(source: &str, lang: Lang) -> Vec<Unit> {
    analyze(source, lang, Need::Units).units
}

/// What one parse collects besides the unit forest.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Need {
    Units,
    /// The comment and declaration-only member ranges of an outline.
    Outline,
    /// The file's import keys (context-v2 § Doors).
    Imports,
}

/// One parse: the unit forest and, for outlines, the byte ranges of comments
/// spanning at least [`COMMENT_LINES`] lines and of declaration-only members;
/// for indexing, the import keys. A parse stopped by its budget yields
/// nothing but `stopped`.
#[derive(Default)]
struct Analysis {
    units: Vec<Unit>,
    comments: Vec<(usize, usize)>,
    members: Vec<(usize, usize)>,
    imports: Vec<String>,
    stopped: Option<ParseStopped>,
}

fn analyze(source: &str, lang: Lang, need: Need) -> Analysis {
    if !lang.has_units() || source.len() > MAX_PARSE_BYTES {
        return Analysis::default();
    }
    let mut analysis = Analysis::default();
    let candidates = if lang == Lang::Markdown {
        markdown_sections(source)
    } else {
        tree_units(source, lang, need, &mut analysis)
    };
    if analysis.stopped.is_some() {
        return Analysis {
            stopped: analysis.stopped,
            ..Analysis::default()
        };
    }
    analysis.units = forest(source, lang, need, candidates);
    analysis
}

/// The search documents of `source`, in source order. Document ranges plus
/// whitespace-only gaps tile `[0, len)` without overlap, and each document
/// lies inside its delivery unit. A parse stopped by its budget gives the
/// blocks of an unmapped source, as indexing's fallback does.
pub fn documents(source: &str, lang: Option<Lang>) -> Vec<Document> {
    let units = lang.map_or_else(Vec::new, |lang| units(source, lang));
    tile(source, &units)
}

/// What indexing reads from one parse of a source: its search documents
/// (as [`documents`]) and its import keys (context-v2 § Doors), distinct in
/// order of first appearance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceIndex {
    pub documents: Vec<Document>,
    pub imports: Vec<String>,
}

/// A parse stopped before it finished (context-v2 § Languages): by its work
/// budget, or before it ran, by a source over one of the limits an external
/// scanner needs ([`ScannerLimit`]). Indexing handles it as a parse panic
/// (§ Parallel indexing): the plain blocks of an unmapped source and a named
/// failure. `at` is the byte offset the parser had reached when the budget
/// ran out, or where the source first reaches the scanner limit: the same on
/// every run and thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseStopped {
    pub at: usize,
    pub stop: Stop,
}

/// What stopped a parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The work budget, in units ([`PARSE_WORK_BUDGET`]).
    Budget(u64),
    Scanner(ScannerLimit),
}

impl std::fmt::Display for ParseStopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at = self.at;
        match self.stop {
            Stop::Budget(units) => {
                write!(
                    f,
                    "exceeded the parse work budget of {units} units at byte {at}"
                )
            }
            Stop::Scanner(limit) => {
                write!(f, "over the scanner limit: {limit} at byte {at}")
            }
        }
    }
}

/// The [`SourceIndex`] of `source`, or the stop of a parse that exceeded its
/// work budget.
pub fn index(source: &str, lang: Option<Lang>) -> Result<SourceIndex, ParseStopped> {
    let analysis = lang.map_or_else(Analysis::default, |lang| {
        analyze(source, lang, Need::Imports)
    });
    if let Some(stopped) = analysis.stopped {
        return Err(stopped);
    }
    let mut seen = std::collections::HashSet::new();
    let imports = analysis
        .imports
        .into_iter()
        .filter(|key| seen.insert(key.clone()))
        .collect();
    Ok(SourceIndex {
        documents: tile(source, &analysis.units),
        imports,
    })
}

/// Tiles `source` into documents: each top-level unit's regions, and blocks
/// for the bytes outside them.
fn tile(source: &str, units: &[Unit]) -> Vec<Document> {
    let mut documents = Vec::new();
    let mut cursor = 0;
    for (index, unit) in units.iter().enumerate() {
        if unit.parent.is_none() {
            blocks(source, cursor, unit.start, &mut documents);
            unit_documents(source, units, index, &mut documents);
            cursor = unit.end;
        }
    }
    blocks(source, cursor, source.len(), &mut documents);
    documents
}

/// Leaf body interiors and container gaps shorter than this are not elided.
const ELIDE_LINES: usize = 4;
/// Block comments shorter than this are not elided.
const COMMENT_LINES: usize = 6;

/// One piece of an outline over source bytes: kept bytes verbatim, or one
/// elided run of whole lines, rendered as its first line's indentation then
/// `⋯ <first_line>-<last_line>` (1-based inclusive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Kept {
        start: usize,
        end: usize,
    },
    Elided {
        first_line: usize,
        last_line: usize,
        indent: String,
    },
}

/// The outline of `range` (context-v2 § Outline algorithm). An independent
/// implementation; the design reference is recorded in 001 T006.
pub fn outline(
    source: &str,
    lang: Lang,
    range: std::ops::Range<usize>,
    unfold_until: usize,
    unfold_limit: usize,
) -> Vec<Segment> {
    Outliner::new(source, lang).segments(range, unfold_until, unfold_limit)
}

/// Kept bytes verbatim; each elided run as one marker line.
pub fn render_outline(source: &str, segments: &[Segment]) -> String {
    let mut out = String::new();
    for segment in segments {
        match segment {
            Segment::Kept { start, end } => out.push_str(&source[*start..*end]),
            Segment::Elided {
                first_line,
                last_line,
                indent,
            } => {
                out.push_str(indent);
                out.push_str(&format!("⋯ {first_line}-{last_line}\n"));
            }
        }
    }
    out
}

/// One elidable span: 1-based inclusive lines, nested by line containment.
struct Span {
    first: usize,
    last: usize,
    parent: Option<usize>,
    children: Vec<usize>,
}

/// One parse of a source, reusable for every outline range and form.
pub struct Outliner<'a> {
    source: &'a str,
    /// Byte offset of each line's start; line `n` (1-based) is index `n - 1`.
    line_starts: Vec<usize>,
    /// Every elidable span of the source, sorted by first line.
    spans: Vec<Span>,
}

impl<'a> Outliner<'a> {
    /// Units and comments come from one parse; a language without units or a
    /// source over [`MAX_PARSE_BYTES`] has no elidable span, so its outline is
    /// its text.
    pub fn new(source: &'a str, lang: Lang) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter(|&(i, b)| b == b'\n' && i + 1 < source.len())
                .map(|(i, _)| i + 1),
        );
        let mut outliner = Self {
            source,
            line_starts,
            spans: Vec::new(),
        };
        let analysis = analyze(source, lang, Need::Outline);
        outliner.spans = outliner.elidable_spans(&analysis);
        outliner
    }

    /// The 1-based line holding byte `at`.
    fn line_of(&self, at: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= at)
    }

    /// The byte offset where 1-based `line` starts.
    fn line_start(&self, line: usize) -> usize {
        self.line_starts[line - 1]
    }

    /// The byte offset just past 1-based `line`, its LF included.
    fn line_end(&self, line: usize) -> usize {
        self.line_starts
            .get(line)
            .copied()
            .unwrap_or(self.source.len())
    }

    /// The byte offset where 1-based `line`'s content ends, before its LF or
    /// CRLF terminator.
    fn content_end(&self, line: usize) -> usize {
        let start = self.line_start(line);
        let text = &self.source[start..self.line_end(line)];
        let text = text
            .strip_suffix('\n')
            .map_or(text, |text| text.strip_suffix('\r').unwrap_or(text));
        start + text.len()
    }

    /// The last line of a unit's signature: the line of its body's opening
    /// delimiter, or, for a body without one (a Python block, a Markdown
    /// section), the line of the last non-whitespace byte before the body.
    fn signature_end(&self, unit: &Unit, body_start: usize) -> usize {
        let bytes = self.source.as_bytes();
        if matches!(bytes.get(body_start), Some(b'{' | b'(' | b'[')) {
            return self.line_of(body_start);
        }
        let before = bytes[unit.start..body_start]
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map_or(unit.start, |offset| unit.start + offset);
        self.line_of(before)
    }

    /// The body's closing line when its closing delimiter is on its own line.
    fn closing_line(&self, body_end: usize) -> Option<usize> {
        let last = body_end.checked_sub(1)?;
        if !matches!(self.source.as_bytes()[last], b'}' | b')' | b']') {
            return None;
        }
        let line = self.line_of(last);
        self.source[self.line_start(line)..last]
            .trim()
            .is_empty()
            .then_some(line)
    }

    fn elidable_spans(&self, analysis: &Analysis) -> Vec<Span> {
        let units = &analysis.units;
        // Every signature line, closing line and declaration-only member line
        // is mandatory; no elidable span may contain one.
        let mut mandatory = vec![false; self.line_starts.len() + 1];
        let mut bodies: Vec<Option<(usize, Option<usize>, usize)>> =
            Vec::with_capacity(units.len());
        for unit in units {
            let body = unit
                .body
                .filter(|(body_start, body_end)| body_start < body_end)
                .map(|(body_start, body_end)| {
                    let signature_end = self.signature_end(unit, body_start);
                    mandatory[self.line_of(unit.decl)..=signature_end].fill(true);
                    let closing = self.closing_line(body_end);
                    if let Some(line) = closing {
                        mandatory[line] = true;
                    }
                    (signature_end, closing, body_end)
                });
            bodies.push(body);
        }
        for &(start, end) in &analysis.members {
            mandatory[self.line_of(start)..=self.line_of(end - 1)].fill(true);
        }
        let mut spans: Vec<(usize, usize)> = Vec::new();
        // Maximal runs of non-mandatory lines of `first..=last`.
        let mut push_run = |first: usize, last: usize, minimum: usize| {
            let mut line = first;
            while line <= last {
                if mandatory[line] {
                    line += 1;
                    continue;
                }
                let run = line;
                while line <= last && !mandatory[line] {
                    line += 1;
                }
                if line - run >= minimum {
                    spans.push((run, line - 1));
                }
            }
        };
        for (unit, body) in units.iter().zip(&bodies) {
            let Some((signature_end, closing, body_end)) = *body else {
                continue;
            };
            let first = signature_end + 1;
            let last = match closing {
                Some(line) => line - 1,
                None => self.line_of(body_end - 1),
            };
            // Markdown section bodies elide whatever their length.
            let minimum = if unit.kind == UnitKind::Section {
                1
            } else {
                ELIDE_LINES
            };
            if unit.children.is_empty() {
                push_run(first, last, minimum);
                continue;
            }
            // Container gaps: interior lines outside member units.
            let mut cursor = first;
            for &child in &unit.children {
                let child_first = self.line_of(units[child].start);
                let child_last = self.line_of(units[child].end - 1);
                if child_first > cursor {
                    push_run(cursor, (child_first - 1).min(last), minimum);
                }
                cursor = cursor.max(child_last + 1);
            }
            push_run(cursor, last, minimum);
        }
        // A unit's leading documentation before its declaration start is one
        // elidable span when it has at least 2 lines; it replaces any
        // block-comment span inside it.
        let mut docs: Vec<(usize, usize)> = Vec::new();
        for unit in units.iter().filter(|unit| unit.decl > unit.start) {
            let first = self.line_of(unit.start);
            let Some(last) = self.line_of(unit.decl).checked_sub(1) else {
                continue;
            };
            if last > first && !mandatory[first..=last].iter().any(|&m| m) {
                docs.push((first, last));
            }
        }
        spans.extend_from_slice(&docs);
        spans.sort_unstable();
        let regions = spans.len();
        // Block comments made of whole lines that hide no mandatory line.
        for &(start, end) in &analysis.comments {
            let first = self.line_of(start);
            let last = self.line_of(end - 1);
            let whole_lines = self.source[self.line_start(first)..start].trim().is_empty()
                && self.source[end..self.line_end(last)].trim().is_empty();
            if last - first + 1 >= COMMENT_LINES
                && whole_lines
                && !mandatory[first..=last].iter().any(|&m| m)
                && !docs.iter().any(|&(f, l)| f <= first && last <= l)
            {
                spans.push((first, last));
            }
        }
        // Interior and gap spans are disjoint; a comment nests in the one
        // that contains it, if any.
        let mut out: Vec<Span> = spans
            .iter()
            .map(|&(first, last)| Span {
                first,
                last,
                parent: None,
                children: Vec::new(),
            })
            .collect();
        for comment in regions..out.len() {
            let (first, last) = (out[comment].first, out[comment].last);
            let candidate = out[..regions].partition_point(|span| span.first <= first);
            if let Some(region) = candidate.checked_sub(1)
                && out[region].last >= last
            {
                out[comment].parent = Some(region);
            }
        }
        // Sort by first line, then remap parent/child links.
        let mut order: Vec<usize> = (0..out.len()).collect();
        order.sort_by_key(|&index| (out[index].first, std::cmp::Reverse(out[index].last)));
        let mut position = vec![0usize; out.len()];
        for (new, &old) in order.iter().enumerate() {
            position[old] = new;
        }
        let mut sorted: Vec<Span> = order
            .iter()
            .map(|&old| Span {
                first: out[old].first,
                last: out[old].last,
                parent: out[old].parent.map(|parent| position[parent]),
                children: Vec::new(),
            })
            .collect();
        for index in 0..sorted.len() {
            if let Some(parent) = sorted[index].parent {
                sorted[parent].children.push(index);
            }
        }
        sorted
    }

    /// The outline of `range` with the given unfold thresholds.
    pub fn segments(
        &self,
        range: std::ops::Range<usize>,
        unfold_until: usize,
        unfold_limit: usize,
    ) -> Vec<Segment> {
        let end = range.end.min(self.source.len());
        let start = range.start.min(end);
        if start == end {
            return Vec::new();
        }
        let lines = |index: usize| self.spans[index].last - self.spans[index].first + 1;
        let considered: Vec<bool> = self
            .spans
            .iter()
            .map(|span| self.line_start(span.first) >= start && self.content_end(span.last) <= end)
            .collect();
        let children = |index: usize| {
            self.spans[index]
                .children
                .iter()
                .copied()
                .filter(|&child| considered[child])
        };
        // Outermost considered spans start folded.
        let mut folded = vec![false; self.spans.len()];
        let mut queue = std::collections::VecDeque::new();
        let mut visible = self.line_of(end - 1) - self.line_of(start) + 1;
        for (index, span) in self.spans.iter().enumerate() {
            let outermost = span.parent.is_none_or(|parent| !considered[parent]);
            if considered[index] && outermost {
                folded[index] = true;
                visible -= lines(index) - 1;
                queue.push_back(index);
            }
        }
        // Breadth-first unfold while fewer than `unfold_until` lines are
        // visible; an unfold that would exceed `unfold_limit` is skipped with
        // its subtree. A marker counts as one visible line.
        while visible < unfold_until {
            let Some(index) = queue.pop_front() else {
                break;
            };
            let nested: usize = children(index).map(|child| lines(child) - 1).sum();
            let after = visible + (lines(index) - 1 - nested);
            if after > unfold_limit {
                continue;
            }
            visible = after;
            folded[index] = false;
            for child in children(index) {
                folded[child] = true;
                queue.push_back(child);
            }
        }
        let mut segments = Vec::new();
        let mut cursor = start;
        for (index, span) in self.spans.iter().enumerate() {
            if !folded[index] {
                continue;
            }
            let span_start = self.line_start(span.first);
            if cursor < span_start {
                segments.push(Segment::Kept {
                    start: cursor,
                    end: span_start,
                });
            }
            let line = &self.source[span_start..self.line_end(span.first)];
            let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            segments.push(Segment::Elided {
                first_line: span.first,
                last_line: span.last,
                indent: line[..indent].to_owned(),
            });
            cursor = self.line_end(span.last).min(end);
        }
        if cursor < end {
            segments.push(Segment::Kept { start: cursor, end });
        }
        segments
    }

    /// The rendered outline of `range`.
    pub fn render(
        &self,
        range: std::ops::Range<usize>,
        unfold_until: usize,
        unfold_limit: usize,
    ) -> String {
        render_outline(
            self.source,
            &self.segments(range, unfold_until, unfold_limit),
        )
    }
}

/// The unit candidates of one parse; for an outline, also the comment and
/// declaration-only member ranges it needs, and for indexing the import keys.
/// A source over a [`ScannerLimit`] is stopped before the parser runs.
fn tree_units(source: &str, lang: Lang, need: Need, extras: &mut Analysis) -> Vec<Candidate> {
    let mut out = Vec::new();
    let Some(grammar) = lang.grammar() else {
        return out;
    };
    let bytes = source.as_bytes();
    if let Some(stopped) = over_scanner_limit(lang, bytes) {
        extras.stopped = Some(stopped);
        return out;
    }
    let tree = match parse(source, &grammar) {
        Ok(Some(tree)) => tree,
        Ok(None) => return out,
        Err(stopped) => {
            extras.stopped = Some(stopped);
            return out;
        }
    };
    // Iterative pre-order walk: error-recovered trees can be deep. The
    // ancestors of the current node are kept on the heap, because
    // `Node::parent` (and so `prev_sibling`) re-descends from the root. Per
    // depth, `runs` holds the consecutive leading-run nodes (§ Unit forest)
    // just before the current node at that depth, and `facts` what the
    // ancestor at that depth says about its children ([`Facts`]), read once.
    let mut cursor = tree.walk();
    let mut ancestors: Vec<tree_sitter::Node> = Vec::new();
    let mut facts: Vec<Facts> = Vec::new();
    let mut runs: Vec<Vec<(tree_sitter::Node, Leading)>> = vec![Vec::new()];
    'walk: loop {
        let node = cursor.node();
        // Units, members and imports are named nodes; keyword tokens such
        // as F#'s `namespace` or Haskell's `import` share their kind names.
        if node.is_named() {
            // A Rust declaration-only item is a unit and stays a mandatory
            // outline member, so outlines keep its lines as before.
            let at = Walk {
                lang,
                ancestors: &ancestors,
                facts: &facts,
                source: bytes,
            };
            candidates(&at, node, &runs, need, &mut out);
            if need == Need::Imports {
                import_keys(lang, node, bytes, &mut extras.imports);
            } else if need == Need::Outline {
                if is_member_signature(&at, node) {
                    // A template wrapper's lines belong to the signature too.
                    let outer = at.outermost_wrapper(node);
                    extras.members.push((outer.start_byte(), outer.end_byte()));
                } else if node.kind().ends_with("comment")
                    && node.end_position().row + 1 >= node.start_position().row + COMMENT_LINES
                {
                    extras.comments.push((node.start_byte(), node.end_byte()));
                }
            }
        }
        if cursor.goto_first_child() {
            facts.push(Facts::of(lang, node));
            ancestors.push(node);
            runs.push(Vec::new());
            continue;
        }
        loop {
            // The current node's subtree is done: it now precedes its next sibling.
            let finished = cursor.node();
            let run = runs.last_mut().expect("one run per depth");
            match leading_kind(lang, finished, bytes) {
                Some(kind) => run.push((finished, kind)),
                None => run.clear(),
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
            ancestors.pop();
            facts.pop();
            runs.pop();
        }
    }
    close_statement_forms(bytes, &mut out);
    out
}

/// Where the walk stands: the language, the current node's ancestors
/// (outermost first), their [`Facts`] and the source.
struct Walk<'a, 't> {
    lang: Lang,
    ancestors: &'a [tree_sitter::Node<'t>],
    facts: &'a [Facts],
    source: &'a [u8],
}

impl<'t> Walk<'_, 't> {
    /// The kind of the `n`th ancestor up (1: the parent).
    fn above(&self, n: usize) -> Option<&'static str> {
        let at = self.ancestors.len().checked_sub(n)?;
        Some(self.ancestors[at].kind())
    }

    /// How many ancestors, innermost first, wrap the current node
    /// ([`is_wrapper`]).
    fn wrappers(&self) -> usize {
        self.facts
            .iter()
            .rev()
            .take_while(|facts| facts.wrapper)
            .count()
    }

    /// The outermost of the wrappers (decorator, `export`, `template`, a
    /// declaration with one declarator or spec, a Python expression
    /// statement) directly enclosing `node`, or `node` itself; it supplies
    /// the range.
    fn outermost_wrapper(&self, node: tree_sitter::Node<'t>) -> tree_sitter::Node<'t> {
        match self.wrappers() {
            0 => node,
            wrappers => self.ancestors[self.ancestors.len() - wrappers],
        }
    }
}

/// What a node says about its children, read once when the walk enters it:
/// a node with many children is never rescanned per child (001 T008 review
/// M2). `wrapper`: [`is_wrapper`]; `constructors`: a Haskell sum type's
/// constructor list has more than one constructor.
#[derive(Clone, Copy)]
struct Facts {
    wrapper: bool,
    constructors: bool,
}

impl Facts {
    fn of(lang: Lang, node: tree_sitter::Node) -> Self {
        Self {
            wrapper: is_wrapper(lang, node),
            constructors: lang == Lang::Haskell
                && node.kind() == "data_constructors"
                && count_named_up_to(node, &["data_constructor"], 2) == 2,
        }
    }
}

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
fn parse(
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
fn over_scanner_limit(lang: Lang, source: &[u8]) -> Option<ParseStopped> {
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

/// Extends each statement-form container (see [`Open`]) over its members:
/// to the next container's start in the same parent, or (chained, or the
/// last) to the parent's end, without the whitespace before that point
/// unless a member's range covers it (Perl's last block runs over the
/// file's trailing whitespace and comments). The bytes after its own
/// statement are its body.
fn close_statement_forms(source: &[u8], candidates: &mut [Candidate]) {
    let mut open: Vec<(usize, usize, usize)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, c)| c.open.map(|open| (open.parent, c.start, index)))
        .collect();
    if open.is_empty() {
        return;
    }
    open.sort_unstable();
    let mut spans: Vec<(usize, usize)> = candidates.iter().map(|c| (c.start, c.end)).collect();
    spans.sort_unstable();
    // Latest member end of the next chained container of the same parent,
    // whose window lies inside this one's; walking backwards reads each
    // member once.
    let mut carried = 0;
    for (position, &(parent, start, index)) in open.iter().enumerate().rev() {
        let Some(Open {
            parent_end, form, ..
        }) = candidates[index].open
        else {
            continue;
        };
        let next = open
            .get(position + 1)
            .filter(|next| next.0 == parent)
            .map(|next| next.1);
        let (end, inherited) = match form {
            Form::Block => {
                carried = 0;
                continue;
            }
            Form::Statement => (next.unwrap_or(parent_end), 0),
            Form::Chained => (parent_end, if next.is_some() { carried } else { 0 }),
        };
        let own = candidates[index].end;
        let mut end = end.max(own);
        let members = spans.partition_point(|span| span.0 < start)
            ..spans.partition_point(|span| span.0 < next.unwrap_or(end).min(end));
        let reach = spans[members]
            .iter()
            .map(|span| span.1)
            .max()
            .unwrap_or(0)
            .max(inherited);
        carried = reach;
        let floor = reach.clamp(own, end);
        while end > floor && source[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        candidates[index].end = end;
        candidates[index].body = (own < end).then_some((own, end));
    }
}

/// Whether `node` is a namespace or package container of [`Open`]'s kind.
fn statement_form(lang: Lang, node: tree_sitter::Node) -> Option<Form> {
    let braced = node.child_by_field_name("body").is_some();
    match (lang, node.kind()) {
        (Lang::CSharp, "file_scoped_namespace_declaration") => Some(Form::Statement),
        (Lang::Php, "namespace_definition") | (Lang::Perl, "package_statement") => {
            Some(if braced { Form::Block } else { Form::Statement })
        }
        (Lang::Scala, "package_clause") if !braced => Some(Form::Chained),
        _ => None,
    }
}

/// A declaration-only member (a trait method without a body, an interface,
/// protocol or abstract member, a C/C++ prototype, a Haskell or F#
/// signature): not a unit, yet a signature that an outline never hides.
fn is_member_signature(at: &Walk, node: tree_sitter::Node) -> bool {
    let (lang, source) = (at.lang, at.source);
    let above = |n: usize| at.above(n);
    let bodiless = || node.child_by_field_name("body").is_none();
    match lang {
        Lang::Rust => matches!(node.kind(), "function_signature_item" | "associated_type"),
        Lang::Go => node.kind() == "method_elem",
        Lang::TypeScript | Lang::Tsx => matches!(
            node.kind(),
            "method_signature"
                | "property_signature"
                | "abstract_method_signature"
                | "call_signature"
                | "construct_signature"
                | "index_signature"
        ),
        Lang::C | Lang::Cpp => {
            matches!(node.kind(), "declaration" | "field_declaration") && declares_function(node)
        }
        Lang::CSharp => {
            above(1) == Some("declaration_list")
                && above(2) == Some("interface_declaration")
                && match node.kind() {
                    "method_declaration" => bodiless(),
                    kind => matches!(
                        kind,
                        "property_declaration" | "event_declaration" | "indexer_declaration"
                    ),
                }
        }
        Lang::FSharp => node.kind() == "member_signature",
        Lang::FSharpSignature => matches!(node.kind(), "member_signature" | "value_definition"),
        Lang::VbNet => {
            matches!(node.kind(), "method_declaration" | "property_declaration")
                && above(1) == Some("interface_block")
        }
        Lang::Php => node.kind() == "method_declaration" && bodiless(),
        Lang::Kotlin => {
            node.kind() == "function_declaration"
                && named_child_of(node, &["function_body"]).is_none()
                && matches!(above(1), Some("class_body" | "enum_class_body"))
        }
        Lang::Swift => matches!(
            node.kind(),
            "protocol_function_declaration" | "protocol_property_declaration"
        ),
        Lang::Scala => node.kind() == "function_declaration",
        Lang::Dart => {
            node.kind() == "declaration"
                && node.named_child(0).is_some_and(|first| {
                    matches!(
                        first.kind(),
                        "function_signature" | "getter_signature" | "setter_signature"
                    )
                })
        }
        Lang::Elixir => {
            node.kind() == "call"
                && elixir_definition(node, source).is_some_and(|(keyword, _)| {
                    matches!(keyword, ElixirDef::Function) && elixir_body(node, source).is_none()
                })
        }
        Lang::Haskell => node.kind() == "signature",
        _ => false,
    }
}

/// Whether one of a declaration's declarator chains holds a function
/// declarator (a prototype rather than a variable or field).
fn declares_function(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    node.children_by_field_name("declarator", &mut cursor)
        .any(|mut declarator| {
            loop {
                if declarator.kind() == "function_declarator" {
                    return true;
                }
                let inner = declarator.child_by_field_name("declarator").or_else(|| {
                    matches!(
                        declarator.kind(),
                        "reference_declarator" | "parenthesized_declarator"
                    )
                    .then(|| unnamed_declarator(declarator))
                    .flatten()
                });
                match inner {
                    Some(inner) => declarator = inner,
                    None => return false,
                }
            }
        })
}

/// The candidates `node` gives: none, itself, or for a declaration that
/// binds several names ([`bindings`]) one per binding.
fn candidates(
    at: &Walk,
    node: tree_sitter::Node,
    runs: &[Vec<(tree_sitter::Node, Leading)>],
    need: Need,
    out: &mut Vec<Candidate>,
) {
    let (lang, source) = (at.lang, at.source);
    // Whether `node` gives a unit comes first: only then is its leading run
    // read, so a long run of comments is walked once, by the unit after it,
    // not once per comment (001 T008 review R3).
    let bindings = bindings(at, node);
    let kind = match bindings {
        Some(_) => None,
        None => match unit_kind(at, node) {
            Some(kind) => Some(kind),
            None => return,
        },
    };
    let outer = at.outermost_wrapper(node);
    // `outer` sits at the depth of the outermost wrapper (or of `node`).
    let (start, decl) = leading_run(outer, &runs[at.ancestors.len() - at.wrappers()], source);
    if let Some(bindings) = bindings {
        // The declaration's first binding keeps its start and leading run;
        // each binding ends where its own text does.
        for binding in bindings {
            let (start, decl, head) = match binding.start {
                None => (start, decl, outer.start_byte()),
                Some(own) => (own, own, own),
            };
            out.push(named_candidate(
                lang,
                binding.kind,
                Some(Named::whole(binding.name)),
                source,
                need,
                Candidate {
                    start,
                    decl,
                    head,
                    end: binding.end,
                    kind: binding.kind,
                    name: None,
                    qualifier: None,
                    name_range: None,
                    address: Vec::new(),
                    body: binding.body,
                    open: None,
                },
            ));
        }
        return;
    }
    let Some(kind) = kind else {
        return;
    };
    let open = statement_form(lang, node).and_then(|form| {
        let parent = at.ancestors.last()?;
        Some(Open {
            parent: parent.id(),
            parent_end: parent.end_byte(),
            form,
        })
    });
    out.push(named_candidate(
        lang,
        kind,
        unit_name(lang, node, source),
        source,
        need,
        Candidate {
            start,
            decl,
            head: outer.start_byte(),
            end: outer.end_byte(),
            kind,
            name: None,
            qualifier: None,
            name_range: None,
            address: Vec::new(),
            body: body_range(lang, node, source),
            open,
        },
    ));
}

/// `candidate` with its name, name range, qualifier and address filled in
/// from `named` ([`resolve_name`]).
fn named_candidate(
    lang: Lang,
    kind: UnitKind,
    named: Option<Named>,
    source: &[u8],
    need: Need,
    candidate: Candidate,
) -> Candidate {
    let resolved = named.and_then(|named| resolve_name(lang, named, source, need));
    let name = resolved
        .as_ref()
        .and_then(|resolved| {
            std::str::from_utf8(source.get(resolved.span.0..resolved.span.1)?).ok()
        })
        .map(|text| definition_name(lang, text).to_owned())
        .filter(|name| !name.is_empty());
    // A Rust `impl` (and each container like it) names the type it extends:
    // it is a container, not a definition of that type (context-v2
    // § Definitions and addresses).
    let name_range = resolved
        .as_ref()
        .filter(|_| kind != UnitKind::Impl && name.is_some())
        .map(|resolved| resolved.span);
    let (qualifier, address) = match resolved {
        Some(resolved) if name.is_some() => (resolved.qualifier, resolved.address),
        _ => (None, Vec::new()),
    };
    Candidate {
        name,
        qualifier,
        name_range,
        address,
        ..candidate
    }
}

/// One name a declaration binds among several ([`bindings`]).
struct Binding<'t> {
    /// Where the binding's own text starts (its `and`, or its name); `None`
    /// for the declaration's first, which starts with the declaration.
    start: Option<usize>,
    end: usize,
    name: tree_sitter::Node<'t>,
    kind: UnitKind,
    body: Option<(usize, usize)>,
}

/// The bindings of a declaration that binds several names, each a unit with
/// its own range (001 T008 review M3, M4): an F# `let rec f … and g …`
/// group, one per head and body (a function at any depth; a value at module
/// level, as a lone `let`); a module-level Swift `let a = 1, b = 2`, one per
/// name and its type and value. The first binding starts with the
/// declaration; each later one at its `and` (F#) or its name (Swift), and
/// each ends with its body (F#) or before the comma after it (Swift). `None`
/// for a declaration with fewer than two bindings, which stays one unit.
fn bindings<'t>(at: &Walk<'_, 't>, node: tree_sitter::Node<'t>) -> Option<Vec<Binding<'t>>> {
    match (at.lang, node.kind()) {
        (Lang::FSharp | Lang::FSharpSignature, "function_or_value_defn") => {
            const HEADS: [&str; 2] = ["function_declaration_left", "value_declaration_left"];
            if count_named_up_to(node, &HEADS, 2) < 2 {
                return None;
            }
            let module_level = at.above(1) == Some("declaration_expression")
                && matches!(
                    at.above(2),
                    Some("module_defn" | "namespace" | "named_module" | "file")
                );
            let mut out = Vec::new();
            // The current binding's `and` and head, until its body.
            let mut and = None;
            let mut head = None;
            let mut cursor = node.walk();
            let mut more = cursor.goto_first_child();
            while more {
                let child = cursor.node();
                match child.kind() {
                    "and" => and = Some(child.start_byte()),
                    kind if HEADS.contains(&kind) => head = Some(child),
                    _ if cursor.field_name() == Some("body") => {
                        if let Some(left) = head.take() {
                            let start = and.take();
                            let binding = |kind, name| Binding {
                                start,
                                end: child.end_byte(),
                                name,
                                kind,
                                body: Some((child.start_byte(), child.end_byte())),
                            };
                            if left.kind() == "function_declaration_left" {
                                out.extend(
                                    named_child_of(left, &["identifier"])
                                        .map(|name| binding(UnitKind::Fn, name)),
                                );
                            } else if module_level {
                                let kind = if has_child_kind(left, "mutable") {
                                    UnitKind::Static
                                } else {
                                    UnitKind::Const
                                };
                                out.extend(
                                    named_child_of(left, &["identifier_pattern"])
                                        .and_then(|pattern| {
                                            named_child_of(pattern, &["long_identifier_or_op"])
                                        })
                                        .and_then(|name| named_child_of(name, &["identifier"]))
                                        .map(|name| binding(kind, name)),
                                );
                            }
                        }
                    }
                    _ => {}
                }
                more = cursor.goto_next_sibling();
            }
            Some(out)
        }
        (Lang::Swift, "property_declaration") if at.above(1) == Some("source_file") => {
            let mut cursor = node.walk();
            let names = node
                .children_by_field_name("name", &mut cursor)
                .take(2)
                .count();
            if names < 2 {
                return None;
            }
            cursor.reset(node);
            let kind = swift_binding_kind(node, at.source);
            let mut out: Vec<Binding> = Vec::new();
            // Whether the latest name is a binding (a plain name, not a
            // tuple pattern): its type, value and so on extend it.
            let mut current = false;
            let mut first = true;
            let mut more = cursor.goto_first_child();
            while more {
                let child = cursor.node();
                if cursor.field_name() == Some("name") {
                    current = false;
                    if let Some(name) = child.child_by_field_name("bound_identifier") {
                        out.push(Binding {
                            start: (!first).then_some(child.start_byte()),
                            end: child.end_byte(),
                            name,
                            kind,
                            body: None,
                        });
                        current = true;
                    }
                    first = false;
                } else if current
                    && child.kind() != ","
                    && let Some(last) = out.last_mut()
                {
                    last.end = child.end_byte();
                }
                more = cursor.goto_next_sibling();
            }
            Some(out)
        }
        _ => None,
    }
}

/// A module-level Swift property is a `const` when bound with `let`.
fn swift_binding_kind(declaration: tree_sitter::Node, source: &[u8]) -> UnitKind {
    let constant = named_child_of(declaration, &["value_binding_pattern"])
        .is_some_and(|binding| text(binding, source) == Some("let"));
    if constant {
        UnitKind::Const
    } else {
        UnitKind::Static
    }
}

/// A definition's stored name: without a trailing run of `?`, `!` and `'`
/// where the language strips it, unless nothing would be left.
fn definition_name(lang: Lang, text: &str) -> &str {
    if !lang.strips_name_suffix() {
        return text;
    }
    match text.trim_end_matches(['?', '!', '\'']) {
        "" => text,
        stripped => stripped,
    }
}

/// What a node can be in a unit's leading run (context-v2 § Unit forest).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leading {
    Doc,
    Attribute,
}

fn leading_kind(lang: Lang, node: tree_sitter::Node, source: &[u8]) -> Option<Leading> {
    let doc_block = || {
        node.utf8_text(source)
            .is_ok_and(|text| text.starts_with("/**"))
    };
    match (lang, node.kind()) {
        (Lang::Rust, "attribute_item") => Some(Leading::Attribute),
        (Lang::Rust, "line_comment" | "block_comment") => {
            node.child_by_field_name("outer").map(|_| Leading::Doc)
        }
        (Lang::Java, "block_comment")
        | (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "comment") => {
            doc_block().then_some(Leading::Doc)
        }
        (Lang::Go, "comment") => Some(Leading::Doc),
        // A type's `<Attribute>` block precedes its declaration as a sibling
        // (a member's is inside it).
        (Lang::VbNet, "attribute_block") => Some(Leading::Attribute),
        _ => None,
    }
}

/// The start of `outer`'s leading run and its declaration start: the first
/// attribute of the run, else `outer`'s start. `run` holds the consecutive
/// leading-run nodes before `outer`; each counted node starts its line and is
/// separated from the next node only by whitespace without a blank line;
/// without a run both are `outer`'s start.
fn leading_run(
    outer: tree_sitter::Node,
    run: &[(tree_sitter::Node, Leading)],
    source: &[u8],
) -> (usize, usize) {
    let mut start = outer.start_byte();
    let mut decl = start;
    let mut next = outer;
    for &(previous, kind) in run.iter().rev() {
        let line_start = source[..previous.start_byte()]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |at| at + 1);
        // Any Unicode whitespace (NBSP and form feed are legal in these
        // grammars); node bounds and the byte after an LF are char boundaries.
        let is_space = |bytes: &[u8]| {
            std::str::from_utf8(bytes).is_ok_and(|text| text.chars().all(char::is_whitespace))
        };
        let starts_line = is_space(&source[line_start..previous.start_byte()]);
        let gap_is_space = is_space(&source[previous.end_byte()..next.start_byte()]);
        // A node that ends with its LF ends on the row before its end position.
        let end = previous.end_position();
        let last_row = if end.column == 0 && previous.end_byte() > previous.start_byte() {
            end.row.saturating_sub(1)
        } else {
            end.row
        };
        let no_blank_line = next.start_position().row <= last_row + 1;
        if !(starts_line && gap_is_space && no_blank_line) {
            break;
        }
        start = previous.start_byte();
        if kind == Leading::Attribute {
            decl = start;
        }
        next = previous;
    }
    (start, decl)
}

/// The unit kind of `node` where the walk stands (context-v2 § Unit kinds
/// and § Languages).
fn unit_kind(at: &Walk, node: tree_sitter::Node) -> Option<UnitKind> {
    use UnitKind::*;
    let (lang, source, ancestors) = (at.lang, at.source, at.ancestors);
    let kind = node.kind();
    // The kind of the `n`th ancestor up (1: the parent).
    let above = |n: usize| at.above(n);
    match lang {
        Lang::Rust => Some(match kind {
            "function_item" => Fn,
            "struct_item" => Struct,
            "enum_item" => Enum,
            "enum_variant" => Variant,
            "union_item" => Union,
            "trait_item" => Trait,
            "impl_item" => Impl,
            "mod_item" => Mod,
            "macro_definition" => Macro,
            "const_item" => Const,
            "static_item" => Static,
            "type_item" => Type,
            // A trait alias (`trait Thin = Pointee + Sized;`) names a bound
            // list as a type alias names a type. Const and auto traits, impl
            // restrictions, `const impl` and macros 2.0 keep the node kinds
            // above (the tree-sitter-rust fork, context-v2 § Languages).
            "trait_alias_item" => Type,
            // Declaration-only trait and extern-block items: required
            // methods and foreign `fn`s, and associated types. Valueless
            // `const` and `static` items are `const_item`/`static_item`.
            "function_signature_item" => Fn,
            "associated_type" => Type,
            _ => return None,
        }),
        Lang::Python => match kind {
            "function_definition" => Some(Fn),
            "class_definition" => Some(Class),
            // A module-level assignment to one identifier.
            "assignment"
                if above(1) == Some("expression_statement")
                    && above(2) == Some("module")
                    && node
                        .child_by_field_name("left")
                        .is_some_and(|left| left.kind() == "identifier") =>
            {
                Some(Static)
            }
            _ => None,
        },
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => match kind {
            "function_declaration" | "generator_function_declaration" => Some(Fn),
            "class_declaration" => Some(Class),
            "method_definition" => Some(Method),
            "interface_declaration" => Some(Interface),
            "type_alias_declaration" => Some(Type),
            "enum_declaration" => Some(Enum),
            "enum_assignment" => Some(Variant),
            // A bare member: the enum body's `name` field, an identifier or
            // a quoted name.
            "property_identifier" | "string" if above(1) == Some("enum_body") => Some(Variant),
            "variable_declarator" => declarator_kind(at, node),
            _ => None,
        },
        Lang::Go => match kind {
            "function_declaration" => Some(Fn),
            "method_declaration" => Some(Method),
            "type_spec" | "type_alias" => Some(Type),
            "const_spec"
                if above(1) == Some("const_declaration") && above(2) == Some("source_file") =>
            {
                Some(Const)
            }
            "var_spec"
                if (above(1) == Some("var_declaration") && above(2) == Some("source_file"))
                    || (above(1) == Some("var_spec_list")
                        && above(2) == Some("var_declaration")
                        && above(3) == Some("source_file")) =>
            {
                Some(Static)
            }
            _ => None,
        },
        Lang::C | Lang::Cpp => {
            let with_body = |kind| node.child_by_field_name("body").map(|_| kind);
            match kind {
                "function_definition" => Some(Fn),
                "struct_specifier" => with_body(Struct),
                "class_specifier" => with_body(Class),
                "union_specifier" => with_body(Union),
                "enum_specifier" => with_body(Enum),
                "enumerator" => Some(Variant),
                "namespace_definition" => Some(Mod),
                _ => None,
            }
        }
        Lang::Java => Some(match kind {
            "class_declaration" | "record_declaration" => Class,
            "interface_declaration" => Interface,
            "enum_declaration" => Enum,
            "enum_constant" => Variant,
            "method_declaration" | "constructor_declaration" => Method,
            _ => return None,
        }),
        Lang::CSharp => Some(match kind {
            "namespace_declaration" | "file_scoped_namespace_declaration" => Mod,
            "class_declaration" | "record_declaration" => Class,
            "struct_declaration" => Struct,
            "interface_declaration" => Interface,
            "enum_declaration" => Enum,
            "enum_member_declaration" => Variant,
            "delegate_declaration" => Type,
            // An interface's bodiless members are signatures; elsewhere a
            // bodiless method (`extern`, `abstract`, `partial`) is the only
            // declaration of its body-less API.
            "method_declaration"
                if node.child_by_field_name("body").is_none()
                    && above(2) == Some("interface_declaration") =>
            {
                return None;
            }
            "method_declaration" | "constructor_declaration" | "destructor_declaration" => Method,
            "local_function_statement" => Fn,
            _ => return None,
        }),
        Lang::FSharp | Lang::FSharpSignature => Some(match kind {
            "namespace" | "named_module" | "module_defn" => Mod,
            "anon_type_defn" | "record_type_defn" => Class,
            "union_type_defn" | "enum_type_defn" => Enum,
            "union_type_case" | "enum_type_case" => Variant,
            "interface_type_defn" => Interface,
            "type_abbrev_defn" | "delegate_type_defn" | "exception_definition" => Type,
            // `type T with …` extends a type defined elsewhere.
            "type_extension" => Impl,
            "method_or_prop_defn" => Method,
            // A `let` function at any depth; a value only at module level.
            "function_or_value_defn" => {
                if named_child_of(node, &["function_declaration_left"]).is_some() {
                    Fn
                } else if above(1) == Some("declaration_expression")
                    && matches!(
                        above(2),
                        Some("module_defn" | "namespace" | "named_module" | "file")
                    )
                {
                    let mutable = named_child_of(node, &["value_declaration_left"])
                        .is_some_and(|left| has_child_kind(left, "mutable"));
                    if mutable { Static } else { Const }
                } else {
                    return None;
                }
            }
            _ => return None,
        }),
        Lang::VbNet => Some(match kind {
            "namespace_block" | "module_block" => Mod,
            "class_block" => Class,
            "structure_block" => Struct,
            "interface_block" => Interface,
            "enum_block" => Enum,
            "enum_member" => Variant,
            "delegate_declaration" => Type,
            "method_declaration" if above(1) == Some("interface_block") => return None,
            "method_declaration" | "constructor_declaration" => Method,
            // A `Module`'s constants and fields are module-level.
            "const_declaration" if above(1) == Some("module_block") => Const,
            "field_declaration"
                if above(1) == Some("module_block")
                    && count_named_up_to(node, &["variable_declarator"], 2) == 1 =>
            {
                Static
            }
            _ => return None,
        }),
        Lang::Php => Some(match kind {
            "namespace_definition" => Mod,
            "class_declaration" => Class,
            "interface_declaration" => Interface,
            "trait_declaration" => Trait,
            "enum_declaration" => Enum,
            "enum_case" => Variant,
            "function_definition" => Fn,
            "method_declaration"
                if node.child_by_field_name("body").is_none()
                    && above(2) == Some("interface_declaration") =>
            {
                return None;
            }
            "method_declaration" => Method,
            // A top-level (or namespace-level) `const`; class constants are
            // members, like fields.
            "const_element"
                if above(1) == Some("const_declaration")
                    && (above(2) == Some("program")
                        || (above(2) == Some("compound_statement")
                            && above(3) == Some("namespace_definition"))) =>
            {
                Const
            }
            _ => return None,
        }),
        Lang::Perl => Some(match kind {
            "package_statement" => Mod,
            "function_definition" | "function_definition_without_sub" => Fn,
            "use_constant_statement" => Const,
            _ => return None,
        }),
        Lang::Bash => match kind {
            "function_definition" => Some(Fn),
            // A script-level assignment: bare, one of several on one line
            // (`A=1 B=2`), or declared, not `local`. An assignment before a
            // command (`A=1 cmd`) only sets that command's environment.
            "variable_assignment"
                if above(1) == Some("program")
                    || (above(1) == Some("variable_assignments")
                        && above(2) == Some("program")) =>
            {
                Some(Static)
            }
            "variable_assignment"
                if above(1) == Some("declaration_command") && above(2) == Some("program") =>
            {
                // The keyword comes first, then the options (bash reads
                // options only before the first name), then the names.
                let declaration = *ancestors.last()?;
                let mut parts = children(declaration);
                let keyword = parts.next().map(|keyword| keyword.kind());
                let read_only = keyword == Some("readonly")
                    || parts.take_while(|part| part.kind() == "word").any(|flag| {
                        text(flag, source)
                            .is_some_and(|flag| flag.starts_with('-') && flag.contains('r'))
                    });
                match keyword {
                    Some("local") => None,
                    _ if read_only => Some(Const),
                    _ => Some(Static),
                }
            }
            _ => None,
        },
        Lang::PowerShell => Some(match kind {
            "function_statement" => Fn,
            "class_statement" => Class,
            "class_method_definition" => Method,
            "enum_statement" => Enum,
            "enum_member" => Variant,
            _ => return None,
        }),
        Lang::Ruby => Some(match kind {
            "module" => Mod,
            "class" => Class,
            "method" | "singleton_method" => Method,
            // A constant assigned at top level or in a class or module body.
            "assignment"
                if node
                    .child_by_field_name("left")
                    .is_some_and(|left| left.kind() == "constant")
                    && (above(1) == Some("program")
                        || (above(1) == Some("body_statement")
                            && matches!(above(2), Some("class" | "module")))) =>
            {
                Const
            }
            _ => return None,
        }),
        Lang::Kotlin => {
            let member = matches!(above(1), Some("class_body" | "enum_class_body"));
            Some(match kind {
                "class_declaration" => {
                    if has_child_kind(node, "interface") {
                        Interface
                    } else if named_child_of(node, &["modifiers"]).is_some_and(|modifiers| {
                        named_children(modifiers).any(|modifier| {
                            modifier.kind() == "class_modifier"
                                && text(modifier, source) == Some("enum")
                        })
                    }) {
                        Enum
                    } else {
                        Class
                    }
                }
                "object_declaration" => Class,
                // An unnamed companion is no unit: its members belong to
                // the class.
                "companion_object" if node.child_by_field_name("name").is_some() => Class,
                "function_declaration" if member => {
                    named_child_of(node, &["function_body"])?;
                    Method
                }
                "function_declaration" => Fn,
                "secondary_constructor" => Method,
                "type_alias" => Type,
                "enum_entry" => Variant,
                "property_declaration" if above(1) == Some("source_file") => {
                    let constant = has_child_kind(node, "val")
                        || named_child_of(node, &["modifiers"])
                            .is_some_and(|modifiers| text(modifiers, source) == Some("const"));
                    if constant { Const } else { Static }
                }
                _ => return None,
            })
        }
        Lang::Swift => {
            let member = matches!(above(1), Some("class_body" | "enum_class_body"));
            Some(match kind {
                "class_declaration" => match node.child_by_field_name("declaration_kind")?.kind() {
                    "struct" => Struct,
                    "enum" => Enum,
                    // An `extension` extends a type defined elsewhere.
                    "extension" => Impl,
                    _ => Class,
                },
                "protocol_declaration" => Interface,
                "function_declaration" if member => Method,
                "function_declaration" => Fn,
                "init_declaration" | "deinit_declaration" => Method,
                "typealias_declaration" => Type,
                // Each name of `case a, b` is a member.
                "simple_identifier" if above(1) == Some("enum_entry") => Variant,
                "property_declaration" if above(1) == Some("source_file") => {
                    swift_binding_kind(node, source)
                }
                _ => return None,
            })
        }
        Lang::Scala => {
            let member = above(1) == Some("template_body")
                && matches!(
                    above(2),
                    Some(
                        "class_definition"
                            | "object_definition"
                            | "trait_definition"
                            | "enum_definition"
                            | "package_object"
                            | "given_definition"
                            | "extension_definition"
                    )
                );
            let top = above(1) == Some("compilation_unit")
                || (above(1) == Some("template_body") && above(2) == Some("package_clause"));
            Some(match kind {
                "package_clause" | "package_object" => Mod,
                "class_definition" | "object_definition" => Class,
                "trait_definition" => Trait,
                "enum_definition" => Enum,
                "simple_enum_case" | "full_enum_case" => Variant,
                "function_definition" if member => Method,
                "function_definition" => Fn,
                "type_definition" => Type,
                "val_definition" if top => Const,
                "var_definition" if top => Static,
                _ => return None,
            })
        }
        Lang::Lua => match kind {
            "function_declaration" => Some(
                if node
                    .child_by_field_name("name")
                    .is_some_and(|name| name.kind() == "method_index_expression")
                {
                    Method
                } else {
                    Fn
                },
            ),
            // One target and one value: a function value anywhere; else a
            // chunk-level (`local` or global) variable.
            "assignment_statement" => {
                let targets = named_child_of(node, &["variable_list"])?;
                let values = named_child_of(node, &["expression_list"])?;
                if targets.named_child_count() != 1 || values.named_child_count() != 1 {
                    return None;
                }
                let function = values
                    .named_child(0)
                    .is_some_and(|value| value.kind() == "function_definition");
                let chunk = above(1) == Some("chunk")
                    || (above(1) == Some("variable_declaration") && above(2) == Some("chunk"));
                if function {
                    Some(Fn)
                } else if chunk
                    && targets
                        .named_child(0)
                        .is_some_and(|target| target.kind() == "identifier")
                {
                    Some(Static)
                } else {
                    None
                }
            }
            _ => None,
        },
        Lang::Dart => Some(match kind {
            "class_declaration" | "extension_type_declaration" => Class,
            "mixin_declaration" => Trait,
            // An `extension … on T` extends a type defined elsewhere.
            "extension_declaration" => Impl,
            "enum_declaration" => Enum,
            "enum_constant" => Variant,
            "type_alias" => Type,
            // A named function at any depth: a block's
            // `local_function_declaration` too (001 T008 review M5).
            "function_declaration" | "local_function_declaration" => Fn,
            "method_declaration" => Method,
            // A bodiless constructor (`A.named();`).
            "constructor_signature"
            | "constant_constructor_signature"
            | "factory_constructor_signature"
            | "redirecting_factory_constructor_signature"
                if above(1) == Some("declaration") =>
            {
                Method
            }
            "static_final_declaration" | "initialized_identifier"
                if above(2) == Some("top_level_variable_declaration") =>
            {
                // `const`/`final` come before the declaration's list.
                let declaration = ancestors[ancestors.len() - 2];
                let constant = children(declaration)
                    .take_while(|part| {
                        !matches!(
                            part.kind(),
                            "static_final_declaration_list" | "initialized_identifier_list"
                        )
                    })
                    .any(|part| matches!(part.kind(), "const" | "final"));
                if constant { Const } else { Static }
            }
            _ => return None,
        }),
        Lang::Elixir => match kind {
            "call" => match elixir_definition(node, source)?.0 {
                ElixirDef::Module => Some(Mod),
                ElixirDef::Protocol => Some(Interface),
                ElixirDef::Implementation => Some(Impl),
                ElixirDef::Macro => Some(Macro),
                ElixirDef::Delegate => Some(Fn),
                // A bodiless head (a protocol function, a default-argument
                // head) is a signature.
                ElixirDef::Function => elixir_body(node, source).map(|_| Fn),
            },
            // A module attribute holding a value (`@timeout 5000`).
            "unary_operator" if above(1) == Some("do_block") => {
                let operand = node.child_by_field_name("operand")?;
                let attribute = text(operand.child_by_field_name("target")?, source)?;
                (text(node.child_by_field_name("operator")?, source) == Some("@")
                    && operand.kind() == "call"
                    && !ELIXIR_RESERVED_ATTRIBUTES.contains(&attribute))
                .then_some(Const)
            }
            _ => None,
        },
        Lang::Haskell => Some(match kind {
            // An equation where declarations stand (top level, a class or
            // instance, `where`/`let` bindings); `function` is also the
            // kind of a function type `a -> b`.
            "function"
                if matches!(
                    above(1),
                    Some(
                        "declarations"
                            | "class_declarations"
                            | "instance_declarations"
                            | "local_binds"
                    )
                ) =>
            {
                Fn
            }
            // A top-level value; a class or instance method's equation.
            "bind" if above(1) == Some("declarations") => Const,
            "bind"
                if matches!(
                    above(1),
                    Some("class_declarations" | "instance_declarations")
                ) =>
            {
                Fn
            }
            "data_type" | "newtype" | "type_synonym" | "type_family" | "data_family" => Type,
            // Constructors of a sum type are its members; a lone constructor
            // shares its type's name and stays part of it. The list is
            // classified once, when the walk enters it ([`Facts`]).
            "data_constructor" if at.facts.last().is_some_and(|facts| facts.constructors) => {
                Variant
            }
            "class" => Interface,
            // An `instance` implements a class for a type defined elsewhere.
            "instance" => Impl,
            _ => return None,
        }),
        _ => None,
    }
}

/// Module attributes that document, type or configure a module rather than
/// hold a named value.
const ELIXIR_RESERVED_ATTRIBUTES: &[&str] = &[
    "after_compile",
    "after_verify",
    "before_compile",
    "behaviour",
    "callback",
    "compile",
    "deprecated",
    "derive",
    "dialyzer",
    "doc",
    "enforce_keys",
    "external_resource",
    "file",
    "impl",
    "macrocallback",
    "moduledoc",
    "on_definition",
    "on_load",
    "opaque",
    "optional_callbacks",
    "spec",
    "type",
    "typedoc",
    "typep",
    "vsn",
];

/// What an Elixir definition call (`target` text) defines.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ElixirDef {
    Module,
    Protocol,
    Implementation,
    Function,
    Macro,
    Delegate,
}

/// An Elixir call that defines something, with its first argument (the
/// module alias or the function head).
fn elixir_definition<'t>(
    call: tree_sitter::Node<'t>,
    source: &[u8],
) -> Option<(ElixirDef, tree_sitter::Node<'t>)> {
    let definition = match text(call.child_by_field_name("target")?, source)? {
        "defmodule" => ElixirDef::Module,
        "defprotocol" => ElixirDef::Protocol,
        "defimpl" => ElixirDef::Implementation,
        "def" | "defp" | "defguard" | "defguardp" | "defn" | "defnp" => ElixirDef::Function,
        "defmacro" | "defmacrop" => ElixirDef::Macro,
        "defdelegate" => ElixirDef::Delegate,
        _ => return None,
    };
    let arguments = named_child_of(call, &["arguments"])?;
    Some((definition, arguments.named_child(0)?))
}

/// An Elixir definition's `do … end` block, or its `do:` keyword pair.
fn elixir_body<'t>(call: tree_sitter::Node<'t>, source: &[u8]) -> Option<tree_sitter::Node<'t>> {
    named_child_of(call, &["do_block"]).or_else(|| {
        let keywords = named_child_of(named_child_of(call, &["arguments"])?, &["keywords"])?;
        named_children(keywords).find(|pair| {
            pair.child_by_field_name("key")
                .and_then(|key| text(key, source))
                .is_some_and(|key| key.trim() == "do:")
        })
    })
}

/// `node`'s children in order, by one cursor walk: tree-sitter's indexed
/// child access rescans the sibling list on each call (001 T008 review M2).
fn children<'t>(node: tree_sitter::Node<'t>) -> impl Iterator<Item = tree_sitter::Node<'t>> {
    let mut cursor = node.walk();
    let mut started = false;
    std::iter::from_fn(move || {
        let moved = if started {
            cursor.goto_next_sibling()
        } else {
            started = true;
            cursor.goto_first_child()
        };
        moved.then(|| cursor.node())
    })
}

fn named_children<'t>(node: tree_sitter::Node<'t>) -> impl Iterator<Item = tree_sitter::Node<'t>> {
    children(node).filter(|child| child.is_named())
}

/// The first named child of one of `kinds`.
fn named_child_of<'t>(
    node: tree_sitter::Node<'t>,
    kinds: &[&str],
) -> Option<tree_sitter::Node<'t>> {
    named_children(node).find(|child| kinds.contains(&child.kind()))
}

/// How many named children are of one of `kinds`, counting no further than
/// `limit`.
fn count_named_up_to(node: tree_sitter::Node, kinds: &[&str], limit: usize) -> usize {
    named_children(node)
        .filter(|child| kinds.contains(&child.kind()))
        .take(limit)
        .count()
}

/// Whether a child (named or not) is of `kind`: an anonymous keyword token
/// such as `mutable` or `interface`.
fn has_child_kind(node: tree_sitter::Node, kind: &str) -> bool {
    children(node).any(|child| child.kind() == kind)
}

fn text<'s>(node: tree_sitter::Node, source: &'s [u8]) -> Option<&'s str> {
    node.utf8_text(source).ok()
}

/// A JavaScript-family declarator with an identifier name: `fn` when its
/// value is a function (anywhere, when its declaration has no other
/// declarator; T005's rule) and, at module level, otherwise `const` for
/// `const` and `static` for `let`/`var`.
fn declarator_kind(at: &Walk, node: tree_sitter::Node) -> Option<UnitKind> {
    let declaration = *at.ancestors.last()?;
    if !matches!(
        declaration.kind(),
        "lexical_declaration" | "variable_declaration"
    ) || node
        .child_by_field_name("name")
        .is_none_or(|name| name.kind() != "identifier")
    {
        return None;
    }
    let module = at.above(2) == Some("program")
        || (at.above(2) == Some("export_statement") && at.above(3) == Some("program"));
    let function = node
        .child_by_field_name("value")
        .is_some_and(|value| matches!(value.kind(), "arrow_function" | "function_expression"));
    // A declaration with one declarator wraps it ([`is_wrapper`]).
    let alone = at.facts.last().is_some_and(|facts| facts.wrapper);
    if function && (module || alone) {
        return Some(UnitKind::Fn);
    }
    if !module {
        return None;
    }
    let constant = declaration
        .child_by_field_name("kind")
        .is_some_and(|kind| kind.kind() == "const");
    Some(if constant {
        UnitKind::Const
    } else {
        UnitKind::Static
    })
}

/// A unit's name (context-v2 § Unit kinds and § Languages): the node the
/// language's name rule selects, and which of it names the unit.
#[derive(Clone, Copy)]
struct Named<'t> {
    node: tree_sitter::Node<'t>,
    form: NameForm<'t>,
}

#[derive(Clone, Copy)]
enum NameForm<'t> {
    /// The whole node; a quoted JavaScript-family name is the text inside
    /// its quotes ([`name_span`]).
    Whole,
    /// The node's last address part ([`name_parts`]); the parts before it
    /// qualify the unit (`Store` of Ruby's `Outer::Inner::Store`, `make` of
    /// Lua's `M.inner:make`, `Models` of PHP's `App\Models`).
    Last,
    /// A byte range the grammar keeps no node for (VB.NET's `New`).
    Range(usize, usize),
    /// The whole node, qualified by a receiver type (a Kotlin extension
    /// function's `String` of `fun String.shout()`).
    Receiver(tree_sitter::Node<'t>),
}

impl<'t> Named<'t> {
    fn whole(node: tree_sitter::Node<'t>) -> Self {
        Self {
            node,
            form: NameForm::Whole,
        }
    }

    fn last(node: tree_sitter::Node<'t>) -> Self {
        Self {
            node,
            form: NameForm::Last,
        }
    }
}

/// What a [`Named`] gives its candidate: the name's byte range; the
/// qualifier its qualified name puts before the name (`Outer::Inner` of
/// Ruby's `Outer::Inner::Store`, joined with the language's separator; a
/// Kotlin receiver as written); and the name's own address segments,
/// lowercased (context-v2 § Definitions and addresses), all read from the
/// syntax tree. An outline-only analysis computes no address segments.
struct Resolved {
    span: (usize, usize),
    qualifier: Option<String>,
    address: Vec<String>,
}

fn resolve_name(lang: Lang, named: Named, source: &[u8], need: Need) -> Option<Resolved> {
    let text = |(start, end): (usize, usize)| std::str::from_utf8(source.get(start..end)?).ok();
    let addresses = need != Need::Outline;
    let lowercase = |parts: &[(usize, usize)]| -> Vec<String> {
        parts
            .iter()
            .filter_map(|&part| text(part))
            .map(str::to_lowercase)
            .collect()
    };
    let whole = name_span(lang, named.node);
    let mut resolved = match named.form {
        NameForm::Whole => Resolved {
            span: whole,
            qualifier: None,
            address: if addresses {
                lowercase(&name_parts(lang, named.node, source))
            } else {
                Vec::new()
            },
        },
        NameForm::Last => {
            let parts = name_parts(lang, named.node, source);
            match parts.split_last() {
                Some((&last, scope)) => Resolved {
                    span: last,
                    qualifier: (!scope.is_empty()).then(|| {
                        let scope: Vec<&str> =
                            scope.iter().filter_map(|&part| text(part)).collect();
                        scope.join(lang.qname_separator())
                    }),
                    address: if addresses {
                        lowercase(&parts)
                    } else {
                        Vec::new()
                    },
                },
                // A name with no address parts (Lua's `M["x"]`) is whole.
                None => Resolved {
                    span: whole,
                    qualifier: None,
                    address: Vec::new(),
                },
            }
        }
        NameForm::Range(start, end) => Resolved {
            span: (start, end),
            qualifier: None,
            address: if addresses {
                lowercase(&[(start, end)])
            } else {
                Vec::new()
            },
        },
        NameForm::Receiver(receiver) => Resolved {
            span: whole,
            qualifier: text((receiver.start_byte(), receiver.end_byte())).map(str::to_owned),
            address: if addresses {
                let mut parts = name_parts(lang, receiver, source);
                parts.extend(name_parts(lang, named.node, source));
                lowercase(&parts)
            } else {
                Vec::new()
            },
        },
    };
    text(resolved.span)?;
    // The unit's own segment is its stored name's (no `?`/`!`/`'` suffix).
    if let Some(last) = resolved.address.last_mut() {
        let stripped = definition_name(lang, last);
        if stripped.len() < last.len() {
            *last = stripped.to_owned();
        }
    }
    Some(resolved)
}

/// The name the language's rule selects (context-v2 § Unit kinds and
/// § Languages): the `name` field; for a Rust `impl` the `type` field; for
/// C/C++ the innermost identifier of the declarator chain; a bare TypeScript
/// enum member is its own name; a Python assignment's left identifier; and
/// the per-kind rules of the languages whose definitions have no `name`
/// field or a qualified one.
fn unit_name<'t>(lang: Lang, node: tree_sitter::Node<'t>, source: &[u8]) -> Option<Named<'t>> {
    let field = |name: &str| node.child_by_field_name(name);
    let whole = Named::whole;
    match (lang, node.kind()) {
        (Lang::Rust, "impl_item") => field("type").map(whole),
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "property_identifier" | "string") => {
            Some(whole(node))
        }
        (Lang::Python, "assignment") => field("left").map(whole),
        (Lang::C | Lang::Cpp, "function_definition") => {
            Some(whole(innermost_declarator(field("declarator")?)))
        }
        (Lang::CSharp, "namespace_declaration" | "file_scoped_namespace_declaration") => {
            field("name").map(Named::last)
        }
        (Lang::FSharp | Lang::FSharpSignature, kind) => match kind {
            "namespace" | "named_module" => field("name").map(Named::last),
            "exception_definition" => field("exception_name").map(Named::last),
            "module_defn" | "union_type_case" | "enum_type_case" => {
                named_child_of(node, &["identifier"]).map(whole)
            }
            // The extended type, dotted or not: an `impl`'s name.
            "type_extension" => named_child_of(node, &["type_name"]).map(whole),
            "anon_type_defn"
            | "record_type_defn"
            | "union_type_defn"
            | "enum_type_defn"
            | "interface_type_defn"
            | "type_abbrev_defn"
            | "delegate_type_defn" => named_child_of(node, &["type_name"])?
                .child_by_field_name("type_name")
                .map(whole),
            "function_or_value_defn" => {
                match named_child_of(node, &["function_declaration_left"]) {
                    Some(left) => named_child_of(left, &["identifier"]).map(whole),
                    None => {
                        let pattern = named_child_of(
                            named_child_of(node, &["value_declaration_left"])?,
                            &["identifier_pattern"],
                        )?;
                        let name = named_child_of(pattern, &["long_identifier_or_op"])?;
                        named_child_of(name, &["identifier"]).map(whole)
                    }
                }
            }
            "method_or_prop_defn" => {
                let name = field("name")?;
                match name.kind() {
                    "property_or_ident" => name.child_by_field_name("method").map(whole),
                    _ => Some(whole(name)),
                }
            }
            _ => field("name").map(whole),
        },
        (Lang::VbNet, "namespace_block") => field("name").map(Named::last),
        (Lang::VbNet, "field_declaration") => named_child_of(node, &["variable_declarator"])?
            .child_by_field_name("name")
            .map(whole),
        // `Sub New`: the grammar keeps no keyword node, so the name is the
        // `New` before the parameter list.
        (Lang::VbNet, "constructor_declaration") => {
            let from = field("modifiers").map_or(node.start_byte(), |m| m.end_byte());
            let to = field("parameters").map_or(node.end_byte(), |p| p.start_byte());
            let header =
                text(node, source)?.get(from - node.start_byte()..to - node.start_byte())?;
            let at = header.to_ascii_lowercase().rfind("new")?;
            let start = from + at;
            Some(Named {
                node,
                form: NameForm::Range(start, start + 3),
            })
        }
        (Lang::Php, "namespace_definition") => field("name").map(Named::last),
        (Lang::Php, "const_element") => named_child_of(node, &["name"]).map(whole),
        (Lang::Perl, "package_statement") => {
            named_child_of(node, &["package_name"]).map(Named::last)
        }
        (Lang::Perl, "use_constant_statement") => field("constant").map(whole),
        (Lang::PowerShell, "function_statement") => {
            named_child_of(node, &["function_name"]).map(whole)
        }
        (Lang::PowerShell, _) => named_child_of(node, &["simple_name"]).map(whole),
        (Lang::Ruby, "class" | "module") => field("name").map(Named::last),
        // A setter `name=` is named `name`.
        (Lang::Ruby, "method" | "singleton_method") => {
            let name = field("name")?;
            match name.kind() {
                "setter" => name.child_by_field_name("name").map(whole),
                _ => Some(whole(name)),
            }
        }
        (Lang::Ruby, "assignment") => field("left").map(whole),
        (Lang::Kotlin, "type_alias") => field("type").map(whole),
        (Lang::Kotlin, "enum_entry") => named_child_of(node, &["identifier"]).map(whole),
        (Lang::Kotlin, "secondary_constructor") => children(node)
            .find(|child| child.kind() == "constructor")
            .map(whole),
        (Lang::Kotlin, "property_declaration") => named_child_of(
            named_child_of(node, &["variable_declaration"])?,
            &["identifier"],
        )
        .map(whole),
        // An extension function keeps its receiver type as a qualifier.
        (Lang::Kotlin, "function_declaration") => {
            let name = field("name")?;
            let receiver = named_children(node)
                .take_while(|child| child.end_byte() <= name.start_byte())
                .filter(|child| matches!(child.kind(), "user_type" | "nullable_type"))
                .last();
            Some(Named {
                node: name,
                form: receiver.map_or(NameForm::Whole, NameForm::Receiver),
            })
        }
        (Lang::Swift, "simple_identifier") => Some(whole(node)),
        (Lang::Swift, "deinit_declaration") => children(node)
            .find(|child| child.kind() == "deinit")
            .map(whole),
        (Lang::Swift, "property_declaration") => field("name")?
            .child_by_field_name("bound_identifier")
            .map(whole),
        (Lang::Scala, "package_clause") => field("name").map(Named::last),
        (Lang::Scala, "val_definition" | "var_definition") => field("pattern")
            .filter(|pattern| pattern.kind() == "identifier")
            .map(whole),
        (Lang::Lua, "function_declaration") => field("name").map(Named::last),
        (Lang::Lua, "assignment_statement") => named_child_of(node, &["variable_list"])?
            .child_by_field_name("name")
            .map(Named::last),
        (Lang::Dart, "type_alias") => named_child_of(node, &["type_identifier"]).map(whole),
        (Lang::Dart, "extension_declaration") => field("class").map(whole),
        (Lang::Dart, "function_declaration") => dart_signature_name(field("signature")?),
        (Lang::Dart, "local_function_declaration") => {
            dart_signature_name(named_child_of(node, &["function_signature"])?)
        }
        (Lang::Dart, "method_declaration") => {
            dart_signature_name(field("signature")?.named_child(0)?)
        }
        (
            Lang::Dart,
            "constructor_signature"
            | "constant_constructor_signature"
            | "factory_constructor_signature"
            | "redirecting_factory_constructor_signature",
        ) => dart_signature_name(node),
        (Lang::Elixir, "call") => {
            let (definition, first) = elixir_definition(node, source)?;
            match definition {
                ElixirDef::Module | ElixirDef::Protocol => Some(Named::last(first)),
                // The implementing type (`for:`), else the protocol.
                ElixirDef::Implementation => {
                    let target = named_child_of(node, &["arguments"])
                        .and_then(|arguments| named_child_of(arguments, &["keywords"]))
                        .and_then(|keywords| {
                            named_children(keywords).find(|pair| {
                                pair.child_by_field_name("key")
                                    .and_then(|key| text(key, source))
                                    .is_some_and(|key| key.trim() == "for:")
                            })
                        })
                        .and_then(|pair| pair.child_by_field_name("value"));
                    Some(whole(target.unwrap_or(first)))
                }
                ElixirDef::Function | ElixirDef::Macro | ElixirDef::Delegate => {
                    // `f(a)`, `f(a) when guard`, or a zero-arity `f`.
                    let head = match first.kind() {
                        "binary_operator" => first.child_by_field_name("left")?,
                        _ => first,
                    };
                    match head.kind() {
                        "call" => head.child_by_field_name("target").map(whole),
                        "identifier" => Some(whole(head)),
                        _ => None,
                    }
                }
            }
        }
        (Lang::Elixir, "unary_operator") => {
            field("operand")?.child_by_field_name("target").map(whole)
        }
        // An infix equation `a <+> b = …` is named by its operator.
        (Lang::Haskell, "function") => field("name").map(whole).or_else(|| {
            named_child_of(node, &["infix"])?
                .child_by_field_name("operator")
                .map(whole)
        }),
        (Lang::Haskell, "data_constructor") => {
            field("constructor")?.child_by_field_name("name").map(whole)
        }
        // The type an instance is for: an `impl`'s name.
        (Lang::Haskell, "instance") => field("patterns").map(whole),
        _ => field("name").map(whole),
    }
}

/// A name node's byte range: the node's, except that a quoted JavaScript-
/// family name (`"Fast"`, `'get\u0056alue'`) is the source text inside its
/// quotes, as written, escapes and all.
fn name_span(lang: Lang, name: tree_sitter::Node) -> (usize, usize) {
    let (start, end) = (name.start_byte(), name.end_byte());
    match (lang, name.kind()) {
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "string") if end - start >= 2 => {
            (start + 1, end - 1)
        }
        _ => (start, end),
    }
}

/// A name node's address parts (context-v2 § Definitions and addresses),
/// as byte ranges in order, read from the syntax tree, never from the name's
/// text: a plain name is itself, a quoted one the text inside its quotes
/// ([`name_span`]); a generic type or template (`Mapper<fn() -> u8>`,
/// `Box<1 << 2>`, F#'s `List<'T>`) is its base's, so its argument list is
/// never read; a scoped or qualified name (`a::b::Foo`, `ns::Box`, C#'s
/// `Outer.Space`, Ruby's `Outer::Inner`, Lua's `M.inner:make`, PHP's
/// `App\Models`, a Kotlin or Swift `Outer.Inner<T>` type) is its parts' in
/// order; a reference, pointer or nullable type is its referent's; an
/// Elixir alias (`Shapes.Inner`, one token) is each of its dotted segments;
/// any other node (a tuple, an array, a function or trait-object type, a
/// type argument list) has none. The walk keeps its pending nodes on the
/// heap: a path's nesting is bounded only by the source.
fn name_parts(lang: Lang, name: tree_sitter::Node, source: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pending = vec![name];
    while let Some(node) = pending.pop() {
        match node.kind() {
            "generic_type" | "template_type" | "template_function" | "template_method" => {
                let base = node
                    .child_by_field_name("type")
                    .or_else(|| node.child_by_field_name("name"))
                    .or_else(|| node.named_child(0));
                pending.extend(base);
            }
            // F#: the `type_name` field of a type's head (`List<'T>`).
            "type_name" if matches!(lang, Lang::FSharp | Lang::FSharpSignature) => {
                pending.extend(node.child_by_field_name("type_name"));
            }
            "scoped_type_identifier"
            | "scoped_identifier"
            | "qualified_identifier"
            | "nested_type_identifier"
            | "nested_namespace_specifier"
            | "nested_identifier"
            | "qualified_type"
            | "qualified_name"
            | "long_identifier"
            | "namespace_name"
            | "package_name"
            | "package_identifier"
            | "scope_resolution"
            | "dot_index_expression"
            | "method_index_expression"
            | "user_type" => {
                let mut cursor = node.walk();
                let parts: Vec<_> = node
                    .named_children(&mut cursor)
                    .filter(|part| !part.kind().contains("comment"))
                    .collect();
                pending.extend(parts.into_iter().rev());
            }
            "reference_type" | "pointer_type" => pending.extend(node.child_by_field_name("type")),
            // Dart: a type's name before its arguments (`List` of
            // `List<int>`).
            "type" if lang == Lang::Dart => pending.extend(node.named_child(0)),
            // Haskell: an instance's first type, and of an applied type its
            // constructor (`Maybe` of `(Maybe a)`).
            "type_patterns" | "parens" if lang == Lang::Haskell => {
                pending.extend(node.named_child(0));
            }
            "apply" if lang == Lang::Haskell => {
                pending.extend(node.child_by_field_name("constructor"));
            }
            // Elixir: `__MODULE__.Inner`.
            "dot" if lang == Lang::Elixir => {
                pending.extend(node.child_by_field_name("right"));
                pending.extend(node.child_by_field_name("left"));
            }
            "nullable_type" => {
                let mut cursor = node.walk();
                let referent = node
                    .named_children(&mut cursor)
                    .find(|part| part.kind() != "type_modifiers");
                pending.extend(referent);
            }
            "alias" if lang == Lang::Elixir => {
                let mut at = node.start_byte();
                for segment in source[at..node.end_byte()].split(|&b| b == b'.') {
                    let lead = segment
                        .iter()
                        .take_while(|b| b.is_ascii_whitespace())
                        .count();
                    let trail = segment[lead..]
                        .iter()
                        .rev()
                        .take_while(|b| b.is_ascii_whitespace())
                        .count();
                    if lead + trail < segment.len() {
                        out.push((at + lead, at + segment.len() - trail));
                    }
                    at += segment.len() + 1;
                }
            }
            _ if node.named_child_count() == 0 || node.kind() == "string" => {
                let (start, end) = name_span(lang, node);
                if start < end && std::str::from_utf8(&source[start..end]).is_ok() {
                    out.push((start, end));
                }
            }
            _ => {}
        }
    }
    out
}

/// A Dart signature's name: its last named `name` child (`make` of
/// `factory A.make()`); a method signature's inner signature first.
fn dart_signature_name(signature: tree_sitter::Node) -> Option<Named> {
    let mut cursor = signature.walk();
    let name = signature
        .children_by_field_name("name", &mut cursor)
        .filter(|name| name.is_named())
        .last();
    name.map(Named::whole)
}

/// C/C++: the innermost identifier of the declarator chain (through pointer,
/// function, reference, parenthesized and qualified declarators).
fn innermost_declarator(mut node: tree_sitter::Node) -> tree_sitter::Node {
    loop {
        if let Some(inner) = node.child_by_field_name("declarator") {
            node = inner;
        } else if node.kind() == "qualified_identifier"
            && let Some(name) = node.child_by_field_name("name")
        {
            node = name;
        } else if matches!(
            node.kind(),
            "reference_declarator" | "parenthesized_declarator"
        ) && let Some(inner) = unnamed_declarator(node)
        {
            node = inner;
        } else {
            return node;
        }
    }
}

/// The nested declarator of a declarator that holds it without a field name
/// (`&`/`&&` or parentheses around it): its last named child that is not a
/// comment or a call modifier.
fn unnamed_declarator(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| !matches!(child.kind(), "comment" | "ms_call_modifier"))
        .last()
}

fn body_range(lang: Lang, node: tree_sitter::Node, source: &[u8]) -> Option<(usize, usize)> {
    let field = |name: &str| node.child_by_field_name(name);
    let span = |body: tree_sitter::Node| Some((body.start_byte(), body.end_byte()));
    match (lang, node.kind()) {
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "variable_declarator") => span(
            field("value")
                .filter(|value| matches!(value.kind(), "arrow_function" | "function_expression"))?
                .child_by_field_name("body")?,
        ),
        // F#: members follow the header; types and modules hold `block`s.
        (Lang::FSharp | Lang::FSharpSignature, kind) => match kind {
            "namespace" | "named_module" => Some((field("name")?.end_byte(), node.end_byte())),
            "function_or_value_defn" => span(field("body")?),
            "method_or_prop_defn" => Some((
                field("args").or_else(|| field("name"))?.end_byte(),
                node.end_byte(),
            )),
            "type_extension" => span(named_child_of(node, &["type_extension_elements"])?),
            _ => {
                let mut cursor = node.walk();
                let blocks: Vec<_> = node.children_by_field_name("block", &mut cursor).collect();
                Some((blocks.first()?.start_byte(), blocks.last()?.end_byte()))
            }
        },
        // VB.NET: the lines between a header and its `End` line.
        (Lang::VbNet, _) => {
            let header_end = ["name", "parameters", "return_type", "type_parameters"]
                .iter()
                .filter_map(|name| field(name))
                .map(|part| part.end_byte())
                .max()?;
            interior_lines(source, header_end, node.end_byte())
        }
        (Lang::PowerShell, _) => {
            let braces: Vec<_> = children(node)
                .filter(|child| matches!(child.kind(), "{" | "}"))
                .collect();
            let (open, close) = (braces.first()?, braces.last()?);
            (open.kind() == "{" && close.kind() == "}" && open.start_byte() < close.start_byte())
                .then(|| (open.start_byte(), close.end_byte()))
        }
        (Lang::Kotlin, _) => span(named_child_of(
            node,
            &["class_body", "enum_class_body", "function_body", "block"],
        )?),
        (Lang::Lua, "assignment_statement") => span(
            named_child_of(
                named_child_of(node, &["expression_list"])?,
                &["function_definition"],
            )?
            .child_by_field_name("body")?,
        ),
        (Lang::Dart, "local_function_declaration") => {
            span(named_child_of(node, &["function_body"])?)
        }
        (Lang::Elixir, "call") => {
            let block = named_child_of(node, &["do_block"])?;
            let open = block.child(0).filter(|open| open.kind() == "do")?;
            interior_lines(source, open.end_byte(), block.end_byte())
        }
        (Lang::Elixir, _) => None,
        (Lang::Haskell, "function" | "bind") => {
            Some((field("match")?.start_byte(), node.end_byte()))
        }
        (Lang::Haskell, "data_type") => span(field("constructors")?),
        (Lang::Haskell, "newtype") => span(field("constructor")?),
        (Lang::Haskell, "class" | "instance") => span(field("declarations")?),
        (Lang::Haskell, _) => None,
        _ => span(field("body").or_else(|| {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "block" | "declaration_list"))
        })?),
    }
}

/// A keyword-closed body: from the end of the header's line to the start of
/// the closing line (`End Sub`, `end`), when lines lie between them.
fn interior_lines(source: &[u8], header_end: usize, end: usize) -> Option<(usize, usize)> {
    let start = header_end
        + source
            .get(header_end..end)?
            .iter()
            .position(|&b| b == b'\n')?;
    let content_end = source[..end]
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())?
        + 1;
    let close = source[..content_end].iter().rposition(|&b| b == b'\n')? + 1;
    (start < close).then_some((start, close))
}

/// Whether `node` wraps the unit directly inside it, supplying its range: a
/// decorator or Python expression statement, an `export`, a C++ `template`,
/// a JavaScript-family declaration with one declarator, a Go declaration
/// with one spec — for a grouped `var ( … )`, through the `var_spec_list`
/// the grammar puts between the declaration and its specs — and the new
/// languages' declaration and modifier wrappers (an F# module-level `let`
/// or attributed type, a VB.NET type declaration, a PHP `const` or shell
/// declaration with one name, Ruby's `private def`, a Swift `case` or Scala
/// `case` with one name, a Lua `local`, a Dart class member or top-level
/// variable with one name).
fn is_wrapper(lang: Lang, node: tree_sitter::Node) -> bool {
    let single = |kinds: &[&str]| count_named_up_to(node, kinds, 2) == 1;
    let one_method = |list: tree_sitter::Node| {
        list.named_child_count() == 1
            && list
                .named_child(0)
                .is_some_and(|only| matches!(only.kind(), "method" | "singleton_method"))
    };
    match (lang, node.kind()) {
        (Lang::Python, "decorated_definition" | "expression_statement") => true,
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "export_statement") => true,
        (
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript,
            "lexical_declaration" | "variable_declaration",
        ) => single(&["variable_declarator"]),
        (Lang::Go, "type_declaration" | "const_declaration") => {
            single(&["type_spec", "type_alias", "const_spec"])
        }
        (Lang::Go, "var_spec_list") => single(&["var_spec"]),
        (Lang::Go, "var_declaration") => {
            let mut cursor = node.walk();
            let mut specs = node
                .named_children(&mut cursor)
                .filter(|child| matches!(child.kind(), "var_spec" | "var_spec_list"));
            match (specs.next(), specs.next()) {
                (Some(spec), None) => spec.kind() == "var_spec" || is_wrapper(lang, spec),
                _ => false,
            }
        }
        (Lang::Cpp, "template_declaration") => true,
        // A local `let … in …` continues past its binding; a module-level
        // one holds just its attributes and binding.
        (Lang::FSharp | Lang::FSharpSignature, "declaration_expression") => {
            node.child_by_field_name("in").is_none()
        }
        (Lang::FSharp | Lang::FSharpSignature, "type_definition") => {
            named_children(node)
                .filter(|child| child.kind().ends_with("_defn") || child.kind() == "type_extension")
                .take(2)
                .count()
                == 1
        }
        (Lang::FSharp | Lang::FSharpSignature, "member_defn") => true,
        (Lang::VbNet, "type_declaration") => true,
        (Lang::Php, "const_declaration") => single(&["const_element"]),
        (Lang::Bash, "declaration_command") => single(&["variable_assignment"]),
        (Lang::Ruby, "argument_list") => one_method(node),
        (Lang::Ruby, "call") => {
            node.child_by_field_name("receiver").is_none()
                && node.child_by_field_name("block").is_none()
                && node
                    .child_by_field_name("arguments")
                    .is_some_and(|arguments| {
                        arguments.kind() == "argument_list" && one_method(arguments)
                    })
        }
        (Lang::Swift, "enum_entry") => single(&["simple_identifier"]),
        (Lang::Scala, "enum_case_definitions") => single(&["simple_enum_case", "full_enum_case"]),
        (Lang::Lua, "variable_declaration") => true,
        (Lang::Dart, "class_member") => true,
        (Lang::Dart, "declaration") => node.named_child(0).is_some_and(|first| {
            matches!(
                first.kind(),
                "constructor_signature"
                    | "constant_constructor_signature"
                    | "factory_constructor_signature"
                    | "redirecting_factory_constructor_signature"
            )
        }),
        (Lang::Dart, "top_level_variable_declaration") => named_child_of(
            node,
            &[
                "static_final_declaration_list",
                "initialized_identifier_list",
            ],
        )
        .is_some_and(|list| list.named_child_count() == 1),
        (Lang::Dart, "static_final_declaration_list" | "initialized_identifier_list") => {
            node.named_child_count() == 1
        }
        _ => false,
    }
}

/// Markdown heading sections: from a heading to the next heading of equal or
/// higher rank (lower or equal level number), or EOF. Headings inside code
/// fences are not headings.
fn markdown_sections(source: &str) -> Vec<Candidate> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    // (level, start, end of the heading itself, text)
    let mut headings: Vec<(usize, usize, usize, String)> = Vec::new();
    let mut open: Option<(usize, usize, String)> = None;
    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                open = Some((level as usize, range.start, String::new()));
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, _, heading)) = open.as_mut() {
                    heading.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start, text)) = open.take() {
                    headings.push((level, start, range.end, text));
                }
            }
            _ => {}
        }
    }
    headings
        .iter()
        .enumerate()
        .map(|(index, (level, start, heading_end, text))| {
            let end = headings[index + 1..]
                .iter()
                .find(|next| next.0 <= *level)
                .map_or(source.len(), |next| next.1);
            let name = cut_utf8(text.trim(), NAME_BYTES);
            Candidate {
                start: *start,
                decl: *start,
                head: *start,
                end,
                kind: UnitKind::Section,
                name: (!name.is_empty()).then(|| name.to_owned()),
                qualifier: None,
                name_range: None,
                address: Vec::new(),
                body: (*heading_end < end).then_some((*heading_end, end)),
                open: None,
            }
        })
        .collect()
}

fn cut_utf8(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut cut = limit;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

/// Builds the forest: a unit strictly nested in another is a child of its
/// nearest enclosing unit; zero-width or invalid ranges, a duplicate of an
/// identical range and the later of two partially overlapping ranges are not
/// units. An outline-only analysis propagates no address qualifiers.
fn forest(source: &str, lang: Lang, need: Need, mut candidates: Vec<Candidate>) -> Vec<Unit> {
    candidates.retain(|c| {
        c.start < c.end
            && c.end <= source.len()
            && source.is_char_boundary(c.start)
            && source.is_char_boundary(c.end)
    });
    // Outer first among equal starts; the sort is stable, so of two identical
    // ranges the one met first in the walk (the outer node) is kept.
    candidates.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut units: Vec<Unit> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    // Per unit: the unit itself when named, else its nearest named ancestor.
    let mut nearest_named: Vec<Option<usize>> = Vec::new();
    // Per unit: the last of its name's address segments (its own name's;
    // the scope before it is already among its qualifiers).
    let mut lasts: Vec<Option<String>> = Vec::new();
    for candidate in candidates {
        while stack
            .last()
            .is_some_and(|&top| units[top].end <= candidate.start)
        {
            stack.pop();
        }
        if let Some(&top) = stack.last() {
            let enclosing = &units[top];
            let partial = candidate.end > enclosing.end;
            let duplicate = candidate.start == enclosing.start && candidate.end == enclosing.end;
            if partial || duplicate {
                continue;
            }
        }
        let parent = stack.last().copied();
        let enclosing_named = parent.and_then(|parent| nearest_named[parent]);
        let separator = lang.qname_separator();
        let qname = candidate.name.as_deref().map(|name| {
            let prefix = enclosing_named.and_then(|named| units[named].qname.as_deref());
            match candidate.qualifier.as_deref() {
                Some(qualifier) => {
                    qualified(prefix, separator, &format!("{qualifier}{separator}{name}"))
                }
                None => qualified(prefix, separator, name),
            }
        });
        // The qualified name's address segments minus the unit's own name:
        // the enclosing named unit's qualifiers and own segment, then the
        // scope of the unit's own name (`ns` of `ns::Box`).
        let mut address = candidate.address;
        let last = address.pop();
        let qualifiers = match candidate.name {
            None => Vec::new(),
            Some(_) if need == Need::Outline => Vec::new(),
            Some(_) => {
                let mut qualifiers = enclosing_named.map_or_else(Vec::new, |named| {
                    let mut inherited = units[named].qualifiers.clone();
                    inherited.extend(lasts[named].iter().cloned());
                    inherited
                });
                qualifiers.append(&mut address);
                keep_qualifier_tail(&mut qualifiers);
                qualifiers
            }
        };
        let index = units.len();
        nearest_named.push(match candidate.name {
            Some(_) => Some(index),
            None => enclosing_named,
        });
        lasts.push(last);
        units.push(Unit {
            kind: candidate.kind,
            name: candidate.name,
            qname,
            name_range: candidate.name_range,
            qualifiers,
            start: candidate.start,
            decl: candidate.decl,
            head: candidate.head,
            end: candidate.end,
            body: candidate.body,
            parent,
            children: Vec::new(),
        });
        if let Some(parent) = parent {
            units[parent].children.push(index);
        }
        stack.push(index);
    }
    units
}

/// `prefix`, the separator and `name` joined, keeping the last
/// [`QNAME_BYTES`] bytes from a UTF-8 boundary. The tail of a kept tail plus
/// the separator and name is the tail of the whole join, so building from the
/// parent's kept qualified name is exact.
fn qualified(prefix: Option<&str>, separator: &str, name: &str) -> String {
    let mut joined = String::with_capacity(
        prefix.map_or(0, |prefix| prefix.len() + separator.len()) + name.len(),
    );
    if let Some(prefix) = prefix {
        joined.push_str(prefix);
        joined.push_str(separator);
    }
    joined.push_str(name);
    if joined.len() > QNAME_BYTES {
        let mut start = joined.len() - QNAME_BYTES;
        while !joined.is_char_boundary(start) {
            start += 1;
        }
        joined.drain(..start);
    }
    joined
}

/// A unit keeps at most this many qualifiers, its innermost (001 T008 review
/// R5): a byte bound alone lets one-letter names keep 256 strings per unit,
/// copied into every unit, delivery unit and document of a deep nesting.
const QUALIFIERS: usize = 16;

/// Keeps the innermost qualifiers, at most [`QUALIFIERS`] of them and within
/// [`QNAME_BYTES`] bytes in all, as a qualified name keeps its tail: deep
/// named nesting stays linear.
fn keep_qualifier_tail(qualifiers: &mut Vec<String>) {
    let mut bytes = 0usize;
    let kept = qualifiers
        .iter()
        .rev()
        .take(QUALIFIERS)
        .take_while(|qualifier| {
            bytes += qualifier.len();
            bytes <= QNAME_BYTES
        })
        .count();
    qualifiers.drain(..qualifiers.len() - kept);
}

fn delivery(unit: &Unit) -> DeliveryUnit {
    DeliveryUnit {
        start: unit.start,
        head: unit.head,
        end: unit.end,
        kind: unit.kind,
        name: unit.name.clone(),
        qname: unit.qname.clone(),
        name_range: unit.name_range,
        qualifiers: unit.qualifiers.clone(),
    }
}

/// A leaf is one region; a container's bytes minus its direct children form
/// residual regions delivered as the container. The walk keeps its frames on
/// the heap: unit nesting depth is bounded only by the source size.
fn unit_documents(source: &str, units: &[Unit], root: usize, out: &mut Vec<Document>) {
    struct Frame {
        unit: usize,
        next_child: usize,
        cursor: usize,
        own: DeliveryUnit,
    }
    let frame = |index: usize| Frame {
        unit: index,
        next_child: 0,
        cursor: units[index].start,
        own: delivery(&units[index]),
    };
    let mut stack = vec![frame(root)];
    while let Some(top) = stack.last_mut() {
        let unit = &units[top.unit];
        if let Some(&child) = unit.children.get(top.next_child) {
            region(source, top.cursor, units[child].start, &top.own, out);
            top.next_child += 1;
            top.cursor = units[child].end;
            stack.push(frame(child));
        } else {
            region(source, top.cursor, unit.end, &top.own, out);
            stack.pop();
        }
    }
}

/// Bytes outside all top-level units: split after each run of blank lines,
/// then merged in order up to [`BLOCK_BYTES`]; each block is its own
/// delivery unit.
fn blocks(source: &str, start: usize, end: usize, out: &mut Vec<Document>) {
    if start >= end {
        return;
    }
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut piece_start = start;
    let mut offset = start;
    let mut previous_blank = false;
    for line in source[start..end].split_inclusive('\n') {
        let blank = line.trim().is_empty();
        if previous_blank && !blank {
            pieces.push((piece_start, offset));
            piece_start = offset;
        }
        previous_blank = blank;
        offset += line.len();
    }
    pieces.push((piece_start, end));
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (piece_start, piece_end) in pieces {
        match merged.last_mut() {
            Some(last) if piece_end - last.0 <= BLOCK_BYTES => last.1 = piece_end,
            _ => merged.push((piece_start, piece_end)),
        }
    }
    for (block_start, block_end) in merged {
        let unit = DeliveryUnit {
            start: block_start,
            head: block_start,
            end: block_end,
            kind: UnitKind::Block,
            name: None,
            qname: None,
            name_range: None,
            qualifiers: Vec::new(),
        };
        region(source, block_start, block_end, &unit, out);
    }
}

/// One region: no document when whitespace-only; over [`SPLIT_OVER_BYTES`] it
/// splits at line boundaries into parts of at most [`PART_BYTES`] (a longer
/// single line at UTF-8 boundaries), each keeping the region's delivery unit.
fn region(source: &str, start: usize, end: usize, unit: &DeliveryUnit, out: &mut Vec<Document>) {
    if start >= end || source[start..end].trim().is_empty() {
        return;
    }
    let mut push = |start: usize, end: usize| {
        if !source[start..end].trim().is_empty() {
            out.push(Document {
                start,
                end,
                unit: unit.clone(),
            });
        }
    };
    if end - start <= SPLIT_OVER_BYTES {
        push(start, end);
        return;
    }
    let mut part_start = start;
    let mut offset = start;
    for line in source[start..end].split_inclusive('\n') {
        let line_end = offset + line.len();
        if line.len() > PART_BYTES {
            push(part_start, offset);
            let mut chunk_start = offset;
            while line_end - chunk_start > PART_BYTES {
                let mut cut = chunk_start + PART_BYTES;
                while !source.is_char_boundary(cut) {
                    cut -= 1;
                }
                push(chunk_start, cut);
                chunk_start = cut;
            }
            part_start = chunk_start;
        } else if line_end - part_start > PART_BYTES {
            push(part_start, offset);
            part_start = offset;
        }
        offset = line_end;
    }
    push(part_start, end);
}

/// The `foundry_code` analysis without its length filter: byte ranges of the
/// subtokens of `text`, split on non-alphanumerics (including `_`), at
/// lower→Upper and Upper→Upper+lower boundaries (`HTTPServer` → `HTTP`,
/// `Server`) and at letter↔digit boundaries. Callers lowercase and keep
/// subtokens of at least 2 characters.
pub fn code_subtokens(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let end_of = |index: usize| chars.get(index).map_or(text.len(), |(offset, _)| *offset);
    let mut run_start: Option<usize> = None;
    for index in 0..=chars.len() {
        let alphanumeric = chars.get(index).is_some_and(|(_, c)| c.is_alphanumeric());
        match (run_start, alphanumeric) {
            (None, true) => run_start = Some(index),
            (Some(start), false) => {
                let mut piece = start;
                for i in start + 1..index {
                    let (previous, current) = (chars[i - 1].1, chars[i].1);
                    let next_lower = i + 1 < index && chars[i + 1].1.is_lowercase();
                    let boundary = (previous.is_lowercase() && current.is_uppercase())
                        || (previous.is_uppercase() && current.is_uppercase() && next_lower)
                        || previous.is_alphabetic() != current.is_alphabetic();
                    if boundary {
                        out.push((end_of(piece), end_of(i)));
                        piece = i;
                    }
                }
                out.push((end_of(piece), end_of(index)));
                run_start = None;
            }
            _ => {}
        }
    }
    out
}

/// Byte ranges of the maximal `[A-Za-z_$][A-Za-z0-9_$]*` runs of `text`: the
/// query's identifier runs and, lowercased with at least 2 characters, the
/// `foundry_ident` tokens.
pub fn identifier_runs(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let start_byte = |b: u8| b.is_ascii_alphabetic() || b == b'_' || b == b'$';
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if start_byte(bytes[i]) {
            let start = i;
            while i < bytes.len() && (start_byte(bytes[i]) || bytes[i].is_ascii_digit()) {
                i += 1;
            }
            out.push((start, i));
        } else {
            // Leftmost matching: a byte that cannot start a run (a digit, a
            // non-identifier byte) is skipped on its own.
            i += 1;
        }
    }
    out
}

/// The import keys one node contributes (context-v2 § Doors import keys):
/// for an import statement, the bound names it introduces — a named import
/// or its alias, the last segment of a `use`/`import` path, the names of
/// `from m import a, b`, the last segment of a `using`/`open`/`Imports`
/// namespace or declaration, an `#include "x/y.h"` as `y` — and a
/// `require`/`source`/`include` path's file stem (a Lua module path's last
/// segment). Glob imports (`use m::*`, `import a.*`, `from m import *`,
/// `import x._`) and Go's `.` and `_` imports give no key.
fn import_keys(lang: Lang, node: tree_sitter::Node, source: &[u8], out: &mut Vec<String>) {
    let text = |node: tree_sitter::Node| node.utf8_text(source).ok().map(str::to_owned);
    let mut cursor = node.walk();
    match (lang, node.kind()) {
        (Lang::Rust, "use_declaration") => {
            if let Some(argument) = node.child_by_field_name("argument") {
                rust_use_keys(argument, source, out);
            }
        }
        (Lang::Python, "import_statement" | "import_from_statement") => {
            for name in node.children_by_field_name("name", &mut cursor) {
                let bound = match name.kind() {
                    "aliased_import" => name.child_by_field_name("alias"),
                    // A dotted name binds its last segment.
                    _ => last_named_child(name),
                };
                out.extend(bound.and_then(text));
            }
        }
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "import_statement") => {
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "import_clause" => {
                        let mut parts = child.walk();
                        for part in child.named_children(&mut parts) {
                            match part.kind() {
                                "identifier" => out.extend(text(part)),
                                "namespace_import" => {
                                    out.extend(last_named_child(part).and_then(text));
                                }
                                "named_imports" => {
                                    let mut specifiers = part.walk();
                                    for specifier in part.named_children(&mut specifiers) {
                                        let bound =
                                            specifier.child_by_field_name("alias").or_else(|| {
                                                specifier
                                                    .child_by_field_name("name")
                                                    .filter(|name| name.kind() == "identifier")
                                            });
                                        out.extend(bound.and_then(text));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "import_require_clause" => {
                        let mut parts = child.walk();
                        let bound = child
                            .named_children(&mut parts)
                            .find(|part| part.kind() == "identifier");
                        out.extend(bound.and_then(text));
                    }
                    _ => {}
                }
            }
        }
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "call_expression") => {
            let is_require = node
                .child_by_field_name("function")
                .is_some_and(|function| text(function).as_deref() == Some("require"));
            let argument = node
                .child_by_field_name("arguments")
                .filter(|arguments| arguments.named_child_count() == 1)
                .and_then(|arguments| arguments.named_child(0))
                .filter(|argument| argument.kind() == "string");
            if is_require && let Some(argument) = argument {
                out.extend(text(argument).and_then(|path| file_stem(&path)));
            }
        }
        (Lang::Go, "import_spec") => match node.child_by_field_name("name") {
            Some(name) if name.kind() == "package_identifier" => out.extend(text(name)),
            Some(_) => {}
            None => {
                let path = node.child_by_field_name("path").and_then(text);
                out.extend(path.and_then(|path| {
                    let path = path.trim_matches(|c| c == '"' || c == '`');
                    let last = path.rsplit('/').next().unwrap_or(path);
                    (!last.is_empty()).then(|| last.to_owned())
                }));
            }
        },
        (Lang::C | Lang::Cpp, "preproc_include") => {
            let path = node
                .child_by_field_name("path")
                .filter(|path| matches!(path.kind(), "string_literal" | "system_lib_string"));
            out.extend(path.and_then(text).and_then(|path| file_stem(&path)));
        }
        (Lang::Cpp, "using_declaration") => {
            let named = node
                .named_children(&mut cursor)
                .filter(|child| matches!(child.kind(), "identifier" | "qualified_identifier"))
                .last();
            out.extend(named.and_then(text).and_then(|name| {
                let last = name.rsplit("::").next().unwrap_or(&name).trim();
                (!last.is_empty()).then(|| last.to_owned())
            }));
        }
        (Lang::Java, "import_declaration") => {
            let mut glob = false;
            let mut path = None;
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "asterisk" => glob = true,
                    "identifier" => path = Some(child),
                    "scoped_identifier" => path = child.child_by_field_name("name"),
                    _ => {}
                }
            }
            if !glob {
                out.extend(path.and_then(text));
            }
        }
        (Lang::CSharp, "using_directive") => match node.child_by_field_name("name") {
            Some(alias) => out.extend(text(alias)),
            None => out.extend(
                node.named_children(&mut cursor)
                    .filter(|child| {
                        matches!(
                            child.kind(),
                            "identifier" | "qualified_name" | "alias_qualified_name"
                        )
                    })
                    .last()
                    .and_then(text)
                    .and_then(|path| last_segment(&path, &[".", "::"])),
            ),
        },
        (Lang::FSharp | Lang::FSharpSignature, "import_decl") => out.extend(
            named_child_of(node, &["long_identifier"])
                .and_then(text)
                .and_then(|path| last_segment(&path, &["."])),
        ),
        (Lang::FSharp, "fsi_directive_decl") => {
            if text(node).is_some_and(|directive| directive.starts_with("#load")) {
                for path in node.named_children(&mut cursor) {
                    out.extend(text(path).and_then(|path| file_stem(&path)));
                }
            }
        }
        (Lang::VbNet, "imports_statement") => {
            for path in node.children_by_field_name("namespace", &mut cursor) {
                out.extend(text(path).and_then(|path| last_segment(&path, &["."])));
            }
        }
        (Lang::Php, "namespace_use_clause") => {
            let bound = node
                .child_by_field_name("alias")
                .or_else(|| named_child_of(node, &["qualified_name", "name"]));
            out.extend(
                bound
                    .and_then(text)
                    .and_then(|name| last_segment(&name, &["\\"])),
            );
        }
        (
            Lang::Php,
            "include_expression"
            | "include_once_expression"
            | "require_expression"
            | "require_once_expression",
        ) => {
            // A literal path, or one concatenated onto `__DIR__`.
            let path = node.named_child(0).and_then(|path| match path.kind() {
                "binary_expression" => path.child_by_field_name("right"),
                _ => Some(path),
            });
            out.extend(
                path.filter(|path| matches!(path.kind(), "string" | "encapsed_string"))
                    .and_then(text)
                    .and_then(|path| file_stem(&path)),
            );
        }
        (Lang::Perl, "use_no_statement" | "require_statement") => {
            let used = node.kind() == "require_statement" || has_child_kind(node, "use");
            if used {
                out.extend(
                    node.child_by_field_name("package_name")
                        .and_then(text)
                        .and_then(|name| last_segment(&name, &["::"])),
                );
            }
        }
        (Lang::Perl, "use_parent_statement") => {
            for child in node.named_children(&mut cursor) {
                let names: Vec<tree_sitter::Node> = match child.kind() {
                    "word_list_qw" => named_children(child)
                        .filter(|item| item.kind() == "list_item")
                        .collect(),
                    kind if kind.starts_with("string") => vec![child],
                    _ => Vec::new(),
                };
                for name in names {
                    out.extend(
                        text(name)
                            .and_then(|name| last_segment(name.trim_matches(['\'', '"']), &["::"])),
                    );
                }
            }
        }
        (Lang::Bash, "command") => {
            let sources = node
                .child_by_field_name("name")
                .and_then(text)
                .is_some_and(|name| matches!(name.as_str(), "source" | "."));
            if sources {
                out.extend(
                    node.child_by_field_name("argument")
                        .and_then(text)
                        .and_then(|path| file_stem(&path)),
                );
            }
        }
        (Lang::PowerShell, "command") => {
            let module = |name: &str| {
                if name.contains(['/', '\\'])
                    || name.to_ascii_lowercase().ends_with(".psm1")
                    || name.to_ascii_lowercase().ends_with(".psd1")
                    || name.to_ascii_lowercase().ends_with(".ps1")
                    || name.to_ascii_lowercase().ends_with(".dll")
                {
                    file_stem(name)
                } else {
                    last_segment(name, &["."])
                }
            };
            // `. ./x.ps1` dot-sources a script. Only a static name gives a
            // key: a bare path or a string holding no variable or
            // subexpression (`. $path`, `. "$name"`, `. "$(Get-X)"` give
            // none; 001 T008 review R8).
            if named_child_of(node, &["command_invokation_operator"])
                .and_then(text)
                .as_deref()
                == Some(".")
            {
                let script = node
                    .child_by_field_name("command_name")
                    .and_then(|name| match name.kind() {
                        "command_name_expr" if name.named_child_count() == 1 => name.named_child(0),
                        _ => Some(name),
                    })
                    .filter(|name| match name.kind() {
                        "command_name" => true,
                        "string_literal" => !named_children(*name).any(|string| {
                            matches!(
                                string.kind(),
                                "expandable_string_literal" | "expandable_here_string_literal"
                            ) && string.named_child_count() > 0
                        }),
                        _ => false,
                    });
                out.extend(
                    script
                        .and_then(text)
                        .and_then(|path| file_stem(path.trim_matches(['\'', '"']))),
                );
                return;
            }
            let Some(command) = node.child_by_field_name("command_name").and_then(text) else {
                return;
            };
            let elements: Vec<PowerShellElement> = node
                .child_by_field_name("command_elements")
                .map(|elements| {
                    named_children(elements)
                        .filter_map(|element| powershell_element(element, source))
                        .collect()
                })
                .unwrap_or_default();
            match command.to_ascii_lowercase().as_str() {
                "using" => {
                    if let [
                        PowerShellElement::Argument(kind),
                        PowerShellElement::Argument(names),
                        ..,
                    ] = elements.as_slice()
                        && let (Some(kind), Some(name)) = (kind.first(), names.first())
                    {
                        if kind.eq_ignore_ascii_case("namespace") {
                            out.extend(last_segment(name, &["."]));
                        } else if kind.eq_ignore_ascii_case("module") {
                            out.extend(module(name));
                        }
                    }
                }
                // The `-Name` operand, else the first argument no other
                // parameter takes as its value.
                "import-module" => {
                    let mut elements = elements.iter();
                    while let Some(element) = elements.next() {
                        match element {
                            PowerShellElement::Parameter(parameter) if parameter == "name" => {
                                if let Some(PowerShellElement::Argument(names)) = elements.next() {
                                    out.extend(names.iter().filter_map(|name| module(name)));
                                }
                                return;
                            }
                            PowerShellElement::Parameter(parameter)
                                if IMPORT_MODULE_SWITCHES.contains(&parameter.as_str()) => {}
                            PowerShellElement::Parameter(_) => {
                                elements.next();
                            }
                            PowerShellElement::Argument(names) => {
                                out.extend(names.iter().filter_map(|name| module(name)));
                                return;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        (Lang::Ruby, "call") => {
            if node.child_by_field_name("receiver").is_some() {
                return;
            }
            let method = node.child_by_field_name("method").and_then(text);
            let loads = matches!(
                method.as_deref(),
                Some("require" | "require_relative" | "load" | "autoload")
            );
            if loads {
                out.extend(
                    node.child_by_field_name("arguments")
                        .and_then(|arguments| {
                            named_children(arguments).find(|argument| argument.kind() == "string")
                        })
                        .and_then(text)
                        .and_then(|path| file_stem(&path)),
                );
            }
        }
        (Lang::Kotlin, "import") => {
            if has_child_kind(node, "*") {
                return;
            }
            let alias = named_child_of(node, &["identifier"]);
            out.extend(match alias {
                Some(alias) => text(alias),
                None => named_child_of(node, &["qualified_identifier"])
                    .and_then(last_named_child)
                    .and_then(text),
            });
        }
        (Lang::Swift, "import_declaration") => out.extend(
            named_child_of(node, &["identifier"])
                .and_then(last_named_child)
                .and_then(text),
        ),
        (Lang::Scala, "import_declaration") => {
            let mut path_last = None;
            let mut bound = Vec::new();
            let mut glob = false;
            let renamed = |renamed: tree_sitter::Node| {
                renamed
                    .child_by_field_name("alias")
                    .and_then(text)
                    .filter(|alias| alias != "_")
            };
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "identifier" => path_last = Some(child),
                    "namespace_wildcard" => glob = true,
                    "arrow_renamed_identifier" | "as_renamed_identifier" => {
                        bound.extend(renamed(child));
                    }
                    "namespace_selectors" => {
                        for selector in named_children(child) {
                            match selector.kind() {
                                "identifier" => bound.extend(text(selector)),
                                "arrow_renamed_identifier" | "as_renamed_identifier" => {
                                    bound.extend(renamed(selector));
                                }
                                _ => {}
                            }
                        }
                        glob = true;
                    }
                    _ => {}
                }
            }
            if bound.is_empty() && !glob {
                bound.extend(path_last.and_then(text));
            }
            out.extend(bound);
        }
        (Lang::Lua, "function_call") => {
            let function = node.child_by_field_name("name").and_then(text);
            let path = node
                .child_by_field_name("arguments")
                .and_then(|arguments| {
                    named_children(arguments).find(|argument| argument.kind() == "string")
                })
                .and_then(text);
            match function.as_deref() {
                Some("require") => out.extend(path.and_then(|path| {
                    last_segment(path.trim_matches(['"', '\'', '[', ']']), &[".", "/"])
                })),
                Some("dofile" | "loadfile") => out.extend(path.and_then(|path| file_stem(&path))),
                _ => {}
            }
        }
        (Lang::Dart, "import_specification") => {
            if let Some(alias) = node.child_by_field_name("alias") {
                out.extend(text(alias));
                return;
            }
            let shown: Vec<String> = named_children(node)
                .filter(|combinator| {
                    combinator.kind() == "combinator"
                        && text(*combinator).is_some_and(|text| text.starts_with("show"))
                })
                .flat_map(named_children)
                .filter_map(text)
                .collect();
            if shown.is_empty() {
                out.extend(
                    node.child_by_field_name("uri")
                        .and_then(text)
                        .and_then(|uri| dart_stem(&uri)),
                );
            } else {
                out.extend(shown);
            }
        }
        (Lang::Dart, "part_directive") => {
            out.extend(
                node.child_by_field_name("uri")
                    .and_then(text)
                    .and_then(|uri| dart_stem(&uri)),
            );
        }
        (Lang::Elixir, "call") => {
            let directive = node.child_by_field_name("target").and_then(text);
            if !matches!(
                directive.as_deref(),
                Some("alias" | "import" | "require" | "use")
            ) {
                return;
            }
            let Some(arguments) = named_child_of(node, &["arguments"]) else {
                return;
            };
            let alias = named_child_of(arguments, &["keywords"]).and_then(|keywords| {
                named_children(keywords)
                    .find(|pair| {
                        pair.child_by_field_name("key")
                            .and_then(text)
                            .is_some_and(|key| key.trim() == "as:")
                    })
                    .and_then(|pair| pair.child_by_field_name("value"))
            });
            if let Some(alias) = alias {
                out.extend(text(alias));
                return;
            }
            let Some(first) = arguments.named_child(0) else {
                return;
            };
            let modules: Vec<tree_sitter::Node> = match first.kind() {
                "alias" => vec![first],
                // `alias Outer.{Alpha, Beta}`
                "dot" => first
                    .child_by_field_name("right")
                    .filter(|right| right.kind() == "tuple")
                    .map(|tuple| named_children(tuple).collect())
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            for module in modules {
                out.extend(text(module).and_then(|name| last_segment(&name, &["."])));
            }
        }
        (Lang::Haskell, "import") => {
            let names = node
                .child_by_field_name("names")
                .filter(|names| !has_child_kind(*names, "hiding"));
            if let Some(names) = names {
                let mut listed = names.walk();
                for name in names.children_by_field_name("name", &mut listed) {
                    out.extend(name.named_child(0).and_then(text));
                }
                return;
            }
            let module = node
                .child_by_field_name("alias")
                .or_else(|| node.child_by_field_name("module"));
            out.extend(
                module
                    .and_then(text)
                    .and_then(|name| last_segment(&name, &["."])),
            );
        }
        _ => {}
    }
}

/// One element of a PowerShell command: a parameter (`-Name`, lowercased,
/// without its `-` and a trailing `:`), or an argument's values (a bare
/// token; a static quoted literal, or each of a list of them, without
/// quotes).
enum PowerShellElement {
    Parameter(String),
    Argument(Vec<String>),
}

/// `Import-Module`'s parameters that take no value.
const IMPORT_MODULE_SWITCHES: &[&str] = &[
    "ascustomobject",
    "disablenamechecking",
    "force",
    "global",
    "noclobber",
    "passthru",
    "skipeditioncheck",
    "usewindowspowershell",
];

/// One command element: `None` for a separator or a redirection, which take
/// no position; any operand that is not a static literal (`$prefix`,
/// `(Get-X)`, `"$name"`, `"$(Get-X)"`) is an argument without values, so it
/// still takes its position and a preceding parameter's value (001 T008
/// review R4, R8).
fn powershell_element(element: tree_sitter::Node, source: &[u8]) -> Option<PowerShellElement> {
    let parameter = |text: &str| {
        PowerShellElement::Parameter(
            text.trim_start_matches('-')
                .trim_end_matches(':')
                .to_ascii_lowercase(),
        )
    };
    match element.kind() {
        "command_argument_sep" => None,
        kind if kind.contains("redirection") => None,
        "command_parameter" => Some(parameter(text(element, source)?)),
        "generic_token" => {
            let token = text(element, source)?;
            Some(if token.starts_with('-') {
                parameter(token)
            } else {
                PowerShellElement::Argument(vec![token.to_owned()])
            })
        }
        // `'./Store.psm1'`, `"Store"`, `'A', 'B'`: static string literals,
        // each inside a unary expression of an array literal; anything else
        // in the list, a string holding a variable or a subexpression
        // (`"$name"`, `"$(Get-X)"`) included, makes the whole operand
        // unknown (001 T008 review R8).
        "array_literal_expression" => {
            let expands = |literal: &tree_sitter::Node| {
                named_children(*literal).any(|string| {
                    matches!(
                        string.kind(),
                        "expandable_string_literal" | "expandable_here_string_literal"
                    ) && string.named_child_count() > 0
                })
            };
            let mut literals = Vec::new();
            for item in named_children(element) {
                let Some(literal) = named_child_of(item, &["string_literal"])
                    .filter(|literal| item.named_child_count() == 1 && !expands(literal))
                    .and_then(|literal| text(literal, source))
                else {
                    return Some(PowerShellElement::Argument(Vec::new()));
                };
                let inner = literal
                    .strip_prefix(['\'', '"'])
                    .and_then(|rest| rest.strip_suffix(['\'', '"']));
                literals.push(inner.unwrap_or(literal).to_owned());
            }
            Some(PowerShellElement::Argument(literals))
        }
        _ => Some(PowerShellElement::Argument(Vec::new())),
    }
}

/// The last non-empty piece of `path` split at each of `separators`.
fn last_segment(path: &str, separators: &[&str]) -> Option<String> {
    let mut last = path;
    for separator in separators {
        if let Some((_, after)) = last.rsplit_once(separator) {
            last = after;
        }
    }
    let last = last.trim();
    (!last.is_empty()).then(|| last.to_owned())
}

/// A Dart URI's file stem: `package:a/b.dart` gives `b`, `dart:async`
/// gives `async`.
fn dart_stem(uri: &str) -> Option<String> {
    let uri = uri.trim_matches(['\'', '"']);
    let uri = uri.rsplit_once(':').map_or(uri, |(_, path)| path);
    file_stem(uri)
}

/// The bound names of one Rust `use` argument: a path's last segment, an
/// alias, each member of a list, and `self` in a list as its path's last
/// segment; a glob gives none. Walked on the heap, like the unit walk.
fn rust_use_keys(argument: tree_sitter::Node, source: &[u8], out: &mut Vec<String>) {
    let text = |node: tree_sitter::Node| node.utf8_text(source).ok().map(str::to_owned);
    let last_segment = |path: tree_sitter::Node| match path.kind() {
        "identifier" => text(path),
        "scoped_identifier" => path
            .child_by_field_name("name")
            .filter(|name| name.kind() == "identifier")
            .and_then(text),
        _ => None,
    };
    // Each node with the path of the list that holds it, if any.
    let mut stack = vec![(argument, None)];
    while let Some((node, list_path)) = stack.pop() {
        match node.kind() {
            "identifier" | "scoped_identifier" => out.extend(last_segment(node)),
            "use_as_clause" => out.extend(node.child_by_field_name("alias").and_then(text)),
            "self" => out.extend(list_path.and_then(last_segment)),
            "scoped_use_list" => {
                if let Some(list) = node.child_by_field_name("list") {
                    stack.push((list, node.child_by_field_name("path")));
                }
            }
            "use_list" => {
                let mut cursor = node.walk();
                let members: Vec<_> = node.named_children(&mut cursor).collect();
                // Reversed onto the stack, so members are taken in order.
                stack.extend(members.into_iter().rev().map(|member| (member, list_path)));
            }
            _ => {}
        }
    }
}

fn last_named_child(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    node.named_child(u32::try_from(node.named_child_count().checked_sub(1)?).ok()?)
}

/// The file stem of a quoted or bracketed path (`"x/y.h"`, `<sys/types.h>`,
/// `'./tools/index.js'`): its last component without its extension.
fn file_stem(quoted: &str) -> Option<String> {
    let path = quoted.trim_matches(|c| matches!(c, '"' | '\'' | '`' | '<' | '>'));
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = without_extension(name);
    (!stem.is_empty()).then(|| stem.to_owned())
}

/// A file name without its extension; a name whose only dot leads (a
/// dotfile) or that has none is kept whole.
fn without_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// Whether `c` belongs to an address segment: `[A-Za-z0-9_$]`.
fn segment_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// The address segments of a path (context-v2 § Definitions and addresses):
/// each component, the last without its extension, split on every character
/// outside `[A-Za-z0-9_$]`; lowercased and distinct
/// (`packages/coding-agent/src/tools/index.ts` gives `packages coding agent
/// src tools index`). A query's path tokens split the same way.
pub fn path_segments(path: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut components = path.split(['/', '\\']).peekable();
    while let Some(component) = components.next() {
        let component = if components.peek().is_none() {
            without_extension(component)
        } else {
            component
        };
        for piece in component.split(|c: char| !segment_char(c)) {
            let piece = piece.to_lowercase();
            if !piece.is_empty() && !out.contains(&piece) {
                out.push(piece);
            }
        }
    }
    out
}

/// A definition's address segments (context-v2 § Definitions and
/// addresses): `path`'s segments (from [`path_segments`]), then its
/// [`Unit::qualifiers`] — its qualified name minus its own name, every
/// generic argument list removed, split at the qname separator — lowercased
/// and distinct. The qualifiers come from the syntax tree ([`name_address`]
/// of each enclosing named unit), never from re-reading the qualified name's
/// text: `impl Mapper<fn() -> u8> { fn run }` and `impl Mapper</* > */ u8>`
/// give `run` the qualifier `mapper`, as `UnionFind<Key>::find` gives `find`
/// `unionfind`.
pub fn address_segments(path: &[String], qualifiers: &[String]) -> Vec<String> {
    let mut out = path.to_vec();
    for qualifier in qualifiers {
        if !out.contains(qualifier) {
            out.push(qualifier.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every node of `tree` in pre-order: its kind (id and name), whether it
    /// is named, missing, an error, an extra or holds an error, its byte and
    /// point ranges, the field it fills in its parent and its child count.
    fn shape(tree: &tree_sitter::Tree) -> Vec<String> {
        let mut out = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            out.push(format!(
                "{} {} named={} missing={} error={} extra={} has_error={} {:?} {}-{} field={:?} children={}",
                node.kind_id(),
                node.kind(),
                node.is_named(),
                node.is_missing(),
                node.is_error(),
                node.is_extra(),
                node.has_error(),
                node.byte_range(),
                node.start_position(),
                node.end_position(),
                cursor.field_name(),
                node.child_count(),
            ));
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return out;
                }
            }
        }
    }

    /// One character per read gives the parser the same tree as the whole
    /// buffer at once (the calibration found byte-identical trees on 22,492
    /// real files; 001 T008 review m1): node for node — kind, named,
    /// missing, error and extra status, byte and point ranges, field and
    /// children — for one source per grammar (this file for Rust), as
    /// written, with CRLF line ends, behind a byte-order mark, and empty.
    #[test]
    fn per_character_reads_give_the_whole_buffer_tree() {
        let sources: [(&str, &str); 25] = [
            ("syntax.rs", include_str!("syntax.rs")),
            ("multi.py", "# é ü\ndef f(a):\n    return 'ñ' + a  # 漢字\n"),
            (
                "store.ts",
                "export class Store<T> {\n  get(key: string): T | undefined { return undefined; } // é\n}\n",
            ),
            (
                "view.tsx",
                "export const V = () => <div>é {\"漢\"}</div>;\n",
            ),
            (
                "tools.js",
                "const f = (a) => `x ${a} ü`;\nfunction g() { return /é+/.test('ñ'); }\n",
            ),
            (
                "main.go",
                "package main\n\n// Ü\nfunc main() {\n\ts := \"漢\"\n\t_ = s\n}\n",
            ),
            (
                "lib.c",
                "#include <stdio.h>\n/* é */\nint main(void) { printf(\"ñ\\n\"); return 0; }\n",
            ),
            (
                "lib.cpp",
                "namespace a::b {\ntemplate<typename T> struct Box { T v; }; // 漢\n}\n",
            ),
            (
                "Main.java",
                "/** é */\npublic class Main {\n    public static void main(String[] a) { System.out.println(\"ü\"); }\n}\n",
            ),
            (
                "store.cs",
                include_str!("../tests/fixtures/syntax/store.cs"),
            ),
            (
                "shapes.fs",
                include_str!("../tests/fixtures/syntax/shapes.fs"),
            ),
            (
                "shapes.fsi",
                include_str!("../tests/fixtures/syntax/shapes.fsi"),
            ),
            (
                "shapes.vb",
                include_str!("../tests/fixtures/syntax/shapes.vb"),
            ),
            (
                "account.php",
                include_str!("../tests/fixtures/syntax/account.php"),
            ),
            (
                "shape.pl",
                include_str!("../tests/fixtures/syntax/shape.pl"),
            ),
            (
                "tools.sh",
                include_str!("../tests/fixtures/syntax/tools.sh"),
            ),
            (
                "tools.ps1",
                include_str!("../tests/fixtures/syntax/tools.ps1"),
            ),
            (
                "store.rb",
                include_str!("../tests/fixtures/syntax/store.rb"),
            ),
            (
                "Store.kt",
                include_str!("../tests/fixtures/syntax/Store.kt"),
            ),
            (
                "Store.swift",
                include_str!("../tests/fixtures/syntax/Store.swift"),
            ),
            (
                "shapes.scala",
                include_str!("../tests/fixtures/syntax/shapes.scala"),
            ),
            (
                "module.lua",
                include_str!("../tests/fixtures/syntax/module.lua"),
            ),
            (
                "store.dart",
                include_str!("../tests/fixtures/syntax/store.dart"),
            ),
            (
                "inner.ex",
                include_str!("../tests/fixtures/syntax/inner.ex"),
            ),
            (
                "Shapes.hs",
                include_str!("../tests/fixtures/syntax/Shapes.hs"),
            ),
        ];
        let mut grammars = std::collections::HashSet::new();
        for (path, source) in sources {
            let lang = Lang::from_path(path).unwrap();
            grammars.insert(lang.tag().to_owned() + path.rsplit('.').next().unwrap());
            let grammar = lang.grammar().unwrap();
            let crlf = source.replace('\n', "\r\n");
            let bom = format!("\u{feff}{source}");
            for (form, text) in [
                ("as written", source),
                ("CRLF", &crlf),
                ("BOM", &bom),
                ("empty", ""),
            ] {
                let chars = parse(text, &grammar).unwrap().unwrap();
                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&grammar).unwrap();
                let whole = parser.parse(text, None).unwrap();
                assert_eq!(shape(&chars), shape(&whole), "{path} {form}");
            }
        }
        // Every grammar: TypeScript and TSX, F# and its signatures apart.
        assert_eq!(grammars.len(), 25);
    }

    /// An outline-only analysis reads no address segments: its units carry
    /// no qualifiers and are otherwise the units, while units and indexing
    /// keep their qualifiers (001 T007 review m1).
    #[test]
    fn an_outline_only_analysis_carries_no_qualifiers() {
        for (lang, source) in [
            (
                Lang::Rust,
                "mod graph {\n    impl<T> a::b::Outer<T> {\n        fn edges(&self) {}\n    }\n}\n",
            ),
            (
                Lang::Ruby,
                "class Outer::Inner::Store\n  def get\n  end\nend\n",
            ),
            (
                Lang::Cpp,
                "namespace a::b {\nstruct ns::Box {\n    void run() {}\n};\n}\n",
            ),
        ] {
            let units = analyze(source, lang, Need::Units).units;
            let outline = analyze(source, lang, Need::Outline).units;
            assert!(
                units.iter().any(|unit| !unit.qualifiers.is_empty()),
                "{source}"
            );
            assert!(
                outline.iter().all(|unit| unit.qualifiers.is_empty()),
                "{source}"
            );
            let without = |units: Vec<Unit>| -> Vec<Unit> {
                units
                    .into_iter()
                    .map(|unit| Unit {
                        qualifiers: Vec::new(),
                        ..unit
                    })
                    .collect()
            };
            assert_eq!(without(units.clone()), outline, "{source}");
            let indexed = index(source, Some(lang)).unwrap();
            assert!(
                indexed
                    .documents
                    .iter()
                    .any(|document| !document.unit.qualifiers.is_empty()),
                "{source}"
            );
        }
    }
}
