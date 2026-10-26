/** Reflect: the object-model operations as functions. */
use super::*;
use crate::v9::vm::heap::CONFIGURABLE;
use crate::v9::vm::realm::{NativeFn, Static, realm, string, symbol};
mod diagnostics;

/** The Reflect namespace. */
pub(super) fn install(b: &mut Builder) {
    let reflect = b.object(Some(b.ids.object_prototype));
    b.global("Reflect", reflect);
    let methods: [(&'static str, u32, NativeFn); 13] = [
        ("apply", 3, apply),
        ("construct", 2, construct),
        ("defineProperty", 3, define_property),
        ("deleteProperty", 2, delete_property),
        ("get", 2, get),
        ("getOwnPropertyDescriptor", 2, own_descriptor),
        ("getPrototypeOf", 1, get_prototype),
        ("has", 2, has),
        ("isExtensible", 1, is_extensible),
        ("ownKeys", 1, own_keys),
        ("preventExtensions", 1, prevent_extensions),
        ("set", 3, set),
        ("setPrototypeOf", 2, set_prototype),
    ];
    for (name, length, function) in methods {
        b.method(reflect, name, length, function);
    }
    b.put(
        reflect,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Reflect")),
        CONFIGURABLE,
    );
}

/** The target object argument. */
fn target(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    match call.arg(0) {
        JsValue::Object(id) => Ok(id),
        _ => Err(vm.type_error("Reflect method called on non-object")),
    }
}

/** Reflect.apply. */
fn apply(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.arg(0)) {
        return diagnostics::reject_apply(vm, call.arg(0));
    }
    let args = super::function::list_from_array_like(vm, &call.arg(2))?;
    vm.forward_call(call.arg(0), call.arg(1), args)
}

/** Reflect.construct. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_constructor(&call.arg(0)) {
        return constructor_error(vm, call.arg(0));
    }
    let new_target = if call.args.len() > 2 {
        Some(call.arg(2))
    } else {
        None
    };
    // Both are checked before the argument list is read, as the specification orders it.
    if new_target.as_ref().is_some_and(|t| !vm.is_constructor(t)) {
        return constructor_error(vm, call.arg(2));
    }
    let args = super::function::list_from_array_like(vm, &call.arg(1))?;
    vm.construct(call.arg(0), args, new_target)
}

/** Reflect.defineProperty. */
fn define_property(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    let desc = super::object::to_descriptor(vm, &call.arg(2))?;
    vm.define(id, property, desc).map(JsValue::Boolean)
}

/** Reflect.deleteProperty. */
fn delete_property(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    vm.delete(id, &property).map(JsValue::Boolean)
}

/** Reflect.get. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    let receiver = if call.args.len() > 2 {
        call.arg(2)
    } else {
        JsValue::Object(id)
    };
    vm.get_from(id, &property, &receiver)
}

/** Reflect.getOwnPropertyDescriptor. */
fn own_descriptor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    super::from_descriptor(vm, id, &property)
}

/** Reflect.getPrototypeOf. */
fn get_prototype(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    Ok(super::integrity::get_prototype_of(vm, id)?.map_or(JsValue::Null, JsValue::Object))
}

/** Reflect.has. */
fn has(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    vm.has_property(id, &property).map(JsValue::Boolean)
}

/** Reflect.isExtensible. */
fn is_extensible(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    super::integrity::is_extensible(vm, id).map(JsValue::Boolean)
}

/** Reflect.ownKeys. */
fn own_keys(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let keys: Vec<JsValue> = vm.own_keys(id)?.iter().map(Key::value).collect();
    Ok(JsValue::Object(vm.new_array(keys)?))
}

/** Reflect.preventExtensions. */
fn prevent_extensions(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    super::integrity::prevent_extensions(vm, id).map(JsValue::Boolean)
}

/** Reflect.set. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    let receiver = if call.args.len() > 3 {
        call.arg(3)
    } else {
        JsValue::Object(id)
    };
    vm.set_from(id, property, call.arg(2), &receiver)
        .map(JsValue::Boolean)
}

/** Reflect.setPrototypeOf. */
fn set_prototype(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = target(vm, &call)?;
    let proto = match call.arg(1) {
        JsValue::Object(proto) => Some(proto),
        JsValue::Null => None,
        _ => return Err(vm.type_error("Object prototype may only be an Object or null")),
    };
    super::integrity::set_prototype_of(vm, id, proto).map(JsValue::Boolean)
}

/** Constructor diagnostics use retained function source without invoking authored toString or proxy traps. */
fn constructor_error(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    let display = if vm.is_callable(&value) {
        let call = Call {
            this: value,
            args: &[],
            new_target: None,
            callee: realm().ids.function_prototype,
        };
        match super::function::to_string(vm, call)? {
            JsValue::String(source) => source.lossy(),
            _ => String::new(),
        }
    } else {
        vm.callee_text(&value)
    };
    Err(vm.type_error(&format!("{display} is not a constructor")))
}
