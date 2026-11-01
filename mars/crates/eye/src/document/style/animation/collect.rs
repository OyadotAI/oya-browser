/** Resolve bounded opacity tracks from winning declarations and the existing keyframe parser. */
use super::{
    Keyframes, LONGHANDS,
    model::{AnimationDirection, AnimationFill, OpacityAnimation, OpacityKeyframe},
};
use crate::document::style::{
    ComputedStyle, TransitionEasing, cascade::Chosen, constants::ANIMATIONS, transition,
};
use std::collections::BTreeMap;

/** Validate entire longhand lists at declaration time, preserving earlier valid declarations. */
pub(in crate::document::style) fn valid_value(key: &str, value: &str) -> bool {
    transition::parts(value).is_some_and(|parts| {
        parts.into_iter().all(|value| match key {
            "animation-name" => {
                !super::unquote(value).is_empty()
                    && value.len() <= super::super::constants::CUSTOM_PROPERTY_BYTES
                    && super::super::syntax::tokens(value).len() == 1
            }
            "animation-duration" => transition::time(value).is_some_and(|time| time >= 0.0),
            "animation-delay" => transition::time(value).is_some(),
            "animation-timing-function" => TransitionEasing::parse(value).is_some(),
            "animation-iteration-count" => {
                value == "infinite"
                    || value.parse::<f64>().is_ok_and(|n| {
                        n.is_finite() && (0.0..=crate::constants::CSS_MAX_SCALAR).contains(&n)
                    })
            }
            "animation-direction" => super::DIRECTIONS.contains(&value),
            "animation-fill-mode" => super::FILLS.contains(&value),
            "animation-play-state" => super::STATES.contains(&value),
            _ => false,
        })
    })
}

/** Keep opaque frame values out of the static fallback; only fully resolved opacity tracks run. */
pub(in crate::document::style) fn tracks(
    chosen: &Chosen,
    keyframes: &Keyframes,
    resolve: impl Fn(&str, &str) -> Option<String>,
    parent: &ComputedStyle,
) -> Vec<OpacityAnimation> {
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
    super::split_top(names, b',')
        .into_iter()
        .take(ANIMATIONS)
        .enumerate()
        .filter_map(|(index, name)| {
            let name = super::unquote(name);
            let set = keyframes.sets.get(name).filter(|set| !set.masked)?;
            let pick = |key: &str| super::pick(&values, key, index);
            let mut frames: Vec<OpacityKeyframe> = Vec::new();
            for frame in &set.frames {
                let opacity = frame
                    .declarations
                    .iter()
                    .rfind(|d| d.name == "opacity")
                    .and_then(|d| resolve("opacity", &d.value))
                    .and_then(|value| opacity(&value, parent));
                let easing = frame
                    .declarations
                    .iter()
                    .rfind(|d| d.name == "animation-timing-function")
                    .and_then(|d| resolve("animation-timing-function", &d.value))
                    .and_then(|value| TransitionEasing::parse(&value));
                if opacity.is_none() && easing.is_none() {
                    continue;
                }
                if let Some(existing) = frames.iter_mut().find(|f| f.offset == frame.offset) {
                    if opacity.is_some() {
                        existing.opacity = opacity;
                    }
                    if easing.is_some() {
                        existing.easing = easing;
                    }
                } else {
                    frames.push(OpacityKeyframe {
                        offset: frame.offset,
                        opacity,
                        easing,
                    });
                }
            }
            if !frames.iter().any(|f| f.opacity.is_some()) {
                return None;
            }
            // Easing-only intermediate frames do not create a synthetic opacity keyframe.
            frames.retain(|f| f.opacity.is_some() || f.offset == 0.0 || f.offset == 1.0);
            for offset in [0.0, 1.0] {
                if !frames.iter().any(|f| f.offset == offset) {
                    frames.push(OpacityKeyframe {
                        offset,
                        opacity: None,
                        easing: None,
                    });
                }
            }
            frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            let animation = OpacityAnimation {
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
        .collect()
}

/** Empty timing longhands use CSS initial zero. */
fn time(value: &str) -> f64 {
    transition::time(value).unwrap_or(0.0)
}

/** Reuse computed opacity parsing and CSS-wide handling rather than a second numeric grammar. */
fn opacity(value: &str, parent: &ComputedStyle) -> Option<f64> {
    let mut style = ComputedStyle::default();
    if super::super::declaration::wide(value) {
        super::super::resolve::keyword(&mut style, parent, "opacity", value);
    } else {
        super::super::apply::apply(
            &mut style,
            "opacity",
            value,
            crate::document::RenderContext::default(),
            &mut Vec::new(),
        )
        .ok()?;
    }
    Some(style.opacity)
}
