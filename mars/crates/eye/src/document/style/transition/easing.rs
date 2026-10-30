/** Named, cubic-bezier and steps easing evaluate deterministically without unbounded iteration. */
use crate::document::style::constants::{BEZIER_ITERATIONS, TRANSITION_STEPS};
use serde::{Deserialize, Serialize};

/** Serializable timing functions retain the computed declaration independently of a running transition. */
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum TransitionEasing {
    /// Progress maps directly to elapsed active time.
    Linear,
    /// Two control points; x values must lie in the unit interval.
    CubicBezier([f64; 4]),
    /// Count and endpoint jump behavior of a staircase timing function.
    Steps { count: u32, position: StepPosition },
}

/** CSS step endpoints determine whether the first or last interval includes an extra jump. */
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum StepPosition {
    /// Jump at the beginning of each interval.
    Start,
    /// Jump at the end of each interval.
    End,
    /// Neither endpoint contributes a jump.
    None,
    /// Both endpoints contribute a jump.
    Both,
}

impl Default for TransitionEasing {
    /** CSS ease is the default, not linear interpolation of time. */
    fn default() -> Self {
        Self::CubicBezier([0.25, 0.1, 0.25, 1.0])
    }
}

impl TransitionEasing {
    /** Accept the bounded easing grammar and reject malformed control points and steps counts. */
    pub(super) fn parse(value: &str) -> Option<Self> {
        let easing = match value {
            "linear" => Self::Linear,
            "ease" => Self::default(),
            "ease-in" => Self::CubicBezier([0.42, 0.0, 1.0, 1.0]),
            "ease-out" => Self::CubicBezier([0.0, 0.0, 0.58, 1.0]),
            "ease-in-out" => Self::CubicBezier([0.42, 0.0, 0.58, 1.0]),
            "step-start" => Self::Steps {
                count: 1,
                position: StepPosition::Start,
            },
            "step-end" => Self::Steps {
                count: 1,
                position: StepPosition::End,
            },
            _ => function(value)?,
        };
        easing.valid().then_some(easing)
    }

    /** Wire values obey the same constraints as CSS parsing before any sampling occurs. */
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Linear => true,
            Self::CubicBezier(points) => {
                points.iter().all(|point| {
                    point.is_finite() && point.abs() <= crate::constants::CSS_MAX_SCALAR
                }) && (0.0..=1.0).contains(&points[0])
                    && (0.0..=1.0).contains(&points[2])
            }
            Self::Steps { count, position } => {
                (1..=TRANSITION_STEPS).contains(count)
                    && (*position != StepPosition::None || *count > 1)
            }
        }
    }

    /** Return eased active progress; interpolation clamps opacity after permitted overshoot. */
    pub(super) fn sample(&self, progress: f64) -> f64 {
        match self {
            Self::Linear => progress,
            Self::CubicBezier(points) => bezier(*points, progress),
            Self::Steps { count, position } => steps(*count, *position, progress),
        }
    }
}

/** Parse functional forms without treating commas inside a function as transition-list separators. */
fn function(value: &str) -> Option<TransitionEasing> {
    if let Some(arguments) = value
        .strip_prefix("cubic-bezier(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let values = super::parse::parts(arguments)?
            .into_iter()
            .map(|part| part.parse::<f64>().ok())
            .collect::<Option<Vec<_>>>()?;
        return Some(TransitionEasing::CubicBezier(values.try_into().ok()?));
    }
    let arguments = value.strip_prefix("steps(")?.strip_suffix(')')?;
    let values = super::parse::parts(arguments)?;
    if values.is_empty() || values.len() > 2 {
        return None;
    }
    let count = values[0].parse().ok()?;
    let position = match values.get(1).copied().unwrap_or("end") {
        "start" | "jump-start" => StepPosition::Start,
        "end" | "jump-end" => StepPosition::End,
        "jump-none" => StepPosition::None,
        "jump-both" => StepPosition::Both,
        _ => return None,
    };
    Some(TransitionEasing::Steps { count, position })
}

/** Solve the monotone x component with fixed work, then evaluate the y component. */
fn bezier(points: [f64; 4], progress: f64) -> f64 {
    if progress == 0.0 || progress == 1.0 {
        return progress;
    }
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..BEZIER_ITERATIONS {
        let mid = (low + high) / 2.0;
        if component(points[0], points[2], mid) < progress {
            low = mid;
        } else {
            high = mid;
        }
    }
    component(points[1], points[3], (low + high) / 2.0)
}

/** Cubic Bernstein polynomial with fixed endpoints zero and one. */
fn component(first: f64, second: f64, time: f64) -> f64 {
    let inverse = 1.0 - time;
    3.0 * inverse * inverse * time * first
        + 3.0 * inverse * time * time * second
        + time * time * time
}

/** Step sampling follows the jump position, including the terminal value at progress one. */
fn steps(count: u32, position: StepPosition, progress: f64) -> f64 {
    let count = f64::from(count);
    let initial = if matches!(position, StepPosition::Start | StepPosition::Both) {
        1.0
    } else {
        0.0
    };
    let jumps = match position {
        StepPosition::Both => count + 1.0,
        StepPosition::None => count - 1.0,
        _ => count,
    };
    ((progress * count).floor() + initial).clamp(0.0, jumps) / jumps
}
