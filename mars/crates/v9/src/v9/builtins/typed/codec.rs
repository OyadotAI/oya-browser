/** Uint8Array hexadecimal and base64 codecs operate on real bytes with bounded partial writes. */
use super::*;
mod base64;
mod decode;
mod hex;

/** Only Uint8Array's own constructor and prototype expose these methods. */
pub(super) fn install(builder: &mut Builder, constructor: ObjectId, prototype: ObjectId) {
    builder.method(constructor, "fromHex", 1, hex::from);
    builder.method(constructor, "fromBase64", 1, base64::from);
    builder.method(prototype, "toHex", 0, hex::to);
    builder.method(prototype, "setFromHex", 1, hex::set);
    builder.method(prototype, "toBase64", 0, base64::to);
    builder.method(prototype, "setFromBase64", 1, base64::set);
}

/** Decoding accepts primitive strings only, and never invokes a supplied object's coercion hooks. */
fn input(vm: &mut Vm, value: JsValue) -> Res<JsString> {
    match value {
        JsValue::String(text) => Ok(text),
        _ => Err(vm.type_error("Input must be a string")),
    }
}

/** Native brands exclude Uint8ClampedArray and proxies, even when they expose matching properties. */
fn receiver(vm: &mut Vm, call: &Call) -> Res<ViewData> {
    let view = brand(vm, call)?;
    if view.invalid {
        return Err(
            vm.type_error("Cannot perform operation on a detached or out-of-bounds typed array")
        );
    }
    Ok(view)
}

/** Encoder options are observed before the final bounds witness, after an unobservable native brand check. */
fn brand(vm: &mut Vm, call: &Call) -> Res<ViewData> {
    let view = call
        .this
        .object()
        .and_then(|id| view_of(vm, id))
        .filter(|view| view.kind == ElementKind::Uint8);
    view.ok_or_else(|| vm.type_error("Receiver is not a Uint8Array"))
}

/** Reads and writes report their exact consumed UTF-16 and emitted byte counts. */
fn result(vm: &mut Vm, read: usize, written: usize) -> Res<JsValue> {
    let object = vm.new_object()?;
    vm.put(object, "read", JsValue::Number(read as f64))?;
    vm.put(object, "written", JsValue::Number(written as f64))?;
    Ok(JsValue::Object(object))
}

/** Parsing may fail after valid complete chunks; those completed bytes are still written first. */
fn commit(vm: &mut Vm, view: &ViewData, bytes: &[u8]) -> Res<()> {
    if !write_range(vm, view.buffer, view.offset, bytes) {
        return Err(vm.type_error("Uint8Array backing is no longer writable"));
    }
    Ok(())
}

/** Options distinguish undefined from null and require objects without primitive boxing. */
fn options(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    match value {
        JsValue::Undefined | JsValue::Object(_) => Ok(value),
        _ => Err(vm.type_error("Options must be an object")),
    }
}

/** Missing options have no getters; provided options use the ordinary observable Get operation. */
fn option(vm: &mut Vm, value: &JsValue, name: &str) -> Res<JsValue> {
    if matches!(value, JsValue::Undefined) {
        Ok(JsValue::Undefined)
    } else {
        vm.get(value, &Key::from(name))
    }
}
