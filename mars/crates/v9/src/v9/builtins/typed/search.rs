/** Typed-array searches use an actual branded view and its captured internal length, not array-like properties. */
use super::*;

/** ValidateTypedArray precedes argument coercion; later resize or detachment affects each indexed read. */
pub(super) fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    if call.args.is_empty() || view.length == 0 {
        return Ok(JsValue::Boolean(false));
    }
    let start = vm.relative_index(&call.arg(1), view.length, 0)?;
    let searched = call.arg(0);
    for index in start..view.length {
        vm.charge(1)?;
        if load(vm, id, index)
            .unwrap_or(JsValue::Undefined)
            .same_value_zero(&searched)
        {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

/** Search only live typed elements, using strict equality and the original witness length. */
pub(super) fn index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    if view.length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let start = vm.relative_index(&call.arg(1), view.length, 0)?;
    let searched = call.arg(0);
    for index in start..view.length {
        vm.charge(1)?;
        if load(vm, id, index).is_some_and(|value| value.strict_equal(&searched)) {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

/** A missing start means the last element; explicit undefined converts to zero. */
pub(super) fn last_index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    if view.length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let relative = if call.args.len() > 1 {
        vm.to_integer(&call.arg(1))?
    } else {
        view.length as f64 - 1.0
    };
    let start = if relative < 0.0 {
        view.length as f64 + relative
    } else {
        relative.min(view.length as f64 - 1.0)
    };
    let searched = call.arg(0);
    let mut index = start;
    while index >= 0.0 {
        vm.charge(1)?;
        if load(vm, id, index as usize).is_some_and(|value| value.strict_equal(&searched)) {
            return Ok(JsValue::Number(index));
        }
        index -= 1.0;
    }
    Ok(JsValue::Number(-1.0))
}
