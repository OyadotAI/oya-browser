/** SharedArray and SharedStructType: V8's harmony shared containers under --harmony-struct. A shared
container holds only shareable values (primitives and other shared containers), so shared storage
never points at an isolate-local object. Conformance host only. */
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{ENUMERABLE, Key, Object, Prop, Slot, WRITABLE};
use crate::v9::vm::realm::{Builder, banks};
use crate::v9::vm::{Call, Res, Vm};
use std::collections::BTreeSet;
use std::sync::OnceLock;

/** Private mark on a SharedArray: its elements are checked for shareability on every store. */
const ARRAY_MARK: &str = "shared-array";
/** Private mark on a SharedStructType instance: its layout is fixed to the declared fields. */
const STRUCT_MARK: &str = "shared-struct";
/** Private slot of a struct constructor: its prototype object. */
const PROTOTYPE_SLOT: usize = 0;
/** Private slot of a struct constructor: the array of declared field names. */
const FIELDS_SLOT: usize = 1;
/** Message when a non-shareable value is stored; V8 prints the value, these are the two shapes. */
const OBJECT_NOT_SHARED: &str = "#<Object> cannot be shared";
/** Message when a symbol is stored; symbols are not shareable here. */
const SYMBOL_NOT_SHARED: &str = "Symbol() cannot be shared";
/** Message when a shared struct gains a property outside its layout, from a store. */
const FIXED_LAYOUT_ADD: &str = "Cannot add property {}, object has fixed layout";
/** Message when a shared struct gains a property outside its layout, from a definition. */
const FIXED_LAYOUT_DEFINE: &str = "Cannot define property {}, object has fixed layout";
/** Message when a SharedArray length is negative, fractional past the limit, or too large. */
const SIZE_OUT_OF_RANGE: &str = "SharedArray length out of range";
/** Message when SharedStructType is called without new. */
const REQUIRES_NEW: &str = "Constructor SharedStructType requires 'new'";
/** Message when a field name is not a string. */
const FIELD_NAME_NOT_STRING: &str = "SharedStructType field names must be strings";
/** Message when a field name repeats. */
const DUPLICATE_FIELD: &str = "Duplicate SharedStructType field name";
/** The constructor's declared `length` for both globals. */
const CONSTRUCTOR_LENGTH: u32 = 1;

/** Intrinsic identities of the two globals and the struct instance native, per bank. */
struct Natives {
    array: ObjectId,
    struct_type: ObjectId,
    instance: u32,
}
static NATIVES: OnceLock<Natives> = OnceLock::new();

/** Which shared container an object is, read from its private mark. */
#[derive(PartialEq)]
enum Kind {
    Array,
    Struct,
}

/** Creates the two constructors and the struct-instance native. */
pub(super) fn install(builder: &mut Builder) {
    let array_prototype = builder.object(Some(builder.ids.object_prototype));
    let array = builder.constructor(
        "SharedArray",
        CONSTRUCTOR_LENGTH,
        array_new,
        array_prototype,
        None,
    );
    let struct_prototype = builder.object(Some(builder.ids.object_prototype));
    let struct_type = builder.constructor(
        "SharedStructType",
        CONSTRUCTOR_LENGTH,
        struct_type_new,
        struct_prototype,
        None,
    );
    let instance = builder.native("", CONSTRUCTOR_LENGTH, struct_instance);
    builder.natives[instance as usize].constructor = true;
    let _ = NATIVES.set(Natives {
        array,
        struct_type,
        instance,
    });
}

/** Exposes both globals on a fresh realm when the actual --harmony-struct flag asked for them. */
pub(in crate::v9) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if !vm.page.environment.conformance_harmony_struct {
        return;
    }
    let natives = NATIVES.get().expect("installed shared intrinsics");
    for (name, id) in [
        ("SharedArray", natives.array),
        ("SharedStructType", natives.struct_type),
    ] {
        let id = id + bank * banks::BANK_STRIDE;
        vm.materialize(id).expect("installed shared intrinsic");
        vm.heap
            .put(global, Key::from(name), Prop::data(JsValue::Object(id)));
    }
}

/** SharedArray(length) works with or without new, as V8's constructor does; elements start undefined. */
fn array_new(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let length = array_length(vm, &call.arg(0))?;
    let prototype = vm.get(&JsValue::Object(call.callee), &Key::from("prototype"))?;
    let id = vm.new_array(vec![JsValue::Undefined; length])?;
    if let JsValue::Object(proto) = prototype {
        vm.set_proto(id, Some(proto))?;
    }
    mark(vm, id, ARRAY_MARK)?;
    // Shared arrays never grow: a store past the end is refused like any non-extensible array.
    crate::v9::builtins::integrity::prevent_extensions(vm, id)?;
    Ok(JsValue::Object(id))
}

/** A length is a non-negative integer no larger than the engine's array budget. */
fn array_length(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    let number = vm.to_number(value)?;
    let integer = if number.is_nan() { 0.0 } else { number.trunc() };
    if integer < 0.0 || integer > vm.limits.array_length as f64 {
        return Err(vm.range_error(SIZE_OUT_OF_RANGE));
    }
    Ok(integer as usize)
}

/** `new SharedStructType(fieldNames)`: a constructor whose instances carry exactly these fields. */
fn struct_type_new(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error(REQUIRES_NEW));
    }
    let names = field_names(vm, &call.arg(0))?;
    let prototype = vm.new_object()?;
    let fields = vm.new_array(names.into_iter().map(JsValue::String).collect())?;
    let slots = vec![JsValue::Object(prototype), JsValue::Object(fields)];
    let natives = NATIVES.get().expect("installed shared intrinsics");
    let ctor = vm.native_closure(natives.instance, slots)?;
    vm.heap.put(
        ctor,
        Key::from("prototype"),
        Prop {
            slot: Slot::Data(JsValue::Object(prototype)),
            attrs: 0,
        },
    );
    vm.heap.put(
        prototype,
        Key::from("constructor"),
        Prop::data(JsValue::Object(ctor)),
    );
    Ok(JsValue::Object(ctor))
}

/** The declared field names: an array of distinct strings. */
fn field_names(vm: &mut Vm, value: &JsValue) -> Res<Vec<JsString>> {
    let JsValue::Object(id) = value else {
        return Err(vm.type_error(FIELD_NAME_NOT_STRING));
    };
    let values: Vec<JsValue> = match vm.heap.get(*id).map(|object| &object.class) {
        Some(Class::Array(elements)) => (0..elements.len())
            .map(|index| elements.get(index).cloned().unwrap_or(JsValue::Undefined))
            .collect(),
        _ => return Err(vm.type_error(FIELD_NAME_NOT_STRING)),
    };
    let mut seen = BTreeSet::new();
    let mut names = Vec::with_capacity(values.len());
    for value in values {
        let JsValue::String(name) = value else {
            return Err(vm.type_error(FIELD_NAME_NOT_STRING));
        };
        if !seen.insert(name.lossy()) {
            return Err(vm.type_error(DUPLICATE_FIELD));
        }
        names.push(name);
    }
    Ok(names)
}

/** One instance of a struct type: a plain object whose declared fields exist from the start. */
fn struct_instance(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error(REQUIRES_NEW));
    };
    let slots = vm.slots(call.callee);
    let (Some(JsValue::Object(fallback)), Some(JsValue::Object(fields))) =
        (slots.get(PROTOTYPE_SLOT), slots.get(FIELDS_SLOT))
    else {
        return Err(vm.type_error(REQUIRES_NEW));
    };
    let proto = vm.prototype_for(&target, *fallback)?;
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    mark(vm, id, STRUCT_MARK)?;
    let names = declared_names(vm, *fields);
    for name in names {
        let attrs = WRITABLE | ENUMERABLE;
        vm.heap.put(
            id,
            Key::Str(name),
            Prop {
                slot: Slot::Data(JsValue::Undefined),
                attrs,
            },
        );
    }
    Ok(JsValue::Object(id))
}

/** The field names stored on a constructor, read back from its fields array. */
fn declared_names(vm: &Vm, fields: ObjectId) -> Vec<JsString> {
    let Some(Class::Array(elements)) = vm.heap.get(fields).map(|object| &object.class) else {
        return Vec::new();
    };
    (0..elements.len())
        .filter_map(|index| match elements.get(index) {
            Some(JsValue::String(name)) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/** Writes a private mark on a new shared container. */
fn mark(vm: &mut Vm, id: ObjectId, name: &str) -> Res<()> {
    vm.heap
        .put(id, Key::private(name), Prop::data(JsValue::Boolean(true)));
    Ok(())
}

/** Which container this is, from its private mark; ordinary objects have none. */
fn kind(vm: &Vm, id: ObjectId) -> Option<Kind> {
    let object = vm.heap.get(id)?;
    if object.props.get(&Key::private(ARRAY_MARK)).is_some() {
        Some(Kind::Array)
    } else if object.props.get(&Key::private(STRUCT_MARK)).is_some() {
        Some(Kind::Struct)
    } else {
        None
    }
}

/** A value may enter shared storage only if it is a primitive or already a shared container. */
fn check_shareable(vm: &mut Vm, value: &JsValue) -> Res<()> {
    match value {
        JsValue::Object(id) if kind(vm, *id).is_none() => Err(vm.type_error(OBJECT_NOT_SHARED)),
        JsValue::Symbol(_) => Err(vm.type_error(SYMBOL_NOT_SHARED)),
        _ => Ok(()),
    }
}

/** Store hook for ordinary writes: a shared array checks each element; a shared struct takes only its declared fields. */
pub(in crate::v9) fn check_store(vm: &mut Vm, id: ObjectId, key: &Key, value: &JsValue) -> Res<()> {
    match kind(vm, id) {
        Some(Kind::Array) if key.index().is_some() => check_shareable(vm, value),
        Some(Kind::Struct) => check_struct_key(vm, id, key, value),
        _ => Ok(()),
    }
}

/** Definition hook: a shared struct cannot grow a property, and a defined value must be shareable. */
pub(in crate::v9) fn check_define(
    vm: &mut Vm,
    id: ObjectId,
    key: &Key,
    value: Option<&JsValue>,
) -> Res<()> {
    match kind(vm, id) {
        Some(Kind::Struct) if !declares(vm, id, key) => Err(vm.type_error(
            &FIXED_LAYOUT_DEFINE.replace("{}", &crate::v9::vm::property::key_text(key)),
        )),
        Some(_) => value.map_or(Ok(()), |value| check_shareable(vm, value)),
        None => Ok(()),
    }
}

/** A struct store to an undeclared field fails; a declared field takes a shareable value. */
fn check_struct_key(vm: &mut Vm, id: ObjectId, key: &Key, value: &JsValue) -> Res<()> {
    if !declares(vm, id, key) {
        return Err(
            vm.type_error(&FIXED_LAYOUT_ADD.replace("{}", &crate::v9::vm::property::key_text(key)))
        );
    }
    check_shareable(vm, value)
}

/** A declared field exists as an own property; nothing else may be added to a shared struct. */
fn declares(vm: &Vm, id: ObjectId, key: &Key) -> bool {
    vm.heap
        .get(id)
        .is_some_and(|object| object.props.get(key).is_some())
}
