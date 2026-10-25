/** `filter: blur()` on a solid box, painted as the soft glow Blink's blur filter produces.
 * Blink rasterizes the box and convolves it with a Gaussian (FEGaussianBlur, extent 3 sigma);
 * Eye's display list has no filter, so the blurred result is described as a radial gradient. */
use super::{DisplayBackground, DisplayItem, GradientStop, svg};

/** Blink grows a blurred box's visual rect by three standard deviations on every side. */
const EXTENT: f64 = 3.0;
/** How far below half the box a corner radius may fall and the box still read as round. */
const ROUND_SLACK: f64 = 0.5;
/** Samples of the blurred profile from the center out to the edge of the grown box. */
const SAMPLES: usize = 12;

/**
 * Replaces a blurred box's solid fill with its blurred profile: the box grows by 3 sigma and a
 * radial ramp carries the coverage of the shape under the Gaussian at each distance from its
 * center. Boxes with text, drawings or image layers are left as they are.
 * ponytail: the profile is an erf edge scaled so its center matches a blurred disc; a rectangle
 * reads rounder than Blink's. A real blur in the desktop painter is the upgrade.
 */
pub(super) fn glow(item: &mut DisplayItem, sigma: f64) {
    if sigma <= 0.0
        || !item.lines.is_empty()
        || !item.vector.is_empty()
        || !item.backgrounds.is_empty()
        || item.tag == "img"
    {
        return;
    }
    let Some(color) = item.background.as_deref().and_then(svg::channels) else {
        return;
    };
    if color[3] <= 0.0 {
        return;
    }
    let half = (item.width.min(item.height) / 2.0).max(0.0);
    let spread = EXTENT * sigma;
    // A slight blur of a square-cornered box barely moves its edges; the radial profile only
    // describes a round box or one the blur swamps.
    let round = item
        .radius
        .iter()
        .all(|corner| *corner >= half - ROUND_SLACK);
    if !round && spread < half {
        return;
    }
    let reach = half + spread;
    let center = disc_center(half, sigma) / phi(half / sigma).max(f64::EPSILON);
    let stops = (0..=SAMPLES)
        .map(|step| {
            let position = step as f64 / SAMPLES as f64;
            let coverage = (phi((half - position * reach) / sigma) * center).clamp(0.0, 1.0);
            let alpha = color[3] * coverage;
            GradientStop {
                color: svg::literal([color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha]),
                position,
            }
        })
        .collect();
    item.x -= spread;
    item.y -= spread;
    item.width += 2.0 * spread;
    item.height += 2.0 * spread;
    item.backgrounds = vec![DisplayBackground::Radial {
        cx: item.width / 2.0,
        cy: item.height / 2.0,
        rx: item.width / 2.0,
        ry: item.height / 2.0,
        stops,
    }];
    item.background = None;
    item.radius = [0.0; 4];
    item.shadows.clear();
}
/** Coverage at the center of a disc of `radius` blurred by `sigma`: 1 - exp(-r^2 / 2 sigma^2). */
fn disc_center(radius: f64, sigma: f64) -> f64 {
    1.0 - (-(radius * radius) / (2.0 * sigma * sigma)).exp()
}
/** The standard normal CDF, via Abramowitz and Stegun 7.1.26 (error under 1.5e-7). */
fn phi(x: f64) -> f64 {
    const P: f64 = 0.327_591_1;
    const A: [f64; 5] = [
        0.254_829_592,
        -0.284_496_736,
        1.421_413_741,
        -1.453_152_027,
        1.061_405_429,
    ];
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + P * z);
    let poly = A.iter().rev().fold(0.0, |sum, a| sum * t + a) * t;
    let erf = 1.0 - poly * (-z * z).exp();
    0.5 * (1.0 + erf.copysign(x))
}
