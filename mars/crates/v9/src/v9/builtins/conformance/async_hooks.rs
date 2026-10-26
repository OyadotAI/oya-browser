/** D8 async hooks observe genuine Promise resources and callback execution, without running a foreign event system. */
mod lifecycle;
mod state;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Prop};
use crate::v9::vm::realm::{Builder, banks};
use crate::v9::vm::{Call, Res, Thrown, Vm};
pub(in crate::v9) use lifecycle::{
    after as async_hook_after, before as async_hook_before, init as async_hook_init,
    resolve as async_hook_resolve, resource as async_hook_resource, unwind as async_hook_unwind,
};
use std::sync::OnceLock;

/** Namespace and instance-method native identities contain no isolate-owned data. */
struct Natives {
    namespace: ObjectId,
    enable: u32,
    disable: u32,
}
static NATIVES: OnceLock<Natives> = OnceLock::new();
/** The namespace is exposed only when actual host flags enabled promise hooks for this isolate. */
pub(super) fn install(builder: &mut Builder) {
    let namespace = builder.object(Some(builder.ids.object_prototype));
    builder.method(namespace, "createHook", 0, create);
    builder.method(namespace, "executionAsyncId", 0, execution);
    builder.method(namespace, "triggerAsyncId", 0, trigger);
    let _ = NATIVES.set(Natives {
        namespace,
        enable: builder.native("enable", 0, enable),
        disable: builder.native("disable", 0, disable),
    });
}
/** Fresh global contexts share the isolate hook registry while retaining their own intrinsic namespace. */
pub(in crate::v9) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if vm.page.environment.conformance_async_hooks {
        let id = NATIVES.get().unwrap().namespace + bank * banks::BANK_STRIDE;
        vm.heap.put(
            global,
            Key::from("async_hooks"),
            Prop::data(JsValue::Object(id)),
        );
    }
}
/** Callback properties are read once in the same order as d8's native wrapper; non-functions are ignored. */
fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.args.len() != 1 || call.arg(0).object().is_none() {
        let error = vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            "Invalid arguments passed to createHook",
        )?;
        return Err(Thrown::Value(JsValue::Object(error)));
    }
    let records = state::queue(vm, "records")?;
    let record = state::object(vm, Class::Plain)?;
    vm.pin(JsValue::Object(record));
    state::put(vm, record, "enabled", JsValue::Boolean(false));
    for name in ["init", "before", "after", "promiseResolve"] {
        let callback = vm.get(&call.arg(0), &super::super::key(name))?;
        if vm.is_callable(&callback) {
            state::put(vm, record, name, callback);
        }
    }
    vm.array_append(&JsValue::Object(records), JsValue::Object(record))?;
    let object = vm.new_object()?;
    vm.pin(JsValue::Object(object));
    state::put(vm, object, "async-hook-record", JsValue::Object(record));
    for (name, native) in [
        ("enable", NATIVES.get().unwrap().enable),
        ("disable", NATIVES.get().unwrap().disable),
    ] {
        let function = vm.native_closure(native, Vec::new())?;
        vm.put(object, name, JsValue::Object(function))?;
    }
    vm.check_heap_bounds()?;
    Ok(JsValue::Object(object))
}
/** Enable and disable change only future dispatch; resource ids and current contexts always remain real. */
fn enabled(vm: &mut Vm, call: Call, value: bool) -> Res<JsValue> {
    let record = state::record(vm, &call.this)?;
    state::put(vm, record, "enabled", JsValue::Boolean(value));
    Ok(JsValue::Undefined)
}
/** Instance enable is an ordinary non-constructible native method. */
fn enable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    enabled(vm, call, true)
}
/** Disabling never removes retained callbacks or renumbers existing resources. */
fn disable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    enabled(vm, call, false)
}
/** The active id belongs to the currently executing promise job, independent of enabled callbacks. */
fn execution(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    Ok(JsValue::Number(state::context(vm)?.0 as f64))
}
/** Trigger identity is the real parent promise's id, or zero for a root promise. */
fn trigger(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    Ok(JsValue::Number(state::context(vm)?.1 as f64))
}
/** Hook exceptions are reported by d8's native TryCatch; termination and engine failures retain their severity. */
fn dispatch(vm: &mut Vm, event: &str, args: &[JsValue]) -> Res<()> {
    let records = state::queue(vm, "records")?;
    let mut index = 0;
    loop {
        vm.charge(1)?;
        let record = state::array(vm, records)?.get(index).cloned();
        let Some(record) = record else {
            break;
        };
        index += 1;
        let record = record.object().ok_or_else(state::invalid)?;
        if state::get(vm, record, "enabled") != Some(JsValue::Boolean(true)) {
            continue;
        }
        let Some(callback) = state::get(vm, record, event) else {
            continue;
        };
        if !vm.is_callable(&callback) {
            return Err(state::invalid());
        }
        match vm.call(callback, JsValue::Undefined, args.to_vec()) {
            Ok(_) => {}
            Err(Thrown::Value(error)) => {
                vm.note_conformance_native_error(&error);
                let text = vm.describe(&error);
                vm.page
                    .console("error", format!("Async hook exception: {text}"));
                break;
            }
            Err(fatal) => return Err(fatal),
        }
    }
    Ok(())
}
