//! Text tokens: the `foundry_code` subtokens and identifier runs of the search
//! index, and the path and address segments of a unit's address. They read
//! only text, never a syntax tree.
use super::*;

/// The `foundry_code` analysis without its length filter: byte ranges of the
/// subtokens of `text`, split on non-alphanumerics (including `_`), at
/// lower→Upper and Upper→Upper+lower boundaries (`HTTPServer` → `HTTP`,
/// `Server`) and at letter↔digit boundaries. Callers lowercase and keep
/// subtokens of at least 2 characters.
pub fn code_subtokens(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    // The open subtoken: its start offset and its last character.
    let mut piece: Option<(usize, char)> = None;
    while let Some((offset, current)) = chars.next() {
        if !current.is_alphanumeric() {
            if let Some((start, _)) = piece.take() {
                out.push((start, offset));
            }
            continue;
        }
        let mut start = offset;
        if let Some((open, previous)) = piece {
            let next_lower = chars
                .peek()
                .is_some_and(|&(_, next)| next.is_alphanumeric() && next.is_lowercase());
            let boundary = (previous.is_lowercase() && current.is_uppercase())
                || (previous.is_uppercase() && current.is_uppercase() && next_lower)
                || previous.is_alphabetic() != current.is_alphabetic();
            if boundary {
                out.push((open, offset));
            } else {
                start = open;
            }
        }
        piece = Some((start, current));
    }
    if let Some((start, _)) = piece {
        out.push((start, text.len()));
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
