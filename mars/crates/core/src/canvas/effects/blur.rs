/** Gaussian blur approximated by three separable box passes, so cost is linear in pixels whatever the radius. */
/** Three passes of box filters match a Gaussian's variance closely; browsers use the same approximation. */
pub const PASSES: usize = 3;

/** Box widths whose three successive passes match the variance of a Gaussian with this sigma. */
pub fn box_widths(sigma: f64) -> [usize; PASSES] {
    // A blur wider than the largest canvas changes nothing more, so page input (a filter or shadow
    // blur of 1e300px, NaN) is bounded here, where every blur path sizes its boxes.
    let sigma = if sigma.is_finite() {
        sigma.clamp(0.0, f64::from(crate::canvas::EDGE))
    } else {
        0.0
    };
    let ideal = (12.0 * sigma * sigma / PASSES as f64 + 1.0).sqrt();
    let mut lower = ideal.floor() as i64;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let upper = lower + 2;
    let passes = PASSES as i64;
    let correction = (12.0 * sigma * sigma
        - (passes * lower * lower + 4 * passes * lower + 3 * passes) as f64)
        / (-4.0 * lower as f64 - 4.0);
    let narrow = correction.round().clamp(0.0, passes as f64) as usize;
    std::array::from_fn(|pass| {
        if pass < narrow {
            lower as usize
        } else {
            upper as usize
        }
    })
}

/** Blur one channel in place; a sigma that is not positive leaves the channel untouched. */
pub fn blur(values: &mut [f32], width: usize, height: usize, sigma: f64) {
    if sigma.is_nan() || sigma <= 0.0 || width == 0 || height == 0 {
        return;
    }
    for size in box_widths(sigma) {
        let radius = (size - 1) / 2;
        for row in values.chunks_exact_mut(width).take(height) {
            box_pass(row, radius);
        }
        vertical_pass(values, width, height, radius);
    }
}

/** A running sum makes one box pass cost one addition and one subtraction per sample. */
fn box_pass(line: &mut [f32], radius: usize) {
    let len = line.len();
    let source = line.to_vec();
    let window = (2 * radius + 1) as f32;
    let mut sum: f32 = source.iter().take(radius.min(len)).sum();
    for (index, out) in line.iter_mut().enumerate() {
        if index + radius < len {
            sum += source[index + radius];
        }
        *out = sum / window;
        if index >= radius {
            sum -= source[index - radius];
        }
    }
}

/** Vertical passes use the same running sum, one column at a time over the whole buffer. */
fn vertical_pass(values: &mut [f32], width: usize, height: usize, radius: usize) {
    let mut column = vec![0.0f32; height];
    for x in 0..width {
        for (y, cell) in column.iter_mut().enumerate() {
            *cell = values[y * width + x];
        }
        box_pass(&mut column, radius);
        for (y, cell) in column.iter().enumerate() {
            values[y * width + x] = *cell;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Absurd and non-finite blur sizes stay within the canvas edge instead of overflowing. */
    #[test]
    fn huge_or_non_finite_sigmas_stay_bounded() {
        let bound = 2 * crate::canvas::EDGE as usize + 3;
        for sigma in [1e300, f64::MAX, f64::INFINITY, f64::NAN, -5.0] {
            assert!(
                box_widths(sigma).iter().all(|width| *width <= bound),
                "{sigma}"
            );
        }
    }
}
