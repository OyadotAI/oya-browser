/**
 * `linear()` easing (CSS Easing 2): progress runs piecewise-linearly through authored points.
 * Points without an input percentage are spaced evenly between their neighbours that have one,
 * and inputs never decrease, so the curve is a function of time. Overshoot (outputs past 0 or
 * 1) is kept, as the spec allows.
 */
use crate::document::style::{constants::LINEAR_POINTS, syntax::split_top, syntax::tokens};

/** One authored point: its input (progress, when given) and its output value. */
type Stop = (Option<f64>, f64);

/** Parses the arguments of `linear(...)` into resolved `(input, output)` points, or None. */
pub(super) fn parse(arguments: &str) -> Option<Vec<(f64, f64)>> {
    let items = split_top(arguments, b',');
    if items.is_empty() || items.len() > LINEAR_POINTS {
        return None;
    }
    let mut stops = Vec::with_capacity(items.len());
    for item in items {
        stops.extend(stop(item.trim())?);
    }
    if stops.is_empty() || stops.len() > LINEAR_POINTS {
        return None;
    }
    Some(resolve(stops))
}

/** One item's stops: an output with none, one or two input percentages (two make a jump). */
fn stop(item: &str) -> Option<Vec<Stop>> {
    let mut words = tokens(item).into_iter();
    let output = finite(words.next()?)?;
    let inputs = words
        .map(|word| finite(word.strip_suffix('%')?).map(|percent| percent / 100.0))
        .collect::<Option<Vec<f64>>>()?;
    Some(match inputs.as_slice() {
        [] => vec![(None, output)],
        _ if inputs.len() <= 2 => inputs.iter().map(|input| (Some(*input), output)).collect(),
        _ => return None,
    })
}

/** A parsed finite number, or None. */
fn finite(word: &str) -> Option<f64> {
    word.parse::<f64>().ok().filter(|n| n.is_finite())
}

/**
 * Pins the first and last inputs to 0 and 1, clamps explicit inputs to be nondecreasing, and
 * spaces each run of missing inputs evenly between the explicit inputs around it.
 */
fn resolve(mut stops: Vec<Stop>) -> Vec<(f64, f64)> {
    let last = stops.len() - 1;
    stops[0].0.get_or_insert(0.0);
    stops[last].0.get_or_insert(1.0);
    let mut anchors: Vec<(usize, f64)> = Vec::new();
    for (index, (input, _)) in stops.iter().enumerate() {
        if let Some(input) = *input {
            let floor = anchors
                .last()
                .map_or(input, |(_, previous)| previous.max(input));
            anchors.push((index, floor));
        }
    }
    let mut inputs = vec![0.0; stops.len()];
    for pair in anchors.windows(2) {
        let ((start, from), (end, to)) = (pair[0], pair[1]);
        for (offset, slot) in inputs[start..=end].iter_mut().enumerate() {
            let fraction = offset as f64 / (end - start) as f64;
            *slot = from + (to - from) * fraction;
        }
    }
    stops
        .iter()
        .zip(inputs)
        .map(|((_, output), input)| (input, *output))
        .collect()
}

/**
 * The output at `progress` on resolved points: linear between neighbours, held beyond the ends.
 * At an input shared by two points (a jump) the later point wins, as the spec's stops do.
 */
pub(super) fn sample(points: &[(f64, f64)], progress: f64) -> f64 {
    let Some(first) = points.first() else {
        return progress;
    };
    if progress < first.0 {
        return first.1;
    }
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if progress < x1 {
            return y0 + (y1 - y0) * (progress - x0) / (x1 - x0);
        }
    }
    points.last().map_or(progress, |last| last.1)
}

#[cfg(test)]
mod tests {
    /** Unlisted inputs are spaced evenly between the explicit ones around them. */
    #[test]
    fn missing_inputs_space_evenly_between_anchors() {
        let points = super::parse("0, 1, 0.5 50%, 2").unwrap();
        let inputs: Vec<f64> = points.iter().map(|point| point.0).collect();
        assert_eq!(inputs, vec![0.0, 0.25, 0.5, 1.0]);
    }
    /** Samples interpolate between points and hold the end values outside them. */
    #[test]
    fn sampling_interpolates_and_holds_the_ends() {
        let points = super::parse("0, 1").unwrap();
        assert_eq!(super::sample(&points, 0.25), 0.25);
        assert_eq!(super::sample(&points, 2.0), 1.0);
        assert_eq!(super::sample(&points, -1.0), 0.0);
    }
    /** Two points at one input make a jump: progress at that input takes the later output. */
    #[test]
    fn a_double_input_makes_a_jump() {
        let points = super::parse("0, 1 50%, 0 50%, 1").unwrap();
        assert!((super::sample(&points, 0.49) - 0.98).abs() < 1e-12);
        assert_eq!(super::sample(&points, 0.5), 0.0);
    }
    /** Inputs never decrease, so a later smaller input is pulled up to the one before it. */
    #[test]
    fn inputs_never_decrease() {
        let points = super::parse("0, 1 80%, 2 40%, 3").unwrap();
        let inputs: Vec<f64> = points.iter().map(|point| point.0).collect();
        assert_eq!(inputs, vec![0.0, 0.8, 0.8, 1.0]);
    }
    /** An empty list and a point with three inputs are malformed and drop the declaration. */
    #[test]
    fn malformed_points_are_rejected() {
        assert!(super::parse("").is_none());
        assert!(super::parse("0 10% 20% 30%").is_none());
        assert!(super::parse("0, nope").is_none());
    }
}
