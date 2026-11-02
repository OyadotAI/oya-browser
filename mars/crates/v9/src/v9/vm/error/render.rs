/** How V8 writes a value inside an error message, without running page code: `#<Foo>` for an
instance whose class kept Object.prototype.toString, `[object Array]` for a builtin that has its
own, a function as its source, `Symbol(x)`, numbers and strings as they are. Pages that fingerprint
the engine compare these texts, so every site that names a value goes through here. */
use super::super::data::{Class, ElementKind};
use super::super::heap::{Key, Slot};
use super::super::realm::symbol;
use super::super::{Call, Vm};
use crate::v9::value::{JsValue, ObjectId};

/** UTF-16 units of a function source or symbol description V8 prints before abbreviating. */
const PRINTED_UNITS: usize = 128;
/** The head of an abbreviated function source. */
const SOURCE_HEAD: usize = 111;
/** The tail of an abbreviated function source. */
const SOURCE_TAIL: usize = 2;
/** The head and tail kept of an abbreviated symbol description. */
const SYMBOL_EDGE: usize = 56;
/** Marks the cut in an abbreviated text. */
const OMITTED: &str = "...<omitted>...";
/** Prototype depth searched for the inert properties the rendering reads. */
const PEEK_DEPTH: usize = 8;

impl Vm<'_> {
    /** A value as V8's messages print it (its NoSideEffectsToString). */
    pub(in crate::v9) fn v8_value_text(&mut self, value: &JsValue) -> String {
        v8_value_text(self, value)
    }
    /** A value with its type in front, as V8 prints a callee or iterable it has no source text
    for: `number 1`, `string "s"`, `object null`, `undefined`, and a bare type for objects. */
    pub(in crate::v9) fn v8_callsite_text(&mut self, value: &JsValue) -> String {
        v8_callsite_text(self, value)
    }
}

/** A value as V8's messages print it (its NoSideEffectsToString). */
pub(in crate::v9) fn v8_value_text(vm: &mut Vm, value: &JsValue) -> String {
    match value {
        JsValue::Object(id) => object_text(vm, *id),
        JsValue::Symbol(id) => symbol_text(vm, *id),
        other => primitive_text(other),
    }
}

/** A value with its type in front, as V8 prints a callee or iterable it has no source text for. */
pub(in crate::v9) fn v8_callsite_text(vm: &mut Vm, value: &JsValue) -> String {
    let kind = vm.type_of(value);
    match value {
        JsValue::String(text) => format!("{kind} \"{}\"", text.lossy()),
        JsValue::Null | JsValue::Boolean(_) => format!("{kind} {}", primitive_text(value)),
        JsValue::Number(_) => format!("{kind} {}", primitive_text(value)),
        #[cfg(feature = "conformance")]
        JsValue::Smi(_) | JsValue::SharedNumber(_) | JsValue::LocalNumber(_) => {
            format!("{kind} {}", primitive_text(value))
        }
        _ => kind.to_string(),
    }
}

/** Primitives other than symbols print as ToString would, strings unquoted. */
fn primitive_text(value: &JsValue) -> String {
    match value {
        JsValue::Undefined | JsValue::Hole => "undefined".into(),
        JsValue::Null => "null".into(),
        JsValue::Boolean(value) => value.to_string(),
        JsValue::String(text) => text.lossy(),
        #[cfg(feature = "conformance")]
        JsValue::Smi(value) => crate::v9::value::number_to_string(*value),
        JsValue::Number(value) => crate::v9::value::number_to_string(*value),
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(value) | JsValue::LocalNumber(value) => {
            crate::v9::value::number_to_string(value.value)
        }
        JsValue::BigInt(value) => value.to_string(),
        JsValue::BigIntWide(value) => value.diagnostic(),
        JsValue::Symbol(_) | JsValue::Object(_) => String::new(),
    }
}

/** `Symbol(description)`, a long description abbreviated in the middle. */
fn symbol_text(vm: &Vm, id: u32) -> String {
    let description: Vec<u16> = crate::v9::builtins::symbol_text(vm, id)
        .encode_utf16()
        .collect();
    let shown = if description.len() > PRINTED_UNITS {
        let head = String::from_utf16_lossy(&description[..SYMBOL_EDGE]);
        let tail = String::from_utf16_lossy(&description[description.len() - SYMBOL_EDGE..]);
        format!("{head}{OMITTED}{tail}")
    } else {
        String::from_utf16_lossy(&description)
    };
    format!("Symbol({shown})")
}

/** An object prints through its target when it is a proxy, as its source when callable, as
`Name: message` when it is an error, as `#<Ctor>` when it kept Object.prototype.toString, and as
`[object Tag]` otherwise. */
fn object_text(vm: &mut Vm, id: ObjectId) -> String {
    let id = match unproxied(vm, id) {
        Some(id) => id,
        None => return "null".into(),
    };
    if vm.is_callable(&JsValue::Object(id)) {
        return function_text(vm, id);
    }
    let to_string = data_property(vm, id, &Key::from("toString")).and_then(|value| value.object());
    let intrinsic = |vm: &mut Vm, proto: ObjectId| {
        data_property(vm, proto, &Key::from("toString")).and_then(|value| value.object())
    };
    let (object_prototype, error_prototype) = {
        let ids = &super::super::realm::realm().ids;
        (ids.object_prototype, ids.error_prototype)
    };
    let is_error = vm
        .heap
        .get(id)
        .is_some_and(|object| matches!(object.class, Class::Error));
    if is_error || (to_string.is_some() && to_string == intrinsic(vm, error_prototype)) {
        return error_text(vm, id);
    }
    if to_string.is_some()
        && to_string == intrinsic(vm, object_prototype)
        && let Some(name) = constructor_text(vm, id)
    {
        return format!("#<{name}>");
    }
    format!("[object {}]", builtin_tag(vm, id))
}

/** A proxy prints as its target; a revoked one as `null`, which V8 reads from the cleared target. */
fn unproxied(vm: &Vm, mut id: ObjectId) -> Option<ObjectId> {
    for _ in 0..PEEK_DEPTH {
        match vm.heap.get(id).map(|object| &object.class) {
            Some(Class::Proxy(proxy)) => id = proxy.target?,
            _ => return Some(id),
        }
    }
    Some(id)
}

/** The function's source as Function.prototype.toString gives it, abbreviated past 128 units. */
fn function_text(vm: &mut Vm, id: ObjectId) -> String {
    let call = Call {
        this: JsValue::Object(id),
        args: &[],
        new_target: None,
        callee: id,
    };
    let source = match crate::v9::builtins::function_to_string(vm, call) {
        Ok(JsValue::String(text)) => text.lossy(),
        _ => return "function () { [native code] }".into(),
    };
    let units: Vec<u16> = source.encode_utf16().collect();
    if units.len() <= PRINTED_UNITS {
        return source;
    }
    let head = String::from_utf16_lossy(&units[..SOURCE_HEAD]);
    let tail = String::from_utf16_lossy(&units[units.len() - SOURCE_TAIL..]);
    format!("{head}{OMITTED}{tail}")
}

/** `Name: message` from the error's data properties, either alone when the other is empty. */
fn error_text(vm: &mut Vm, id: ObjectId) -> String {
    let text = |vm: &mut Vm, name: &str| match data_property(vm, id, &Key::from(name)) {
        Some(JsValue::String(text)) => text.lossy(),
        _ => String::new(),
    };
    let name = text(vm, "name");
    let message = text(vm, "message");
    match (name.is_empty(), message.is_empty()) {
        (true, _) => message,
        (_, true) => name,
        _ => format!("{name}: {message}"),
    }
}

/** The non-empty name of the object's `constructor` data property when that is a function. */
fn constructor_text(vm: &mut Vm, id: ObjectId) -> Option<String> {
    let constructor = data_property(vm, id, &Key::constructor())?;
    let JsValue::Object(ctor) = constructor else {
        return None;
    };
    if !vm.is_callable(&constructor) {
        return None;
    }
    match data_property(vm, ctor, &Key::from("name")) {
        Some(JsValue::String(name)) if !name.is_empty() => Some(name.lossy()),
        _ => None,
    }
}

/** The tag of `[object Tag]`: a Symbol.toStringTag data property, else the builtin kind. */
fn builtin_tag(vm: &mut Vm, id: ObjectId) -> String {
    if let Some(JsValue::String(tag)) = data_property(vm, id, &Key::Sym(symbol::TO_STRING_TAG)) {
        return tag.lossy();
    }
    if let Some(tag) = crate::v9::platform::interface_tag(vm, id) {
        return tag;
    }
    class_tag(vm, id)
}

/** What V8 calls the object's class: its builtin kind, `Object` for anything a script made. */
fn class_tag(vm: &Vm, id: ObjectId) -> String {
    let ids = &super::super::realm::realm().ids;
    if let Some(tag) = [
        (ids.number_prototype, "Number"),
        (ids.boolean_prototype, "Boolean"),
        (ids.string_prototype, "String"),
    ]
    .iter()
    .find_map(|(prototype, tag)| (*prototype == id).then_some(*tag))
    {
        return tag.into();
    }
    let Some(object) = vm.heap.get(id) else {
        return "Object".into();
    };
    match &object.class {
        Class::Array(_) => "Array",
        Class::Error => "Error",
        Class::Boolean(_) => "Boolean",
        Class::Number(_) => "Number",
        Class::String(_) => "String",
        Class::Symbol(_) => "Symbol",
        Class::BigInt(_) | Class::BigIntWide(_) => "BigInt",
        Class::Date(_) => "Date",
        Class::RegExp(_) => "RegExp",
        Class::Map(_) => "Map",
        Class::Set(_) => "Set",
        Class::WeakMap(_) => "WeakMap",
        Class::WeakSet(_) => "WeakSet",
        Class::WeakRef(_) => "WeakRef",
        Class::FinalizationRegistry(_) => "FinalizationRegistry",
        Class::Promise(_) => "Promise",
        Class::Arguments | Class::MappedArguments(_) => "Arguments",
        Class::Generator(_) => "Generator",
        Class::AsyncGenerator(_) => "AsyncGenerator",
        Class::ArrayBuffer(_) | Class::SparseWasmBuffer(_) => "ArrayBuffer",
        Class::SharedArrayBuffer(_) => "SharedArrayBuffer",
        Class::View(view) => return view_tag(view.kind),
        Class::Namespace(_) => "Module",
        _ => "Object",
    }
    .into()
}

/** A typed array's constructor name, or `DataView`. */
fn view_tag(kind: ElementKind) -> String {
    match kind {
        ElementKind::DataView => "DataView".into(),
        other => format!("{other:?}Array"),
    }
}

/** A data property along the prototype chain, never a getter and never a proxy trap. */
fn data_property(vm: &mut Vm, mut id: ObjectId, key: &Key) -> Option<JsValue> {
    for _ in 0..PEEK_DEPTH {
        if let Ok(Some(prop)) = vm.own(id, key) {
            return match prop.slot {
                Slot::Data(value) => Some(value),
                Slot::Accessor(..) => None,
            };
        }
        id = vm.proto_of(id)?;
    }
    None
}
