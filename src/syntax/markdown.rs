//! Markdown units: pulldown-cmark headings become section candidates, their
//! names cut at a UTF-8 boundary. The tree-sitter languages stay in the parent
//! module.
use super::*;

/// Markdown heading sections: from a heading to the next heading of equal or
/// higher rank (lower or equal level number), or EOF. Headings inside code
/// fences are not headings.
pub(super) fn markdown_sections(source: &str) -> Vec<Candidate> {
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
