/** Resolve scale tracks only when every contributing transform has genuine axis provenance. */
use super::super::{
    Keyframes, LONGHANDS,
    model::{AnimationDirection, AnimationFill},
};
use super::model::{ScaleAnimation, ScaleKeyframe};
use crate::document::style::{
    ComputedStyle, TransitionEasing, cascade::Chosen, constants::ANIMATIONS, transition,
};
use std::collections::BTreeMap;
/** Keep opaque frame values out of the static fallback; only fully resolved factors tracks run. */
pub(in crate::document::style) fn tracks(
    chosen: &Chosen,
    keyframes: &Keyframes,
    resolve: impl Fn(&str, &str) -> Option<String>,
    parent: &ComputedStyle,
) -> Vec<ScaleAnimation> {
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
            let mut frames: Vec<ScaleKeyframe> = Vec::new();
            for frame in &set.frames {
                let declaration = frame
                    .declarations
                    .iter()
                    .filter(|d| d.name == "transform")
                    .last();
                let factors = frame
                    .declarations
                    .iter()
                    .filter(|d| d.name == "transform")
                    .last()
                    .and_then(|d| resolve("transform", &d.value))
                    .and_then(|value| factors(&value, parent));
                if declaration.is_some() && factors.is_none() {
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
                if factors.is_none() && easing.is_none() {
                    continue;
                }
                if let Some(existing) = frames.iter_mut().find(|f| f.offset == frame.offset) {
                    if factors.is_some() {
                        existing.factors = factors;
                    }
                    if easing.is_some() {
                        existing.easing = easing;
                    }
                } else {
                    frames.push(ScaleKeyframe {
                        offset: frame.offset,
                        factors,
                        easing,
                    });
                }
            }
            if !frames.iter().any(|f| f.factors.is_some()) {
                return None;
            }
            // Easing-only intermediate frames do not create a synthetic factors keyframe.
            frames.retain(|f| f.factors.is_some() || f.offset == 0.0 || f.offset == 1.0);
            for offset in [0.0, 1.0] {
                if !frames.iter().any(|f| f.offset == offset) {
                    frames.push(ScaleKeyframe {
                        offset,
                        factors: None,
                        easing: None,
                    });
                }
            }
            frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            let animation = ScaleAnimation {
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

/** Scale keyframes retain independent authored factors, with CSS-wide values resolved once. */
fn factors(value: &str, parent: &ComputedStyle) -> Option<[f64; 2]> {
    match value {
        "inherit" => crate::document::Scale::of(parent).map(|value| [value.x, value.y]),
        "initial" | "unset" => Some([1.0; 2]),
        _ => crate::document::style::paint::scale(value),
    }
}
