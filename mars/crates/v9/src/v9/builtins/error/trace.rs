/** V8 CallSite methods inspect recorded activations without exposing strict or ShadowRealm objects. */
use super::*;
use crate::v9::vm::error::{CALLSITE_KEY, FRAMES_KEY};
use crate::v9::vm::heap::{Prop, Slot};
mod formatting;
pub(super) use formatting::skip_constructor;
pub(in crate::v9) use formatting::stack;

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
    match data(vm, id, "callSiteName") {
        JsValue::String(name) if !name.as_units().is_empty() => Ok(JsValue::String(name)),
        _ if data(vm, id, "callSiteEval") == JsValue::Boolean(true) => Ok(JsValue::string("eval")?),
        _ => Ok(JsValue::Null),
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

/** Method calls retain their inferred source name without consulting mutable user descriptors. */
fn method_name(vm: &Vm, id: ObjectId) -> Res<JsValue> {
    if toplevel(vm, id)
        || data(vm, id, "callSiteConstruct") == JsValue::Boolean(true)
        || data(vm, id, "callSiteWasm") == JsValue::Boolean(true)
    {
        Ok(JsValue::Null)
    } else {
        nullable_name(vm, id)
    }
}

/** Type names come from actual receiver storage and cannot call a user toString hook. */
fn type_name(vm: &mut Vm, id: ObjectId) -> Res<JsValue> {
    if toplevel(vm, id) || data(vm, id, "callSiteWasm") == JsValue::Boolean(true) {
        return Ok(JsValue::Null);
    }
    let value = data(vm, id, "callSiteThis");
    let name = value
        .object()
        .and_then(|object| vm.heap.get(object))
        .map(|object| match object.class {
            Class::Array(_) => "Array",
            Class::Function(_) => "Function",
            Class::Error => "Error",
            _ => "Object",
        })
        .unwrap_or("Object");
    text(vm, name)
}

/** Default formatting uses the same actual source coordinates exposed by the numeric accessors. */
fn render(vm: &Vm, id: ObjectId) -> String {
    let number = |key| match data(vm, id, key) {
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
    if name.is_empty()
        && data(vm, id, "callSiteEval") != JsValue::Boolean(true)
        && data(vm, id, "callSiteConstruct") != JsValue::Boolean(true)
        && toplevel(vm, id)
    {
        return format!("{prefix}{location}");
    }
    let name = if name.is_empty() {
        if data(vm, id, "callSiteEval") == JsValue::Boolean(true) {
            "eval"
        } else {
            "<anonymous>"
        }
    } else {
        &name
    };
    format!("{prefix}{name} ({location})")
}
