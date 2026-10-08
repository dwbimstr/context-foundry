//! Syntax units and search documents (context-v2 § Syntax units and search
//! documents; 001 T005). The language is chosen by file extension only;
//! tree-sitter grammars (Markdown: pulldown-cmark headings) yield a
//! source-ordered interval forest of units, and every source is tiled into
//! search documents that each name their delivery unit. Parsing is
//! deterministic, error-tolerant and has no time-based limit; zero units, an
//! unmapped language or a source over 1 MiB falls back to blocks.

/// Sources above this size are not parsed: their outline equals their text
/// and their documents are blocks.
pub const MAX_PARSE_BYTES: usize = 1024 * 1024;
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

/// A source language, chosen by file extension only.
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
    /// The extension map; every other extension (and a dotfile without a
    /// stem) is unmapped: no tag, no units, blocks only.
    pub fn from_path(path: &str) -> Option<Self> {
        let name = path.rsplit('/').next().unwrap_or(path);
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
            "md" | "markdown" => Self::Markdown,
            "toml" => Self::Toml,
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "sh" | "bash" => Self::Bash,
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
            Self::Toml | Self::Json | Self::Yaml | Self::Bash | Self::Sql | Self::Html | Self::Css
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
            _ => return None,
        })
    }

    fn qname_separator(self) -> &'static str {
        match self {
            Self::Rust | Self::Cpp => "::",
            _ => ".",
        }
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
/// block/declaration_list child), `None` when the unit has no elidable
/// interior. `name_range` is the byte range of the unit's name node when the
/// unit is a definition (context-v2 § Definitions and addresses): a
/// programming-language unit with a name, except a Rust `impl`, which
/// extends a type defined elsewhere. Units are stored in source (pre-)order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub kind: UnitKind,
    pub name: Option<String>,
    pub qname: Option<String>,
    pub name_range: Option<(usize, usize)>,
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
    /// The definition's name node (see [`Unit::name_range`]); `None` for a
    /// block, a section and an `impl`.
    pub name_range: Option<(usize, usize)>,
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
    name_range: Option<(usize, usize)>,
    body: Option<(usize, usize)>,
}

/// The unit forest of `source`; empty for a language without units or a
/// source over [`MAX_PARSE_BYTES`].
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
/// for indexing, the import keys.
#[derive(Default)]
struct Analysis {
    units: Vec<Unit>,
    comments: Vec<(usize, usize)>,
    members: Vec<(usize, usize)>,
    imports: Vec<String>,
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
    analysis.units = forest(source, lang, candidates);
    analysis
}

/// The search documents of `source`, in source order. Document ranges plus
/// whitespace-only gaps tile `[0, len)` without overlap, and each document
/// lies inside its delivery unit.
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

/// The [`SourceIndex`] of `source`.
pub fn index(source: &str, lang: Option<Lang>) -> SourceIndex {
    let analysis = lang.map_or_else(Analysis::default, |lang| {
        analyze(source, lang, Need::Imports)
    });
    let mut seen = std::collections::HashSet::new();
    let imports = analysis
        .imports
        .into_iter()
        .filter(|key| seen.insert(key.clone()))
        .collect();
    SourceIndex {
        documents: tile(source, &analysis.units),
        imports,
    }
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
fn tree_units(source: &str, lang: Lang, need: Need, extras: &mut Analysis) -> Vec<Candidate> {
    let mut out = Vec::new();
    let Some(grammar) = lang.grammar() else {
        return out;
    };
    let mut parser = tree_sitter::Parser::new();
    // Every grammar is a locked dependency whose ABI the tests load; a
    // failure here would be a build defect, surfaced by those tests.
    if parser.set_language(&grammar).is_err() {
        return out;
    }
    let Some(tree) = parser.parse(source, None) else {
        return out;
    };
    let bytes = source.as_bytes();
    // Iterative pre-order walk: error-recovered trees can be deep. The
    // ancestors of the current node are kept on the heap, because
    // `Node::parent` (and so `prev_sibling`) re-descends from the root. Per
    // depth, `runs` holds the consecutive leading-run nodes (§ Unit forest)
    // just before the current node at that depth.
    let mut cursor = tree.walk();
    let mut ancestors: Vec<tree_sitter::Node> = Vec::new();
    let mut runs: Vec<Vec<(tree_sitter::Node, Leading)>> = vec![Vec::new()];
    loop {
        let node = cursor.node();
        // A Rust declaration-only item is a unit and stays a mandatory
        // outline member, so outlines keep its lines as before.
        if let Some(candidate) = candidate(lang, node, &ancestors, &runs, bytes) {
            out.push(candidate);
        }
        if need == Need::Imports {
            import_keys(lang, node, bytes, &mut extras.imports);
        } else if need == Need::Outline {
            if is_member_signature(lang, node) {
                // A template wrapper's lines belong to the signature too.
                let outer = outermost_wrapper(lang, node, &ancestors);
                extras.members.push((outer.start_byte(), outer.end_byte()));
            } else if node.kind().ends_with("comment")
                && node.end_position().row + 1 >= node.start_position().row + COMMENT_LINES
            {
                extras.comments.push((node.start_byte(), node.end_byte()));
            }
        }
        if cursor.goto_first_child() {
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
                return out;
            }
            ancestors.pop();
            runs.pop();
        }
    }
}

/// A declaration-only member (a trait method without a body, an interface or
/// abstract member, a C/C++ prototype): not a unit, yet a signature that an
/// outline never hides.
fn is_member_signature(lang: Lang, node: tree_sitter::Node) -> bool {
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

fn candidate(
    lang: Lang,
    node: tree_sitter::Node,
    ancestors: &[tree_sitter::Node],
    runs: &[Vec<(tree_sitter::Node, Leading)>],
    source: &[u8],
) -> Option<Candidate> {
    let kind = unit_kind(lang, node, ancestors)?;
    let name_node = unit_name(lang, node);
    let name = name_node
        .and_then(|name| name.utf8_text(source).ok())
        .map(str::to_owned);
    // A Rust `impl` names the type it extends: it is a container, not a
    // definition of that type (context-v2 § Definitions and addresses).
    let name_range = name_node
        .filter(|_| kind != UnitKind::Impl && name.is_some())
        .map(|name| (name.start_byte(), name.end_byte()));
    let body = body_range(lang, node);
    let outer = outermost_wrapper(lang, node, ancestors);
    // `outer` sits at the depth of the outermost wrapper (or of `node`).
    let wrappers = ancestors
        .iter()
        .rev()
        .take_while(|ancestor| is_wrapper(lang, **ancestor))
        .count();
    let (start, decl) = leading_run(outer, &runs[ancestors.len() - wrappers], source);
    Some(Candidate {
        start,
        decl,
        head: outer.start_byte(),
        end: outer.end_byte(),
        kind,
        name,
        name_range,
        body,
    })
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

/// The outermost of the wrappers (decorator, `export`, `template`, a
/// declaration with one declarator or spec, a Python expression statement)
/// directly enclosing `node`, or `node` itself; it supplies the range.
fn outermost_wrapper<'t>(
    lang: Lang,
    node: tree_sitter::Node<'t>,
    ancestors: &[tree_sitter::Node<'t>],
) -> tree_sitter::Node<'t> {
    ancestors
        .iter()
        .rev()
        .take_while(|ancestor| is_wrapper(lang, **ancestor))
        .last()
        .copied()
        .unwrap_or(node)
}

/// The unit kind of `node` under `ancestors` (context-v2 § Unit kinds).
fn unit_kind(
    lang: Lang,
    node: tree_sitter::Node,
    ancestors: &[tree_sitter::Node],
) -> Option<UnitKind> {
    use UnitKind::*;
    let kind = node.kind();
    // The kind of the `n`th ancestor up (1: the parent).
    let above = |n: usize| {
        ancestors
            .len()
            .checked_sub(n)
            .map(|at| ancestors[at].kind())
    };
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
            "property_identifier" if above(1) == Some("enum_body") => Some(Variant),
            "variable_declarator" => declarator_kind(node, ancestors),
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
        _ => None,
    }
}

/// A JavaScript-family declarator with an identifier name: `fn` when its
/// value is a function (anywhere, when its declaration has no other
/// declarator; T005's rule) and, at module level, otherwise `const` for
/// `const` and `static` for `let`/`var`.
fn declarator_kind(node: tree_sitter::Node, ancestors: &[tree_sitter::Node]) -> Option<UnitKind> {
    let declaration = *ancestors.last()?;
    if !matches!(
        declaration.kind(),
        "lexical_declaration" | "variable_declaration"
    ) || node
        .child_by_field_name("name")
        .is_none_or(|name| name.kind() != "identifier")
    {
        return None;
    }
    let above = |n: usize| {
        ancestors
            .len()
            .checked_sub(n)
            .map(|at| ancestors[at].kind())
    };
    let module = above(2) == Some("program")
        || (above(2) == Some("export_statement") && above(3) == Some("program"));
    let function = node
        .child_by_field_name("value")
        .is_some_and(|value| matches!(value.kind(), "arrow_function" | "function_expression"));
    if function && (module || declarators(declaration) == 1) {
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

fn declarators(declaration: tree_sitter::Node) -> usize {
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "variable_declarator")
        .count()
}

/// The node the language's name rule selects (context-v2 § Unit kinds): the
/// `name` field; for a Rust `impl` the `type` field; for C/C++ the innermost
/// identifier of the declarator chain; a bare TypeScript enum member is its
/// own name; a Python assignment's left identifier.
fn unit_name(lang: Lang, node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    match (lang, node.kind()) {
        (Lang::Rust, "impl_item") => node.child_by_field_name("type"),
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "property_identifier") => Some(node),
        (Lang::Python, "assignment") => node.child_by_field_name("left"),
        (Lang::C | Lang::Cpp, "function_definition") => Some(innermost_declarator(
            node.child_by_field_name("declarator")?,
        )),
        _ => node.child_by_field_name("name"),
    }
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

fn body_range(lang: Lang, node: tree_sitter::Node) -> Option<(usize, usize)> {
    let body = match (lang, node.kind()) {
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "variable_declarator") => node
            .child_by_field_name("value")
            .filter(|value| matches!(value.kind(), "arrow_function" | "function_expression"))?
            .child_by_field_name("body"),
        _ => node.child_by_field_name("body").or_else(|| {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "block" | "declaration_list"))
        }),
    }?;
    Some((body.start_byte(), body.end_byte()))
}

/// Whether `node` wraps the unit directly inside it, supplying its range: a
/// decorator or Python expression statement, an `export`, a C++ `template`,
/// a JavaScript-family declaration with one declarator and a Go declaration
/// with one spec.
fn is_wrapper(lang: Lang, node: tree_sitter::Node) -> bool {
    let single = |kinds: &[&str]| {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .filter(|child| kinds.contains(&child.kind()))
            .count()
            == 1
    };
    match (lang, node.kind()) {
        (Lang::Python, "decorated_definition" | "expression_statement") => true,
        (Lang::TypeScript | Lang::Tsx | Lang::JavaScript, "export_statement") => true,
        (
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript,
            "lexical_declaration" | "variable_declaration",
        ) => single(&["variable_declarator"]),
        (Lang::Go, "type_declaration" | "const_declaration" | "var_declaration") => {
            single(&["type_spec", "type_alias", "const_spec", "var_spec"])
        }
        (Lang::Cpp, "template_declaration") => true,
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
                name_range: None,
                body: (*heading_end < end).then_some((*heading_end, end)),
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
/// units.
fn forest(source: &str, lang: Lang, mut candidates: Vec<Candidate>) -> Vec<Unit> {
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
        let qname = candidate.name.as_deref().map(|name| {
            let prefix = enclosing_named.and_then(|named| units[named].qname.as_deref());
            qualified(prefix, lang.qname_separator(), name)
        });
        let index = units.len();
        nearest_named.push(match candidate.name {
            Some(_) => Some(index),
            None => enclosing_named,
        });
        units.push(Unit {
            kind: candidate.kind,
            name: candidate.name,
            qname,
            name_range: candidate.name_range,
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

fn delivery(unit: &Unit) -> DeliveryUnit {
    DeliveryUnit {
        start: unit.start,
        head: unit.head,
        end: unit.end,
        kind: unit.kind,
        name: unit.name.clone(),
        qname: unit.qname.clone(),
        name_range: unit.name_range,
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
/// `from m import a, b`, the last segment of a C++ `using` namespace or
/// declaration, an `#include "x/y.h"` as `y` — and a JavaScript `require`
/// path's file stem. Glob imports (`use m::*`, `import a.*`,
/// `from m import *`) and Go's `.` and `_` imports give no key.
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
        _ => {}
    }
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
    node.named_child(node.named_child_count().checked_sub(1)?)
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

/// A definition's address segments: `path`'s segments (from
/// [`path_segments`]), then its qualified name's — every generic argument
/// list (`<…>`, `[…]`, nested) removed first, split on the language's qname
/// separator, minus the unit's own name (`UnionFind<Key>::find` gives
/// `unionfind`) — lowercased and distinct.
pub fn address_segments(path: &[String], lang: Lang, qname: &str) -> Vec<String> {
    let mut out = path.to_vec();
    let mut depth = 0usize;
    let stripped: String = qname
        .chars()
        .filter(|&c| match c {
            '<' | '[' => {
                depth += 1;
                false
            }
            '>' | ']' => {
                depth = depth.saturating_sub(1);
                false
            }
            _ => depth == 0,
        })
        .collect();
    let mut parts: Vec<&str> = stripped.split(lang.qname_separator()).collect();
    // The last part is the unit's own name.
    parts.pop();
    for part in parts {
        let part = part.trim().to_lowercase();
        if !part.is_empty() && !out.contains(&part) {
            out.push(part);
        }
    }
    out
}
