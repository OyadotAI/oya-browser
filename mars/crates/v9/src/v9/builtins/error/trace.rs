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
        "getFileName" | "getScriptNameOrSourceURL" => Ok(data(vm, id, "callSiteFile")),
        "getLineNumber" | "getEnclosingLineNumber" => Ok(data(vm, id, "callSiteLine")),
        "getColumnNumber" | "getEnclosingColumnNumber" => Ok(data(vm, id, "callSiteColumn")),
        "getPosition" => Ok(data(vm, id, "callSitePosition")),
        "isEval" => Ok(data(vm, id, "callSiteEval")),
        "isConstructor" => Ok(data(vm, id, "callSiteConstruct")),
        "isAsync" => Ok(data(vm, id, "callSiteAsync")),
        "isNative" => Ok(data(vm, id, "callSiteNative")),
        "isPromiseAll" => Ok(JsValue::Boolean(false)),
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
    match data(vm, id, "callSiteName") {
        JsValue::String(name) if !name.as_units().is_empty() => Ok(JsValue::String(name)),
        _ => Ok(JsValue::Null),
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
    if toplevel(vm, id) || data(vm, id, "callSiteConstruct") == JsValue::Boolean(true) {
        Ok(JsValue::Null)
    } else {
        nullable_name(vm, id)
    }
}

/** Type names come from actual receiver storage and cannot call a user toString hook. */
fn type_name(vm: &mut Vm, id: ObjectId) -> Res<JsValue> {
    if toplevel(vm, id) {
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
    let string = |key| match data(vm, id, key) {
        JsValue::String(text) => text.lossy(),
        other => format!("{other:?}"),
    };
    let number = |key| match data(vm, id, key) {
        JsValue::Number(value) => value as u32,
        _ => 0,
    };
    let name = string("callSiteName");
    if data(vm, id, "callSiteNative") == JsValue::Boolean(true) {
        return format!("{name} (native)");
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
    format!(
        "{name} ({} line {}, column {})",
        string("callSiteFile"),
        number("callSiteLine"),
        number("callSiteColumn")
    )
}
