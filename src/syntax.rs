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
        }
    }
}

/// One unit of the forest. `start..end` is its byte range (a wrapper such as a
/// decorator, `export` or `template` supplies it); `body` is the `body` field
/// (else the block/declaration_list child), `None` when the unit has no
/// elidable interior. Units are stored in source (pre-)order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub kind: UnitKind,
    pub name: Option<String>,
    pub qname: Option<String>,
    pub start: usize,
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
    pub end: usize,
    pub kind: UnitKind,
    pub name: Option<String>,
    pub qname: Option<String>,
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
    end: usize,
    kind: UnitKind,
    name: Option<String>,
    body: Option<(usize, usize)>,
}

/// The unit forest of `source`; empty for a language without units or a
/// source over [`MAX_PARSE_BYTES`].
pub fn units(source: &str, lang: Lang) -> Vec<Unit> {
    analyze(source, lang, false).units
}

/// One parse: the unit forest and, for outlines, the byte ranges of comments
/// spanning at least [`COMMENT_LINES`] lines and of declaration-only members.
#[derive(Default)]
struct Analysis {
    units: Vec<Unit>,
    comments: Vec<(usize, usize)>,
    members: Vec<(usize, usize)>,
}

fn analyze(source: &str, lang: Lang, for_outline: bool) -> Analysis {
    if !lang.has_units() || source.len() > MAX_PARSE_BYTES {
        return Analysis::default();
    }
    let mut analysis = Analysis::default();
    let candidates = if lang == Lang::Markdown {
        markdown_sections(source)
    } else {
        tree_units(source, lang, for_outline.then_some(&mut analysis))
    };
    analysis.units = forest(source, lang, candidates);
    analysis
}

/// The search documents of `source`, in source order. Document ranges plus
/// whitespace-only gaps tile `[0, len)` without overlap, and each document
/// lies inside its delivery unit.
pub fn documents(source: &str, lang: Option<Lang>) -> Vec<Document> {
    let units = lang.map_or_else(Vec::new, |lang| units(source, lang));
    let mut documents = Vec::new();
    let mut cursor = 0;
    for (index, unit) in units.iter().enumerate() {
        if unit.parent.is_none() {
            blocks(source, cursor, unit.start, &mut documents);
            unit_documents(source, &units, index, &mut documents);
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
        let analysis = analyze(source, lang, true);
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
                    mandatory[self.line_of(unit.start)..=signature_end].fill(true);
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

/// The unit candidates of one parse; with `extras`, also the comment and
/// declaration-only member ranges an outline needs.
fn tree_units(source: &str, lang: Lang, mut extras: Option<&mut Analysis>) -> Vec<Candidate> {
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
    // `Node::parent` re-descends from the root.
    let mut cursor = tree.walk();
    let mut ancestors: Vec<tree_sitter::Node> = Vec::new();
    loop {
        let node = cursor.node();
        if let Some(candidate) = candidate(lang, node, &ancestors, bytes) {
            out.push(candidate);
        } else if let Some(extras) = extras.as_deref_mut() {
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
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return out;
            }
            ancestors.pop();
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
    source: &[u8],
) -> Option<Candidate> {
    let kind = unit_kind(lang, node)?;
    let name = unit_name(lang, node, source);
    let body = body_range(lang, node);
    let outer = outermost_wrapper(lang, node, ancestors);
    Some(Candidate {
        start: outer.start_byte(),
        end: outer.end_byte(),
        kind,
        name,
        body,
    })
}

/// The outermost of the wrappers (decorator, `export`, `template`) directly
/// enclosing `node`, or `node` itself; it supplies the range.
fn outermost_wrapper<'t>(
    lang: Lang,
    node: tree_sitter::Node<'t>,
    ancestors: &[tree_sitter::Node<'t>],
) -> tree_sitter::Node<'t> {
    ancestors
        .iter()
        .rev()
        .take_while(|ancestor| is_wrapper(lang, ancestor.kind()))
        .last()
        .copied()
        .unwrap_or(node)
}

fn unit_kind(lang: Lang, node: tree_sitter::Node) -> Option<UnitKind> {
    use UnitKind::*;
    let kind = node.kind();
    match lang {
        Lang::Rust => Some(match kind {
            "function_item" => Fn,
            "struct_item" => Struct,
            "enum_item" => Enum,
            "union_item" => Union,
            "trait_item" => Trait,
            "impl_item" => Impl,
            "mod_item" => Mod,
            "macro_definition" => Macro,
            "const_item" => Const,
            "static_item" => Static,
            "type_item" => Type,
            _ => return None,
        }),
        Lang::Python => Some(match kind {
            "function_definition" => Fn,
            "class_definition" => Class,
            _ => return None,
        }),
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => match kind {
            "function_declaration" | "generator_function_declaration" => Some(Fn),
            "class_declaration" => Some(Class),
            "method_definition" => Some(Method),
            "interface_declaration" => Some(Interface),
            "type_alias_declaration" => Some(Type),
            "enum_declaration" => Some(Enum),
            "lexical_declaration" | "variable_declaration" => function_declarator(node).map(|_| Fn),
            _ => None,
        },
        Lang::Go => Some(match kind {
            "function_declaration" => Fn,
            "method_declaration" => Method,
            "type_declaration" => Type,
            _ => return None,
        }),
        Lang::C | Lang::Cpp => {
            let with_body = |kind| node.child_by_field_name("body").map(|_| kind);
            match kind {
                "function_definition" => Some(Fn),
                "struct_specifier" => with_body(Struct),
                "class_specifier" => with_body(Class),
                "union_specifier" => with_body(Union),
                "enum_specifier" => with_body(Enum),
                "namespace_definition" => Some(Mod),
                _ => None,
            }
        }
        Lang::Java => Some(match kind {
            "class_declaration" | "record_declaration" => Class,
            "interface_declaration" => Interface,
            "enum_declaration" => Enum,
            "method_declaration" | "constructor_declaration" => Method,
            _ => return None,
        }),
        _ => None,
    }
}

/// A `lexical_declaration`/`variable_declaration` is a unit when it has
/// exactly one declarator and that declarator's value is a function.
fn function_declarator(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = node.walk();
    let mut declarators = node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "variable_declarator");
    let declarator = declarators.next()?;
    if declarators.next().is_some() {
        return None;
    }
    let value = declarator.child_by_field_name("value")?;
    matches!(value.kind(), "arrow_function" | "function_expression").then_some(declarator)
}

fn unit_name(lang: Lang, node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let text = |node: tree_sitter::Node| node.utf8_text(source).ok().map(str::to_owned);
    match (lang, node.kind()) {
        (Lang::Rust, "impl_item") => node.child_by_field_name("type").and_then(text),
        (
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript,
            "lexical_declaration" | "variable_declaration",
        ) => function_declarator(node)?
            .child_by_field_name("name")
            .and_then(text),
        (Lang::C | Lang::Cpp, "function_definition") => text(innermost_declarator(
            node.child_by_field_name("declarator")?,
        )),
        _ => node.child_by_field_name("name").and_then(text),
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
        (
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript,
            "lexical_declaration" | "variable_declaration",
        ) => function_declarator(node)?
            .child_by_field_name("value")?
            .child_by_field_name("body"),
        _ => node.child_by_field_name("body").or_else(|| {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "block" | "declaration_list"))
        }),
    }?;
    Some((body.start_byte(), body.end_byte()))
}

fn is_wrapper(lang: Lang, kind: &str) -> bool {
    match lang {
        Lang::Python => kind == "decorated_definition",
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => kind == "export_statement",
        Lang::Cpp => kind == "template_declaration",
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
                end,
                kind: UnitKind::Section,
                name: (!name.is_empty()).then(|| name.to_owned()),
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
            start: candidate.start,
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
        end: unit.end,
        kind: unit.kind,
        name: unit.name.clone(),
        qname: unit.qname.clone(),
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
            end: block_end,
            kind: UnitKind::Block,
            name: None,
            qname: None,
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
