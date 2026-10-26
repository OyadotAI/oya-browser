/** A new formatting context beside floats whose height depends on its width (CSS 2.1 9.5, Blink's
 * LayoutNewFormattingContext): placed in the band beside the floats at its top, it may grow tall
 * enough to reach a wider float further down, and then it must take the narrower band that
 * float leaves (floats-bfc-003: 200px wide it is 50px tall and overlaps the second float, so it
 * takes 100px and becomes 100px tall). */
use super::flow::*;

/** Narrowing passes: measure at the band's width, then at the narrower band its height reaches. */
const PASSES: usize = 2;

/** The (left, room) a formatting-context root at `top` keeps once its own height is known,
 * starting from the band `placed` at its top. Only an auto-width, auto-height box without side
 * margins changes, since only its width follows the band.
 *
 * ponytail: a box that fits no narrower band stays where it was; Blink then moves it below. */
pub(super) fn refit(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &Flow,
    (top, placed): (f64, (f64, f64)),
) -> EngineResult<(f64, f64)> {
    let style = &node.style;
    let adjusts = style.width.is_none()
        && style.height.is_none()
        && style.margin[1] == 0.0
        && style.margin[3] == 0.0
        && !flow.floats.is_empty()
        // A generated box's stand-in shares its element's id, so measuring it would lay out the
        // element again, and its ::after again, without end (a clearfix `display: table`).
        && !super::generated::is_generated(node);
    if !adjusts {
        return Ok(placed);
    }
    let (mut left, mut room) = placed;
    for _ in 0..PASSES {
        let width = border_width(node, flow.width, room, None);
        let height = context.measure_height(node.local_id, width)?;
        let (band_left, band) = flow.room_at_height(top, height.max(super::float::LINE_FIT_SLACK));
        if band + super::float::LINE_FIT_SLACK >= room {
            break;
        }
        (left, room) = (band_left, band);
    }
    Ok((left, room))
}
