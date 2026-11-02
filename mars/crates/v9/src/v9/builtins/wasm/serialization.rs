/** Conformance serialization carries owned source bytes and recompiles them after exact integrity checks. */
use super::*;
const ENVELOPE: &[u8] = b"MARSWASM\x01";

/** d8's explicit testing surface does not expose private Module records to author reflection. */
pub(super) fn install(b: &mut Builder, d8: ObjectId) {
    let namespace = b.object(Some(b.ids.object_prototype));
    b.alias(d8, "wasm", namespace);
    b.method(namespace, "serializeModule", 1, serialize);
    b.method(namespace, "deserializeModule", 2, deserialize);
}
/** Deterministic source serialization never includes pointers, native code or process authority. */
fn serialize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, _) = module(vm, &call.arg(0))?;
    let source = bytes(vm, &get(vm, id, MODULE).unwrap())?;
    vm.charge(source.len())?;
    let mut encoded = ENVELOPE.to_vec();
    encoded.extend(&source);
    buffer(vm, encoded).map(JsValue::Object)
}
/** Option getters run before copying input, and both supplied byte sequences are then immutable copies. */
fn deserialize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !call.arg(0).object().is_some_and(|id| {
        matches!(
            vm.heap.get(id).map(|o| &o.class),
            Some(Class::ArrayBuffer(_))
        )
    }) {
        return Err(vm.type_error("First argument must be an ArrayBuffer"));
    }
    if !call
        .arg(1)
        .object()
        .and_then(|id| super::super::view_of(vm, id))
        .is_some_and(|v| v.kind != crate::v9::vm::data::ElementKind::DataView)
    {
        return Err(vm.type_error("Second argument must be a TypedArray"));
    }
    let options = imports::options(vm, &call.arg(2))?;
    if call
        .arg(0)
        .object()
        .is_some_and(|id| super::super::buffer_detached(vm, id))
        || call
            .arg(1)
            .object()
            .and_then(|id| super::super::view_of(vm, id))
            .is_some_and(|v| v.invalid)
    {
        return Err(vm.type_error("detached WebAssembly serialization input"));
    }
    let encoded = bytes(vm, &call.arg(0))?;
    let source = bytes(vm, &call.arg(1))?;
    if encoded.strip_prefix(ENVELOPE) != Some(source.as_slice()) {
        return Err(vm.type_error("Trying to deserialize manipulated bytes"));
    }
    vm.charge(source.len())?;
    finish_module(vm, source, options, None).map(JsValue::Object)
}
