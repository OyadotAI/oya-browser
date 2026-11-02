/** Stroke planning for decoration lines and outline rings: widths, dash lengths and segment counts stay bounded whatever CSS asks for. */
/// Narrowest stroke a decoration or outline paints, in points; thinner authored values paint at this width.
pub(super) const MIN_STROKE: f64 = 0.5;
/// Shortest dash or dot, in points; a shorter one would be a dust of segments the GPU must draw.
pub(super) const MIN_DASH: f64 = 1.0;
/// Most dashes, dots or wave halves one line may paint; a line that would need more is drawn solid.
pub(super) const MAX_SEGMENTS: usize = 4096;

/** A stroke width raised to `MIN_STROKE`, so zero, tiny and non-finite widths still paint a visible line. */
pub(super) fn stroke_width(value: f64) -> f64 {
    if value.is_finite() && value >= MIN_STROKE {
        value
    } else {
        MIN_STROKE
    }
}

/**
 * The dash pattern a stroke of `length` points paints, with each dash raised to `MIN_DASH`. None
 * means paint solid: the pattern has no positive length, the length is not finite, or it would
 * need more than `MAX_SEGMENTS` dashes.
 */
pub(super) fn dash_pattern(pattern: &[f64], length: f64) -> Option<Vec<f64>> {
    let raw: f64 = pattern.iter().filter(|part| part.is_finite()).sum();
    if raw <= 0.0 || !length.is_finite() {
        return None;
    }
    let dashes: Vec<f64> = pattern.iter().map(|part| part.max(MIN_DASH)).collect();
    let period: f64 = dashes.iter().sum();
    let count = length / period * dashes.len() as f64;
    if count.is_nan() || count > MAX_SEGMENTS as f64 {
        return None;
    }
    Some(dashes)
}

/**
 * The half periods a wave `length` points long needs at `half` each, or None when it would need
 * more than `MAX_SEGMENTS` or the half period is not a positive finite length (paint solid then).
 */
pub(super) fn wave_halves(length: f64, half: f64) -> Option<usize> {
    if !(half.is_finite() && half > 0.0 && length.is_finite()) {
        return None;
    }
    let count = (length / half).ceil().max(0.0);
    if count.is_nan() || count > MAX_SEGMENTS as f64 {
        return None;
    }
    Some(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A zero, negative or NaN width paints at the minimum, never at a width that vanishes. */
    #[test]
    fn degenerate_widths_raise_to_the_minimum() {
        for value in [0.0, -3.0, f64::NAN, 1e-9] {
            assert_eq!(stroke_width(value), MIN_STROKE, "{value}");
        }
        assert_eq!(stroke_width(2.0), 2.0);
    }

    /** A tiny dotted outline on a long box paints a bounded number of dots, or none and solid. */
    #[test]
    fn tiny_dotted_outline_paints_bounded_segments() {
        let width = stroke_width(1e-9);
        let dots = [0.0, width].map(|part| part * 2.0);
        let short = dash_pattern(&dots, 400.0).expect("a short box keeps its dots");
        let period: f64 = short.iter().sum();
        assert!(400.0 / period * short.len() as f64 <= MAX_SEGMENTS as f64);
        assert_eq!(
            dash_pattern(&dots, 1e9),
            None,
            "a huge box falls back to solid"
        );
    }

    /** A zero-sum or non-finite pattern is solid, so it can never loop over zero-length dashes. */
    #[test]
    fn zero_sum_and_non_finite_patterns_are_solid() {
        assert_eq!(dash_pattern(&[0.0, 0.0], 100.0), None);
        assert_eq!(dash_pattern(&[3.0, 3.0], f64::INFINITY), None);
        assert_eq!(dash_pattern(&[3.0, 3.0], f64::NAN), None);
    }

    /** A wave at zero thickness terminates: its half period is clamped, so the count is bounded. */
    #[test]
    fn wavy_line_with_zero_thickness_terminates() {
        let half = stroke_width(0.0) * 2.0;
        assert_eq!(wave_halves(100.0, half), Some(100));
        assert_eq!(
            wave_halves(1e9, half),
            None,
            "a huge run falls back to solid"
        );
        assert_eq!(wave_halves(100.0, 0.0), None);
        assert_eq!(wave_halves(100.0, f64::NAN), None);
    }
}
