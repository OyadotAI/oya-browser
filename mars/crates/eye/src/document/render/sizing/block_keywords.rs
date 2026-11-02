/** Content keywords in `min-height` and `max-height`. On the block axis `min-content`,
 * `max-content` and `fit-content` all name the content height (Blink's
 * ResolveMinBlockLength/ResolveMaxBlockLength take the intrinsic block size for each), so a
 * `min-height: max-content; height: 0` box still wraps its content and a `max-height:
 * max-content` box grows no taller than it. */
use crate::document::Node;

/**
 * Clamps a border-box `height`, already clamped by the length bounds, by the keyword bounds, with
 * `natural` the border-box content height: the keyword max first, then the minimum (a keyword or
 * the length one), which wins as CSS 2.1 10.7 has it.
 */
pub(in crate::document::render) fn clamp_content_bounds(
    node: &Node,
    height: f64,
    natural: f64,
    definite: Option<f64>,
) -> f64 {
    let [min_fit, max_fit] = node.style.height_bounds_fit;
    if !min_fit && !max_fit {
        return height;
    }
    let capped = if max_fit { height.min(natural) } else { height };
    let min = if min_fit {
        natural
    } else {
        super::flow::clamp_height(node, 0.0, definite)
    };
    capped.max(min)
}

/** Whether a keyword bound needs the content height, so the box cannot be sized without layout. */
pub(in crate::document::render) fn has_content_bounds(node: &Node) -> bool {
    node.style.height_bounds_fit.contains(&true)
}
