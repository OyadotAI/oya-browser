/** Where a table box goes and how wide it is: a table is a new formatting context, so like any
 * such box it sits beside earlier floats, in the first opportunity its width fits (CSS 2.1 9.5,
 * Blink's LayoutNewFormattingContext), instead of overlapping them (clear-applies-to-001's
 * anonymous table belongs to the right of the floated paragraph). */
use super::super::float::LINE_FIT_SLACK;
use super::super::flow::*;

/** Passes a table takes in one band: measured at the band's width, then shrunk to the narrower
 * band its height reaches past further floats (table-among-floats-001). */
const FIT_PASSES: usize = 2;

/** The table's border-box width and its (x, y). `bounds` is the grid's (minimum, maximum)
 * border-box width; `centered` is a `<center>` parent's legacy centering. */
pub(super) fn frame(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &Flow,
    forced: Option<BorderBox>,
    (bounds, centered): ((f64, f64), bool),
) -> EngineResult<(f64, f64, f64)> {
    if let Some(forced) = forced {
        return Ok((forced.width, forced.x, forced.y));
    }
    let style = &node.style;
    let margins = style.margin[1] + style.margin[3];
    // CSS 2.1 9.5.2: a table's `clear` puts its border edge below the cleared floats
    // (clear-applies-to-013).
    let top = flow.block_top(style.margin[0]);
    let top = flow
        .clearance(style.clear)
        .map_or(top, |clear| top.max(clear));
    let width_in = |avail: f64| width(node, flow.width, (avail - margins).max(0.0), bounds);
    let (y, left, avail) = if super::is_anonymous(node) || flow.floats.is_empty() {
        (top, flow.x, flow.width)
    } else {
        beside(context, node, flow, top, (&width_in, margins))?
    };
    let width = width_in(avail);
    let mut probe = style.clone();
    probe.margin_auto[1] |= centered;
    probe.margin_auto[3] |= centered;
    // Beside a float the border box starts at the float's edge: a negative margin never pulls
    // it over the float (Blink's LayoutNewFormattingContext clamps it at zero).
    let x = if left > flow.x {
        left + style.margin[3].max(0.0)
    } else {
        left + super::super::body::margin_start(&probe, avail, width)
    };
    Ok((width, x, y))
}
/** The table's width in `room`: an anonymous table's wrapping block keeps its own width, an
 * authored width sizes the box `box-sizing` says (a `<table>`'s border box, from the UA sheet)
 * and never below the grid's minimum, and an auto width shrinks to fit. */
fn width(node: &Node, base: f64, room: f64, (minimum, maximum): (f64, f64)) -> f64 {
    if super::is_anonymous(node) {
        border_width(node, base, room, None)
    } else if node.style.width.is_some() {
        border_width(node, base, room, None).max(minimum)
    } else {
        maximum.min(room).max(minimum)
    }
}
/** How wide the table is in a band of some room, and its horizontal margins. */
type Sizer<'s> = (&'s dyn Fn(f64) -> f64, f64);
/** The first band at or below `y` the table fits beside floats over its whole height, as
 * (y, left, room); below the last float the whole line is free. */
fn beside(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &Flow,
    mut y: f64,
    sizer: Sizer<'_>,
) -> EngineResult<(f64, f64, f64)> {
    for _ in 0..=flow.floats.len() {
        if let Some((left, room)) = fit_at(context, node, flow, y, sizer)? {
            return Ok((y, left, room));
        }
        let next = flow
            .floats
            .iter()
            .filter(|f| f.bottom > y + LINE_FIT_SLACK)
            .map(|f| f.bottom)
            .fold(f64::INFINITY, f64::min);
        if !next.is_finite() {
            break;
        }
        y = next;
    }
    let (left, room) = flow.room_at(y);
    Ok((y, left, room))
}
/** The (left, room) of the band at `y` when the table fits it over its own height. */
fn fit_at(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &Flow,
    y: f64,
    (width_in, margins): Sizer<'_>,
) -> EngineResult<Option<(f64, f64)>> {
    let (_, room) = flow.room_at(y);
    let mut width = width_in(room);
    for _ in 0..FIT_PASSES {
        let height = context.measure_height(node.local_id, width)?;
        let (left, band) = flow.room_at_height(y, height.max(LINE_FIT_SLACK));
        if width + margins <= band + LINE_FIT_SLACK {
            return Ok(Some((left, band)));
        }
        width = width_in(band);
    }
    Ok(None)
}
