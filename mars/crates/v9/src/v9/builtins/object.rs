/** Object: the constructor, its reflection statics and Object.prototype. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{CONFIGURABLE, ENUMERABLE, Object, Slot, WRITABLE};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{realm, symbol};

/** Create Object.prototype and Function.prototype before anything links to them. */
pub(super) fn prototypes(b: &mut Builder) {
    let object_prototype = b.object(None);
    b.ids.object_prototype = object_prototype;
    b.ids.function_prototype = b.object(Some(object_prototype));
}

/** The Object constructor and Object.prototype methods. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.ids.object_prototype;
    let ctor = b.constructor("Object", 1, construct, proto, None);
    b.global("Object", ctor);
    for (name, length, function) in [
        ("keys", 1, keys as super::super::vm::realm::NativeFn),
        ("values", 1, values),
        ("entries", 1, entries),
        ("assign", 2, assign),
        ("create", 2, create),
        ("defineProperty", 3, define_property),
        ("defineProperties", 2, define_properties),
        ("getOwnPropertyNames", 1, own_names),
        ("getOwnPropertySymbols", 1, own_symbols),
        ("getOwnPropertyDescriptor", 2, own_descriptor),
        ("getOwnPropertyDescriptors", 1, own_descriptors),
        ("getPrototypeOf", 1, get_prototype),
        ("setPrototypeOf", 2, set_prototype),
        ("freeze", 1, freeze),
        ("seal", 1, seal),
        ("preventExtensions", 1, prevent_extensions),
        ("isFrozen", 1, is_frozen),
        ("isSealed", 1, is_sealed),
        ("isExtensible", 1, is_extensible),
        ("is", 2, is),
        ("fromEntries", 1, from_entries),
        ("hasOwn", 2, has_own),
        ("groupBy", 2, group_by),
    ] {
        b.method(ctor, name, length, function);
    }
    b.method(proto, "hasOwnProperty", 1, has_own_property);
    b.method(proto, "isPrototypeOf", 1, is_prototype_of);
    b.method(proto, "propertyIsEnumerable", 1, property_is_enumerable);
    b.method(proto, "toString", 0, to_string);
    b.method(proto, "toLocaleString", 0, to_locale_string);
    b.method(proto, "valueOf", 0, value_of);
    b.method(proto, "__defineGetter__", 2, define_getter);
    b.method(proto, "__defineSetter__", 2, define_setter);
    b.method(proto, "__lookupGetter__", 1, lookup_getter);
    b.builtin_accessor(proto, "__proto__", Some(proto_get), Some(proto_set));
}

/** `Object(value)` and `new Object()`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    if value.nullish() {
        return Ok(JsValue::Object(vm.new_object()?));
    }
    Ok(JsValue::Object(vm.to_object(&value)?))
}

/** An object argument or a TypeError. */
fn object_arg(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    match value {
        JsValue::Object(id) => Ok(*id),
        JsValue::Undefined | JsValue::Null => {
            Err(vm.type_error("Cannot convert undefined or null to object"))
        }
        other => vm.to_object(other),
    }
}

/** Object.keys. */
fn keys(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let list: Vec<JsValue> = vm.enumerable_keys(id)?.iter().map(Key::value).collect();
    Ok(JsValue::Object(vm.new_array(list)?))
}

/** Object.values. */
fn values(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let mut list = Vec::new();
    for key in vm.enumerable_keys(id)? {
        list.push(vm.get(&JsValue::Object(id), &key)?);
    }
    Ok(JsValue::Object(vm.new_array(list)?))
}

/** Object.entries. */
fn entries(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let result = vm.new_array(Vec::new())?;
    for key in vm.enumerable_keys(id)? {
        let value = vm.get(&JsValue::Object(id), &key)?;
        let pair = vm.new_array(vec![key.value(), value])?;
        vm.array_append(&JsValue::Object(result), JsValue::Object(pair))?;
    }
    Ok(JsValue::Object(result))
}

/** Object.assign. */
fn assign(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = object_arg(vm, &call.arg(0))?;
    for source in call.args.iter().skip(1) {
        if source.nullish() {
            continue;
        }
        let source = vm.to_object(source)?;
        for key in vm.own_keys(source)? {
            let Some(prop) = vm.own(source, &key)? else {
                continue;
            };
            if prop.attrs & ENUMERABLE == 0 {
                continue;
            }
            let value = vm.get(&JsValue::Object(source), &key)?;
            vm.set(&JsValue::Object(target), key, value, true)?;
        }
    }
    Ok(JsValue::Object(target))
}

/** Object.create. */
fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let proto = match call.arg(0) {
        JsValue::Object(id) => Some(id),
        JsValue::Null => None,
        _ => return Err(vm.type_error("Object prototype may only be an Object or null")),
    };
    let id = vm.alloc(Object::new(Class::Plain, proto))?;
    let properties = call.arg(1);
    if !properties.nullish() {
        apply_properties(vm, id, &properties)?;
    }
    Ok(JsValue::Object(id))
}

/** Read a property descriptor object. */
pub(in crate::v9) fn to_descriptor(vm: &mut Vm, value: &JsValue) -> Res<Descriptor> {
    let JsValue::Object(id) = value else {
        return Err(vm.type_error("Property description must be an object"));
    };
    let mut desc = Descriptor::default();
    let target = JsValue::Object(*id);
    let flag = |vm: &mut Vm, name: &str| -> Res<Option<bool>> {
        if vm.has_property(*id, &key(name))? {
            Ok(Some(vm.get(&target, &key(name))?.truthy()))
        } else {
            Ok(None)
        }
    };
    desc.enumerable = flag(vm, "enumerable")?;
    desc.configurable = flag(vm, "configurable")?;
    desc.writable = flag(vm, "writable")?;
    if vm.has_property(*id, &key("value"))? {
        desc.value = Some(vm.get(&target, &key("value"))?);
    }
    for (name, slot) in [("get", 0), ("set", 1)] {
        if vm.has_property(*id, &key(name))? {
            let function = vm.get(&target, &key(name))?;
            let function = match function {
                JsValue::Undefined => None,
                JsValue::Object(f) if vm.is_callable(&function) => Some(f),
                _ => return Err(vm.type_error(&format!("Getter must be a function: {name}"))),
            };
            if slot == 0 {
                desc.get = Some(function);
            } else {
                desc.set = Some(function);
            }
        }
    }
    if desc.accessor() && (desc.value.is_some() || desc.writable.is_some()) {
        return Err(vm.type_error("Invalid property descriptor. Cannot both specify accessors and a value or writable attribute"));
    }
    Ok(desc)
}

/** Fill defaults for a newly defined property. */
fn with_defaults(mut desc: Descriptor, existing: bool) -> Descriptor {
    if !existing {
        desc.enumerable.get_or_insert(false);
        desc.configurable.get_or_insert(false);
        if !desc.accessor() {
            desc.writable.get_or_insert(false);
            desc.value.get_or_insert(JsValue::Undefined);
        }
    }
    desc
}

/** Object.defineProperty. */
fn define_property(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Object(id) = call.arg(0) else {
        return Err(vm.type_error("Object.defineProperty called on non-object"));
    };
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    let desc = to_descriptor(vm, &call.arg(2))?;
    let existing = vm.own(id, &property)?.is_some();
    if !vm.define(id, property.clone(), with_defaults(desc, existing))? {
        return Err(vm.type_error(&format!(
            "Cannot redefine property: {}",
            crate::v9::vm::property::key_text(&property)
        )));
    }
    Ok(JsValue::Object(id))
}

/** Apply a property-descriptor map. */
fn apply_properties(vm: &mut Vm, id: ObjectId, properties: &JsValue) -> Res<()> {
    let source = vm.to_object(properties)?;
    for name in vm.enumerable_keys(source)? {
        let desc_value = vm.get(&JsValue::Object(source), &name)?;
        let desc = to_descriptor(vm, &desc_value)?;
        let existing = vm.own(id, &name)?.is_some();
        vm.define(id, name, with_defaults(desc, existing))?;
    }
    Ok(())
}

/** Object.defineProperties. */
fn define_properties(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Object(id) = call.arg(0) else {
        return Err(vm.type_error("Object.defineProperties called on non-object"));
    };
    apply_properties(vm, id, &call.arg(1))?;
    Ok(JsValue::Object(id))
}

/** Object.getOwnPropertyNames. */
fn own_names(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let list: Vec<JsValue> = vm
        .own_keys(id)?
        .iter()
        .filter(|key| matches!(key, Key::Str(_)))
        .map(Key::value)
        .collect();
    Ok(JsValue::Object(vm.new_array(list)?))
}

/** Object.getOwnPropertySymbols. */
fn own_symbols(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let list: Vec<JsValue> = vm
        .own_keys(id)?
        .iter()
        .filter(|key| matches!(key, Key::Sym(_)))
        .map(Key::value)
        .collect();
    Ok(JsValue::Object(vm.new_array(list)?))
}

/** A descriptor object for an own property. */
pub(in crate::v9) fn from_descriptor(vm: &mut Vm, id: ObjectId, property: &Key) -> Res<JsValue> {
    let Some(prop) = vm.own(id, property)? else {
        return Ok(JsValue::Undefined);
    };
    let result = vm.new_object()?;
    match prop.slot {
        Slot::Data(value) => {
            vm.put(result, "value", value)?;
            vm.put(
                result,
                "writable",
                JsValue::Boolean(prop.attrs & WRITABLE != 0),
            )?;
        }
        Slot::Accessor(get, set) => {
            vm.put(
                result,
                "get",
                get.map_or(JsValue::Undefined, JsValue::Object),
            )?;
            vm.put(
                result,
                "set",
                set.map_or(JsValue::Undefined, JsValue::Object),
            )?;
        }
    }
    vm.put(
        result,
        "enumerable",
        JsValue::Boolean(prop.attrs & ENUMERABLE != 0),
    )?;
    vm.put(
        result,
        "configurable",
        JsValue::Boolean(prop.attrs & CONFIGURABLE != 0),
    )?;
    Ok(JsValue::Object(result))
}

/** Object.getOwnPropertyDescriptor. */
fn own_descriptor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    from_descriptor(vm, id, &property)
}

/** Object.getOwnPropertyDescriptors. */
fn own_descriptors(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let result = vm.new_object()?;
    for property in vm.own_keys(id)? {
        let desc = from_descriptor(vm, id, &property)?;
        vm.put_key(result, property, desc)?;
    }
    Ok(JsValue::Object(result))
}

/** Object.getPrototypeOf. */
fn get_prototype(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    Ok(vm.proto_of(id).map_or(JsValue::Null, JsValue::Object))
}

/** Object.setPrototypeOf. */
fn set_prototype(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call.arg(0);
    let proto = match call.arg(1) {
        JsValue::Object(id) => Some(id),
        JsValue::Null => None,
        _ => return Err(vm.type_error("Object prototype may only be an Object or null")),
    };
    if let JsValue::Object(id) = target
        && !vm.set_proto(id, proto)?
    {
        return Err(vm.type_error("Cyclic __proto__ value"));
    }
    Ok(target)
}

/** Clear attribute bits on every own property and stop extension. */
fn restrict(vm: &mut Vm, id: ObjectId, clear: u8) -> Res<()> {
    vm.materialize(id)?;
    for property in vm.own_keys(id)? {
        if let Some(mut prop) = vm.own(id, &property)? {
            let mut mask = clear;
            if matches!(prop.slot, Slot::Accessor(..)) {
                mask &= !WRITABLE;
            }
            prop.attrs &= !mask;
            if !matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Array(_))) {
                vm.heap.put(id, property, prop);
            }
        }
    }
    if let Some(object) = vm.heap.get_mut(id) {
        object.extensible = false;
    }
    Ok(())
}

/** Object.freeze. */
fn freeze(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    if let JsValue::Object(id) = value {
        restrict(vm, id, WRITABLE | CONFIGURABLE)?;
    }
    Ok(value)
}

/** Object.seal. */
fn seal(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    if let JsValue::Object(id) = value {
        restrict(vm, id, CONFIGURABLE)?;
    }
    Ok(value)
}

/** Object.preventExtensions. */
fn prevent_extensions(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    if let JsValue::Object(id) = value {
        vm.materialize(id)?;
        if let Some(object) = vm.heap.get_mut(id) {
            object.extensible = false;
        }
    }
    Ok(value)
}

/** Whether every own property lacks the given bits and the object is not extensible. */
fn test_restricted(vm: &mut Vm, value: &JsValue, bits: u8) -> Res<bool> {
    let JsValue::Object(id) = value else {
        return Ok(true);
    };
    if vm.heap.get(*id).is_none_or(|object| object.extensible) {
        return Ok(false);
    }
    for property in vm.own_keys(*id)? {
        if let Some(prop) = vm.own(*id, &property)? {
            let bits = if matches!(prop.slot, Slot::Accessor(..)) {
                bits & !WRITABLE
            } else {
                bits
            };
            if prop.attrs & bits != 0 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/** Object.isFrozen. */
fn is_frozen(vm: &mut Vm, call: Call) -> Res<JsValue> {
    test_restricted(vm, &call.arg(0), WRITABLE | CONFIGURABLE).map(JsValue::Boolean)
}

/** Object.isSealed. */
fn is_sealed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    test_restricted(vm, &call.arg(0), CONFIGURABLE).map(JsValue::Boolean)
}

/** Object.isExtensible. */
fn is_extensible(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(match call.arg(0) {
        JsValue::Object(id) => vm.heap.get(id).is_none_or(|object| object.extensible),
        _ => false,
    }))
}

/** Object.is. */
fn is(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(call.arg(0).same_value(&call.arg(1))))
}

/** Object.fromEntries. */
fn from_entries(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = vm.new_object()?;
    for entry in vm.iterate_to_vec(&call.arg(0))? {
        let name = vm.get(&entry, &key("0"))?;
        let value = vm.get(&entry, &key("1"))?;
        let property = vm.to_key(&name)?;
        vm.define(result, property, Descriptor::data(value))?;
    }
    Ok(JsValue::Object(result))
}

/** Object.hasOwn. */
fn has_own(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = object_arg(vm, &call.arg(0))?;
    let name = call.arg(1);
    let property = vm.to_key(&name)?;
    Ok(JsValue::Boolean(vm.own(id, &property)?.is_some()))
}

/** Object.groupBy. */
fn group_by(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let callback = call.arg(1);
    let result = vm.alloc(Object::new(Class::Plain, None))?;
    for (index, item) in vm.iterate_to_vec(&call.arg(0))?.into_iter().enumerate() {
        let group = vm.call(
            callback.clone(),
            JsValue::Undefined,
            vec![item.clone(), JsValue::Number(index as f64)],
        )?;
        let property = vm.to_key(&group)?;
        let list = match vm.own(result, &property)? {
            Some(prop) => match prop.slot {
                Slot::Data(list) => list,
                Slot::Accessor(..) => JsValue::Undefined,
            },
            None => {
                let list = JsValue::Object(vm.new_array(Vec::new())?);
                vm.define(result, property, Descriptor::data(list.clone()))?;
                list
            }
        };
        vm.array_append(&list, item)?;
    }
    Ok(JsValue::Object(result))
}

/** Object.prototype.hasOwnProperty. */
fn has_own_property(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = call.arg(0);
    let property = vm.to_key(&name)?;
    let id = this_object(vm, &call, "Object.prototype.hasOwnProperty")?;
    Ok(JsValue::Boolean(vm.own(id, &property)?.is_some()))
}

/** Object.prototype.isPrototypeOf. */
fn is_prototype_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Object(mut current) = call.arg(0) else {
        return Ok(JsValue::Boolean(false));
    };
    let id = this_object(vm, &call, "Object.prototype.isPrototypeOf")?;
    for _ in 0..crate::v9::vm::property::CHAIN_STEPS {
        match vm.proto_of(current) {
            Some(proto) if proto == id => return Ok(JsValue::Boolean(true)),
            Some(proto) => current = proto,
            None => break,
        }
    }
    Ok(JsValue::Boolean(false))
}

/** Object.prototype.propertyIsEnumerable. */
fn property_is_enumerable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = call.arg(0);
    let property = vm.to_key(&name)?;
    let id = this_object(vm, &call, "Object.prototype.propertyIsEnumerable")?;
    Ok(JsValue::Boolean(
        vm.own(id, &property)?
            .is_some_and(|prop| prop.attrs & ENUMERABLE != 0),
    ))
}

/** The builtin tag Object.prototype.toString reports. */
pub(in crate::v9) fn builtin_tag(vm: &mut Vm, value: &JsValue) -> Res<String> {
    let id = match value {
        JsValue::Undefined | JsValue::Hole => return Ok("Undefined".into()),
        JsValue::Null => return Ok("Null".into()),
        JsValue::Object(id) => *id,
        other => vm.to_object(other)?,
    };
    let tag = vm.get(&JsValue::Object(id), &Key::Sym(symbol::TO_STRING_TAG))?;
    if let JsValue::String(tag) = tag {
        return Ok(tag.lossy());
    }
    if vm.is_callable(&JsValue::Object(id)) {
        return Ok("Function".into());
    }
    if let Some(tag) = crate::v9::platform::interface_tag(vm, id) {
        return Ok(tag);
    }
    Ok(match vm.heap.get(id).map(|o| &o.class) {
        Some(Class::Array(_)) => "Array",
        Some(Class::Error) => "Error",
        Some(Class::Boolean(_)) => "Boolean",
        Some(Class::Number(_)) => "Number",
        Some(Class::String(_)) => "String",
        Some(Class::Date(_)) => "Date",
        Some(Class::RegExp(_)) => "RegExp",
        Some(Class::Arguments) => "Arguments",
        _ => "Object",
    }
    .into())
}

/** Object.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let tag = builtin_tag(vm, &call.this)?;
    text(vm, &format!("[object {tag}]"))
}

/** Object.prototype.toLocaleString calls toString. */
fn to_locale_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let method = vm.get(&call.this, &key("toString"))?;
    vm.call(method, call.this.clone(), Vec::new())
}

/** Object.prototype.valueOf. */
fn value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Object(this_object(
        vm,
        &call,
        "Object.prototype.valueOf",
    )?))
}

/** __defineGetter__. */
fn define_getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_object(vm, &call, "__defineGetter__")?;
    let name = call.arg(0);
    let property = vm.to_key(&name)?;
    let JsValue::Object(getter) = call.arg(1) else {
        return Err(vm.type_error("Object.prototype.__defineGetter__: Expecting function"));
    };
    vm.define(
        id,
        property,
        Descriptor {
            get: Some(Some(getter)),
            enumerable: Some(true),
            configurable: Some(true),
            ..Descriptor::default()
        },
    )?;
    Ok(JsValue::Undefined)
}

/** __defineSetter__. */
fn define_setter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_object(vm, &call, "__defineSetter__")?;
    let name = call.arg(0);
    let property = vm.to_key(&name)?;
    let JsValue::Object(setter) = call.arg(1) else {
        return Err(vm.type_error("Object.prototype.__defineSetter__: Expecting function"));
    };
    vm.define(
        id,
        property,
        Descriptor {
            set: Some(Some(setter)),
            enumerable: Some(true),
            configurable: Some(true),
            ..Descriptor::default()
        },
    )?;
    Ok(JsValue::Undefined)
}

/** __lookupGetter__. */
fn lookup_getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut id = this_object(vm, &call, "__lookupGetter__")?;
    let name = call.arg(0);
    let property = vm.to_key(&name)?;
    for _ in 0..crate::v9::vm::property::CHAIN_STEPS {
        if let Some(prop) = vm.own(id, &property)? {
            return Ok(match prop.slot {
                Slot::Accessor(Some(getter), _) => JsValue::Object(getter),
                _ => JsValue::Undefined,
            });
        }
        match vm.proto_of(id) {
            Some(proto) => id = proto,
            None => break,
        }
    }
    Ok(JsValue::Undefined)
}

/** The __proto__ getter. */
fn proto_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_object(vm, &call, "__proto__")?;
    Ok(vm.proto_of(id).map_or(JsValue::Null, JsValue::Object))
}

/** The __proto__ setter. */
fn proto_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let JsValue::Object(id) = call.this {
        let proto = match call.arg(0) {
            JsValue::Object(proto) => Some(proto),
            JsValue::Null => None,
            _ => return Ok(JsValue::Undefined),
        };
        if !vm.set_proto(id, proto)? {
            return Err(vm.type_error("Cyclic __proto__ value"));
        }
    }
    let _ = realm();
    Ok(JsValue::Undefined)
}
