/** The native seam handed to the IndexedDB bootstrap as `host`; scripts never see it, because the installer drops it once it has captured the functions. */
use super::wire::{
    INDEXED_DB_EFFECT_BYTES, INDEXED_DB_KIND, INDEXED_DB_LIST, INDEXED_DB_OPEN, IndexedDbCommit,
};
use crate::v9::platform::HostRequest;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Prop, Slot};
use crate::v9::vm::realm::{Builder, NativeFn};
use crate::v9::vm::{Call, GLOBAL, Res, Thrown, Vm};
use std::sync::OnceLock;

/** The private global slot holding the bootstrap's end-of-task cleanup function. */
const CLEANUP: &str = "indexedDbCleanup";
/** The seam's functions, created once with the realm. */
static NATIVES: OnceLock<Vec<(&'static str, ObjectId)>> = OnceLock::new();

/** The seam's members: name, argument count, implementation. */
const MEMBERS: [(&str, u32, NativeFn); 9] = [
    ("dispatch", 2, dispatch),
    ("parent", 2, parent),
    ("load", 2, load),
    ("list", 1, list),
    ("commit", 1, commit),
    ("blob", 1, blob),
    ("checkpoint", 0, checkpoint),
    ("cleanup", 1, cleanup),
    ("pending", 0, pending),
];

/** Create the seam's native functions with the realm. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let natives = MEMBERS
        .iter()
        .map(|(name, length, function)| (*name, b.function(name, *length, *function)))
        .collect();
    let _ = NATIVES.set(natives);
}

/** A fresh plain object carrying the seam's functions, for one bootstrap run. */
pub(super) fn host_object(vm: &mut Vm) -> Res<JsValue> {
    let host = vm.new_object()?;
    vm.pin(JsValue::Object(host));
    for (name, function) in NATIVES
        .get()
        .expect("IndexedDB natives are installed with the realm")
    {
        vm.put(host, name, JsValue::Object(*function))?;
    }
    Ok(JsValue::Object(host))
}

/** An object argument, or a TypeError. */
fn object_arg(vm: &mut Vm, call: &Call, index: usize) -> Res<ObjectId> {
    call.arg(index)
        .object()
        .ok_or_else(|| vm.type_error("IndexedDB expected an object."))
}

/** host.dispatch(target, event): dispatch as a trusted task event; answers 1 when canceled, plus 2 when a listener threw. */
fn dispatch(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = object_arg(vm, &call, 0)?;
    let event = object_arg(vm, &call, 1)?;
    let flags = crate::v9::platform::events::dispatch_task_event(vm, target, event)?;
    Ok(JsValue::Number(f64::from(flags)))
}

/** host.parent(target, parent): the next target on an event's path, or null to end it there. */
fn parent(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = object_arg(vm, &call, 0)?;
    let parent = match call.arg(1) {
        JsValue::Object(id) => JsValue::Object(id),
        _ => JsValue::Null,
    };
    let prop = Prop {
        slot: Slot::Data(parent),
        attrs: HIDDEN_ATTRS,
    };
    vm.heap.put(
        target,
        Key::private(crate::v9::platform::events::EVENT_PARENT),
        prop,
    );
    Ok(JsValue::Undefined)
}

/** Queue one coordinator request; the callback later receives the answer text, or undefined on failure. */
fn request(vm: &mut Vm, method: &str, body: String, callback: JsValue) -> Res<JsValue> {
    let Some(callback) = callback.object().filter(|_| vm.is_callable(&callback)) else {
        return Err(vm.type_error("IndexedDB expected a callback."));
    };
    let request = HostRequest {
        id: 0,
        kind: INDEXED_DB_KIND.into(),
        method: method.into(),
        url: vm.page.environment.url.clone(),
        headers: Vec::new(),
        body: Some(body),
        body_bytes: Vec::new(),
        blob_source: None,
        credentials: "omit".into(),
        mode: "same-origin".into(),
    };
    let queued = crate::v9::platform::net::record_indexed(vm, request, callback)?;
    Ok(JsValue::Boolean(queued))
}

/** host.load(nameJson, callback): ask for one database's image. */
fn load(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = vm.to_string(&call.arg(0))?.lossy();
    request(vm, INDEXED_DB_OPEN, name, call.arg(1))
}

/** host.list(callback): ask for the names and versions of the origin's databases. */
fn list(vm: &mut Vm, call: Call) -> Res<JsValue> {
    request(vm, INDEXED_DB_LIST, String::new(), call.arg(0))
}

/** host.commit(json): record a committed transaction for the coordinator; false when it cannot be accepted. */
fn commit(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = vm.to_string(&call.arg(0))?;
    let Ok(text) = text.to_dom() else {
        return Ok(JsValue::Boolean(false));
    };
    let Ok(commit) = serde_json::from_str::<IndexedDbCommit>(&text) else {
        return Ok(JsValue::Boolean(false));
    };
    let used: usize = vm
        .page
        .effects
        .indexed_db
        .iter()
        .map(IndexedDbCommit::bytes)
        .sum();
    if commit.validate().is_err() || used.saturating_add(commit.bytes()) > INDEXED_DB_EFFECT_BYTES {
        return Ok(JsValue::Boolean(false));
    }
    vm.charge(text.len())?;
    vm.page.effects.indexed_db.push(commit);
    Ok(JsValue::Boolean(true))
}

/** host.blob(value): [type, bytes, name, lastModified] for a File, [type, bytes] for a Blob, null otherwise. */
fn blob(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some((kind, bytes, file)) = crate::v9::platform::blob::blob_parts(vm, &call.arg(0)) else {
        return Ok(JsValue::Null);
    };
    let bytes = crate::v9::builtins::typed::new_uint8(vm, bytes)?;
    let mut parts = vec![kind, JsValue::Object(bytes)];
    if let Some((name, modified)) = file {
        parts.extend([name, modified]);
    }
    Ok(JsValue::Object(vm.new_array(parts)?))
}

/** host.checkpoint(): perform a microtask checkpoint, as the end of an event loop task does. */
fn checkpoint(vm: &mut Vm, _: Call) -> Res<JsValue> {
    vm.drain_jobs()?;
    Ok(JsValue::Undefined)
}

/** host.cleanup(fn): remember the bootstrap's end-of-task transaction cleanup. */
fn cleanup(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let function = call.arg(0);
    if !vm.is_callable(&function) {
        return Err(vm.type_error("IndexedDB expected a function."));
    }
    let prop = Prop {
        slot: Slot::Data(function),
        attrs: HIDDEN_ATTRS,
    };
    vm.heap.put(GLOBAL, Key::private(CLEANUP), prop);
    Ok(JsValue::Undefined)
}

/** host.pending(): a transaction was created, so the end of this task must run the cleanup. */
fn pending(vm: &mut Vm, _: Call) -> Res<JsValue> {
    vm.indexed_db_cleanup = true;
    Ok(JsValue::Undefined)
}

/** Run the bootstrap's cleanup after a task's microtask checkpoint, so its transactions become inactive. */
pub(in crate::v9) fn end_of_task(vm: &mut Vm) -> Res<()> {
    vm.indexed_db_cleanup = false;
    let function = match vm
        .heap
        .get(GLOBAL)
        .and_then(|global| global.props.get(&Key::private(CLEANUP)))
    {
        Some(Prop {
            slot: Slot::Data(function),
            ..
        }) => function.clone(),
        _ => return Ok(()),
    };
    match vm.call(function, JsValue::Undefined, Vec::new()) {
        Ok(_) => Ok(()),
        Err(Thrown::Value(error)) => crate::v9::platform::workers::report_exception(vm, &error),
        Err(fatal) => Err(fatal),
    }
}
