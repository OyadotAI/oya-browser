/** Visual client geometry and raw layout metrics share host boxes without conflating their coordinate spaces. */
use super::{NodeBox, PageEnvironment};
use mars_core::geometry::{contains_quad, contains_rect, pin_shift};
use std::collections::BTreeSet;

/** Viewport-relative visual bounds include sticky/fixed movement while retaining full unclipped extent. */
pub(super) fn client(bounds: &NodeBox, environment: &PageEnvironment) -> (f64, f64, f64, f64) {
    (
        bounds.x - environment.scroll_x,
        bounds.y + pin_shift(bounds.pin, environment.scroll_y) - environment.scroll_y,
        bounds.width,
        bounds.height,
    )
}

/** Older hosts omit raw sizes; their existing untransformed rectangle remains a compatible fallback. */
pub(super) fn layout_size(bounds: &NodeBox) -> [f64; 2] {
    bounds
        .layout_size
        .filter(|size| size.iter().all(|value| value.is_finite() && *value >= 0.0))
        .unwrap_or([bounds.width, bounds.height])
}

/** Root layout extents ignore visual scaling/rotation and paint-time sticky offsets. */
pub(super) fn layout_bottom(bounds: &NodeBox) -> f64 {
    let y = bounds
        .layout_position
        .filter(|point| point.iter().all(|value| value.is_finite()))
        .map_or(bounds.y, |point| point[1]);
    y + layout_size(bounds)[1]
}

/** Candidate nodes are deduplicated in reverse paint order; realm membership is checked by the caller. */
pub(super) fn hits(environment: &PageEnvironment, point: [f64; 2]) -> Vec<usize> {
    if point[0] >= environment.viewport_width || point[1] >= environment.viewport_height {
        return Vec::new();
    }
    if !contains_rect(
        [
            0.0,
            0.0,
            environment.viewport_width,
            environment.viewport_height,
        ],
        point,
    ) {
        return Vec::new();
    }
    let point = [
        point[0] + environment.scroll_x,
        point[1] + environment.scroll_y,
    ];
    let mut seen = BTreeSet::new();
    environment
        .geometry
        .iter()
        .rev()
        .filter(|bounds| contains(bounds, point, environment.scroll_y))
        .filter_map(|bounds| seen.insert(bounds.node).then_some(bounds.node))
        .collect()
}

/** Pin translation moves the item and clip together, matching native painter ordering. */
fn contains(bounds: &NodeBox, point: [f64; 2], scroll_y: f64) -> bool {
    let point = [point[0], point[1] - pin_shift(bounds.pin, scroll_y)];
    contains_rect([bounds.x, bounds.y, bounds.width, bounds.height], point)
        && bounds.clip.is_none_or(|clip| contains_rect(clip, point))
        && bounds
            .quad
            .as_ref()
            .is_none_or(|quad| contains_quad(quad, point))
}
