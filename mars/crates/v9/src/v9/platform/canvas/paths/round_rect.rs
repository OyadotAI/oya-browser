/** Rounded-rectangle binding converts full WebIDL radii before selecting path state or validating geometry. */
use super::*;
mod radii;

/** The method appends real closed rounded geometry and participates in ordinary fill and stroke. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "roundRect", 4, method);
}

/** Resource and numeric failures retain the previous path and pixels. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = construct(vm, &call);
    resource_result(vm, result)
}

/** WebIDL conversion happens even when rectangle coordinates later make the operation a no-op. */
fn construct(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if call.args.len() < 4 {
        return Err(vm.type_error("Not enough arguments for canvas rounded rectangle."));
    }
    let mut rectangle = [0.0; 4];
    for (index, value) in rectangle.iter_mut().enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    let radii = radii::convert(vm, &call.arg(4))?;
    if rectangle.iter().any(|value| !value.is_finite()) {
        return Ok(JsValue::Undefined);
    }
    if !(1..=4).contains(&radii.count) {
        return Err(vm.range_error("roundRect requires one to four radii."));
    }
    for radius in &radii.values {
        if radius.iter().any(|value| !value.is_finite()) {
            return Ok(JsValue::Undefined);
        }
        if radius.iter().any(|value| *value < 0.0) {
            return Err(vm.range_error("roundRect radii must be nonnegative."));
        }
    }
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let transform = state.drawing.transform;
    state.path.round_rect(rectangle, &radii.values, transform)?;
    Ok(JsValue::Undefined)
}
