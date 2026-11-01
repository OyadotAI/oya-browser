/** Native storage brands avoid exception-based probing for every object in a persisted graph. */
use crate::v9::value::JsValue;
use crate::v9::vm::data::Class;
use crate::v9::vm::{Call, Res, Vm};

/** Stored values were already cloned; their native class selects an encoder without author hooks. */
pub(super) fn kind(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let object = call.arg(0).object().and_then(|id| vm.heap.get(id));
    let kind = match object.map(|object| &object.class) {
        Some(Class::Array(_)) => "Array",
        Some(Class::Date(_)) => "Date",
        Some(Class::View(_)) => "View",
        Some(Class::ArrayBuffer(_)) => "ArrayBuffer",
        Some(Class::Map(_)) => "Map",
        Some(Class::Set(_)) => "Set",
        Some(Class::RegExp(_)) => "RegExp",
        Some(Class::Boolean(_)) => "Boolean",
        Some(Class::Number(_)) => "Number",
        Some(Class::String(_)) => "String",
        Some(Class::BigInt(_) | Class::BigIntWide(_)) => "BigInt",
        Some(Class::Error) => "Error",
        Some(Class::Plain) if crate::v9::platform::blob::is_blob(vm, &call.arg(0)) => "Blob",
        _ => "Object",
    };
    JsValue::string(kind).map_err(Into::into)
}
