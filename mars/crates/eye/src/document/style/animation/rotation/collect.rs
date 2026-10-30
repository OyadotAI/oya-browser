use super::super::{
    Keyframes, LONGHANDS,
    model::{AnimationDirection, AnimationFill},
};
/** Resolve rotation tracks only when every contributing transform has genuine angular provenance. */
use super::model::{RotationAnimation, RotationKeyframe};
use crate::document::style::{
    ComputedStyle, TransitionEasing, cascade::Chosen, constants::ANIMATIONS, transition,
};
use std::collections::BTreeMap;
/** Keep opaque frame values out of the static fallback; only fully resolved radians tracks run. */
pub(in crate::document::style) fn tracks(
    chosen: &Chosen,
    keyframes: &Keyframes,
    resolve: impl Fn(&str, &str) -> Option<String>,
    parent: &ComputedStyle,
) -> Vec<RotationAnimation> {
    let values: BTreeMap<String, String> = LONGHANDS
        .iter()
        .filter_map(|key| {
            chosen
                .get(*key)
                .map(|(_, value)| (key.to_string(), value.to_string()))
        })
        .collect();
    let Some(names) = values.get("animation-name") else {
        return Vec::new();
    };
    let mut rejected = false;
    let tracks = super::super::split_top(names, b',')
        .into_iter()
        .take(ANIMATIONS)
        .enumerate()
        .filter_map(|(index, name)| {
            let name = super::super::unquote(name);
            let set = keyframes.sets.get(name).filter(|set| !set.masked)?;
            let pick = |key: &str| super::super::pick(&values, key, index);
            let mut frames: Vec<RotationKeyframe> = Vec::new();
            for frame in &set.frames {
                let declaration = frame
                    .declarations
                    .iter()
                    .filter(|d| d.name == "transform")
                    .last();
                let radians = frame
                    .declarations
                    .iter()
                    .filter(|d| d.name == "transform")
                    .last()
                    .and_then(|d| resolve("transform", &d.value))
                    .and_then(|value| radians(&value, parent));
                if declaration.is_some() && radians.is_none() {
                    rejected = true;
                    return None;
                }
                let easing = frame
                    .declarations
                    .iter()
                    .filter(|d| d.name == "animation-timing-function")
                    .last()
                    .and_then(|d| resolve("animation-timing-function", &d.value))
                    .and_then(|value| TransitionEasing::parse(&value));
                if radians.is_none() && easing.is_none() {
                    continue;
                }
                if let Some(existing) = frames.iter_mut().find(|f| f.offset == frame.offset) {
                    if radians.is_some() {
                        existing.radians = radians;
                    }
                    if easing.is_some() {
                        existing.easing = easing;
                    }
                } else {
                    frames.push(RotationKeyframe {
                        offset: frame.offset,
                        radians,
                        easing,
                    });
                }
            }
            if !frames.iter().any(|f| f.radians.is_some()) {
                return None;
            }
            // Easing-only intermediate frames do not create a synthetic radians keyframe.
            frames.retain(|f| f.radians.is_some() || f.offset == 0.0 || f.offset == 1.0);
            for offset in [0.0, 1.0] {
                if !frames.iter().any(|f| f.offset == offset) {
                    frames.push(RotationKeyframe {
                        offset,
                        radians: None,
                        easing: None,
                    });
                }
            }
            frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            let animation = RotationAnimation {
                name: name.into(),
                duration_ms: time(&pick("animation-duration")),
                delay_ms: time(&pick("animation-delay")),
                iterations: match pick("animation-iteration-count").as_str() {
                    "infinite" => None,
                    "" => Some(1.0),
                    value => Some(value.parse().ok()?),
                },
                direction: match pick("animation-direction").as_str() {
                    "reverse" => AnimationDirection::Reverse,
                    "alternate" => AnimationDirection::Alternate,
                    "alternate-reverse" => AnimationDirection::AlternateReverse,
                    _ => AnimationDirection::Normal,
                },
                fill: match pick("animation-fill-mode").as_str() {
                    "forwards" => AnimationFill::Forwards,
                    "backwards" => AnimationFill::Backwards,
                    "both" => AnimationFill::Both,
                    _ => AnimationFill::None,
                },
                paused: pick("animation-play-state") == "paused",
                easing: TransitionEasing::parse(&pick("animation-timing-function"))
                    .unwrap_or_default(),
                frames,
            };
            animation.valid().then_some(animation)
        })
        .collect();
    if rejected { Vec::new() } else { tracks }
}

/** Empty timing longhands use CSS initial zero. */
fn time(value: &str) -> f64 {
    transition::time(value).unwrap_or(0.0)
}

/** Angular keyframes retain original units as unwrapped radians, with CSS-wide values resolved once. */
fn radians(value: &str, parent: &ComputedStyle) -> Option<f64> {
    match value {
        "inherit" => crate::document::Rotation::of(parent).map(|value| value.radians),
        "initial" | "unset" => Some(0.0),
        _ => crate::document::style::paint::rotation(value),
    }
}
