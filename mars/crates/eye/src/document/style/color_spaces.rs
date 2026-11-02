/**
 * CSS Color 4 and 5 color spaces and `color-mix()`, as Blink's Color and ColorMix compute them:
 * `lab()`, `lch()`, `hwb()` and `color()` convert through CIE XYZ to sRGB, and `color-mix()`
 * interpolates premultiplied components in the named space, hues along the shorter arc.
 * Results are clipped into the sRGB gamut, which is what the painter draws.
 */
use super::unsupported;
use crate::protocol::EngineResult;

/** Linear-light sRGB channels and alpha. */
pub(super) type Linear = [f64; 4];

/// CIE Lab constants (CSS Color 4 sample code): kappa and epsilon.
const KAPPA: f64 = 24389.0 / 27.0;
const EPSILON: f64 = 216.0 / 24389.0;
/// The D50 white point Lab is relative to.
const D50: [f64; 3] = [0.964_295_676_429_567_7, 1.0, 0.825_104_602_510_460_2];
/// Linear sRGB to CIE XYZ (D65).
const SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [
        0.412_390_799_265_959_34,
        0.357_584_339_383_878,
        0.180_480_788_401_834_3,
    ],
    [
        0.212_639_005_871_510_27,
        0.715_168_678_767_756,
        0.072_192_315_360_733_71,
    ],
    [
        0.019_330_818_715_591_82,
        0.119_194_779_794_625_98,
        0.950_532_152_249_660_7,
    ],
];
/// CIE XYZ (D65) to linear sRGB.
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [
        3.240_969_941_904_522_6,
        -1.537_383_177_570_094,
        -0.498_610_760_293_003_4,
    ],
    [
        -0.969_243_636_280_879_6,
        1.875_967_501_507_720_2,
        0.041_555_057_407_175_59,
    ],
    [
        0.055_630_079_696_993_66,
        -0.203_976_958_888_976_52,
        1.056_971_514_242_878_6,
    ],
];
/// Bradford adaptation from D50 to D65.
const D50_TO_D65: [[f64; 3]; 3] = [
    [
        0.955_473_421_488_075,
        -0.023_098_454_948_764_71,
        0.063_259_243_200_570_72,
    ],
    [
        -0.028_369_709_333_863_7,
        1.009_995_398_081_304_1,
        0.021_041_441_191_917_323,
    ],
    [
        0.012_314_014_864_481_998,
        -0.020_507_649_298_898_964,
        1.330_365_926_242_124,
    ],
];
/// Bradford adaptation from D65 to D50.
const D65_TO_D50: [[f64; 3]; 3] = [
    [
        1.047_929_792_544_996_9,
        0.022_946_870_601_609_652,
        -0.050_192_266_289_205_24,
    ],
    [
        0.029_627_808_770_055_99,
        0.990_434_426_753_879_9,
        -0.017_073_799_063_418_826,
    ],
    [
        -0.009_243_040_646_204_504,
        0.015_055_191_490_298_152,
        0.751_874_281_428_137_1,
    ],
];
/// Linear Display P3 to CIE XYZ (D65).
const P3_TO_XYZ: [[f64; 3]; 3] = [
    [
        0.486_570_948_648_216_2,
        0.265_667_693_169_093_06,
        0.198_217_285_234_362_5,
    ],
    [
        0.228_974_564_069_748_8,
        0.691_738_521_836_506_4,
        0.079_286_914_093_745,
    ],
    [0.0, 0.045_113_381_858_902_64, 1.043_944_368_900_976],
];

/** A 3x3 matrix times a vector. */
fn multiply(matrix: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0] * v[0] + row[1] * v[1] + row[2] * v[2])
}
/** The sRGB transfer function, sign-preserving as CSS Color 4 extends it. */
pub(super) fn encode(linear: f64) -> f64 {
    let magnitude = linear.abs();
    let encoded = if magnitude <= 0.003_130_8 {
        12.92 * magnitude
    } else {
        1.055 * magnitude.powf(1.0 / 2.4) - 0.055
    };
    encoded.copysign(linear)
}
/** The inverse sRGB transfer function. */
pub(super) fn decode(encoded: f64) -> f64 {
    let magnitude = encoded.abs();
    let linear = if magnitude <= 0.040_45 {
        magnitude / 12.92
    } else {
        ((magnitude + 0.055) / 1.055).powf(2.4)
    };
    linear.copysign(encoded)
}
/** CIE Lab (D50) to linear sRGB. */
pub(super) fn lab_to_linear([l, a, b]: [f64; 3]) -> [f64; 3] {
    let f1 = (l + 16.0) / 116.0;
    let (f0, f2) = (a / 500.0 + f1, f1 - b / 200.0);
    let cube = |f: f64| {
        if f.powi(3) > EPSILON {
            f.powi(3)
        } else {
            (116.0 * f - 16.0) / KAPPA
        }
    };
    let y = if l > KAPPA * EPSILON {
        f1.powi(3)
    } else {
        l / KAPPA
    };
    let xyz50 = [cube(f0) * D50[0], y * D50[1], cube(f2) * D50[2]];
    multiply(&XYZ_TO_SRGB, multiply(&D50_TO_D65, xyz50))
}
/** Linear sRGB to CIE Lab (D50). */
fn linear_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let xyz = multiply(&D65_TO_D50, multiply(&SRGB_TO_XYZ, rgb));
    let f = |v: f64| {
        if v > EPSILON {
            v.cbrt()
        } else {
            (KAPPA * v + 16.0) / 116.0
        }
    };
    let [f0, f1, f2] = [f(xyz[0] / D50[0]), f(xyz[1] / D50[1]), f(xyz[2] / D50[2])];
    [116.0 * f1 - 16.0, 500.0 * (f0 - f1), 200.0 * (f1 - f2)]
}
/** OKLab to linear sRGB. */
pub(super) fn oklab_to_linear([l, a, b]: [f64; 3]) -> [f64; 3] {
    let lc = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let mc = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let sc = (l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        4.076_741_662_1 * lc - 3.307_711_591_3 * mc + 0.230_969_929_2 * sc,
        -1.268_438_004_6 * lc + 2.609_757_401_1 * mc - 0.341_319_396_5 * sc,
        -0.004_196_086_3 * lc - 0.703_418_614_7 * mc + 1.707_614_701 * sc,
    ]
}
/** Linear sRGB to OKLab. */
fn linear_to_oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}
/** Linear Display P3 to linear sRGB. */
pub(super) fn p3_to_linear(p3: [f64; 3]) -> [f64; 3] {
    multiply(&XYZ_TO_SRGB, multiply(&P3_TO_XYZ, p3))
}
/** Polar (lightness, chroma, hue degrees) to rectangular. */
pub(super) fn from_polar([l, c, h]: [f64; 3]) -> [f64; 3] {
    let radians = h.to_radians();
    [l, c * radians.cos(), c * radians.sin()]
}
/** Rectangular to polar; an achromatic color has no hue (`None`, CSS Color 4 "powerless"). */
fn to_polar([l, a, b]: [f64; 3]) -> ([f64; 3], bool) {
    let chroma = a.hypot(b);
    let hue = b.atan2(a).to_degrees().rem_euclid(360.0);
    ([l, chroma, hue], chroma < ACHROMATIC)
}
/// Chroma under which a hue is powerless.
const ACHROMATIC: f64 = 1e-6;

/** The interpolation spaces `color-mix()` supports. */
#[derive(Clone, Copy, PartialEq)]
enum Space {
    Srgb,
    SrgbLinear,
    Lab,
    Lch,
    Oklab,
    Oklch,
    Xyz,
}
impl Space {
    /** The space named after `in`. */
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "srgb" => Self::Srgb,
            "srgb-linear" => Self::SrgbLinear,
            "lab" => Self::Lab,
            "lch" => Self::Lch,
            "oklab" => Self::Oklab,
            "oklch" => Self::Oklch,
            "xyz" | "xyz-d65" | "xyz-d50" => Self::Xyz,
            _ => return None,
        })
    }
    /** Components in this space, and whether a polar hue is powerless. */
    fn encode_linear(self, rgb: [f64; 3]) -> ([f64; 3], bool) {
        match self {
            Self::Srgb => (rgb.map(encode), false),
            Self::SrgbLinear => (rgb, false),
            Self::Xyz => (multiply(&SRGB_TO_XYZ, rgb), false),
            Self::Lab => (linear_to_lab(rgb), false),
            Self::Oklab => (linear_to_oklab(rgb), false),
            Self::Lch => to_polar(linear_to_lab(rgb)),
            Self::Oklch => to_polar(linear_to_oklab(rgb)),
        }
    }
    /** Linear sRGB from components in this space. */
    fn to_linear(self, v: [f64; 3]) -> [f64; 3] {
        match self {
            Self::Srgb => v.map(decode),
            Self::SrgbLinear => v,
            Self::Xyz => multiply(&XYZ_TO_SRGB, v),
            Self::Lab => lab_to_linear(v),
            Self::Oklab => oklab_to_linear(v),
            Self::Lch => lab_to_linear(from_polar(v)),
            Self::Oklch => oklab_to_linear(from_polar(v)),
        }
    }
    /** Whether the third component is a hue. */
    fn polar(self) -> bool {
        matches!(self, Self::Lch | Self::Oklch)
    }
}

/**
 * `color-mix(in <space>, <color> [<percentage>], <color> [<percentage>])` per CSS Color 5:
 * percentages normalise to a sum of 100%, a sum under 100% scales the result's alpha, and
 * components mix premultiplied by alpha. `resolve` turns each color argument into linear sRGB.
 */
pub(super) fn mix(
    arguments: &str,
    resolve: &dyn Fn(&str) -> EngineResult<Option<Linear>>,
) -> EngineResult<Option<Linear>> {
    let parts = super::syntax::split_top(arguments, b',');
    let [method, first, second] = parts.as_slice() else {
        return Err(unsupported());
    };
    let words: Vec<&str> = method.split_whitespace().collect();
    let space = match words.as_slice() {
        ["in", space] | ["in", space, _, "hue"] => Space::named(space).ok_or_else(unsupported)?,
        _ => return Err(unsupported()),
    };
    let (first, p1) = weighted(first)?;
    let (second, p2) = weighted(second)?;
    let (p1, p2) = match (p1, p2) {
        (None, None) => (0.5, 0.5),
        (Some(p), None) => (p, 1.0 - p),
        (None, Some(p)) => (1.0 - p, p),
        (Some(a), Some(b)) => (a, b),
    };
    let sum = p1 + p2;
    if sum <= 0.0 {
        return Err(unsupported());
    }
    let (Some(a), Some(b)) = (resolve(first)?, resolve(second)?) else {
        return Ok(None);
    };
    Ok(Some(interpolate(space, a, b, p2 / sum, sum.min(1.0))))
}
/** A color argument and its optional percentage (0..1). */
fn weighted(part: &str) -> EngineResult<(&str, Option<f64>)> {
    let part = part.trim();
    let percent = |word: &str| {
        word.strip_suffix('%')
            .and_then(|n| n.parse::<f64>().ok())
            .filter(|n| (0.0..=100.0).contains(n))
            .map(|n| n / 100.0)
    };
    if let Some((color, last)) = part.rsplit_once(char::is_whitespace)
        && let Some(p) = percent(last)
    {
        return Ok((color.trim(), Some(p)));
    }
    if let Some((first, color)) = part.split_once(char::is_whitespace)
        && let Some(p) = percent(first)
    {
        return Ok((color.trim(), Some(p)));
    }
    Ok((part, None))
}
/** Premultiplied interpolation `t` of the way from `a` to `b`, then the alpha multiplier. */
fn interpolate(space: Space, a: Linear, b: Linear, t: f64, multiplier: f64) -> Linear {
    let (mut ca, powerless_a) = space.encode_linear([a[0], a[1], a[2]]);
    let (mut cb, powerless_b) = space.encode_linear([b[0], b[1], b[2]]);
    if space.polar() {
        // A powerless hue takes the other color's, so mixing with gray keeps the hue.
        match (powerless_a, powerless_b) {
            (true, false) => ca[2] = cb[2],
            (false, true) => cb[2] = ca[2],
            _ => {}
        }
        let delta = cb[2] - ca[2];
        if delta > 180.0 {
            ca[2] += 360.0;
        } else if delta < -180.0 {
            cb[2] += 360.0;
        }
    }
    let alpha = a[3] + (b[3] - a[3]) * t;
    let mut out = [0.0; 3];
    for i in 0..3 {
        // Hues are not premultiplied.
        let (x, y) = if space.polar() && i == 2 {
            (ca[i], cb[i])
        } else {
            (ca[i] * a[3], cb[i] * b[3])
        };
        let mixed = x + (y - x) * t;
        out[i] = if (space.polar() && i == 2) || alpha == 0.0 {
            mixed
        } else {
            mixed / alpha
        };
    }
    let rgb = space.to_linear(out);
    [rgb[0], rgb[1], rgb[2], alpha * multiplier]
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Lab and OKLab round-trip through linear sRGB. */
    #[test]
    fn conversions_round_trip() {
        let rgb = [0.2, 0.5, 0.8];
        let back = lab_to_linear(linear_to_lab(rgb));
        let ok = oklab_to_linear(linear_to_oklab(rgb));
        for i in 0..3 {
            assert!((back[i] - rgb[i]).abs() < 1e-6);
            assert!((ok[i] - rgb[i]).abs() < 1e-6);
        }
    }
}
