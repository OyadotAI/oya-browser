/** Rounding and clamping helpers shared by the transforms and predictors: the spec's Round2, Round2Signed and Clip3 on 64-bit values. */
/** Round2(x, n): divide by 2^n, rounding half up, with an arithmetic shift for negative values. */
pub(crate) fn round2(x: i64, n: u32) -> i64 {
    if n == 0 {
        x
    } else {
        (x + (1i64 << (n - 1))) >> n
    }
}

/** Round2Signed(x, n): Round2 applied to the magnitude, with the sign restored, so that halves round away from zero. */
pub(crate) fn round2_signed(x: i64, n: u32) -> i64 {
    if x >= 0 { round2(x, n) } else { -round2(-x, n) }
}

/** Clip3(low, high, x): x limited to the closed range [low, high]. */
pub(crate) fn clip3(low: i64, high: i64, x: i64) -> i64 {
    x.clamp(low, high)
}
