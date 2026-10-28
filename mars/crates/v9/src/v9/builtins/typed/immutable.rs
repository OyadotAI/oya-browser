/** Immutable buffers share one read-only backing capability across every typed and DataView alias. */
use super::*;

/** The proposal's operations return real fixed buffers rather than changing ordinary object integrity. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.builtin_accessor(prototype, "immutable", Some(get), None);
    builder.method(prototype, "sliceToImmutable", 2, slice);
}

/** Receiver branding is identical to the other ordinary ArrayBuffer accessors. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    Ok(JsValue::Boolean(buffer_state::immutable(vm, id)))
}

/** Write validation precedes argument coercion in native mutating view methods. */
pub(super) fn writable(vm: &mut Vm, buffer: ObjectId) -> Res<()> {
    if buffer_state::immutable(vm, buffer) {
        Err(vm.type_error("Cannot modify an immutable ArrayBuffer"))
    } else {
        Ok(())
    }
}

/** The receiver's current live witness remains authoritative after user callbacks. */
pub(super) fn typed(vm: &mut Vm, call: &Call) -> Res<(ObjectId, ViewData)> {
    let (id, view) = this_typed(vm, call)?;
    writable(vm, view.buffer)?;
    Ok((id, view))
}

/** Immutable slicing bypasses species and rejects a source that shrank past the selected end. */
fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    if buffer_state::detached(vm, id) {
        return Err(vm.type_error("Cannot slice a detached ArrayBuffer"));
    }
    let length = storage_len(vm, id).unwrap_or(0);
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let end = vm.relative_index(&call.arg(1), length, length)?;
    let count = end.saturating_sub(start);
    if count > BUFFER_BYTES { return Err(vm.range_error("ArrayBuffer slice exceeds resident byte limit")); }
    let output = new_buffer(vm, vec![0; count])?;
    if buffer_state::detached(vm, id) {
        return Err(vm.type_error("Cannot slice a detached ArrayBuffer"));
    }
    if storage_len(vm, id).is_none_or(|length| length < end) {
        return Err(vm.range_error("ArrayBuffer shrank during immutable slicing"));
    }
    let bytes = if count == 0 {
        Vec::new()
    } else {
        read_range(vm, id, start, count).ok_or_else(|| vm.range_error("ArrayBuffer slice exceeds resident byte limit"))?
    };
    if !write_range(vm, output, 0, &bytes) {
        return Err(vm.type_error("Cannot initialize immutable ArrayBuffer"));
    }
    buffer_state::make_immutable(vm, output);
    Ok(JsValue::Object(output))
}

/** Length-preserving generic algorithms get native typed write validation even for empty targets. */
pub(super) fn mutator(vm: &mut Vm, call: Call) -> Res<JsValue> {
    typed(vm, &call)?;
    let name = callee_name(&call);
    let function = realm()
        .object(realm().ids.array_prototype)
        .and_then(|object| object.get(&key(name)))
        .and_then(|(_, property, _)| match property {
            Static::Value(value) => Some(value.clone()),
            _ => None,
        })
        .expect("intrinsic array mutator");
    vm.call(function, call.this, call.args.to_vec())
}
