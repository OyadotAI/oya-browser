/*! Numerically stable Math operations preserve signed zero and avoid platform asymmetry or intermediate overflow. */

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
