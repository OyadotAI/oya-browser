/** Shell queued callbacks retain their actual relevant realm and report errors there without changing internal jobs. */
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, FunctionKind, Job};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Thrown, Vm};
use std::sync::OnceLock;

/** A native code identity carries no callback, realm or authority between isolates. */
static RUN: OnceLock<u32> = OnceLock::new();

/** Queued records use ordinary native captures, sharing their existing validation, tracing and storage budgets. */
pub(super) fn install(builder: &mut Builder) {
    let _ = RUN.set(builder.native("", 0, run));
}

/** Context lookup unwraps actual bound/proxy targets without running observable JavaScript property traps. */
pub(in crate::v9) fn enqueue(vm: &mut Vm, callback: JsValue) -> Res<()> {
    let bank = relevant_realm(vm, &callback)?;
    let context = vm.realms.contexts[bank as usize]
        .global
        .ok_or_else(|| EngineError::invalid("Missing microtask context."))?;
    let task = vm.native_closure(
        *RUN.get().expect("installed shell microtask"),
        vec![callback, JsValue::Object(context)],
    )?;
    vm.enqueue(Job::Callback(JsValue::Object(task)))
}

/** Revoked callable proxies have no target realm and use the enqueuing context, matching GetContextForMicrotask. */
pub(in crate::v9) fn relevant_realm(vm: &mut Vm, callback: &JsValue) -> Res<u32> {
    let mut id = callback
        .object()
        .ok_or_else(|| EngineError::invalid("Invalid callable microtask."))?;
    for _ in 0..vm.limits.frames {
        match vm.heap.get(id).map(|object| &object.class) {
            Some(Class::Proxy(proxy)) => match proxy.target {
                Some(target) => id = target,
                None => return Ok(vm.active_realm),
            },
            Some(Class::Function(function)) => match &function.kind {
                FunctionKind::Bound { target, .. } => id = *target,
                _ => return Ok(vm.object_realm(id)),
            },
            _ => return Ok(vm.object_realm(id)),
        }
    }
    Err(vm.range_error("Maximum call stack size exceeded"))
}

/** The native capture holds both callback and context strongly until this genuine task completes. */
fn captures(vm: &Vm, callee: ObjectId) -> Res<(JsValue, u32)> {
    let slots = vm.slots(callee);
    let [callback, JsValue::Object(context)] = slots.as_slice() else {
        return Err(EngineError::invalid("Invalid queued shell microtask.").into());
    };
    let bank = vm.object_realm(*context);
    if !vm.is_callable(callback)
        || vm
            .realms
            .contexts
            .get(bank as usize)
            .and_then(|realm| realm.global)
            != Some(*context)
    {
        return Err(EngineError::invalid("Invalid queued microtask context.").into());
    }
    Ok((callback.clone(), bank))
}

/** Callable tasks genuinely enter their relevant context, unlike ordinary host callbacks using Function::Call alone. */
fn run(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (callback, bank) = captures(vm, call.callee)?;
    let depth = vm.realm_entries.len();
    vm.realm_entries.push(bank);
    let result = vm.with_realm(bank, |vm| {
        match vm.call(callback, JsValue::Undefined, Vec::new()) {
            Ok(_) => Ok(JsValue::Undefined),
            Err(Thrown::Value(error)) => {
                super::reporting::exception(vm, error).map(|()| JsValue::Undefined)
            }
            Err(fatal) => Err(fatal),
        }
    });
    vm.realm_entries.truncate(depth);
    result
}
