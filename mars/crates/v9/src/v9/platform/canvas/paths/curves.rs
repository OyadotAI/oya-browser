/** Curve bindings finish WebIDL conversion before selecting path state or admitting adaptive geometry. */
use super::*;

/** Expose only quadratic and cubic curves backed by actual Core subdivision and rasterization. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "quadraticCurveTo", 4, method);
    b.host_method(proto, "bezierCurveTo", 6, method);
}

/** Author-visible quota and geometry refusals preserve both existing pixels and current path. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = construct(vm, &call);
    resource_result(vm, result)
}

/** The receiver is checked before conversion; even a nonfinite early coordinate does not skip later callbacks. */
fn construct(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let cubic = dom::native_name(call) == "bezierCurveTo";
    let count = if cubic { 6 } else { 4 };
    if call.args.len() < count {
        return Err(vm.type_error("Not enough arguments for canvas curve."));
    }
    let mut values = [0.0; 6];
    for (index, value) in values.iter_mut().take(count).enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Ok(JsValue::Undefined);
    }
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let transform = state.drawing.transform;
    let [first, second, end] = [
        [values[0], values[1]],
        [values[2], values[3]],
        [values[4], values[5]],
    ];
    if cubic {
        state.path.bezier_curve_to(first, second, end, transform)?;
    } else {
        state.path.quadratic_curve_to(first, second, transform)?;
    }
    Ok(JsValue::Undefined)
}
