//! Outlines (context-v2 § Outline): the elidable spans of a parsed source, the
//! `Outliner` that finds them and the `Segment` rendering. `COMMENT_LINES`,
//! which unit extraction also uses, stays in the parent module.
use super::*;

/// Leaf body interiors and container gaps shorter than this are not elided.
const ELIDE_LINES: usize = 4;

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
