/* Supported timing dictionaries and normalized opacity keyframes reject unimplemented effects explicitly. */
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
            "easing" if vm.to_text(&value)? == "linear" => {}
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
/** Keyframe inputs are either two opacity frames or property-indexed opacity endpoints. */
fn keyframes(vm: &mut Vm, frames: &JsValue) -> Res<Option<(f64, f64)>> {
    if frames.nullish() {
        return Ok(None);
    }
    let Some(id) = frames.object() else {
        return Err(vm.type_error("Animation keyframes must be an object"));
    };
    if matches!(
        vm.heap.get(id).map(|object| &object.class),
        Some(Class::Array(_))
    ) {
        let length = vm.length_of(frames)?;
        if length == 0 {
            return Ok(None);
        }
        if length != 2 {
            return Err(vm.unsupported_error("opacity animations require two keyframes"));
        }
        let first = vm.get(frames, &Key::from("0"))?;
        let second = vm.get(frames, &Key::from("1"))?;
        return Ok(Some((frame(vm, &first, 0.0)?, frame(vm, &second, 1.0)?)));
    }
    let keys = vm.enumerable_keys(id)?;
    for key in &keys {
        if *key != Key::from("opacity") {
            return Err(vm.unsupported_error("animation properties other than numeric opacity"));
        }
    }
    let opacity = vm.get(frames, &Key::from("opacity"))?;
    if opacity == JsValue::Undefined {
        return Ok(None);
    }
    if vm.length_of(&opacity)? != 2 {
        return Err(vm.unsupported_error("opacity animations require two endpoints"));
    }
    let first = vm.get(&opacity, &Key::from("0"))?;
    let last = vm.get(&opacity, &Key::from("1"))?;
    Ok(Some((finite(vm, &first)?, finite(vm, &last)?)))
}
/** Frame metadata is validated along with the CSS value; no unsupported easing is ignored. */
fn frame(vm: &mut Vm, value: &JsValue, offset: f64) -> Res<f64> {
    let Some(id) = value.object() else {
        return Err(vm.type_error("Animation keyframe must be an object"));
    };
    for key in vm.enumerable_keys(id)? {
        if !["opacity", "offset", "easing", "composite"]
            .iter()
            .any(|name| key == Key::from(name))
        {
            return Err(vm.unsupported_error("animation properties other than numeric opacity"));
        }
    }
    let actual = vm.get(value, &Key::from("offset"))?;
    if !actual.nullish() && finite(vm, &actual)? != offset {
        return Err(vm.unsupported_error("opacity keyframe offsets other than zero and one"));
    }
    for (name, expected) in [("easing", "linear"), ("composite", "replace")] {
        let actual = vm.get(value, &Key::from(name))?;
        if actual != JsValue::Undefined && vm.to_text(&actual)? != expected {
            return Err(vm.unsupported_error("keyframe easing or composition"));
        }
    }
    let opacity = vm.get(value, &Key::from("opacity"))?;
    if opacity == JsValue::Undefined {
        return Err(vm.unsupported_error("implicit underlying opacity endpoints"));
    }
    finite(vm, &opacity)
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
        ("easing", "linear"),
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
