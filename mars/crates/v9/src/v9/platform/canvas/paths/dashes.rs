/** Canvas dash bindings fully convert iterable values before selecting and replacing saved pen state. */
use super::*;
use crate::v9::vm::{heap::Key, realm::symbol};

/** These methods feed actual stroke geometry and expose detached copies of the canonical pattern. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "setLineDash", 1, set);
    b.host_method(proto, "getLineDash", 0, get);
    b.accessor(proto, "lineDashOffset", Some(offset), Some(set_offset));
}

/** Returned arrays have no mutable connection to the context's retained dash vector. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let values = vm.page.state.canvases[&node]
        .drawing
        .dash
        .segments
        .iter()
        .copied()
        .map(JsValue::Number)
        .collect();
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Resource refusals are catchable and retain the previously accepted pattern. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = assign(vm, &call);
    resource_result(vm, result)
}

/** Invalid finite/nonnegative values ignore the assignment only after every sequence conversion has run. */
fn assign(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let mut values = sequence(vm, &call.arg(0))?;
    if values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Ok(JsValue::Undefined);
    }
    if values.len() % 2 == 1 {
        values.extend_from_within(..);
    }
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    vm.page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing
        .dash
        .segments = values;
    Ok(JsValue::Undefined)
}

/** WebIDL sequences require an iterable object, and close the iterator when element conversion fails. */
fn sequence(vm: &mut Vm, value: &JsValue) -> Res<Vec<f64>> {
    if value.object().is_none() {
        return Err(vm.type_error("Canvas dash segments must be an iterable object."));
    }
    let method = vm
        .get_method(value, &Key::Sym(symbol::ITERATOR))?
        .ok_or_else(|| vm.type_error("Canvas dash segments must be iterable."))?;
    let iterator = vm.iterator_from_method(value, method)?;
    vm.pin(JsValue::Object(iterator));
    let mut values = Vec::new();
    while let Some(value) = vm.iter_step(iterator)? {
        vm.pin(value.clone());
        if values.len() == limits::DASH_INPUTS {
            let error = quota(vm);
            let _ = vm.iter_close(iterator);
            return Err(error);
        }
        match vm.to_number(&value) {
            Ok(value) => values.push(value),
            Err(error) => {
                let _ = vm.iter_close(iterator);
                return Err(error);
            }
        }
    }
    Ok(values)
}

/** Offsets preserve accepted signed values independently from normalization at draw time. */
fn offset(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    Ok(JsValue::Number(
        vm.page.state.canvases[&node].drawing.dash.offset,
    ))
}

/** Nonfinite offsets are ignored; conversion callbacks finish before the current state is selected. */
fn set_offset(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let value = vm.to_number(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if value.is_finite() {
        vm.page
            .state
            .canvases
            .get_mut(&node)
            .expect("canvas")
            .drawing
            .dash
            .offset = value;
    }
    Ok(JsValue::Undefined)
}
