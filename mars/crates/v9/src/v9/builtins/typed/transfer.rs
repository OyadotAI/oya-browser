/** Buffer transfer copies bytes to a fresh allocation then permanently detaches the source. */
use super::*;

/** The two operations differ only in whether the result retains the resizable maximum. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.method(prototype, "transfer", 0, |vm, call| {
        transfer(vm, call, true, false)
    });
    builder.method(prototype, "transferToFixedLength", 0, |vm, call| {
        transfer(vm, call, false, false)
    });
    builder.method(prototype, "transferToImmutable", 0, |vm, call| {
        transfer(vm, call, false, true)
    });
}

/** Conversion runs before re-checking source detachment and validating the preserved maximum. */
fn transfer(vm: &mut Vm, call: Call, preserve: bool, immutable: bool) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    let length = if matches!(call.arg(0), JsValue::Undefined) {
        storage_len(vm, id).unwrap_or(0)
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
    let existing = storage_len(vm, id).unwrap_or(0).min(length);
    let mut bytes = read_range(vm, id, 0, existing)
        .ok_or_else(|| vm.range_error("Invalid ArrayBuffer transfer length"))?;
    bytes.resize(length, 0);
    let output = new_buffer(vm, bytes)?;
    if let Some(maximum) = maximum {
        buffer_state::set_maximum(vm, output, maximum);
    }
    if immutable {
        buffer_state::make_immutable(vm, output);
    }
    buffer_state::detach(vm, id);
    Ok(JsValue::Object(output))
}
