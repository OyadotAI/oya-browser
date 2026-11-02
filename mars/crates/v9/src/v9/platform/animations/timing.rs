/* Timing dictionaries and keyframes as Chrome accepts them: every valid easing is kept, and
properties V9 does not paint run their timing without a painted effect instead of throwing. */
use super::state::Effect;

use super::*;

/** Convert a finite WebIDL timing number without accepting NaN or infinity. */
fn finite(vm: &mut Vm, value: &JsValue) -> Res<f64> {
    let number = vm.to_number(value)?;
    if !number.is_finite() {
        return Err(vm.type_error("Animation timing values must be finite"));
    }
    Ok(number)
}
/** Options use ordinary Get, so getters run before playback state is created. */
pub(super) fn parse(
    vm: &mut Vm,
    target: JsValue,
    frames: JsValue,
    options: JsValue,
) -> Res<Effect> {
    let mut effect = Effect {
        target,
        duration: 0.0,
        delay: 0.0,
        end_delay: 0.0,
        iterations: 1.0,
        fill: "auto".into(),
        direction: "normal".into(),
        easing: "linear".into(),
        from: None,
        to: None,
    };
    if !options.nullish() {
        if matches!(options, JsValue::Object(_)) {
            dictionary(vm, &mut effect, &options)?
        } else {
            effect.duration = finite(vm, &options)?;
        }
    }
    if effect.duration < 0.0 || effect.iterations < 0.0 {
        return Err(vm.type_error("Animation duration and iterations must be nonnegative"));
    }
    if !(effect.duration * effect.iterations).is_finite() || !effect.end().is_finite() {
        return Err(vm.type_error("Animation active duration must be finite"));
    }
    let values = keyframes(vm, &frames)?;
    if let Some((from, to)) = values {
        effect.from = Some(from);
        effect.to = Some(to)
    }
    Ok(effect)
}
/** Reject unsupported scheduling and composition features rather than silently dropping them. */
fn dictionary(vm: &mut Vm, effect: &mut Effect, options: &JsValue) -> Res<()> {
    for name in [
        "delay",
        "direction",
        "duration",
        "easing",
        "endDelay",
        "fill",
        "iterationStart",
        "iterations",
        "composite",
        "iterationComposite",
        "timeline",
    ] {
        let value = vm.get(options, &Key::from(name))?;
        if value == JsValue::Undefined {
            continue;
        }
        match name {
            "delay" => effect.delay = finite(vm, &value)?,
            "duration" => {
                if value != JsValue::string("auto")? {
                    effect.duration = finite(vm, &value)?
                }
            }
            "endDelay" => effect.end_delay = finite(vm, &value)?,
            "iterations" => effect.iterations = finite(vm, &value)?,
            "fill" => {
                let text = vm.to_text(&value)?;
                if !["auto", "none", "forwards", "backwards", "both"].contains(&text.as_str()) {
                    return Err(vm.type_error("Invalid animation fill"));
                }
                effect.fill = text;
            }
            "direction" => {
                let text = vm.to_text(&value)?;
                if !["normal", "reverse", "alternate", "alternate-reverse"].contains(&text.as_str())
                {
                    return Err(vm.type_error("Invalid animation direction"));
                }
                effect.direction = text;
            }
            "easing" => {
                let text = vm.to_text(&value)?;
                if !valid_easing(&text) {
                    return Err(vm.type_error(&format!("'{text}' is not a valid value for easing")));
                }
                effect.easing = text;
            }
            "iterationStart" if finite(vm, &value)? == 0.0 => {}
            "composite" if vm.to_text(&value)? == "replace" => {}
            "iterationComposite" if vm.to_text(&value)? == "replace" => {}
            "timeline" if value == JsValue::Null => {
                return Err(vm.unsupported_error("null animation timelines"));
            }
            _ => return Err(vm.unsupported_error(&format!("Animation option {name}"))),
        }
    }
    Ok(())
}
/** The painted opacity endpoints of a keyframe input, when it has exactly two opacity frames;
any other shape, and properties V9 does not paint, run their timing with no painted effect. */
fn keyframes(vm: &mut Vm, frames: &JsValue) -> Res<Option<(f64, f64)>> {
    if frames.nullish() {
        return Ok(None);
    }
    let Some(id) = frames.object() else {
        return Err(vm.type_error("Animation keyframes must be an object"));
    };
    let array = matches!(
        vm.heap.get(id).map(|object| &object.class),
        Some(Class::Array(_))
    );
    let (first, last) = if array {
        if vm.length_of(frames)? != 2 {
            return Ok(None);
        }
        let first = vm.get(frames, &Key::from("0"))?;
        let last = vm.get(frames, &Key::from("1"))?;
        (opacity_of(vm, &first)?, opacity_of(vm, &last)?)
    } else {
        let opacity = vm.get(frames, &Key::from("opacity"))?;
        if opacity.object().is_none() || vm.length_of(&opacity)? != 2 {
            return Ok(None);
        }
        let first = vm.get(&opacity, &Key::from("0"))?;
        let last = vm.get(&opacity, &Key::from("1"))?;
        (Some(finite(vm, &first)?), Some(finite(vm, &last)?))
    };
    Ok(first.zip(last))
}
/** A keyframe's numeric opacity, if it sets one. */
fn opacity_of(vm: &mut Vm, value: &JsValue) -> Res<Option<f64>> {
    if value.object().is_none() {
        return Err(vm.type_error("Animation keyframe must be an object"));
    }
    let opacity = vm.get(value, &Key::from("opacity"))?;
    if opacity == JsValue::Undefined {
        return Ok(None);
    }
    finite(vm, &opacity).map(Some)
}
/** CSS easing function syntax: the keywords, cubic-bezier() with x in [0, 1], steps() and
linear(). Anything else is the TypeError Web Animations specifies. */
pub(super) fn valid_easing(text: &str) -> bool {
    const KEYWORDS: [&str; 7] = [
        "linear",
        "ease",
        "ease-in",
        "ease-out",
        "ease-in-out",
        "step-start",
        "step-end",
    ];
    let text = text.trim();
    if KEYWORDS.contains(&text) {
        return true;
    }
    let Some((name, rest)) = text.split_once('(') else {
        return false;
    };
    let Some(arguments) = rest.strip_suffix(')') else {
        return false;
    };
    let parts: Vec<&str> = arguments.split(',').map(str::trim).collect();
    match name.trim() {
        "cubic-bezier" => cubic_bezier(&parts),
        "steps" => steps(&parts),
        "linear" => parts.iter().all(|part| linear_stop(part)),
        _ => false,
    }
}
/** Four numbers whose x coordinates lie in [0, 1]. */
fn cubic_bezier(parts: &[&str]) -> bool {
    let numbers: Vec<f64> = parts.iter().filter_map(|part| part.parse().ok()).collect();
    numbers.len() == 4
        && parts.len() == 4
        && numbers.iter().all(|number| number.is_finite())
        && (0.0..=1.0).contains(&numbers[0])
        && (0.0..=1.0).contains(&numbers[2])
}
/** A positive step count and an optional jump position (jump-none needs two steps). */
fn steps(parts: &[&str]) -> bool {
    const POSITIONS: [&str; 6] = [
        "jump-start",
        "jump-end",
        "jump-none",
        "jump-both",
        "start",
        "end",
    ];
    let Some(Ok(count)) = parts.first().map(|part| part.parse::<u32>()) else {
        return false;
    };
    match parts.get(1) {
        None => parts.len() == 1 && count >= 1,
        Some(&"jump-none") => parts.len() == 2 && count >= 2,
        Some(position) => parts.len() == 2 && count >= 1 && POSITIONS.contains(position),
    }
}
/** One linear() stop: a number, optionally followed by one or two percentages. */
fn linear_stop(part: &str) -> bool {
    let mut words = part.split_whitespace();
    let Some(Ok(value)) = words.next().map(str::parse::<f64>) else {
        return false;
    };
    value.is_finite()
        && words.all(|word| {
            word.strip_suffix('%')
                .and_then(|number| number.parse::<f64>().ok())
                .is_some_and(f64::is_finite)
        })
}
/** A detached plain result cannot mutate the effect's private normalized timing. */
pub(super) fn timing_object(vm: &mut Vm, effect: &Effect) -> Res<JsValue> {
    let id = vm.new_object()?;
    for (name, value) in [
        ("delay", effect.delay),
        ("endDelay", effect.end_delay),
        ("duration", effect.duration),
        ("iterations", effect.iterations),
        ("iterationStart", 0.0),
    ] {
        vm.put(id, name, JsValue::Number(value))?;
    }
    for (name, value) in [
        ("fill", effect.fill.as_str()),
        ("direction", effect.direction.as_str()),
        ("easing", effect.easing.as_str()),
    ] {
        vm.put(id, name, JsValue::string(value)?)?;
    }
    Ok(JsValue::Object(id))
}
/** Normalized frames expose their actual offsets and numeric opacity values. */
pub(super) fn frame_objects(vm: &mut Vm, effect: &Effect) -> Res<JsValue> {
    let mut frames = Vec::new();
    if let (Some(from), Some(to)) = (effect.from, effect.to) {
        for (offset, opacity) in [(0.0, from), (1.0, to)] {
            let id = vm.new_object()?;
            vm.put(id, "opacity", JsValue::Number(opacity))?;
            vm.put(id, "offset", JsValue::Number(offset))?;
            vm.put(id, "computedOffset", JsValue::Number(offset))?;
            vm.put(id, "easing", JsValue::string("linear")?)?;
            vm.put(id, "composite", JsValue::string("replace")?)?;
            frames.push(JsValue::Object(id));
        }
    }
    vm.new_array(frames).map(JsValue::Object)
}
