/*! The hyperbolic functions V8 derives itself from fdlibm (src/base/ieee754.cc): acosh, asinh,
atanh, cosh and sinh, plus Math.hypot's summation. Each is a short formula over log, log1p, exp,
expm1 and sqrt with fdlibm's range splits, so the result matches Chrome bit for bit only when
both the splits and the underlying functions do. */

#![allow(
    clippy::approx_constant,
    clippy::empty_line_after_doc_comments,
    clippy::excessive_precision
)]

/** ln 2, as fdlibm spells it. */
const LN2: f64 = 6.931_471_805_599_452_862_27e-1;
/** The high word of 1.0. */
const ONE_HIGH: i32 = 0x3FF0_0000;
/** The high word of 2.0. */
const TWO_HIGH: i32 = 0x4000_0000;
/** The high word of 2^28: beyond it log(2x) is exact enough. */
const HUGE_HIGH: i32 = 0x41B0_0000;
/** The high word of 2^-28: below it the function is its argument. */
const TINY_HIGH: i32 = 0x3E30_0000;
/** The high word of infinity and NaN. */
const INF_HIGH: i32 = 0x7FF0_0000;
/** The high word of 0.5. */
const HALF_HIGH: i32 = 0x3FE0_0000;
/** The high word of 0.5 ln 2, cosh's first split. */
const HALF_LN2_HIGH: i32 = 0x3FD6_2E43;
/** The high word of 2^-55: below it cosh is exactly 1. */
const COSH_ONE_HIGH: i32 = 0x3C80_0000;
/** The high word of 22, past which exp(-|x|) no longer matters. */
const TWENTY_TWO_HIGH: i32 = 0x4036_0000;
/** The high word of log(maxdouble). */
const LOG_MAX_HIGH: i32 = 0x4086_2E42;
/** log(maxdouble) with an empty low word, sinh's split. */
const LOG_MAX: f64 = 709.782_226_562_5;
/** Past this cosh and sinh overflow. */
const OVERFLOW: f64 = 710.475_860_073_943_9;
/** 2^-28 with an empty low word. */
const TWO_M28: f64 = 3.725_290_298_461_914e-9;
/** The sinh overflow scale: x times it overflows the way fdlibm wants. */
const SINH_HUGE: f64 = 1.0e307;

/** The upper 32 bits of a double, signed, as fdlibm reads them. */
fn high(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

/** The lower 32 bits of a double. */
fn low(x: f64) -> u32 {
    x.to_bits() as u32
}

/** Math.acosh. */
pub(super) fn acosh(x: f64) -> f64 {
    let hx = high(x);
    if hx < ONE_HIGH {
        return f64::NAN;
    }
    if hx >= HUGE_HIGH {
        if hx >= INF_HIGH {
            return x + x;
        }
        return super::fdlibm::log(x) + LN2;
    }
    if ((hx - ONE_HIGH) | low(x) as i32) == 0 {
        return 0.0;
    }
    if hx > TWO_HIGH {
        let t = x * x;
        return super::fdlibm::log(2.0 * x - 1.0 / (x + (t - 1.0).sqrt()));
    }
    let t = x - 1.0;
    libm::log1p(t + (2.0 * t + t * t).sqrt())
}

/** Math.asinh. */
pub(super) fn asinh(x: f64) -> f64 {
    let hx = high(x);
    let ix = hx & 0x7FFF_FFFF;
    if ix >= INF_HIGH {
        return x + x;
    }
    if ix < TINY_HIGH {
        return x;
    }
    let w = if ix > HUGE_HIGH {
        super::fdlibm::log(x.abs()) + LN2
    } else if ix > TWO_HIGH {
        let t = x.abs();
        super::fdlibm::log(2.0 * t + 1.0 / ((x * x + 1.0).sqrt() + t))
    } else {
        let t = x * x;
        libm::log1p(x.abs() + t / (1.0 + (1.0 + t).sqrt()))
    };
    if hx > 0 { w } else { -w }
}

/** Math.atanh. */
pub(super) fn atanh(x: f64) -> f64 {
    let hx = high(x);
    let ix = hx & 0x7FFF_FFFF;
    let lx = low(x);
    if (ix | ((lx | lx.wrapping_neg()) >> 31) as i32) > ONE_HIGH {
        return f64::NAN;
    }
    if ix == ONE_HIGH {
        return if x > 0.0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
    }
    if ix < TINY_HIGH {
        return x;
    }
    let x = x.abs();
    let t = if ix < HALF_HIGH {
        let t = x + x;
        0.5 * libm::log1p(t + t * x / (1.0 - x))
    } else {
        0.5 * libm::log1p((x + x) / (1.0 - x))
    };
    if hx >= 0 { t } else { -t }
}

/** Math.cosh. */
pub(super) fn cosh(x: f64) -> f64 {
    let ix = high(x) & 0x7FFF_FFFF;
    if ix < HALF_LN2_HIGH {
        let t = libm::expm1(x.abs());
        let w = 1.0 + t;
        if ix < COSH_ONE_HIGH {
            return w;
        }
        return 1.0 + (t * t) / (w + w);
    }
    if ix < TWENTY_TWO_HIGH {
        let t = super::fdlibm::exp(x.abs());
        return 0.5 * t + 0.5 / t;
    }
    if ix < LOG_MAX_HIGH {
        return 0.5 * super::fdlibm::exp(x.abs());
    }
    if x.abs() <= OVERFLOW {
        let w = super::fdlibm::exp(0.5 * x.abs());
        return (0.5 * w) * w;
    }
    if ix >= INF_HIGH {
        return x * x;
    }
    f64::INFINITY
}

/** Math.sinh. */
pub(super) fn sinh(x: f64) -> f64 {
    let h = if x < 0.0 { -0.5 } else { 0.5 };
    let ax = x.abs();
    if ax < 22.0 {
        if ax < TWO_M28 {
            return x;
        }
        let t = libm::expm1(ax);
        if ax < 1.0 {
            return h * (2.0 * t - t * t / (t + 1.0));
        }
        return h * (t + t / (t + 1.0));
    }
    if ax < LOG_MAX {
        return h * super::fdlibm::exp(ax);
    }
    if ax <= OVERFLOW {
        let w = super::fdlibm::exp(0.5 * ax);
        return (h * w) * w;
    }
    x * SINH_HUGE
}

/** Math.hypot as V8 computes it: one to three arguments take the straight scaled formula, more
take a Kahan sum, because the two rounding paths give different last bits. */
pub(super) fn hypot(values: &[f64]) -> f64 {
    let absolute: Vec<f64> = values.iter().map(|value| value.abs()).collect();
    match absolute.as_slice() {
        [] => 0.0,
        [a] => *a,
        [a, b] => scaled(&absolute, |m| (a / m) * (a / m) + (b / m) * (b / m)),
        [a, b, c] => scaled(&absolute, |m| {
            let (pa, pb) = ((a / m) * (a / m), (b / m) * (b / m));
            let compensation = (pa + pb) - pa - pb;
            pa + pb + ((c / m) * (c / m) - compensation)
        }),
        _ => scaled(&absolute, |m| kahan(&absolute, m)),
    }
}

/** The shared frame: infinity wins over NaN, all zeros give zero, otherwise the magnitudes are
normalized to the largest before `sum` squares them. */
fn scaled(absolute: &[f64], sum: impl Fn(f64) -> f64) -> f64 {
    if absolute.contains(&f64::INFINITY) {
        return f64::INFINITY;
    }
    let max = absolute.iter().fold(
        0.0_f64,
        |max, value| if *value > max { *value } else { max },
    );
    if absolute.iter().any(|value| value.is_nan()) {
        return f64::NAN;
    }
    if max == 0.0 {
        return 0.0;
    }
    sum(max).sqrt() * max
}

/** Kahan summation of the normalized squares. */
fn kahan(absolute: &[f64], max: f64) -> f64 {
    let (mut sum, mut compensation) = (0.0, 0.0);
    for value in absolute {
        let n = value / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum
}
