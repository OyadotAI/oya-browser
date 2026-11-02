/** Out-of-flow boxes of a grid: the grid area that is their containing block, and their static
 * position, after Blink's `GridLayoutAlgorithm::PlaceOutOfFlowItems`. */
use super::super::flow::*;
use super::{offset, placement};
use crate::document::{AlignItems, Position};

/** One axis of a laid-out grid: where each track starts, from the grid's padding-box edge, and
 * its size. Relative offsets survive a later move of the grid. */
#[derive(Clone, Default)]
pub(in crate::document::render) struct Axis {
    /** Each track's start offset from the padding-box start edge. */
    pub(super) starts: Vec<f64>,
    /** Each track's used size. */
    pub(super) sizes: Vec<f64>,
}
/** A laid-out grid's geometry, kept so the absolutely positioned boxes it contains, children or
 * deeper descendants, take their grid area as containing block (css-grid-2 9.1). */
#[derive(Clone, Default)]
pub(in crate::document::render) struct Frame {
    /** Rows, then columns. */
    pub(super) axes: [Axis; 2],
    /** Explicit row and column counts, as in-flow placement counted them, which line numbers count. */
    pub(super) explicit: [usize; 2],
    /** For each uncollapsed line index, the line index once collapsed tracks are gone. */
    pub(super) lines: [Vec<usize>; 2],
    /** The content box's start and end offsets from the padding-box edges, rows then columns. */
    pub(super) content: [(f64, f64); 2],
    /** A subgrid's adopted style, whose inherited line names its boxes' lines resolve
     * against; None for a grid that inherits nothing. */
    pub(super) adopted: Option<crate::document::SharedStyle>,
    /** `direction: rtl`: the columns' offsets run from the right padding edge leftwards. */
    pub(super) rtl: bool,
}
impl Axis {
    /** The span from line `start` to line `end` in a padding box `extent` long; a missing line,
     * or one past the grid, is the padding edge (Blink's `ComputeOutOfFlowItemContainingRect`). */
    fn area(&self, (start, end): (Option<usize>, Option<usize>), extent: f64) -> (f64, f64) {
        let count = self.sizes.len();
        let after = |track: usize| self.starts[track] + self.sizes[track];
        let from = match start.filter(|line| *line <= count && count > 0) {
            Some(line) if line < count => self.starts[line],
            Some(_) => after(count - 1),
            None => 0.0,
        };
        let to = match end.filter(|line| *line <= count && count > 0) {
            Some(0) => self.starts[0],
            Some(line) => after(line - 1),
            None => extent,
        };
        (from, to.max(from))
    }
}
impl Frame {
    /** The grid area `child` names, as row and column spans from the padding-box origin of a
     * grid whose padding box is `extent` (height, width). Lines count every track, collapsed
     * `auto-fit` ones included. */
    fn area(&self, grid: &Node, child: &Node, extent: [f64; 2]) -> [(f64, f64); 2] {
        let style = self.adopted.as_ref().unwrap_or(&grid.style);
        let rtl = self.rtl;
        let subgrid = self.adopted.is_some();
        let lines = placement::out_of_flow_lines(child, style, self.explicit, subgrid);
        std::array::from_fn(|axis| {
            let map = |line: Option<usize>| line.map(|l| super::remap(&self.lines[axis], l));
            let (start, end) = lines[axis];
            let span = self.axes[axis].area((map(start), map(end)), extent[axis]);
            physical(span, extent[axis], rtl && axis == 1)
        })
    }
}
/** A span measured from the inline-start edge as offsets from the left edge of an `extent`
 * long box: mirrored when the inline start is the right edge. */
fn physical((from, to): (f64, f64), extent: f64, mirrored: bool) -> (f64, f64) {
    if mirrored {
        (extent - to, extent - from)
    } else {
        (from, to)
    }
}
/** A child's block-axis self alignment and whether it is `safe`: `align-self` (kept apart
 * as `align_self_safe` when safe), else the grid's `align-items`. */
fn block_alignment(grid: &crate::document::ComputedStyle, child: &Node) -> (AlignItems, bool) {
    let own = &child.style;
    match (own.align_self, own.align_self_safe) {
        (Some(align), _) => (align, own.align_safe[1]),
        (None, Some(align)) => (align, true),
        (None, None) => (grid.align_items, grid.align_safe[0]),
    }
}
/** `offset`, except that a `safe` alignment of a box overflowing its room keeps it at the
 * start. */
fn safe_offset(align: AlignItems, free: f64, safe: bool) -> f64 {
    if safe && free < 0.0 {
        0.0
    } else {
        offset(align, free)
    }
}
/** Whether a grid is the containing block of its absolutely positioned child. */
fn contains(grid: &Node, child: &Node) -> bool {
    child.style.position == Position::Absolute
        && (grid.style.position != Position::Static || grid.style.transform.is_some())
}
/** The containing block a grid gives an absolutely positioned descendant: the grid area its
 * placement properties name inside the grid's padding box `block`, or None when the grid kept
 * no frame (it was not laid out as a grid). A box inside a nested grid, a subgrid included,
 * keeps the padding box, as the subgrid line-name tests expect. */
pub(in crate::document::render) fn containing_area(
    context: &LayoutContext<'_>,
    grid: &Node,
    child: &Node,
    block: super::super::positioned::Block,
) -> Option<super::super::positioned::Block> {
    let frame = context.grid_frame(grid.local_id)?;
    let mut parent = child.parent;
    while let Some(local) = parent.filter(|local| *local != grid.local_id) {
        let ancestor = context.node(local).ok()?;
        if ancestor.style.display_grid {
            return None;
        }
        parent = ancestor.parent;
    }
    let [(top, bottom), (left, right)] = frame.area(grid, child, [block.height, block.width]);
    Some(super::super::positioned::Block {
        x: block.x + left,
        y: block.y + top,
        width: right - left,
        height: bottom - top,
    })
}
/**
 * Where an absolutely positioned grid child's margin box starts when its insets are `auto`: the
 * static position rectangle is the child's grid area when the grid is its containing block, else
 * the grid's content box, and the child's `justify-self` and `align-self` (or the grid's
 * `*-items`) align it in there. The alignment is unsafe, so a child larger than the rectangle
 * overflows both sides when centered. `origin` is the grid's padding-box origin and `extent`
 * its padding-box (height, width).
 */
pub(super) fn static_position(
    context: &mut LayoutContext<'_>,
    grid: &Node,
    child: &Node,
    frame: &Frame,
    (origin, extent): ((f64, f64), [f64; 2]),
) -> EngineResult<(f64, f64)> {
    let [(top, bottom), (left, right)] = if contains(grid, child) {
        frame.area(grid, child, extent)
    } else {
        frame.content
    };
    let style = &grid.style;
    let margins = child.style.margin;
    let justify = super::justify_of(style, child);
    let (align, safe) = block_alignment(style, child);
    let aligned = |a| matches!(a, AlignItems::Center | AlignItems::End);
    let mut width = 0.0;
    if aligned(justify) || aligned(align) {
        width = super::item_width(context, child, right - left, AlignItems::Start)?;
    }
    let inline = safe_offset(justify, right - left - width - margins[1] - margins[3], {
        let own = child.style.justify_self.is_some();
        if own {
            child.style.align_safe[3]
        } else {
            style.align_safe[2]
        }
    });
    // Under rtl the start is the right edge: the static position is the box's right edge there.
    let x = if frame.rtl {
        context.mark_static_end(child.local_id);
        right - inline
    } else {
        left + inline
    };
    let y = if aligned(align) {
        let height = super::super::flex_layout::item_height(context, child, width)?;
        top + safe_offset(align, bottom - top - height - margins[0] - margins[2], safe)
    } else {
        top
    };
    Ok((origin.0 + x, origin.1 + y))
}
