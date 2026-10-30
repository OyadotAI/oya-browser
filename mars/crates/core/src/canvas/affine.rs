/** Canvas affine matrices keep the Web API's six-component order across engine boundaries. */
use serde::{Deserialize, Serialize};

/** A two-dimensional affine map in Canvas order: a, b, c, d, e, f. */
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine(pub [f64; 6]);

impl Default for Affine {
    /** Every fresh drawing state starts in bitmap coordinates. */
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    /** Identity preserves the existing host and raster behavior. */
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /** Private serialized state must never contain a nonfinite matrix. */
    pub fn finite(self) -> bool {
        self.0.iter().all(|component| component.is_finite())
    }

    /** Canvas operations post-multiply, so later local translation follows earlier scaling. */
    pub fn compose(self, rhs: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = rhs.0;
        Self([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }

    /** Map a point without allocation; callers validate derived geometry before rasterization. */
    pub fn map(self, [x, y]: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * x + c * y + e, b * x + d * y + f]
    }

    /** Singular or numerically unrepresentable transforms produce no raster samples. */
    pub fn inverse(self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.0;
        let scale = a.abs().max(b.abs()).max(c.abs()).max(d.abs());
        if !self.finite() || scale == 0.0 {
            return None;
        }
        let direct = a * d - b * c;
        let scale = if direct.is_finite() && direct != 0.0 {
            1.0
        } else {
            scale
        };
        let determinant = (a / scale) * (d / scale) - (b / scale) * (c / scale);
        if determinant == 0.0 {
            return None;
        }
        let [ia, ib, ic, id] =
            [d / scale, -b / scale, -c / scale, a / scale].map(|value| value / determinant / scale);
        let inverse = Self([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)]);
        inverse.finite().then_some(inverse)
    }
}
