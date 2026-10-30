/** Rounded rectangles normalize corner radii and append a complete closed contour atomically. */
use super::*;
use std::f64::consts::{FRAC_PI_2, PI};

impl Path {
    /** One to four two-axis corner radii follow CSS expansion, including reflected rectangle directions. */
    pub fn round_rect(
        &mut self,
        rect: [f64; 4],
        radii: &[Point],
        transform: Affine,
    ) -> EngineResult<()> {
        self.validate()?;
        if rect.iter().any(|value| !value.is_finite()) || !transform.finite() {
            return Err(invalid());
        }
        let [x, y, width, height] = rect;
        let corners = normalized(width.abs(), height.abs(), radii)?;
        let signs = [
            if width < 0.0 { -1.0 } else { 1.0 },
            if height < 0.0 { -1.0 } else { 1.0 },
        ];
        let transform = transform.compose(Affine([signs[0], 0.0, 0.0, signs[1], x, y]));
        let _allocation = Allocation::new(
            POINTS * (std::mem::size_of::<Point>() + std::mem::size_of::<Subpath>()),
        )?;
        let temporary = rectangle(width.abs(), height.abs(), corners, transform)?;
        self.admit(temporary.count())?;
        self.subpaths.extend(temporary.subpaths);
        Ok(())
    }
}

/** Normalize by the largest radius before summing, so finite huge inputs never overflow into zero radii. */
fn normalized(width: f64, height: f64, radii: &[Point]) -> EngineResult<[Point; 4]> {
    if radii
        .iter()
        .flatten()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid());
    }
    let mut corners = match radii {
        [all] => [*all; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return Err(invalid()),
    };
    let largest = corners.iter().flatten().copied().fold(0.0, f64::max);
    if largest == 0.0 {
        return Ok(corners);
    }
    for radius in &mut corners {
        *radius = [radius[0] / largest, radius[1] / largest];
    }
    let [ul, ur, lr, ll] = corners;
    let mut scale = largest;
    for (edge, sum) in [
        (width, ul[0] + ur[0]),
        (height, ur[1] + lr[1]),
        (width, lr[0] + ll[0]),
        (height, ll[1] + ul[1]),
    ] {
        if sum > 0.0 {
            scale = scale.min(edge / sum);
        }
    }
    for radius in &mut corners {
        *radius = [radius[0] * scale, radius[1] * scale];
    }
    Ok(corners)
}

/** A local positive rectangle makes negative widths/heights precise reflections, preserving winding direction. */
fn rectangle(
    width: f64,
    height: f64,
    corners: [Point; 4],
    transform: Affine,
) -> EngineResult<Path> {
    let [ul, ur, lr, ll] = corners;
    let mut path = Path::default();
    path.move_to([ul[0], 0.0], transform)?;
    corner(
        &mut path,
        [width - ur[0], ur[1]],
        ur,
        [-FRAC_PI_2, 0.0],
        [width, ur[1]],
        transform,
    )?;
    corner(
        &mut path,
        [width - lr[0], height - lr[1]],
        lr,
        [0.0, FRAC_PI_2],
        [width - lr[0], height],
        transform,
    )?;
    corner(
        &mut path,
        [ll[0], height - ll[1]],
        ll,
        [FRAC_PI_2, PI],
        [0.0, height - ll[1]],
        transform,
    )?;
    corner(
        &mut path,
        [ul[0], ul[1]],
        ul,
        [PI, PI + FRAC_PI_2],
        [ul[0], 0.0],
        transform,
    )?;
    path.close();
    path.move_to([0.0, 0.0], transform)?;
    Ok(path)
}

/** A collapsed corner is a straight endpoint connection, not the ellipse API's center-line special case. */
fn corner(
    path: &mut Path,
    center: Point,
    radii: Point,
    angles: [f64; 2],
    end: Point,
    transform: Affine,
) -> EngineResult<()> {
    if radii.contains(&0.0) {
        let (sin, cos) = angles[0].sin_cos();
        path.line_to(
            [center[0] + radii[0] * cos, center[1] + radii[1] * sin],
            transform,
        )?;
        return path.line_to(end, transform);
    }
    path.ellipse(center, radii, 0.0, angles, false, transform)
}

/** Bindings report WebIDL/RangeError details; direct Core callers still reject invalid contracts before mutation. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid canvas rounded rectangle arguments.")
}
