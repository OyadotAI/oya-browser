/** Buffer transfer copies bytes to a fresh allocation then permanently detaches the source. */
use super::*;

/** The two operations differ only in whether the result retains the resizable maximum. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.method(prototype, "transfer", 0, |vm, call| {
        transfer(vm, call, true)
    });
    builder.method(prototype, "transferToFixedLength", 0, |vm, call| {
        transfer(vm, call, false)
    });
}

/** Conversion runs before re-checking source detachment and validating the preserved maximum. */
fn transfer(vm: &mut Vm, call: Call, preserve: bool) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    let length = if matches!(call.arg(0), JsValue::Undefined) {
        buffer_bytes(vm, id).map_or(0, Vec::len)
    } else {
        to_index(vm, &call.arg(0), "ArrayBuffer transfer length")?
    };
    if buffer_state::detached(vm, id) {
        return Err(vm.type_error("Cannot transfer a detached ArrayBuffer"));
    }
    if !buffer_state::buffer_detachable(vm, id) {
        return Err(vm.type_error("ArrayBuffer cannot be detached"));
    }
    let maximum = preserve.then(|| buffer_state::maximum(vm, id)).flatten();
    if length > BUFFER_BYTES || maximum.is_some_and(|maximum| length > maximum) {
        return Err(vm.range_error("Invalid ArrayBuffer transfer length"));
    }
    let mut bytes = buffer_bytes(vm, id).cloned().unwrap_or_default();
    bytes.resize(length, 0);
    let output = new_buffer(vm, bytes)?;
    if let Some(maximum) = maximum {
        buffer_state::set_maximum(vm, output, maximum);
    }
    buffer_state::detach(vm, id);
    Ok(JsValue::Object(output))
}
