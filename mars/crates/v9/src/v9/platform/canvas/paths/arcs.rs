/** Arc bindings preserve conversion order and radius errors before constructing any current-path geometry. */
use super::*;

/** Both exposed methods share a real transformed elliptical-arc implementation. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "arc", 5, method);
    b.host_method(proto, "ellipse", 7, method);
}

/** Numeric and point-budget failures remain catchable and leave the previous path intact. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = construct(vm, &call);
    resource_result(vm, result)
}

/** Coercion runs left-to-right before finite/radius checks and before selecting mutable context state. */
fn construct(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let circle = dom::native_name(call) == "arc";
    let count = if circle { 5 } else { 7 };
    if call.args.len() < count {
        return Err(vm.type_error("Not enough arguments for canvas arc."));
    }
    let mut values = [0.0; 7];
    for (index, value) in values.iter_mut().take(count).enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    let counterclockwise = call.arg(count).truthy();
    if circle && values[2].is_finite() && values[2] < 0.0 {
        return Err(radius_error(vm));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Ok(JsValue::Undefined);
    }
    if !circle && (values[2] < 0.0 || values[3] < 0.0) {
        return Err(radius_error(vm));
    }
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let transform = state.drawing.transform;
    if circle {
        state.path.arc(
            [values[0], values[1]],
            values[2],
            [values[3], values[4]],
            counterclockwise,
            transform,
        )?;
    } else {
        state.path.ellipse(
            [values[0], values[1]],
            [values[2], values[3]],
            values[4],
            [values[5], values[6]],
            counterclockwise,
            transform,
        )?;
    }
    Ok(JsValue::Undefined)
}

/** A finite negative radius is a geometric domain error rather than a resource refusal. */
fn radius_error(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception("IndexSizeError", "Canvas arc radii must be nonnegative.")
}
