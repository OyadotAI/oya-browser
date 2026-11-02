/** V8 CallSite methods inspect recorded activations without exposing strict or ShadowRealm objects. */
use super::*;
use crate::v9::syntax::{INSTANCE_INITIALIZER_NAME, STATIC_INITIALIZER_NAME};
use crate::v9::vm::error::{CALLSITE_KEY, FRAMES_KEY};
use crate::v9::vm::heap::{Prop, Slot};
mod formatting;
mod method;
pub(super) use formatting::skip_constructor;
pub(in crate::v9) use formatting::stack;
use method::{ANONYMOUS, method_call, method_name_of, type_name_of};

/** CallSite has no public constructor; only genuine error captures make branded instances. */
pub(super) fn install(builder: &mut Builder) {
    let proto = builder.object(Some(builder.ids.object_prototype));
    builder.ids.call_site_prototype = proto;
    for name in [
        "getFunction",
        "getThis",
        "getFunctionName",
        "getMethodName",
        "getTypeName",
        "getFileName",
        "getScriptNameOrSourceURL",
        "getLineNumber",
        "getColumnNumber",
        "getEnclosingLineNumber",
        "getEnclosingColumnNumber",
        "getPosition",
        "getEvalOrigin",
        "getPromiseIndex",
        "isToplevel",
        "isEval",
        "isNative",
        "isConstructor",
        "isAsync",
        "isPromiseAll",
        "toString",
    ] {
        builder.method(proto, name, 0, method);
    }
}

/** Brand checks read only private own data and cannot invoke user proxy traps or accessors. */
fn receiver(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| {
            vm.heap
                .get(*id)
                .is_some_and(|object| object.props.get(&Key::private(CALLSITE_KEY)).is_some())
        })
        .ok_or_else(|| vm.type_error("CallSite method called on incompatible receiver"))
}

/** Recorded slots are never observable through normal property access. */
fn data(vm: &Vm, id: ObjectId, name: &str) -> JsValue {
    vm.heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(name)))
        .and_then(|prop| match &prop.slot {
            Slot::Data(value) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or(JsValue::Undefined)
}

/** Getter dispatch follows the immutable native body name, even if script renames the method. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = receiver(vm, &call.this)?;
    let name = realm().native(call.callee).expect("CallSite native").name;
    match name {
        "getFunction" => object_reference(vm, id, "callSiteFunction"),
        "getThis" => object_reference(vm, id, "callSiteThis"),
        "getFunctionName" => nullable_name(vm, id),
        "getMethodName" => method_name(vm, id),
        "getFileName" | "getScriptNameOrSourceURL"
            if data(vm, id, "callSitePromiseAll") == JsValue::Boolean(true) =>
        {
            Ok(JsValue::Null)
        }
        "getLineNumber"
        | "getEnclosingLineNumber"
        | "getColumnNumber"
        | "getEnclosingColumnNumber"
            if data(vm, id, "callSitePromiseAll") == JsValue::Boolean(true) =>
        {
            Ok(JsValue::Null)
        }
        "getFileName" => Ok(data(vm, id, "callSiteFile")),
        "getScriptNameOrSourceURL" => Ok(source_url(vm, id)),
        "getEvalOrigin" => Ok(data(vm, id, "callSiteEvalOrigin")),
        "getLineNumber" | "getEnclosingLineNumber" => Ok(data(vm, id, "callSiteLine")),
        "getColumnNumber" | "getEnclosingColumnNumber" => Ok(data(vm, id, "callSiteColumn")),
        "getPosition" => Ok(data(vm, id, "callSitePosition")),
        "isEval" => Ok(data(vm, id, "callSiteEval")),
        "isConstructor" => Ok(data(vm, id, "callSiteConstruct")),
        "isAsync" => Ok(data(vm, id, "callSiteAsync")),
        "isNative" => Ok(data(vm, id, "callSiteNative")),
        "isPromiseAll" => Ok(JsValue::Boolean(
            data(vm, id, "callSitePromiseAll") == JsValue::Boolean(true),
        )),
        "getPromiseIndex" => Ok(match data(vm, id, "callSitePromiseIndex") {
            #[cfg(feature = "conformance")]
            JsValue::Smi(index) => JsValue::Number(index),
            JsValue::Number(index) => JsValue::Number(index),
            _ => JsValue::Null,
        }),
        "isToplevel" => Ok(JsValue::Boolean(toplevel(vm, id))),
        "getTypeName" => type_name(vm, id),
        "toString" => text(vm, &render(vm, id)),
        _ => Ok(JsValue::Null),
    }
}

/** Strict frames hide objects; a ShadowRealm boundary rejects every remaining foreign object. */
fn object_reference(vm: &mut Vm, id: ObjectId, name: &str) -> Res<JsValue> {
    if data(vm, id, "callSiteStrict") == JsValue::Boolean(true) {
        return Ok(JsValue::Undefined);
    }
    let value = data(vm, id, name);
    #[cfg(feature = "conformance")]
    if let Some(object) = value.object() {
        let bank = vm.object_realm(object);
        if bank != vm.active_realm
            && (vm.realms.contexts[bank as usize].shadow
                || vm.realms.contexts[vm.active_realm as usize].shadow)
        {
            return Err(vm.type_error("CallSite method crossed a ShadowRealm boundary"));
        }
    }
    Ok(value)
}

/** Anonymous functions and top-level scripts have no FunctionName in the CallSite API. */
fn nullable_name(vm: &Vm, id: ObjectId) -> Res<JsValue> {
    if data(vm, id, "callSiteWasm") == JsValue::Boolean(true) {
        return Ok(data(vm, id, "callSiteWasmFunctionName"));
    }
    let declared = data(vm, id, "callSiteFunction")
        .object()
        .and_then(|function| method::own_name(vm, function));
    if let Some(name) = declared {
        return Ok(JsValue::String(name));
    }
    match data(vm, id, "callSiteName") {
        JsValue::String(name) if !name.is_empty() => Ok(JsValue::String(name)),
        _ if data(vm, id, "callSiteEval") == JsValue::Boolean(true) => Ok(JsValue::string("eval")?),
        _ => Ok(data(vm, id, "callSiteFunction")
            .object()
            .and_then(|function| method::static_name(vm, function))
            .map_or(JsValue::Null, JsValue::String)),
    }
}

/** Older transported CallSites predate the distinct sourceURL slot and retain their original file metadata. */
fn source_url(vm: &Vm, id: ObjectId) -> JsValue {
    if vm.heap.get(id).is_some_and(|object| {
        object
            .props
            .get(&Key::private("callSiteSourceURL"))
            .is_some()
    }) {
        data(vm, id, "callSiteSourceURL")
    } else {
        data(vm, id, "callSiteFile")
    }
}

/** A global or absent receiver denotes a top-level call rather than a method receiver. */
fn toplevel(vm: &Vm, id: ObjectId) -> bool {
    let receiver = data(vm, id, "callSiteThis");
    receiver.nullish()
        || receiver.object().is_some_and(|object| {
            object == vm.global_object()
                || vm.heap.get(object).is_some_and(|value| {
                    #[cfg(feature = "conformance")]
                    {
                        value.global
                    }
                    #[cfg(not(feature = "conformance"))]
                    {
                        let _ = value;
                        false
                    }
                })
        })
}

/** A method is a call with a non-global receiver that is not a construction. */
fn is_method_call(vm: &Vm, id: ObjectId) -> bool {
    !toplevel(vm, id) && data(vm, id, "callSiteConstruct") != JsValue::Boolean(true)
}

/** The function's own name as a string, if the frame has one. */
fn frame_name(vm: &Vm, id: ObjectId) -> Option<String> {
    match nullable_name(vm, id) {
        Ok(JsValue::String(name)) => Some(name.lossy()),
        _ => None,
    }
}

/** Method names: an accessor's installing key first, then the property holding the callee, never a user getter. */
fn method_name(vm: &mut Vm, id: ObjectId) -> Res<JsValue> {
    if !is_method_call(vm, id) || data(vm, id, "callSiteWasm") == JsValue::Boolean(true) {
        return Ok(JsValue::Null);
    }
    if let Some(name) = accessor_method(vm, id) {
        return Ok(name);
    }
    let receiver = data(vm, id, "callSiteThis");
    let callee = data(vm, id, "callSiteFunction");
    let name = frame_name(vm, id);
    match method_name_of(vm, &receiver, &callee, name.as_deref()) {
        Some(method) => text(vm, &method),
        None => Ok(JsValue::Null),
    }
}

/** Type names come from the receiver's constructor, read as data so no user hook runs. */
fn type_name(vm: &mut Vm, id: ObjectId) -> Res<JsValue> {
    if !is_method_call(vm, id) || data(vm, id, "callSiteWasm") == JsValue::Boolean(true) {
        return Ok(JsValue::Null);
    }
    let receiver = data(vm, id, "callSiteThis");
    match type_name_of(vm, &receiver) {
        Some(name) => text(vm, &name),
        None => Ok(JsValue::Null),
    }
}

/** Callee text for a frame: a method spelling, a construction, a plain name, or nothing for a bare script. */
fn callee_text(vm: &Vm, id: ObjectId, name: &str) -> Option<String> {
    // Synthetic initializers run with the instance or class as receiver, yet V8 prints them bare.
    if [INSTANCE_INITIALIZER_NAME, STATIC_INITIALIZER_NAME].contains(&name) {
        return Some(name.to_owned());
    }
    let is_eval = data(vm, id, "callSiteEval") == JsValue::Boolean(true);
    if data(vm, id, "callSiteConstruct") == JsValue::Boolean(true) {
        let name = if name.is_empty() { ANONYMOUS } else { name };
        return Some(format!("new {name}"));
    }
    if !toplevel(vm, id) {
        let function = (!name.is_empty()).then_some(name);
        let receiver = data(vm, id, "callSiteThis");
        let callee = data(vm, id, "callSiteFunction");
        let ty = type_name_of(vm, &receiver);
        let method = method_name_of(vm, &receiver, &callee, function);
        return Some(method_call(ty.as_deref(), function, method.as_deref()));
    }
    if !name.is_empty() {
        Some(name.to_owned())
    } else if is_eval {
        Some("eval".to_owned())
    } else {
        None
    }
}

/** How far up the receiver's prototype chain an accessor's property name is searched. */
const ACCESSOR_CHAIN_LIMIT: usize = 64;

/** A getter or setter frame is named by the property that installs it, as V8 reports it, not by its `get `/`set ` function name. */
fn accessor_method(vm: &Vm, id: ObjectId) -> Option<JsValue> {
    let JsValue::Object(callee) = data(vm, id, "callSiteFunction") else {
        return None;
    };
    let JsValue::Object(receiver) = data(vm, id, "callSiteThis") else {
        return None;
    };
    let key = installing_key(vm, receiver, callee)?;
    JsValue::string(&crate::v9::vm::property::key_text(&key)).ok()
}

/** Walks the receiver's prototype chain, bounded, for the key whose accessor slot holds `callee`. */
fn installing_key(vm: &Vm, receiver: ObjectId, callee: ObjectId) -> Option<Key> {
    let mut holder = Some(receiver);
    for _ in 0..ACCESSOR_CHAIN_LIMIT {
        let object = vm.heap.get(holder?)?;
        if let Some((key, _)) = object
            .props
            .iter()
            .find(|(_, prop)| installs(&prop.slot, callee))
        {
            return Some(key.clone());
        }
        holder = object.proto;
    }
    None
}

/** Whether an accessor slot holds the function as its getter or its setter. */
fn installs(slot: &Slot, callee: ObjectId) -> bool {
    matches!(slot, Slot::Accessor(get, set) if *get == Some(callee) || *set == Some(callee))
}

/** Default formatting uses the same actual source coordinates exposed by the numeric accessors. */
fn render(vm: &Vm, id: ObjectId) -> String {
    let number = |key| match data(vm, id, key) {
        #[cfg(feature = "conformance")]
        JsValue::Smi(value) => Some(value as u32),
        JsValue::Number(value) => Some(value as u32),
        _ => None,
    };
    let name = match nullable_name(vm, id).ok() {
        Some(JsValue::String(name)) => name.lossy(),
        _ => String::new(),
    };
    if let JsValue::Number(index) = data(vm, id, "callSitePromiseIndex") {
        return format!("async Promise.{name} (index {})", index as u32);
    }
    if data(vm, id, "callSiteNative") == JsValue::Boolean(true) {
        return format!("{name} (native)");
    }
    if data(vm, id, "callSiteWasm") == JsValue::Boolean(true) {
        let index = match data(vm, id, "callSiteFunction") {
            #[cfg(feature = "conformance")]
            JsValue::Smi(index) => index as u32,
            JsValue::Number(index) => index as u32,
            _ => 0,
        };
        let position = number("callSitePosition").unwrap_or_default();
        let file = match source_url(vm, id) {
            JsValue::String(file) => file.lossy(),
            _ => "<anonymous>".to_owned(),
        };
        let location = format!("{file}:wasm-function[{index}]:0x{position:x}");
        return match (
            data(vm, id, "callSiteWasmModuleName"),
            data(vm, id, "callSiteWasmFunctionName"),
        ) {
            (JsValue::String(module), JsValue::String(function)) => {
                format!("{}.{} ({location})", module.lossy(), function.lossy())
            }
            (JsValue::String(module), _) => format!("{} ({location})", module.lossy()),
            (_, JsValue::String(function)) => format!("{} ({location})", function.lossy()),
            _ => location,
        };
    }
    let source_url = source_url(vm, id);
    let file = match &source_url {
        JsValue::String(file) => file.lossy(),
        _ => "<anonymous>".to_owned(),
    };
    let location = match (number("callSiteLine"), number("callSiteColumn")) {
        (Some(line), Some(column)) => format!("{file}:{line}:{column}"),
        (Some(line), None) => format!("{file}:{line}"),
        _ => file,
    };
    let location = match data(vm, id, "callSiteEvalOrigin") {
        JsValue::String(origin) if source_url == JsValue::Undefined => {
            format!("{}, {location}", origin.lossy())
        }
        _ => location,
    };
    let prefix = if data(vm, id, "callSiteAsync") == JsValue::Boolean(true) {
        "async "
    } else {
        ""
    };
    match callee_text(vm, id, &name) {
        Some(callee) => format!("{prefix}{callee} ({location})"),
        None => format!("{prefix}{location}"),
    }
}
