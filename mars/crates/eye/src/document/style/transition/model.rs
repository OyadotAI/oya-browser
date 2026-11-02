/** Computed transition lists repeat independently according to their property-list index. */
use super::TransitionEasing;
use crate::document::style::constants::{TRANSITION_TIME_MS, TRANSITIONS};
use serde::{Deserialize, Serialize};

/** CSS transition longhands are bounded metadata, not a mutation of authored opacity. */
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TransitionStyle {
    /// Properties in matching order; empty is initial all and the last opacity or all entry wins.
    pub properties: Vec<String>,
    /// Nonnegative durations in scheduler milliseconds; empty is initial zero.
    pub durations: Vec<f64>,
    /// Positive waiting time or negative initial progress, in milliseconds; empty is initial zero.
    pub delays: Vec<f64>,
    /// Timing functions repeat when there are fewer than matching properties; empty is initial ease.
    pub easings: Vec<TransitionEasing>,
}

impl TransitionStyle {
    /** The overwhelmingly common initial style need not inflate native document messages. */
    pub(crate) fn is_initial(&self) -> bool {
        self == &Self::default()
    }
    /** Hostile deserialization cannot turn malformed lists into indexing or non-finite samples. */
    pub(crate) fn valid(&self) -> bool {
        [
            self.properties.len(),
            self.durations.len(),
            self.delays.len(),
            self.easings.len(),
        ]
        .into_iter()
        .all(|count| count <= TRANSITIONS)
            && self
                .properties
                .iter()
                .all(|property| super::parse::property(property))
            && (self.properties.len() == 1
                || !self.properties.iter().any(|property| property == "none"))
            && self
                .durations
                .iter()
                .all(|value| value.is_finite() && (0.0..=TRANSITION_TIME_MS).contains(value))
            && self
                .delays
                .iter()
                .all(|value| value.is_finite() && value.abs() <= TRANSITION_TIME_MS)
            && self.easings.iter().all(TransitionEasing::valid)
    }

    /** Each supported property uses the last matching list entry as CSS Transitions requires. */
    pub(super) fn matching(&self, property: &str) -> Option<(f64, f64, TransitionEasing)> {
        let index = self
            .properties
            .iter()
            .rposition(|name| name == "all" || name == property)
            .or_else(|| self.properties.is_empty().then_some(0))?;
        Some((
            repeated(&self.durations, index),
            repeated(&self.delays, index),
            repeated(&self.easings, index),
        ))
    }
}

/** Empty computed lists use their initial value without per-element heap allocations. */
fn repeated<T: Clone + Default>(values: &[T], index: usize) -> T {
    if values.is_empty() {
        T::default()
    } else {
        values[index % values.len()].clone()
    }
}
