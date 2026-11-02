/** Serializable opacity tracks retain authored keyframes independently of presentation samples. */
use crate::document::style::{
    TransitionEasing,
    constants::{CUSTOM_PROPERTY_BYTES, KEYFRAME_STEPS, TRANSITION_TIME_MS},
};
use serde::{Deserialize, Serialize};

/** One resolved CSS animation; absent endpoint opacity samples the underlying cascade. */
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpacityAnimation {
    /// Case-sensitive animation identity used to preserve playback across recascades.
    pub name: String,
    /// Length of one iteration in scheduler milliseconds.
    pub duration_ms: f64,
    /// Delay relative to creation; negative values begin partway through playback.
    pub delay_ms: f64,
    /// None means infinite; finite fractional counts preserve partial final iterations.
    pub iterations: Option<f64>,
    /// Playback orientation and alternating iteration parity.
    pub direction: AnimationDirection,
    /// Before and after phases that retain a sampled value.
    pub fill: AnimationFill,
    /// Pausing retains local time, including time spent in the delay phase.
    pub paused: bool,
    /// Default easing applies to each keyframe interval, not the entire animation.
    pub easing: TransitionEasing,
    /// Ordered, merged opacity keyframes with implicit endpoints included.
    pub frames: Vec<OpacityKeyframe>,
}

/** CSS playback directions remain typed across the native document boundary. */
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum AnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/** Filling extends an animation's effect without leaving scheduled work after completion. */
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum AnimationFill {
    None,
    Forwards,
    Backwards,
    Both,
}

/** Duplicate offsets merge during cascade; each surviving interval has one starting easing. */
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpacityKeyframe {
    /// Fraction of the iteration in ascending order.
    pub offset: f64,
    /// None is an implicit endpoint using the current underlying opacity.
    pub opacity: Option<f64>,
    /// A keyframe timing function overrides the animation's interval easing.
    pub easing: Option<TransitionEasing>,
}

impl OpacityAnimation {
    /** Refuse hostile wire values before they enter arithmetic or retain excessive metadata. */
    pub(crate) fn valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= CUSTOM_PROPERTY_BYTES
            && self.duration_ms.is_finite()
            && (0.0..=TRANSITION_TIME_MS).contains(&self.duration_ms)
            && self.delay_ms.is_finite()
            && self.delay_ms.abs() <= TRANSITION_TIME_MS
            && self.iterations.is_none_or(|n| {
                n.is_finite() && (0.0..=crate::constants::CSS_MAX_SCALAR).contains(&n)
            })
            && self.easing.valid()
            && (2..=KEYFRAME_STEPS + 2).contains(&self.frames.len())
            && self.frames.first().is_some_and(|f| f.offset == 0.0)
            && self.frames.last().is_some_and(|f| f.offset == 1.0)
            && self
                .frames
                .windows(2)
                .all(|pair| pair[0].offset < pair[1].offset)
            && self.frames.iter().all(|f| {
                (0.0..=1.0).contains(&f.offset)
                    && f.opacity.is_none_or(|v| (0.0..=1.0).contains(&v))
                    && f.easing.as_ref().is_none_or(TransitionEasing::valid)
            })
    }
}
