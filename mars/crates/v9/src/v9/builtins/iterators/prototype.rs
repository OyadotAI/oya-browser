/** Iterator prototype accessors protect the intrinsic home while allowing ordinary derived properties. */
use super::*;
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::realm;
use std::sync::OnceLock;

/** Constructor identity is mapped to the active realm instead of read from an authored property. */
static CONSTRUCTOR: OnceLock<ObjectId> = OnceLock::new();

/** Accessor installation replaces the constructor's initial data property with the standard home guard. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId, constructor: ObjectId) {
    let _ = CONSTRUCTOR.set(constructor);
    builder.builtin_accessor(
        prototype,
        "constructor",
        Some(get_constructor),
        Some(set_constructor),
    );
    let get = builder.function("get [Symbol.toStringTag]", 0, get_tag);
    let set = builder.function("set [Symbol.toStringTag]", 1, set_tag);
    builder.put(
        prototype,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Accessor(Some(get), Some(set)),
        CONFIGURABLE,
    );
}

/** The getter ignores this, including undefined, and returns its creation realm's constructor. */
fn get_constructor(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Object(vm.intrinsic_id(
        *CONSTRUCTOR.get().expect("Iterator installed"),
    )))
}

/** The tag getter is constant and does not access the receiver. */
fn get_tag(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::string("Iterator")?)
}

/** A constructor assignment invokes the standard intrinsic home setter. */
fn set_constructor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    set(vm, call, Key::from("constructor"))
}

/** A tag assignment invokes the same home setter with its symbol key. */
fn set_tag(vm: &mut Vm, call: Call) -> Res<JsValue> {
    set(vm, call, Key::Sym(symbol::TO_STRING_TAG))
}

/** An existing own property uses strict Set; an absent one creates a complete ordinary data property. */
fn set(vm: &mut Vm, call: Call, key: Key) -> Res<JsValue> {
    let Some(id) = call.this.object() else {
        return Err(vm.type_error("Iterator prototype setter called on non-object"));
    };
    if id == realm().ids.iterator_prototype {
        return Err(vm.type_error("Cannot assign to Iterator.prototype intrinsic property"));
    }
    if vm.own(id, &key)?.is_some() {
        vm.set(&call.this, key, call.arg(0), true)?;
    } else {
        let descriptor = Descriptor {
            value: Some(call.arg(0)),
            writable: Some(true),
            enumerable: Some(true),
            configurable: Some(true),
            ..Descriptor::default()
        };
        if !vm.define(id, key, descriptor)? {
            return Err(vm.type_error("Cannot create iterator property"));
        }
    }
    Ok(JsValue::Undefined)
}
