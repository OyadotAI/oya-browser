/** Stack-only clipping computes actual quadrilateral coverage without uncharged pixel buffers. */
use super::{Affine, EngineError, EngineResult};

/** Convex quadrilateral clipping against a box produces at most eight vertices. */
const VERTICES: usize = 8;

/** Negative extents grow in the other direction without changing image orientation. */
pub(super) fn normalized([x, y, width, height]: [f64; 4]) -> EngineResult<[f64; 4]> {
    let right = x + width;
    let bottom = y + height;
    if [x, y, width, height, right, bottom]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(unrepresentable());
    }
    Ok([x.min(right), y.min(bottom), width.abs(), height.abs()])
}

/** A transformed rectangle remains a convex quadrilateral, including reflected transforms. */
pub(super) fn quadrilateral(rect: [f64; 4], matrix: Affine) -> EngineResult<[[f64; 2]; 4]> {
    let [x, y, width, height] = rect;
    let polygon = [
        [x, y],
        [x + width, y],
        [x + width, y + height],
        [x, y + height],
    ]
    .map(|point| matrix.map(point));
    if polygon.iter().flatten().any(|value| !value.is_finite()) {
        return Err(unrepresentable());
    }
    Ok(polygon)
}

/** Arithmetic range is a drawing limitation, distinct from a forged native bitmap or plan. */
fn unrepresentable() -> EngineError {
    EngineError::new(
        "canvas_geometry",
        "Canvas geometry exceeds this build's numeric range.",
        "Use coordinates and transforms whose derived geometry remains finite.",
    )
}

/** Clip in floating point before converting to bounded bitmap indices. */
pub(super) fn bounds(polygon: &[[f64; 2]; 4], size: [u32; 2]) -> [u32; 4] {
    let minimum = |axis| {
        polygon
            .iter()
            .map(|point| point[axis])
            .fold(f64::INFINITY, f64::min)
    };
    let maximum = |axis| {
        polygon
            .iter()
            .map(|point| point[axis])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    [
        minimum(0).floor().clamp(0.0, f64::from(size[0])) as u32,
        minimum(1).floor().clamp(0.0, f64::from(size[1])) as u32,
        maximum(0).ceil().clamp(0.0, f64::from(size[0])) as u32,
        maximum(1).ceil().clamp(0.0, f64::from(size[1])) as u32,
    ]
}

/** Return exact polygon/pixel intersection area rather than a supersampling approximation. */
pub(super) fn coverage(polygon: &[[f64; 2]; 4], x: u32, y: u32) -> f64 {
    let mut points = [[0.0; 2]; VERTICES];
    points[..polygon.len()].copy_from_slice(polygon);
    let mut count = polygon.len();
    for (axis, edge, greater) in [
        (0, f64::from(x), true),
        (0, f64::from(x) + 1.0, false),
        (1, f64::from(y), true),
        (1, f64::from(y) + 1.0, false),
    ] {
        (points, count) = clip(&points, count, axis, edge, greater);
    }
    area(&points[..count]).clamp(0.0, 1.0)
}

/** Each clipping pass only adds the two possible intersections of a convex polygon. */
fn clip(
    input: &[[f64; 2]; VERTICES],
    count: usize,
    axis: usize,
    edge: f64,
    greater: bool,
) -> ([[f64; 2]; VERTICES], usize) {
    let mut output = [[0.0; 2]; VERTICES];
    let mut length = 0;
    if count == 0 {
        return (output, length);
    }
    let inside = |point: [f64; 2]| {
        if greater {
            point[axis] >= edge
        } else {
            point[axis] <= edge
        }
    };
    let mut previous = input[count - 1];
    for current in input.iter().take(count).copied() {
        if inside(current) != inside(previous) {
            output[length] = intersection(previous, current, axis, edge);
            length += 1;
        }
        if inside(current) {
            output[length] = current;
            length += 1;
        }
        previous = current;
    }
    (output, length)
}

/** A scaled ratio avoids overflowing subtraction for valid but very distant endpoints. */
fn intersection(from: [f64; 2], to: [f64; 2], axis: usize, edge: f64) -> [f64; 2] {
    let scale = from[axis]
        .abs()
        .max(to[axis].abs())
        .max(edge.abs())
        .max(1.0);
    let amount = (edge / scale - from[axis] / scale) / (to[axis] / scale - from[axis] / scale);
    let other = 1 - axis;
    let mut point = [0.0; 2];
    point[axis] = edge;
    point[other] = from[other] * (1.0 - amount) + to[other] * amount;
    point
}

/** After clipping, all coordinates lie inside one pixel, keeping the shoelace sum bounded. */
fn area(points: &[[f64; 2]]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let origin = points[0];
    points
        .windows(2)
        .skip(1)
        .map(|pair| {
            let a = [pair[0][0] - origin[0], pair[0][1] - origin[1]];
            let b = [pair[1][0] - origin[0], pair[1][1] - origin[1]];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>()
        .abs()
        * 0.5
}
