/** Inline box fragments (CSS 2.1 9.2.1.1 and 10.8, css-break-3 `box-decoration-break: slice`):
 * an inline box that spans several lines, or is split by a block inside it, paints one box per
 * line it reaches, each around that line's runs; only the first takes the start border and
 * padding and only the last the end ones. Layout keeps one box over all of them, which painted a
 * block inside a bordered span inside the span's border (block-in-inline-insert-001a). */
use super::flow::{DisplayItem, Document, Node};
use std::collections::BTreeMap;

/** A rectangle as (left, top, right, bottom). */
type Rect = (f64, f64, f64, f64);

/** Replaces every inline box item that reaches more than one line, or holds a block, with one
 * item per line. */
pub(super) fn split(document: &Document, items: &mut Vec<DisplayItem>) {
    let positions = super::sizing::positions(document);
    let node = |local: usize| {
        positions
            .get(local)
            .copied()
            .flatten()
            .and_then(|at| document.nodes.get(at))
    };
    let mut lines: Lines = BTreeMap::new();
    let mut splits = Vec::new();
    for index in (0..items.len()).rev() {
        let Some(owner) = node(items[index].local_id) else {
            continue;
        };
        // Layout hid it (a line clamp cut it away), so it reaches no line its box must cover.
        if items[index].opacity == 0.0 {
            continue;
        }
        // A flex or grid item is blockified: it is no inline box and sits on no line.
        if owner.parent.and_then(node).is_some_and(container) {
            continue;
        }
        let Some(rects) = line_rects(owner, &items[index], &mut lines, (&mut splits, index)) else {
            continue;
        };
        if let Some(parent) = owner.parent.and_then(node).filter(|p| inline_box(p)) {
            let entry = lines.entry(parent.local_id).or_default();
            rects.into_iter().for_each(|rect| merge(entry, rect));
        }
    }
    // Later indexes first, so earlier ones stay valid while fragments are inserted.
    for (index, rects) in splits {
        if let Some(owner) = node(items[index].local_id) {
            let mut pieces = pieces(owner, &items[index], rects);
            follow_blocks(document, &positions, owner, (items, &mut pieces));
            items.splice(index..=index, pieces);
        }
    }
}
/** Gives each fragment below a block of its box that block's paint order, so it paints over the
 * block's content as Blink's line after the block-in-inline does (block-in-inline-insert-001a's
 * bottom border stayed under the block's descender). */
fn follow_blocks(
    document: &Document,
    positions: &[Option<usize>],
    owner: &Node,
    (items, pieces): (&[DisplayItem], &mut [DisplayItem]),
) {
    // A positioned box paints from its stacking layer, where its own order already holds.
    if owner.style.position != crate::document::Position::Static {
        return;
    }
    let blocks: Vec<(f64, usize)> = document
        .nodes
        .iter()
        .filter(|node| node.parent == Some(owner.local_id))
        .filter(|node| super::block_in_inline::is_block(node))
        .filter_map(|node| {
            let item = items.iter().find(|item| item.local_id == node.local_id)?;
            Some((item.y, last_descendant(document, positions, node)))
        })
        .collect();
    for piece in pieces.iter_mut() {
        piece.paint_after = blocks
            .iter()
            .filter(|(top, _)| *top <= piece.y)
            .map(|(_, last)| *last)
            .next_back();
    }
}
/** The identity of the last node in `node`'s subtree, in document order. */
fn last_descendant(document: &Document, positions: &[Option<usize>], node: &Node) -> usize {
    let Some(start) = positions.get(node.local_id).copied().flatten() else {
        return node.local_id;
    };
    let within = |candidate: &Node| {
        let mut parent = candidate.parent;
        for _ in 0..crate::constants::DOCUMENT_DEPTH {
            match parent {
                Some(local) if local == node.local_id => return true,
                Some(local) => {
                    parent = positions
                        .get(local)
                        .copied()
                        .flatten()
                        .and_then(|at| document.nodes.get(at))
                        .and_then(|n| n.parent)
                }
                None => return false,
            }
        }
        false
    };
    document.nodes[start + 1..]
        .iter()
        .take_while(|candidate| within(candidate))
        .last()
        .map_or(node.local_id, |last| last.local_id)
}
/** Each inline box's line rectangles gathered so far from its descendants, by identity. */
type Lines = BTreeMap<usize, Vec<Rect>>;
/** Fragments to make: the item's index and its line rectangles. */
type Splits = Vec<(usize, Vec<Rect>)>;
/**
 * What one item adds to its parent's lines: an inline box its line rectangles with its edges
 * (recording a split when it has several, or a block made its box taller than its one line), an
 * atomic inline its box. A `::before` or `::after` run joins its own element's lines instead.
 */
fn line_rects(
    owner: &Node,
    item: &DisplayItem,
    lines: &mut Lines,
    (splits, index): (&mut Splits, usize),
) -> Option<Vec<Rect>> {
    if owner.style.inline_block && !generated(item) {
        return Some(vec![(
            item.x,
            item.y,
            item.x + item.width,
            item.y + item.height,
        )]);
    }
    if !inline_box(owner) {
        return None;
    }
    if generated(item) {
        if inline_generated(owner, item) {
            let entry = lines.entry(item.local_id).or_default();
            own_runs(owner, item)
                .into_iter()
                .for_each(|rect| merge(entry, rect));
        }
        return None;
    }
    let mut rects = lines.remove(&item.local_id).unwrap_or_default();
    own_runs(owner, item)
        .into_iter()
        .for_each(|rect| merge(&mut rects, rect));
    let edged: Vec<Rect> = rects.iter().map(|rect| with_edges(owner, *rect)).collect();
    let taller = edged
        .first()
        .is_some_and(|one| item.height > one.3 - one.1 + TALLER);
    if rects.len() > 1 || taller {
        splits.push((index, rects));
    }
    Some(edged)
}
/** How much taller than its one line an inline box must be to hold a block (a rounding margin). */
const TALLER: f64 = 1.0;
/** Whether a generated box of an inline element is itself inline, so its runs are the
 * element's; a block one sits between the element's fragments. */
fn inline_generated(owner: &Node, item: &DisplayItem) -> bool {
    let after = item.tag == "::after";
    owner
        .style
        .generated
        .iter()
        .any(|generated| generated.after == after && generated.style.inline)
}
/** A flex or grid container, whose children are blockified items. */
fn container(node: &Node) -> bool {
    node.style.display_flex || node.style.display_grid
}
/** A text-bearing inline element (not atomic, replaced or out of flow), as line.rs fits them. */
fn inline_box(node: &Node) -> bool {
    node.style.inline
        && !node.style.inline_block
        && !node.style.display_contents
        && node.style.float == crate::document::style::Float::None
        && !super::flow::out_of_flow(node)
        && ![
            "img", "canvas", "svg", "input", "button", "select", "textarea", "br",
        ]
        .contains(&node.tag.as_str())
}
/** A `::before` or `::after` box, which shares its element's identity. */
fn generated(item: &DisplayItem) -> bool {
    item.tag.starts_with("::")
}
/** The content area of each of the item's own runs: the font's ascent plus descent around its
 * baseline, as line.rs places them. */
fn own_runs(node: &Node, item: &DisplayItem) -> Vec<Rect> {
    let (_, content) = super::sizing::face_metrics(&node.style);
    let leading = super::sizing::half_leading(&node.style);
    item.lines
        .iter()
        .map(|line| {
            let top = line.y + leading;
            (line.x, top, line.x + line.width, top + content)
        })
        .collect()
}
/** Adds a rectangle to the line it sits on (either one's middle inside the other, since a
 * font's content area can be taller than its line), or as a new line. */
fn merge(lines: &mut Vec<Rect>, rect: Rect) {
    let middle = |r: &Rect| (r.1 + r.3) / 2.0;
    let within = |m: f64, r: &Rect| r.1 <= m && m <= r.3;
    let same = |line: &&mut Rect| within(middle(&rect), line) || within(middle(line), &rect);
    match lines.iter_mut().find(same) {
        Some(line) => {
            *line = (
                line.0.min(rect.0),
                line.1.min(rect.1),
                line.2.max(rect.2),
                line.3.max(rect.3),
            )
        }
        None => lines.push(rect),
    }
}
/** A fragment's rectangle grown by the box's padding and borders on every side, which is what
 * it adds to a parent's line. */
fn with_edges(node: &Node, rect: Rect) -> Rect {
    let sides = super::sizing::border_sides(&node.style);
    let pad = node.style.padding;
    (
        rect.0 - pad[3] - sides[3],
        rect.1 - pad[0] - sides[0],
        rect.2 + pad[1] + sides[1],
        rect.3 + pad[2] + sides[2],
    )
}
/** One item per line, top to bottom: the start edge (left, or right in rtl) only on the first
 * and the end edge only on the last; each keeps the box's own runs on its line. */
fn pieces(node: &Node, item: &DisplayItem, mut rects: Vec<Rect>) -> Vec<DisplayItem> {
    rects.sort_by(|a, b| a.1.total_cmp(&b.1));
    let runs = own_runs(node, item);
    let mut out: Vec<DisplayItem> = (0..rects.len())
        .map(|at| piece(node, item, &rects, at))
        .collect();
    for (line, run) in item.lines.iter().zip(runs) {
        out[nearest(&rects, &run)].lines.push(line.clone());
    }
    out
}
/** The line whose middle is closest to the run's. */
fn nearest(rects: &[Rect], run: &Rect) -> usize {
    let middle = |r: &Rect| (r.1 + r.3) / 2.0;
    let distance = |r: &Rect| (middle(r) - middle(run)).abs();
    (0..rects.len())
        .min_by(|a, b| distance(&rects[*a]).total_cmp(&distance(&rects[*b])))
        .unwrap_or(0)
}
/** Squares the corners on a fragment's cut sides: under `box-decoration-break: slice` the
 * radii round the whole box's ends, not every line's. */
pub(super) fn square_cut_corners(piece: &mut DisplayItem) {
    for (cut, corners) in [(piece.slice_cut[0], [0, 3]), (piece.slice_cut[1], [1, 2])] {
        for corner in corners.into_iter().filter(|_| cut) {
            piece.radius[corner] = 0.0;
            if let Some(vertical) = &mut piece.radius_y {
                vertical[corner] = 0.0;
            }
        }
    }
}
/** The fragment on line `at`, without runs yet: its rect grown by the edges it keeps. */
fn piece(node: &Node, item: &DisplayItem, rects: &[Rect], at: usize) -> DisplayItem {
    let (start, end) = if node.style.rtl { (1, 3) } else { (3, 1) };
    let (left, top, right, bottom) = with_edges(node, rects[at]);
    let mut piece = item.clone();
    piece.lines = Vec::new();
    if piece.border > 0.0 {
        // Sides are cut one by one, so the uniform width becomes per-side widths.
        piece.border_widths = [piece.border; 4];
        piece.border = 0.0;
    }
    let mut cut = [0.0; 4];
    for (side, keeps) in [(start, at == 0), (end, at + 1 == rects.len())] {
        if !keeps {
            cut[side] = piece.border_widths[side] + node.style.padding[side];
            piece.border_widths[side] = 0.0;
            piece.slice_cut[usize::from(side == 1)] = true;
        }
    }
    piece.x = left + cut[3];
    piece.y = top;
    piece.width = (right - cut[1] - left - cut[3]).max(0.0);
    piece.height = (bottom - top).max(0.0);
    piece
}
