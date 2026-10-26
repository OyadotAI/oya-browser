/** Numerically stable Math operations preserve signed zero and avoid platform asymmetry or intermediate overflow. */

/** Evaluating one sign and restoring it guarantees the odd symmetry required near either pole. */
pub(super) fn atanh(value: f64) -> f64 {
    value.abs().atanh().copysign(value)
}

/** Rounding the fractional difference avoids changing representable integers or losing negative zero. */
pub(super) fn round(value: f64) -> f64 {
    if !value.is_finite() || value.fract() == 0.0 {
        return value;
    }
    let floor = value.floor();
    let result = if value - floor < 0.5 {
        floor
    } else {
        floor + 1.0
    };
    if result == 0.0 {
        result.copysign(value)
    } else {
        result
    }
}

/** Every argument has already been converted, and infinity takes priority over NaN. */
pub(super) fn hypot(values: &[f64]) -> f64 {
    if values.iter().any(|value| value.is_infinite()) {
        return f64::INFINITY;
    }
    if values.iter().any(|value| value.is_nan()) {
        return f64::NAN;
    }
    let maximum = values
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if maximum == 0.0 {
        return 0.0;
    }
    scaled_sum(values, maximum).sqrt() * maximum
}

/** Kahan summation retains small normalized squares when many magnitudes differ. */
fn scaled_sum(values: &[f64], maximum: f64) -> f64 {
    let (mut sum, mut correction) = (0.0, 0.0);
    for value in values {
        let scaled = value.abs() / maximum;
        let term = scaled * scaled - correction;
        let next = sum + term;
        correction = (next - sum) - term;
        sum = next;
    }
    sum
}
