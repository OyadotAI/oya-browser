/** D8's standalone serializer reads and writes bounded native V8 wire records through V9's own graph implementation. */
use super::*;
use crate::v9::vm::data::{Bytes, Class};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
/** Native FunctionTemplate construction is retained while serialized data never carries executable authority. */
pub(super) fn install(builder: &mut Builder, d8: crate::v9::value::ObjectId) {
    let namespace = builder.object(Some(builder.ids.object_prototype));
    for (name, length, function) in [
        ("serialize", 0, serialize as crate::v9::vm::realm::NativeFn),
        ("deserialize", 1, deserialize),
    ] {
        let prototype = builder.object(Some(builder.ids.object_prototype));
        let function = builder.constructor(name, length, function, prototype, None);
        builder.alias(namespace, name, function);
    }
    builder.alias(d8, "serializer", namespace);
}
/** All arguments share one real stream; ordinary functions and capabilities without a delegate are rejected. */
fn serialize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = super::workers::serialize(vm, call.args)?;
    vm.alloc(Object::new(
        Class::ArrayBuffer(Bytes(bytes)),
        Some(realm().ids.array_buffer_prototype),
    ))
    .map(JsValue::Object)
}
/** Buffer brands are checked before reading any bytes; decoding allocates only after structural validation. */
fn deserialize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = match call
        .arg(0)
        .object()
        .and_then(|id| vm.heap.get(id))
        .map(|object| &object.class)
    {
        Some(Class::ArrayBuffer(bytes)) => bytes.0.clone(),
        _ => {
            let error = vm.make_error(
                crate::v9::vm::error::ErrorKind::Error,
                "Can only deserialize from an ArrayBuffer",
            )?;
            return Err(crate::v9::vm::Thrown::Value(JsValue::Object(error)));
        }
    };
    let value = super::workers::deserialize(vm, &bytes)?;
    if call.new_target.is_some() && value.object().is_none() {
        return super::files::receiver(vm, call.new_target);
    }
    Ok(value)
}
