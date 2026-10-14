//! Search documents (context-v2 § Syntax units and search documents): the
//! tiling of a source into each top-level unit's regions, delivered as their
//! unit, and blocks for the bytes outside them. Parsing and the unit forest
//! stay in the parent module.
use super::*;

/// Tiles `source` into documents: each top-level unit's regions, and blocks
/// for the bytes outside them.
pub(super) fn tile(source: &str, units: &[Unit]) -> Vec<Document> {
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
