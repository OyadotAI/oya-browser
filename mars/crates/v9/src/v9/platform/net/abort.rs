/** Native signal identity binds fetch cancellation without consulting script-overridable properties. */
use super::{Pending, headers};
use crate::constants::VM_HOST_REQUESTS;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::platform::{PageState, Target};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, HostData, PromiseState};
use crate::v9::vm::heap::{Heap, Key, Object, Slot};
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Res, Thrown, Vm};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/** These immutable private names cannot be supplied by author property lookups. */
const ABORTED: &str = "nativeSignalAborted";
const REASON: &str = "nativeSignalReason";
const SOURCE: &str = "nativeSignalSource";
const CONTROLLER: &str = "nativeControllerSignal";
/** Requests and fetched responses retain the same internal cancellation source. */
pub(super) const SIGNAL: &str = "nativeFetchSignal";

/** Saved-state validation visits the heap, so its immutable private keys are built only once. */
fn key(name: &str) -> &'static Key {
    static KEYS: OnceLock<BTreeMap<&'static str, Key>> = OnceLock::new();
    KEYS.get_or_init(|| {
        [ABORTED, REASON, SOURCE, CONTROLLER, SIGNAL]
            .into_iter()
            .map(|name| (name, headers::private_key(name)))
            .collect()
    })
    .get(name)
    .expect("native signal slot")
}

/** A private boolean brands a genuine signal; public properties never establish identity. */
fn branded(heap: &Heap, id: ObjectId) -> bool {
    matches!(heap.get(id).map(|object| &object.class), Some(Class::Plain))
        && matches!(private(heap, id, ABORTED), Some(JsValue::Boolean(_)))
}

/** Read only data slots written by this module, without invoking author callbacks. */
fn private(heap: &Heap, id: ObjectId, name: &str) -> Option<JsValue> {
    match &heap.get(id)?.props.get(key(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}

/** Native consumers reject ordinary objects even if their prototype or properties mimic a signal. */
pub(in crate::v9) fn identity(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| branded(&vm.heap, *id))
        .ok_or_else(|| vm.type_error("The supplied value is not a native AbortSignal"))
}

/** A new root signal participates in ordinary private-slot tracing and heap accounting. */
pub(in crate::v9) fn new_signal(vm: &mut Vm) -> Res<ObjectId> {
    let id = vm.alloc(Object::new(
        Class::Plain,
        Some(realm().ids.abort_signal_prototype),
    ))?;
    headers::put_private(vm, id, ABORTED, JsValue::Boolean(false));
    headers::put_private(vm, id, REASON, JsValue::Undefined);
    vm.put(id, "onabort", JsValue::Null)?;
    Ok(id)
}

/** A dependent has its own observable identity and keeps its ultimate source alive. */
fn dependent(vm: &mut Vm, source: ObjectId) -> Res<ObjectId> {
    let id = new_signal(vm)?;
    if let Some(reason) = reason(vm, source) {
        set_aborted(vm, id, reason);
    } else {
        let root = headers::get_private(vm, source, SOURCE).unwrap_or(JsValue::Object(source));
        headers::put_private(vm, id, SOURCE, root);
    }
    Ok(id)
}

/** RequestInit.signal is read once, after the other request option conversions have completed. */
pub(super) fn for_request(vm: &mut Vm, input: &JsValue, init: &JsValue) -> Res<ObjectId> {
    let supplied = super::member(vm, init, "signal")?;
    let inherited = matches!(
        super::super::host_data(vm, input),
        Some(HostData::Request(_))
    );
    let source = if supplied == JsValue::Undefined && inherited {
        input
            .object()
            .and_then(|id| headers::get_private(vm, id, SIGNAL))
    } else if supplied.nullish() {
        None
    } else {
        Some(supplied)
    };
    match source {
        Some(source) => {
            let id = identity(vm, &source)?;
            dependent(vm, id)
        }
        None => new_signal(vm),
    }
}

/** Store cancellation privately so replacing a public signal accessor cannot retarget a request. */
pub(super) fn bind(vm: &mut Vm, object: ObjectId, signal: ObjectId) {
    headers::put_private(vm, object, SIGNAL, JsValue::Object(signal));
}

/** Every read of Request.signal returns its single dependent signal. */
pub(super) fn request_signal(vm: &mut Vm, request: ObjectId) -> Res<JsValue> {
    let value = headers::get_private(vm, request, SIGNAL)
        .ok_or_else(|| EngineError::invalid("Missing Request signal."))?;
    identity(vm, &value)?;
    Ok(value)
}

/** An aborted signal preserves the exact reason, including undefined-like primitive values. */
pub(super) fn reason(vm: &Vm, signal: ObjectId) -> Option<JsValue> {
    (headers::get_private(vm, signal, ABORTED) == Some(JsValue::Boolean(true)))
        .then(|| headers::get_private(vm, signal, REASON).unwrap_or(JsValue::Undefined))
}

/** Fetched bodies keep cancellation after the fetch promise has fulfilled. */
pub(super) fn body_reason(vm: &Vm, response: ObjectId) -> Option<JsValue> {
    let signal = headers::get_private(vm, response, SIGNAL)?.object()?;
    reason(vm, signal)
}

/** Getter semantics and throwIfAborted use private state rather than replaceable own properties. */
pub(in crate::v9) fn attribute(vm: &mut Vm, value: &JsValue, name: &str) -> Res<JsValue> {
    let id = identity(vm, value)?;
    Ok(match name {
        "aborted" => JsValue::Boolean(reason(vm, id).is_some()),
        _ => headers::get_private(vm, id, REASON).unwrap_or(JsValue::Undefined),
    })
}

/** Controllers retain their signal privately and expose a read-only prototype getter. */
pub(in crate::v9) fn bind_controller(vm: &mut Vm, controller: ObjectId, signal: ObjectId) {
    headers::put_private(vm, controller, CONTROLLER, JsValue::Object(signal));
}

/** Controller methods cannot be redirected by assigning an own public signal property. */
pub(in crate::v9) fn controller_signal(vm: &mut Vm, controller: &JsValue) -> Res<ObjectId> {
    let value = controller
        .object()
        .and_then(|id| headers::get_private(vm, id, CONTROLLER))
        .ok_or_else(|| vm.type_error("Illegal AbortController invocation"))?;
    identity(vm, &value)
}

/** Capture weak dependents from the bounded live heap; sources never retain dead descendants. */
fn followers(vm: &mut Vm, source: ObjectId) -> Res<Vec<ObjectId>> {
    vm.charge(vm.heap.objects.len())?;
    let key = key(SOURCE);
    let mut ids = vec![source];
    ids.extend(vm.heap.objects.iter().filter_map(|(id, object)| {
        matches!(object.props.get(key).map(|prop| &prop.slot),
            Some(Slot::Data(JsValue::Object(root))) if *root == source)
        .then_some(*id)
    }));
    Ok(ids)
}

/** Abort algorithms run before each signal's event, after all dependent states are updated. */
pub(in crate::v9) fn mark_aborted(vm: &mut Vm, signal: ObjectId, value: JsValue) -> Res<()> {
    if reason(vm, signal).is_some() {
        return Ok(());
    }
    let reason = default_reason(vm, value)?;
    let signals = followers(vm, signal)?;
    for id in &signals {
        set_aborted(vm, *id, reason.clone());
        vm.pin(JsValue::Object(*id));
    }
    for id in signals {
        cancel_pending(vm, id, &reason)?;
        super::super::fire(vm, Target::Object(id), "abort", false)?;
    }
    Ok(())
}

/** Omitted reasons become one shared AbortError object for the source and every dependent. */
fn default_reason(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    if value != JsValue::Undefined {
        return Ok(value);
    }
    match vm.dom_exception("AbortError", "signal is aborted without reason") {
        Thrown::Value(value) => Ok(value),
        fatal => Err(fatal),
    }
}

/** Setting state invokes no script, so every dependent is aborted before any event callback. */
fn set_aborted(vm: &mut Vm, signal: ObjectId, reason: JsValue) {
    headers::put_private(vm, signal, ABORTED, JsValue::Boolean(true));
    headers::put_private(vm, signal, REASON, reason);
}

/** Remove pending ownership first; any late host response consequently has no promise to settle. */
fn cancel_pending(vm: &mut Vm, signal: ObjectId, reason: &JsValue) -> Res<()> {
    vm.charge(vm.page.state.pending.len().saturating_add(1))?;
    let pending = std::mem::take(&mut vm.page.state.pending);
    let (cancelled, retained): (Vec<_>, Vec<_>) = pending
        .into_iter()
        .partition(|pending| !pending.xhr && pending.signal == Some(signal));
    vm.page.state.pending = retained;
    for pending in cancelled {
        cancel_effect(vm, &pending)?;
        vm.settle(pending.owner, PromiseState::Rejected(reason.clone()))?;
    }
    Ok(())
}

/** Previously published source snapshots need an explicit host cancellation; new ones do not escape. */
fn cancel_effect(vm: &mut Vm, pending: &Pending) -> Res<()> {
    let before = vm.page.effects.requests.len();
    vm.page
        .effects
        .requests
        .retain(|request| request.id != pending.id);
    if vm.page.effects.requests.len() != before {
        return Ok(());
    }
    if vm.page.effects.cancelled_requests.len() >= VM_HOST_REQUESTS {
        return Err(EngineError::limit("cancelled host requests", VM_HOST_REQUESTS).into());
    }
    vm.page.effects.cancelled_requests.push(pending.id);
    Ok(())
}

/** Serialized fetch and signal links must retain genuine native identities and canonical roots. */
pub(super) fn validate(page: &PageState, heap: &Heap) -> EngineResult<()> {
    let mut requests = BTreeSet::new();
    for pending in &page.pending {
        if pending.id == 0
            || pending.id > page.next_request
            || !requests.insert(pending.id)
            || pending.xhr && pending.signal.is_some()
            || pending.signal.is_some_and(|id| !branded(heap, id))
            || !pending.xhr && pending.signal.is_none()
            || !heap.get(pending.owner).is_some_and(|object| {
                if pending.xhr {
                    page.xhrs.contains_key(&pending.owner)
                } else {
                    matches!(object.class, Class::Promise(_))
                }
            })
        {
            return Err(invalid());
        }
    }
    for (id, object) in &heap.objects {
        if object.props.get(key(ABORTED)).is_some() {
            validate_signal(heap, *id)?;
        }
        for name in [SIGNAL, CONTROLLER] {
            if object.props.get(key(name)).is_some()
                && !private(heap, *id, name)
                    .and_then(|value| value.object())
                    .is_some_and(|signal| branded(heap, signal))
            {
                return Err(invalid());
            }
        }
        if matches!(&object.class, Class::Host(host) if matches!(host.as_ref(), HostData::Request(_)))
            && private(heap, *id, SIGNAL).is_none()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/** Flattened dependencies exclude cycles and preserve a single source's exact abort state. */
fn validate_signal(heap: &Heap, id: ObjectId) -> EngineResult<()> {
    if !branded(heap, id) || private(heap, id, REASON).is_none() {
        return Err(invalid());
    }
    let has_source = heap
        .get(id)
        .is_some_and(|object| object.props.get(key(SOURCE)).is_some());
    if has_source {
        let source = private(heap, id, SOURCE).ok_or_else(invalid)?;
        let source = source.object().ok_or_else(invalid)?;
        if source == id
            || !branded(heap, source)
            || private(heap, source, SOURCE).is_some()
            || private(heap, source, ABORTED) != private(heap, id, ABORTED)
            || !private(heap, source, REASON)
                .zip(private(heap, id, REASON))
                .is_some_and(|(source, dependent)| source.same_value(&dependent))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/** Private saved state is never permission to forge a signal or retarget cancellation. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid native fetch signal state.")
}
