/** The ECMAScript standard library V9 implements in Rust: one file per builtin family. */
mod array;
mod bigint;
mod clone;
mod collections;
mod date;
mod encoding;
mod error;
mod function;
mod globals;
mod intl;
mod iterators;
mod json;
mod math;
mod number;
mod object;
pub(in crate::v9) mod promise;
mod proxy;
mod reflect;
mod regexp;
mod streams;
mod string;
mod symbol;
pub(in crate::v9) mod typed;
mod weak;

pub(in crate::v9) use error::legacy_code;
pub(in crate::v9) use function::{dynamic_base, evaluate as evaluate_source};
pub(in crate::v9) use json::json_parse_text;
pub(in crate::v9) use object::{from_descriptor, to_descriptor};
pub(in crate::v9) use typed::{load as typed_load, random_values, store as typed_store};

/** Whether a value is an Array. */
pub(in crate::v9) fn is_array(vm: &Vm, value: &JsValue) -> bool {
    array::is_array_value(vm, value)
}

/** A symbol's description for display. */
pub(in crate::v9) fn symbol_text(vm: &Vm, id: u32) -> String {
    symbol::symbol_description(vm, id).unwrap_or_default()
}

use super::value::{JsString, JsValue, ObjectId};
use super::vm::heap::Key;
use super::vm::realm::Builder;
use super::vm::{Call, Res, Vm};

/** Install every builtin, in dependency order. Prototypes come first so constructors can link them. */
pub(in crate::v9) fn install(builder: &mut Builder) {
    object::prototypes(builder);
    function::install(builder);
    object::install(builder);
    symbol::install(builder);
    bigint::install(builder);
    error::install(builder);
    iterators::install(builder);
    array::install(builder);
    string::install(builder);
    number::install(builder);
    math::install(builder);
    intl::install(builder);
    json::install(builder);
    date::install(builder);
    regexp::install(builder);
    collections::install(builder);
    promise::install(builder);
    reflect::install(builder);
    globals::install(builder);
    clone::install(builder);
    encoding::install(builder);
    streams::install(builder);
    weak::install(builder);
    proxy::install(builder);
}

/** A string argument, converted. */
pub(in crate::v9) fn arg_string(vm: &mut Vm, call: &Call, index: usize) -> Res<JsString> {
    let value = call.arg(index);
    vm.to_string(&value)
}

/** A number argument, converted. */
pub(in crate::v9) fn arg_number(vm: &mut Vm, call: &Call, index: usize) -> Res<f64> {
    let value = call.arg(index);
    vm.to_number(&value)
}

/** `this` as an object, or a TypeError naming the method. */
pub(in crate::v9) fn this_object(vm: &mut Vm, call: &Call, method: &str) -> Res<ObjectId> {
    match &call.this {
        JsValue::Object(id) => Ok(*id),
        JsValue::Undefined | JsValue::Null => {
            Err(vm.type_error(&format!("{method} called on null or undefined")))
        }
        other => vm.to_object(other),
    }
}

/** A string from Rust text. */
pub(in crate::v9) fn text(vm: &mut Vm, text: &str) -> Res<JsValue> {
    let _ = vm;
    Ok(JsValue::String(JsString::from_utf8(text)?))
}

/** A key from Rust text. */
pub(in crate::v9) fn key(name: &str) -> Key {
    Key::from(name)
}
