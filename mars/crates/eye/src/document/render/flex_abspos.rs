/** Static positions of a flex container's absolutely positioned children: each sits where it
would as the container's sole flex item (css-flexbox-1 4.1), so `justify-content` and
`align-self` move it, rather than at the content box's corner. */
use super::{
    flex_layout::{item_intrinsic, justification, physical_align, row_justify, self_alignment},
    sizing::{LayoutContext, border_height, border_width},
};
use crate::{
    document::{AlignItems, FlexDirection, JustifyContent, Node},
    protocol::EngineResult,
};

/** The margin-box origin of `child` laid out as the sole item of `parent`, whose content box
starts at `origin` and measures `inner` (width, height) once its items are placed. Auto margins
count as zero, as Blink's static position takes them. */
pub(super) fn static_position(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
    origin: (f64, f64),
    inner: (f64, f64),
) -> EngineResult<(f64, f64)> {
    let (width, height) = border_size(context, child, inner)?;
    let margin = |side: usize| {
        if child.style.margin_auto[side] {
            0.0
        } else {
            child.style.margin[side]
        }
    };
    let free_x = inner.0 - width - margin(1) - margin(3);
    let free_y = inner.1 - height - margin(0) - margin(2);
    let (x, y) = if parent.style.flex_direction == FlexDirection::Column {
        (
            cross_offset(parent, child, free_x),
            main_offset(parent, free_y, false),
        )
    } else {
        (
            main_offset(parent, free_x, true),
            cross_offset(parent, child, free_y),
        )
    };
    if centered_x(parent, child, free_x) {
        // The box centers on the line's middle; place_in centers its width there.
        context.mark_static_center(child.local_id);
        return Ok((origin.0 + inner.0 / 2.0, origin.1 + y));
    }
    Ok((origin.0 + x, origin.1 + y))
}

/** Whether the container centers the child horizontally: `justify-content: center` in a row,
`align-self: center` in a column, unless `safe` alignment falls back to the start for a child
that overflows (`cross_offset`). */
fn centered_x(parent: &Node, child: &Node, free_x: f64) -> bool {
    if parent.style.flex_direction == FlexDirection::Column {
        cross_offset(parent, child, free_x) == free_x / 2.0
            && physical_align(parent, self_alignment(parent, child)) == AlignItems::Center
    } else {
        row_justify(parent) == JustifyContent::Center
    }
}

/** The child's border-box size: its authored or shrink-to-fit width in the container's inner
width, and its authored height or the height its content takes at that width. */
fn border_size(
    context: &mut LayoutContext<'_>,
    child: &Node,
    (inner_width, inner_height): (f64, f64),
) -> EngineResult<(f64, f64)> {
    let natural = item_intrinsic(context, child)?.max_content;
    let width = border_width(child, inner_width, inner_width, Some(natural));
    let authored = child
        .style
        .height
        .and_then(|height| height.resolve_size(Some(inner_height)))
        .map(|height| border_height(child, height));
    let height = match authored {
        Some(height) => height,
        None => super::flex_layout::item_height(context, child, width)?,
    };
    Ok((width, height))
}

/** The main-axis offset `justify-content` gives a sole item with `free` space: a distribution
centers it or falls back to the start, and a right-to-left row measures from the right. */
fn main_offset(parent: &Node, free: f64, row: bool) -> f64 {
    let kind = match (row, parent.style.justify_content) {
        (true, _) => row_justify(parent),
        (false, JustifyContent::Left | JustifyContent::Right) => JustifyContent::LogicalStart,
        (false, kind) => kind,
    };
    let (lead, _) = justification(kind, free, 1, parent.style.flex_reverse);
    if row && parent.style.rtl {
        free - lead
    } else {
        lead
    }
}

/** The cross-axis offset the child's `align-self` gives it in `free` space; `wrap-reverse`
swaps start and end, and stretch and baseline sit at the start. `safe` alignment falls back to
the start only when the child overflows its containing block, which is known here when the
container is positioned and so is that block; an outer one leaves the alignment unsafe. */
fn cross_offset(parent: &Node, child: &Node, free: f64) -> f64 {
    let containing = parent.style.position != crate::document::style::Position::Static;
    let free = if containing {
        super::flex_layout::safe_free(child, free)
    } else {
        free
    };
    match physical_align(parent, self_alignment(parent, child)) {
        AlignItems::Center => free / 2.0,
        AlignItems::End => free,
        _ => 0.0,
    }
}
