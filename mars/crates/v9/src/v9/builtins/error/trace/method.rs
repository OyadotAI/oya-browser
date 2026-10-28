/** V8's method-call naming: a receiver's type plus the property that holds the callee, spelled "Type.name [as alias]". */
use super::*;
use crate::v9::vm::data::FunctionKind;
use crate::v9::vm::heap::{Key, Slot};
use crate::v9::vm::realm::{Static, realm_for, symbol};

/** Prototype walks stop here; real chains are far shorter, so this only guards a corrupt cycle. */
const CHAIN_LIMIT: usize = 1024;
/** The type name when neither a constructor nor a toStringTag names the receiver. */
const OBJECT_TYPE: &str = "Object";
/** The name shown for a method-call callee that has neither a name nor a property key. */
pub(super) const ANONYMOUS: &str = "<anonymous>";
/** The creation-default name of a function built by the Function constructor. */
const DYNAMIC_FUNCTION_NAME: &str = "anonymous";
/** Marks functions created from source text by the Function constructor. */
const DYNAMIC_SOURCE_KEY: &str = "dynamic-function-source";

/** One prototype step reads heap objects and static intrinsics through the same lookup. */
fn proto_of(vm: &Vm, id: ObjectId) -> Option<ObjectId> {
    match vm.heap.get(id) {
        Some(object) => object.proto,
        None => realm_for(id).object(id).and_then(|def| def.proto),
    }
}

/** The receiver and its prototypes, nearest first. */
fn chain(vm: &Vm<'_>, start: ObjectId) -> Vec<ObjectId> {
    std::iter::successors(Some(start), |id| proto_of(vm, *id))
        .take(CHAIN_LIMIT)
        .collect()
}

/** Own data value of one key; accessors stay unrun so naming never executes user code. */
fn own_data(vm: &Vm, id: ObjectId, key: &Key) -> Option<JsValue> {
    match vm.heap.get(id) {
        Some(object) => match &object.props.get(key)?.slot {
            Slot::Data(value) => Some(value.clone()),
            _ => None,
        },
        None => match realm_for(id).object(id)?.get(key)? {
            (_, Static::Value(value), _) => Some(value.clone()),
            _ => None,
        },
    }
}

/** Every own data entry of one object, for finding the key a callee is stored under. */
fn own_entries(vm: &Vm, id: ObjectId) -> Vec<(Key, JsValue)> {
    match vm.heap.get(id) {
        Some(object) => object
            .props
            .iter()
            .filter_map(|(key, prop)| match &prop.slot {
                Slot::Data(value) => Some((key.clone(), value.clone())),
                _ => None,
            })
            .collect(),
        None => realm_for(id)
            .object(id)
            .map(|def| {
                def.props
                    .iter()
                    .filter_map(|(key, value, _)| match value {
                        Static::Value(value) => Some((key.clone(), value.clone())),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/** A native constructor's registered name is its own data `name`, used when the frame has no compiled name. */
pub(super) fn static_name(vm: &Vm, function: ObjectId) -> Option<JsString> {
    match own_data(vm, function, &Key::from("name"))? {
        JsValue::String(name) if !name.is_empty() => Some(name),
        _ => None,
    }
}

/** A function's ordinary name text; an author's own name property is read as data, never by a getter. */
pub(super) fn function_name(vm: &Vm, id: ObjectId) -> Option<String> {
    let own = match vm.ordinary_name_snapshot(id) {
        Some(name) if name.prefixes == 0 && name.wrapped == 0 => name.tail,
        _ => own_data(vm, id, &Key::from("name"))?,
    };
    match own {
        JsValue::String(name) if !name.is_empty() => Some(name.lossy()),
        _ => None,
    }
}

/** An own data `name` string is the debug name V8 reports, ahead of the compiled name; accessors are never run. */
pub(super) fn own_name(vm: &Vm, function: ObjectId) -> Option<JsString> {
    // Natives keep their immutable identity, so only script closures can report a rewritten name.
    let closure = vm
        .heap
        .get(function)
        .and_then(|object| object.function())
        .is_some_and(|data| matches!(data.kind, FunctionKind::Closure { .. }));
    if !closure {
        return None;
    }
    match own_data(vm, function, &Key::from("name"))? {
        JsValue::String(name) if !name.is_empty() && !dynamic_default(vm, function, &name) => {
            Some(name)
        }
        _ => None,
    }
}

/** `new Function` stores "anonymous" as its creation-default name; V8 reports no debug name for that, so the frame prints as eval. */
fn dynamic_default(vm: &Vm, function: ObjectId, name: &JsString) -> bool {
    name.lossy() == DYNAMIC_FUNCTION_NAME
        && vm.heap.get(function).is_some_and(|object| {
            object
                .props
                .get(&Key::private(DYNAMIC_SOURCE_KEY))
                .is_some()
        })
}

/** Wrapper types name primitive receivers the way V8 names their boxed constructors. */
fn primitive_type(value: &JsValue) -> Option<&'static str> {
    match value {
        #[cfg(feature = "conformance")]
        JsValue::Smi(_) => Some("Number"),
        JsValue::Number(_) => Some("Number"),
        JsValue::String(_) => Some("String"),
        JsValue::Boolean(_) => Some("Boolean"),
        JsValue::Symbol(_) => Some("Symbol"),
        JsValue::BigInt(_) | JsValue::BigIntWide(_) => Some("BigInt"),
        _ => None,
    }
}

/** V8 names a receiver by its constructor, then its Symbol.toStringTag, then Object. */
pub(super) fn type_name_of(vm: &Vm, receiver: &JsValue) -> Option<String> {
    let Some(start) = receiver.object() else {
        return primitive_type(receiver).map(str::to_owned);
    };
    // A proxy names itself, so its traps and target never decide the frame's type.
    if vm.proxy_of(start).is_some() {
        return Some("Proxy".to_owned());
    }
    // A class or function receiver is named by its own name, so static methods read "X.method".
    if vm.heap.get(start).and_then(|o| o.function()).is_some()
        && let Some(name) = function_name(vm, start)
    {
        return Some(name);
    }
    let tag = chain(vm, start).into_iter().find_map(|id| {
        match own_data(vm, id, &Key::Sym(symbol::TO_STRING_TAG))? {
            JsValue::String(tag) => Some(tag.lossy()),
            _ => None,
        }
    });
    let constructor = || {
        chain(vm, start).into_iter().find_map(|id| {
            match own_data(vm, id, &Key::from("constructor"))? {
                JsValue::Object(function) => function_name(vm, function),
                _ => None,
            }
        })
    };
    Some(
        tag.or_else(constructor)
            .unwrap_or_else(|| OBJECT_TYPE.to_owned()),
    )
}

/** A method is found under the callee's own name when that key still holds it on the chain. */
fn holds_under(vm: &Vm, start: ObjectId, name: &str, callee: &JsValue) -> bool {
    chain(vm, start)
        .into_iter()
        .find_map(|id| own_data(vm, id, &Key::from(name)))
        .is_some_and(|value| value == *callee)
}

/** Without that name, the one key holding the callee wins; two different keys are ambiguous. */
// ponytail: scans every own key on the chain per frame; V8 does the same at format time, add a reverse index if formatting shows up in profiles.
fn unique_key(vm: &Vm, start: ObjectId, callee: &JsValue) -> Option<String> {
    let mut found: Option<String> = None;
    for id in chain(vm, start) {
        for (key, value) in own_entries(vm, id) {
            let Key::Str(name) = key else { continue };
            if value != *callee {
                continue;
            }
            let text = name.lossy();
            if found.as_ref().is_some_and(|current| *current != text) {
                return None;
            }
            found = Some(text);
        }
    }
    found
}

/** The property a callee was reached through, as V8's GetMethodName reports it. */
pub(super) fn method_name_of(
    vm: &Vm,
    receiver: &JsValue,
    callee: &JsValue,
    function: Option<&str>,
) -> Option<String> {
    let start = receiver.object()?;
    match function {
        Some(name) if holds_under(vm, start, name, callee) => Some(name.to_owned()),
        _ => unique_key(vm, start, callee).or_else(|| native_identity(vm, callee, function)),
    }
}

/** A native moved off its prototype keeps its immutable identity, the name it was registered with. */
fn native_identity(vm: &Vm, callee: &JsValue, function: Option<&str>) -> Option<String> {
    let id = callee.object()?;
    let native = match vm.heap.get(id) {
        Some(object) => object
            .function()
            .is_some_and(|data| matches!(data.kind, FunctionKind::Native { .. })),
        None => realm_for(id).native(id).is_some(),
    };
    if native {
        function.map(str::to_owned)
    } else {
        None
    }
}

/** A name ends in ".method" when the method is the name's last segment, the case V8 spells without an alias. */
fn ends_with_method(name: &str, method: &str) -> bool {
    name == method
        || name
            .strip_suffix(method)
            .is_some_and(|rest| rest.ends_with('.'))
}

/** V8's method-call spelling: Type.name, Type.method for anonymous callees, and [as method] for aliases. */
pub(super) fn method_call(
    type_name: Option<&str>,
    function: Option<&str>,
    method: Option<&str>,
) -> String {
    let type_name = type_name.filter(|name| !name.is_empty());
    let method = method.filter(|name| !name.is_empty());
    match function.filter(|name| !name.is_empty()) {
        Some(name) => {
            let mut text = match type_name {
                Some(ty) if !name.starts_with(ty) => format!("{ty}."),
                _ => String::new(),
            };
            text.push_str(name);
            if let Some(method) = method.filter(|method| !ends_with_method(name, method)) {
                text.push_str(&format!(" [as {method}]"));
            }
            text
        }
        None => {
            let prefix = type_name.map_or(String::new(), |ty| format!("{ty}."));
            format!("{prefix}{}", method.unwrap_or(ANONYMOUS))
        }
    }
}
