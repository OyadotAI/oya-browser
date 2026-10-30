/** Endpoint SVG arcs normalize radii and emit at most one cubic per quarter turn. */
use super::*;
use std::f64::consts::FRAC_PI_2;
/** Endpoint arcs never sweep more than one complete ellipse. */
const MAX_PIECES: f64 = 4.0;

/// SVG 2 Appendix B.2.5 and F.6: an endpoint arc becomes at most four cubics, one per quarter
/// turn. Zero radii draw a line and a coincident end draws nothing, as the specification says.
pub(super) fn cubics(
    from: (f64, f64),
    radii: (f64, f64),
    rotation: f64,
    large: bool,
    sweep: bool,
    to: (f64, f64),
) -> EngineResult<Vec<[f64; 6]>> {
    if from == to {
        return Ok(Vec::new());
    }
    if radii.0 == 0.0 || radii.1 == 0.0 {
        return Ok(vec![[from.0, from.1, to.0, to.1, to.0, to.1]]);
    }
    let ellipse = Ellipse::new(from, radii, rotation, large, sweep, to)?;
    if ![
        ellipse.center.0,
        ellipse.center.1,
        ellipse.radii.0,
        ellipse.radii.1,
        ellipse.start,
        ellipse.delta,
    ]
    .iter()
    .all(|value| value.is_finite())
    {
        return Err(numeric());
    }
    let pieces = (ellipse.delta.abs() / FRAC_PI_2).ceil().max(1.0);
    if pieces > MAX_PIECES {
        return Err(numeric());
    }
    let step = ellipse.delta / pieces;
    // Each piece's controls sit k along the tangents, with k = 4/3 tan(step / 4).
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let result: Vec<_> = (0..pieces as usize)
        .map(|index| {
            let (a, b) = (
                ellipse.start + step * index as f64,
                ellipse.start + step * (index + 1) as f64,
            );
            let (p0, d0) = (ellipse.point(a), ellipse.tangent(a));
            let (p1, d1) = (ellipse.point(b), ellipse.tangent(b));
            [
                p0.0 + k * d0.0,
                p0.1 + k * d0.1,
                p1.0 - k * d1.0,
                p1.1 - k * d1.1,
                p1.0,
                p1.1,
            ]
        })
        .collect();
    if result.iter().flatten().any(|value| !value.is_finite()) {
        return Err(numeric());
    }
    Ok(result)
}

/** The center parameterization of one endpoint arc (SVG 2 Appendix B.2.4 and B.2.5). */
struct Ellipse {
    center: (f64, f64),
    radii: (f64, f64),
    /** Cosine and sine of the x-axis rotation. */
    axis: (f64, f64),
    /** The start angle and the signed sweep, in radians. */
    start: f64,
    delta: f64,
}
impl Ellipse {
    /** Out-of-range radii scale up until the end point is reachable, per B.2.5. */
    fn new(
        from: (f64, f64),
        radii: (f64, f64),
        rotation: f64,
        large: bool,
        sweep: bool,
        to: (f64, f64),
    ) -> EngineResult<Self> {
        let (sin, cos) = rotation.sin_cos();
        let (hx, hy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
        let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
        let lambda = (x1 / radii.0).powi(2) + (y1 / radii.1).powi(2);
        let scale = lambda.sqrt().max(1.0);
        let (rx, ry) = (radii.0 * scale, radii.1 * scale);
        let numerator = (rx * ry).powi(2) - (rx * y1).powi(2) - (ry * x1).powi(2);
        let denominator = (rx * y1).powi(2) + (ry * x1).powi(2);
        if ![lambda, scale, rx, ry, numerator, denominator]
            .iter()
            .all(|value| value.is_finite())
            || denominator == 0.0
        {
            return Err(numeric());
        }
        let sign = if large == sweep { -1.0 } else { 1.0 };
        let ratio = numerator / denominator;
        if !ratio.is_finite() {
            return Err(numeric());
        }
        let root = sign * ratio.max(0.0).sqrt();
        let (cx1, cy1) = (root * rx * y1 / ry, -root * ry * x1 / rx);
        let center = (
            cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0,
            sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0,
        );
        let start = ((y1 - cy1) / ry).atan2((x1 - cx1) / rx);
        let end = ((-y1 - cy1) / ry).atan2((-x1 - cx1) / rx);
        let mut delta = end - start;
        if sweep && delta < 0.0 {
            delta += 2.0 * std::f64::consts::PI;
        } else if !sweep && delta > 0.0 {
            delta -= 2.0 * std::f64::consts::PI;
        }
        Ok(Self {
            center,
            radii: (rx, ry),
            axis: (cos, sin),
            start,
            delta,
        })
    }
    /** The point at parametric angle `t`. */
    fn point(&self, t: f64) -> (f64, f64) {
        let (x, y) = (self.radii.0 * t.cos(), self.radii.1 * t.sin());
        self.rotate(x, y, self.center)
    }
    /** The derivative at `t`, the direction the cubic controls follow. */
    fn tangent(&self, t: f64) -> (f64, f64) {
        let (x, y) = (-self.radii.0 * t.sin(), self.radii.1 * t.cos());
        self.rotate(x, y, (0.0, 0.0))
    }
    /** Applies the x-axis rotation and then an offset. */
    fn rotate(&self, x: f64, y: f64, offset: (f64, f64)) -> (f64, f64) {
        let (cos, sin) = self.axis;
        (cos * x - sin * y + offset.0, sin * x + cos * y + offset.1)
    }
}
