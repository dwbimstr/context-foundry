//! The unit forest (context-v2 § Unit forest): candidates become a source-ordered
//! interval forest of units with their qualified names and bounded qualifier
//! tails. Unit rules and parsing stay in the parent module.
use super::*;

/// Builds the forest: a unit strictly nested in another is a child of its
/// nearest enclosing unit; zero-width or invalid ranges, a duplicate of an
/// identical range and the later of two partially overlapping ranges are not
/// units. An outline-only analysis propagates no address qualifiers.
pub(super) fn forest(
    source: &str,
    lang: Lang,
    need: Need,
    mut candidates: Vec<Candidate>,
) -> Vec<Unit> {
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
