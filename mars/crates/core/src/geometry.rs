/** Inert affine box geometry shared by DOM measurement, native clicks and presentation export. */
use crate::canvas::Affine;

/** Ordered corners follow the rectangle perimeter, retaining orientation under mirrored transforms. */
pub type Quad = [[f64; 2]; 4];
/** Layout observers reject dimensions beyond the native geometry transport's finite scalar envelope. */
const BOX_METRICS_LIMIT: f64 = 1_000_000_000.0;

/** Actual untransformed layout measurements; absence means the host has not measured these boxes. */
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxMetrics {
    /// Content rectangle relative to the padding edge: padding-left, padding-top, content width and height.
    pub content: [f64; 4],
    /// Physical border-box width and height, excluding visual transforms.
    pub border: [f64; 2],
    /// Logical inline/block axes exchange physical width/height in vertical writing modes.
    pub vertical: bool,
}

/** Wrapper elements have no paint-node identity, so their measured boxes travel separately. */
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WrapperMetrics {
    /// Actual root layout box, absent until measured by the host.
    pub html: Option<BoxMetrics>,
    /// Actual body layout box, absent until measured by the host.
    pub body: Option<BoxMetrics>,
}

impl BoxMetrics {
    /** Transport cannot introduce negative extents, nonfinite values or unbounded observer arithmetic. */
    pub fn valid(&self) -> bool {
        self.content
            .iter()
            .chain(&self.border)
            .all(|value| value.is_finite() && (0.0..=BOX_METRICS_LIMIT).contains(value))
    }
}

/** Map the untransformed border rectangle through the same affine matrix used by native paint. */
pub fn quad([x, y, width, height]: [f64; 4], transform: Option<[f64; 6]>) -> Quad {
    let matrix = transform.map(Affine).unwrap_or(Affine::IDENTITY);
    [
        [x, y],
        [x + width, y],
        [x + width, y + height],
        [x, y + height],
    ]
    .map(|point| matrix.map(point))
}

/** Bounds include the entire transformed rectangle; overflow clips do not shrink DOM client rectangles. */
pub fn bounds(quad: &Quad) -> Option<[f64; 4]> {
    if quad.iter().flatten().any(|value| !value.is_finite()) {
        return None;
    }
    let left = quad
        .iter()
        .map(|point| point[0])
        .fold(f64::INFINITY, f64::min);
    let top = quad
        .iter()
        .map(|point| point[1])
        .fold(f64::INFINITY, f64::min);
    let right = quad
        .iter()
        .map(|point| point[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = quad
        .iter()
        .map(|point| point[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let bounds = [left, top, right - left, bottom - top];
    bounds
        .iter()
        .all(|value| value.is_finite())
        .then_some(bounds)
}

/** Empty or nonfinite rectangles never acquire a hit, even when their collapsed edge contains the point. */
pub fn contains_rect([x, y, width, height]: [f64; 4], point: [f64; 2]) -> bool {
    [x, y, width, height, point[0], point[1]]
        .iter()
        .all(|value| value.is_finite())
        && width > 0.0
        && height > 0.0
        && (x + width).is_finite()
        && (y + height).is_finite()
        && point[0] >= x
        && point[0] <= x + width
        && point[1] >= y
        && point[1] <= y + height
}

/** Convex, nondegenerate quadrilaterals admit their boundary but reject empty AABB corners. */
pub fn contains_quad(quad: &Quad, point: [f64; 2]) -> bool {
    if !point.iter().all(|value| value.is_finite()) || bounds(quad).is_none() {
        return false;
    }
    let mut orientation = 0.0;
    for index in 0..quad.len() {
        let a = quad[index];
        let b = quad[(index + 1) % quad.len()];
        let c = quad[(index + 2) % quad.len()];
        let turn = cross(a, b, c);
        if !turn.is_finite() || turn == 0.0 {
            return false;
        }
        if orientation == 0.0 {
            orientation = turn.signum();
        }
        let side = cross(a, b, point);
        if !side.is_finite() || turn.signum() != orientation || side * orientation < 0.0 {
            return false;
        }
    }
    true
}

/** Oriented area avoids inversion and keeps reflected 2D maps in the same hit algorithm. */
fn cross(a: [f64; 2], b: [f64; 2], point: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0])
}

/** Sticky and fixed movement follows Eye's pin model without modifying raw layout measurements. */
pub fn pin_shift(pin: Option<[f64; 3]>, scroll_y: f64) -> f64 {
    let Some([top, start, room]) = pin else {
        return 0.0;
    };
    if ![top, start, room, scroll_y]
        .iter()
        .all(|value| value.is_finite())
        || room < 0.0
    {
        return 0.0;
    }
    (scroll_y.max(0.0) + top - start).clamp(0.0, room) - (top - start).clamp(0.0, room)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflected_affine_quads_keep_orientation_independent_hit_results() {
        let quad = quad(
            [0.0, 0.0, 100.0, 50.0],
            Some([-1.0, 0.0, 0.0, 2.0, 100.0, 10.0]),
        );
        assert_eq!(bounds(&quad), Some([0.0, 10.0, 100.0, 100.0]));
        assert!(contains_quad(&quad, [50.0, 50.0]));
        assert!(!contains_quad(&quad, [50.0, 5.0]));
    }

    #[test]
    fn singular_nonfinite_and_self_crossing_quads_have_no_hit_area() {
        for quad in [
            [[0.0; 2]; 4],
            [[f64::NAN; 2]; 4],
            [[0.0, 0.0], [10.0, 10.0], [0.0, 10.0], [10.0, 0.0]],
        ] {
            assert!(!contains_quad(&quad, [5.0, 5.0]));
        }
    }
}
