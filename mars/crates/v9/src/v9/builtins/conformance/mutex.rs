/** D8's local mutex methods hold a real per-object lock across callback execution. */
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, SharedGc, SharedGcValue};
use crate::v9::vm::heap::{Key, Object, Prop};
use crate::v9::vm::realm::{Builder, banks};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/** Mutex wrappers are refused by the worker clone boundary until shared lock transport exists. */
pub(super) const MUTEX_BRAND: &str = "conformance-local-mutex";
const MUTEX_STORE: &str = "conformance-mutex-store";
static CONSTRUCTOR: OnceLock<ObjectId> = OnceLock::new();
/** Resolve a constructor identity in the requesting intrinsic bank. */
pub(super) fn constructor_id(bank: u32) -> ObjectId { *CONSTRUCTOR.get().expect("mutex installed") + bank * banks::BANK_STRIDE }

/** Register functions independently of flag-gated realm exposure. */
pub(super) fn install(builder: &mut Builder) {
    let proto = builder.object(Some(builder.ids.object_prototype));
    let constructor = builder.constructor("Mutex", 0, construct, proto, None);
    builder.symbol_method(
        constructor,
        crate::v9::vm::realm::symbol::HAS_INSTANCE,
        "[Symbol.hasInstance]",
        1,
        has_instance,
    );
    for (name, length, callback) in [
        ("lock", 2, lock as crate::v9::vm::realm::NativeFn),
        ("tryLock", 2, try_lock),
        ("lockWithTimeout", 3, lock_timeout),
        ("isMutex", 1, is_mutex_method),
    ] {
        builder.method(constructor, name, length, callback);
    }
    let _ = CONSTRUCTOR.set(constructor);
}

/** Only d8 realms requesting the real harmony-struct flag receive this local capability. */
pub(super) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if !vm.page.environment.conformance_d8 || !vm.page.environment.conformance_harmony_struct {
        return;
    }
    let id = *CONSTRUCTOR.get().expect("mutex installed") + bank * banks::BANK_STRIDE;
    let atomics = vm
        .heap
        .get(global)
        .and_then(|object| object.props.get(&Key::from("Atomics")))
        .and_then(|prop| match &prop.slot {
            crate::v9::vm::heap::Slot::Data(JsValue::Object(id)) => Some(*id),
            _ => None,
        });
    let Some(atomics) = atomics else {
        return;
    };
    vm.materialize(id).expect("mutex constructor");
    vm.materialize(atomics).expect("Atomics intrinsic");
    vm.heap
        .put(atomics, Key::from("Mutex"), Prop::data(JsValue::Object(id)));
}

/** Each instance owns an inaccessible lock state and uses the requested constructor prototype. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call.new_target.unwrap_or(JsValue::Object(call.callee));
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let prototype = vm.prototype_for(&target, fallback)?;
    let store = super::arena_transport::new_synchronizer(vm, false)?;
    create(vm, store, prototype).map(JsValue::Object)
}
/** Each wrapper points to one brokered synchronized backing shared across workers. */
fn create(vm: &mut Vm, store: SharedGc, prototype: ObjectId) -> Res<ObjectId> {
    let store = super::arena_transport::grant(vm, store)?;
    let mut native = Object::new(Class::SharedContainerRoot(store), None);
    native.extensible = false;
    let backing = vm.alloc(native)?;
    vm.pin(JsValue::Object(backing));
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap.put(
        object,
        Key::private(MUTEX_BRAND),
        Prop::data(JsValue::Boolean(true)),
    );
    vm.heap.put(
        object,
        Key::private(MUTEX_STORE),
        Prop::data(JsValue::Object(backing)),
    );
    crate::v9::builtins::integrity::prevent_extensions(vm, object)?;
    Ok(object)
}

/** The worker broker transfers an owned backing rather than cloning an independent lock. */
pub(super) fn storage(vm: &mut Vm, object: ObjectId) -> Res<SharedGc> {
    let backing = vm
        .heap
        .get(object)
        .and_then(|o| o.props.get(&Key::private(MUTEX_STORE)))
        .and_then(|p| match &p.slot {
            crate::v9::vm::heap::Slot::Data(JsValue::Object(id)) => Some(*id),
            _ => None,
        });
    match backing.and_then(|id| vm.heap.get(id)).map(|o| &o.class) {
        Some(Class::SharedContainerRoot(store)) if store.metadata()?.0.as_ref() == super::arena_transport::MUTEX_META => Ok(store.clone()),
        _ => Err(vm.type_error("Invalid Atomics.Mutex storage")),
    }
}
/** Restore a receiver-local wrapper around the same broker-granted backing. */
pub(super) fn restore(vm: &mut Vm, store: SharedGc) -> Res<ObjectId> {
    let ids: Vec<_> = vm.heap.objects.keys().copied().filter(|id| is_mutex(vm, *id)).collect();
    for id in ids {
        let other = storage(vm, id)?;
        if other.node() == store.node() && other.arena().same_store(&store.arena()) { return Ok(id); }
    }
    let constructor =
        *CONSTRUCTOR.get().expect("mutex installed") + vm.active_realm * banks::BANK_STRIDE;
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let prototype = vm.prototype_for(&JsValue::Object(constructor), fallback)?;
    create(vm, store, prototype)
}

/** Only engine-branded mutex objects pass cross-realm checks. */
pub(super) fn is_mutex(vm: &Vm, object: ObjectId) -> bool {
    vm.heap
        .get(object)
        .is_some_and(|object| object.props.get(&Key::private(MUTEX_BRAND)).is_some())
}
fn is_mutex_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(
        call.arg(0).object().is_some_and(|id| is_mutex(vm, id)),
    ))
}
/** Branded mutexes remain recognizable through same-isolate realm and prototype boundaries. */
fn has_instance(vm: &mut Vm, call: Call) -> Res<JsValue> {
    super::shared::shared_has_instance(vm, call)
}

/** Validate both arguments before touching the lock or invoking author code. */
fn arguments(vm: &mut Vm, call: &Call) -> Res<(ObjectId, JsValue)> {
    let object = call
        .arg(0)
        .object()
        .filter(|id| is_mutex(vm, *id))
        .ok_or_else(|| vm.type_error("Invalid Atomics.Mutex"))?;
    let callback = call.arg(1);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("Mutex callback must be callable"));
    }
    Ok((object, callback))
}
/** Owner zero means unlocked; the main isolate and every worker have distinct identities. */
pub(super) fn owner(vm: &Vm) -> u64 {
    vm.page
        .environment
        .conformance_worker_id
        .unwrap_or(0)
        .saturating_add(1)
}
/** Native queue exhaustion and cancellation remain distinct engine failures. */
pub(super) fn wait_failure(vm: &mut Vm, failure: crate::v9::vm::data::SharedWaitFailure) -> crate::v9::vm::Thrown {
    match failure {
        crate::v9::vm::data::SharedWaitFailure::Cancelled => crate::v9::platform::timeout().into(),
        crate::v9::vm::data::SharedWaitFailure::Limit => crate::protocol::EngineError::new("limit_exceeded", "Shared synchronization waiter limit exceeded.", "Reduce concurrent waiters.").into(),
        crate::v9::vm::data::SharedWaitFailure::Invalid => vm.type_error("Invalid synchronization state"),
    }
}
pub(super) fn locked(vm: &mut Vm, object: ObjectId) -> Res<bool> {
    let me = owner(vm) as i128;
    Ok(storage(vm, object)?.read(0)?.0 == SharedGcValue::BigInt(me))
}
/** Release publishes zero before waking one contender. */
pub(super) fn release(vm: &mut Vm, object: ObjectId) -> Res<()> {
    let store = storage(vm, object)?;
    let me = SharedGcValue::BigInt(owner(vm) as i128);
    let (previous, _) = store.update(0, |value| if *value == me { SharedGcValue::BigInt(0) } else { value.clone() })?;
    if previous != me {
        return Err(vm.type_error("Mutex is not locked by this agent"));
    }
    store.notify_waiters(1);
    Ok(())
}
/** Competing agents wait on the observed owner value and recheck it after every wake. */
pub(super) fn acquire(
    vm: &mut Vm,
    object: ObjectId,
    timeout: Option<Duration>,
    recursive_error: bool,
) -> Res<bool> {
    let store = storage(vm, object)?;
    let me = owner(vm) as i128;
    let started = Instant::now();
    loop {
        let (previous, _) = store.update(0, |value| if *value == SharedGcValue::BigInt(0) { SharedGcValue::BigInt(me) } else { value.clone() })?;
        let SharedGcValue::BigInt(previous) = previous else { return Err(vm.type_error("Invalid Atomics.Mutex storage")); };
        if previous == 0 {
            return Ok(true);
        }
        if previous == me {
            return if recursive_error {
                Err(vm.type_error("Recursive mutex lock"))
            } else {
                Ok(false)
            };
        }
        let remaining = timeout.map(|timeout| timeout.saturating_sub(started.elapsed()));
        if remaining == Some(Duration::ZERO) {
            return Ok(false);
        }
        if vm.page.state.atomics_wait_disallowed { return Err(vm.type_error("Atomics wait is not allowed")); }
        let mut poll_error = None;
        let wait = store.wait_owner(previous, remaining, || match vm.wait_safepoint() {
            Ok(expired) => expired,
            Err(error) => { poll_error = Some(error); true }
        });
        if let Some(error) = poll_error { return Err(error); }
        let wait = wait.map_err(|failure| wait_failure(vm, failure))?;
        if wait == Some(false) {
            return Ok(false);
        }
    }
}
/** Always release after a callback, including a throw or native limit. */
fn invoke(vm: &mut Vm, object: ObjectId, callback: JsValue) -> Res<JsValue> {
    let roots = vm.pins.len();
    vm.pin(JsValue::Object(object));
    let result = vm.call(callback, JsValue::Undefined, vec![]);
    let release = release(vm, object);
    vm.pins.truncate(roots);
    if matches!(result, Err(crate::v9::vm::Thrown::Fatal(_))) {
        return result;
    }
    release?;
    result
}
fn lock(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, callback) = arguments(vm, &call)?;
    acquire(vm, object, None, true)?;
    invoke(vm, object, callback)
}
fn try_lock(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, callback) = arguments(vm, &call)?;
    let success = acquire(vm, object, Some(Duration::ZERO), false)?;
    let value = if success {
        invoke(vm, object, callback)?
    } else {
        JsValue::Undefined
    };
    vm.pin(value.clone());
    let result = vm.new_object()?;
    vm.put(result, "success", JsValue::Boolean(success))?;
    vm.put(result, "value", value)?;
    Ok(JsValue::Object(result))
}
fn lock_timeout(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(timeout) = call.arg(2).number_value() else {
        return Err(vm.type_error("Timeout must be a number"));
    };
    let (object, callback) = arguments(vm, &call)?;
    let success = acquire(
        vm,
        object,
        Some(Duration::from_millis(timeout.max(0.0) as u64)),
        false,
    )?;
    let value = if success {
        invoke(vm, object, callback)?
    } else {
        JsValue::Undefined
    };
    vm.pin(value.clone());
    let result = vm.new_object()?;
    vm.put(result, "success", JsValue::Boolean(success))?;
    vm.put(result, "value", value)?;
    Ok(JsValue::Object(result))
}
