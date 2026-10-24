/** Line boxes: baseline and vertical alignment of each finished line, then text-align and inline bounds. */
use super::flow::*;
use crate::document::style::VerticalAlign;

/** What a line fragment moves when its line settles. */
#[derive(Clone, Copy, Debug)]
pub(super) enum Piece {
    /** One painted text run: the item and the index of the run in its lines. */
    Text { item: usize, line: usize },
    /** An atomic inline (inline-block, image, control, svg): every item in `start..end`. */
    Atomic { start: usize, end: usize },
}
/** One fragment on the current line, measured from the line's top when it was placed. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Frag {
    pub piece: Piece,
    /** Distance from the fragment's top to its baseline. */
    pub ascent: f64,
    /** Fragment height, margins included for atomics. */
    pub height: f64,
    pub align: VerticalAlign,
    /** Left margin of an atomic, so text-align measures its margin box. */
    pub margin: (f64, f64),
}
/** The block's own font metrics every line includes, as CSS's strut does. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Strut {
    pub ascent: f64,
    pub height: f64,
    pub font: f64,
}
/** Horizontal alignment of finished lines inside their available width. */
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Align {
    #[default]
    Left,
    Center,
    Right,
}
/** A finished line waiting for `settle`: each fragment with its vertical offset, and the room it had. */
pub(super) struct Finished {
    frags: Vec<(Frag, f64)>,
    left: f64,
    width: f64,
}
/** Subscripts drop by a fifth of the parent font, as Chromium does. */
const SUB_SHIFT: f64 = 0.2;
/** Superscripts rise by a third of the parent font. */
const SUPER_SHIFT: f64 = 1.0 / 3.0;
/** Half the parent's x-height, which `middle` centers on, as a share of its font size. */
const HALF_X_HEIGHT: f64 = 0.25;

/** Baseline-aligns the current fragments; returns the line height and stores the offsets for `settle`. */
pub(super) fn finish(flow: &mut Flow) -> f64 {
    let frags = std::mem::take(&mut flow.frags);
    if frags.is_empty() {
        return 0.0;
    }
    let strut = flow.strut;
    let font = strut.map_or(0.0, |strut| strut.font);
    let raise = |frag: &Frag| match frag.align {
        VerticalAlign::Sub => -(font * SUB_SHIFT + 1.0),
        VerticalAlign::Super => font * SUPER_SHIFT + 1.0,
        _ => 0.0,
    };
    let anchored = |frag: &Frag| matches!(frag.align, VerticalAlign::Top | VerticalAlign::Bottom);
    // The baseline sits as low as the tallest ascent above it needs.
    let mut baseline = strut.map_or(0.0, |strut| strut.ascent);
    for frag in frags.iter().filter(|frag| !anchored(frag)) {
        baseline = baseline.max(match frag.align {
            VerticalAlign::Middle => frag.height / 2.0 + font * HALF_X_HEIGHT,
            VerticalAlign::TextTop => 0.0,
            _ => frag.ascent + raise(frag),
        });
    }
    let top_of = |frag: &Frag| match frag.align {
        VerticalAlign::Middle => baseline - font * HALF_X_HEIGHT - frag.height / 2.0,
        VerticalAlign::TextTop => baseline - strut.map_or(frag.ascent, |strut| strut.ascent),
        VerticalAlign::TextBottom => {
            baseline + strut.map_or(0.0, |strut| strut.height - strut.ascent) - frag.height
        }
        _ => baseline - frag.ascent - raise(frag),
    };
    let lowest = frags
        .iter()
        .filter(|frag| !anchored(frag))
        .map(&top_of)
        .fold(0.0_f64, f64::min);
    let mut height = strut.map_or(0.0, |strut| baseline - strut.ascent + strut.height) - lowest;
    for frag in frags.iter().filter(|frag| !anchored(frag)) {
        height = height.max(top_of(frag) - lowest + frag.height);
    }
    for frag in frags.iter().filter(|frag| anchored(frag)) {
        height = height.max(frag.height);
    }
    let placed = frags
        .into_iter()
        .map(|frag| {
            let top = match frag.align {
                VerticalAlign::Top => 0.0,
                VerticalAlign::Bottom => height - frag.height,
                _ => top_of(&frag) - lowest,
            };
            (frag, top)
        })
        .collect();
    let (left, width) = flow.room_at(flow.y);
    flow.finished.push(Finished {
        frags: placed,
        left,
        width,
    });
    height
}
/** Applies every finished line's offsets and alignment to the items, then refits inline boxes. */
pub(super) fn settle(
    context: &mut LayoutContext<'_>,
    flow: &mut Flow,
    items: &mut [DisplayItem],
    from: usize,
) -> EngineResult<()> {
    let align = flow.align;
    // A block of only blocks has no lines; its nested blocks already refit their own inlines.
    if flow.finished.is_empty() {
        return Ok(());
    }
    for line in std::mem::take(&mut flow.finished) {
        context.charge(line.frags.len() + 1)?;
        let mut extent: Option<(f64, f64)> = None;
        for (frag, dy) in &line.frags {
            move_piece(items, frag.piece, 0.0, *dy);
            if let Some((left, right)) = bounds(items, frag) {
                let (l, r) = extent.unwrap_or((left, right));
                extent = Some((l.min(left), r.max(right)));
            }
        }
        let Some((left, right)) = extent else {
            continue;
        };
        let free = (line.width - (right - left)).max(0.0);
        let dx = line.left - left
            + match align {
                Align::Left => 0.0,
                Align::Center => free / 2.0,
                Align::Right => free,
            };
        if align != Align::Left && dx != 0.0 {
            for (frag, _) in &line.frags {
                move_piece(items, frag.piece, dx, 0.0);
            }
        }
    }
    refit_inlines(context, items, from)
}
/** Horizontal extent of a fragment, margins included for atomics. */
fn bounds(items: &[DisplayItem], frag: &Frag) -> Option<(f64, f64)> {
    match frag.piece {
        Piece::Text { item, line } => {
            let line = items.get(item)?.lines.get(line)?;
            Some((line.x, line.x + line.width))
        }
        Piece::Atomic { start, .. } => {
            let item = items.get(start)?;
            Some((item.x - frag.margin.0, item.x + item.width + frag.margin.1))
        }
    }
}
/** Moves one fragment; atomics move with every item they painted. */
fn move_piece(items: &mut [DisplayItem], piece: Piece, dx: f64, dy: f64) {
    match piece {
        Piece::Text { item, line } => {
            if let Some(line) = items
                .get_mut(item)
                .and_then(|item| item.lines.get_mut(line))
            {
                line.x += dx;
                line.y += dy;
            }
        }
        Piece::Atomic { start, end } => {
            for item in items.iter_mut().take(end).skip(start) {
                translate(item, dx, dy);
            }
        }
    }
}
/** Shifts every absolute coordinate an item carries. */
pub(super) fn translate(item: &mut DisplayItem, dx: f64, dy: f64) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    item.x += dx;
    item.y += dy;
    for line in item.lines.iter_mut().chain(item.validation.iter_mut()) {
        line.x += dx;
        line.y += dy;
    }
    for path in &mut item.vector {
        path.translate(dx, dy);
    }
}
/** A text-bearing inline element (not atomic, not replaced) whose box spans its runs. */
fn inline_box(context: &LayoutContext<'_>, item: &DisplayItem) -> bool {
    context.node(item.local_id).is_ok_and(|node| {
        node.style.inline
            && !node.style.inline_block
            && !["img", "svg", "input", "button", "select", "textarea", "br"]
                .contains(&node.tag.as_str())
    })
}
/** Inline boxes cover their own runs and their inline descendants, plus padding and borders. */
fn refit_inlines(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    from: usize,
) -> EngineResult<()> {
    type Rect = (f64, f64, f64, f64);
    let mut extents = std::collections::BTreeMap::<usize, Rect>::new();
    let union = |a: Option<Rect>, b: Rect| match a {
        Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
        None => b,
    };
    for index in (from..items.len()).rev() {
        context.charge(items[index].lines.len() + 1)?;
        let inline = inline_box(context, &items[index]);
        let item = &items[index];
        let rect = if inline {
            let advance = item.line_height.unwrap_or(item.font_size);
            let mut rect = extents.remove(&item.local_id);
            for line in &item.lines {
                rect = Some(union(
                    rect,
                    (line.x, line.y, line.x + line.width, line.y + advance),
                ));
            }
            let Some(rect) = rect else {
                continue;
            };
            let node = context.node(item.local_id)?;
            let sides = super::sizing::border_sides(&node.style);
            let pad = node.style.padding;
            let rect = (
                rect.0 - pad[3] - sides[3],
                rect.1 - pad[0] - sides[0],
                rect.2 + pad[1] + sides[1],
                rect.3 + pad[2] + sides[2],
            );
            let item = &mut items[index];
            item.x = rect.0;
            item.y = rect.1;
            item.width = (rect.2 - rect.0).max(0.0);
            item.height = (rect.3 - rect.1).max(0.0);
            rect
        } else {
            (item.x, item.y, item.x + item.width, item.y + item.height)
        };
        let node = context.node(items[index].local_id)?;
        if !inline && !node.style.inline && !node.style.inline_block {
            continue;
        }
        let Some(parent) = node.parent else {
            continue;
        };
        if context.node(parent).is_ok_and(|parent| {
            parent.style.inline
                && !parent.style.inline_block
                && !["img", "svg", "input", "button", "select", "textarea", "br"]
                    .contains(&parent.tag.as_str())
        }) {
            let current = extents.remove(&parent);
            extents.insert(parent, union(current, rect));
        }
    }
    Ok(())
}
