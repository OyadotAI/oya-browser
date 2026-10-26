/** Weak references and finalization registries over the VM's precise weak-edge collector. */
use super::*;
use crate::v9::vm::data::{Class, FinalizationCell, FinalizationData};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{Static, symbol};
use std::sync::OnceLock;
mod cleanup;
#[cfg(feature = "conformance")]
pub(in crate::v9) use cleanup::cancel_for_realm as cancel_finalizers_for_realm;
pub(in crate::v9) use cleanup::schedule as schedule_cleanup;

/** The prototypes and weak-identity task native are fixed when this engine builds its realm. */
struct Natives {
    weak: ObjectId,
    registry: ObjectId,
}
static NATIVES: OnceLock<Natives> = OnceLock::new();

/** Install real brands and stable prototype methods rather than ordinary-object simulations. */
pub(super) fn install(b: &mut Builder) {
    let weak = prototype(b, "WeakRef");
    let ctor = b.constructor("WeakRef", 1, construct_weak_ref, weak, None);
    b.global("WeakRef", ctor);
    b.method(weak, "deref", 0, deref);
    let registry = prototype(b, "FinalizationRegistry");
    let ctor = b.constructor(
        "FinalizationRegistry",
        1,
        construct_registry,
        registry,
        None,
    );
    b.global("FinalizationRegistry", ctor);
    b.method(registry, "register", 2, register);
    b.method(registry, "unregister", 1, unregister);
    cleanup::install(b);
    let _ = NATIVES.set(Natives { weak, registry });
}

/** The standard toStringTag lives on each prototype, with the usual configurable descriptor. */
fn prototype(b: &mut Builder, name: &str) -> ObjectId {
    let prototype = b.object(Some(b.ids.object_prototype));
    b.put(
        prototype,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(crate::v9::vm::realm::string(name)),
        CONFIGURABLE,
    );
    prototype
}

/** Constructing respects new.target's prototype while retaining the intrinsic fallback. */
fn instance(
    vm: &mut Vm,
    call: &Call,
    class: Class,
    fallback: ObjectId,
    name: &str,
) -> Res<ObjectId> {
    let target = call
        .new_target
        .as_ref()
        .ok_or_else(|| vm.type_error(&format!("Constructor {name} requires 'new'")))?;
    let fallback = vm.intrinsic_id(fallback);
    let proto = vm.prototype_for(target, fallback)?;
    vm.alloc(Object::new(class, Some(proto)))
}

/** A new weak target is kept alive until the current task and all its microtasks finish. */
fn construct_weak_ref(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Constructor WeakRef requires 'new'"));
    }
    let target = call.arg(0);
    if !vm.can_hold_weakly(&target) {
        return Err(vm.type_error("WeakRef: invalid target"));
    }
    vm.keep_weak_target(target.clone());
    let id = instance(
        vm,
        &call,
        Class::WeakRef(Some(target)),
        NATIVES.get().unwrap().weak,
        "WeakRef",
    )?;
    Ok(JsValue::Object(id))
}

/** Dereferencing is brand checked and protects a still-live target through this checkpoint. */
fn deref(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call
        .this
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| match &object.class {
            Class::WeakRef(target) => Some(target.clone()),
            _ => None,
        })
        .ok_or_else(|| {
            vm.type_error("Method WeakRef.prototype.deref called on incompatible receiver")
        })?;
    let value = target.unwrap_or(JsValue::Undefined);
    vm.keep_weak_target(value.clone());
    Ok(value)
}

/** Cleanup callbacks and holdings stay alive only while their registry is strongly reachable. */
fn construct_registry(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Constructor FinalizationRegistry requires 'new'"));
    }
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("FinalizationRegistry: cleanup must be callable"));
    }
    let data = FinalizationData {
        callback: callback.object().unwrap(),
        cells: Vec::new(),
        scheduled: false,
    };
    let id = instance(
        vm,
        &call,
        Class::FinalizationRegistry(Box::new(data)),
        NATIVES.get().unwrap().registry,
        "FinalizationRegistry",
    )?;
    Ok(JsValue::Object(id))
}

/** Validate the receiver before any target or token processing. */
fn registry(vm: &mut Vm, value: &JsValue, method: &str) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| {
            matches!(
                vm.heap.get(*id).map(|object| &object.class),
                Some(Class::FinalizationRegistry(_))
            )
        })
        .ok_or_else(|| {
            vm.type_error(&format!(
                "Method FinalizationRegistry.prototype.{method} called on incompatible receiver"
            ))
        })
}

/** Registrations retain holdings while targets and optional tokens remain weak identities. */
fn register(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = registry(vm, &call.this, "register")?;
    let (target, holdings, token) = (call.arg(0), call.arg(1), call.arg(2));
    if !vm.can_hold_weakly(&target) {
        return Err(vm.type_error("FinalizationRegistry.prototype.register: invalid target"));
    }
    if target == holdings {
        return Err(vm.type_error(
            "FinalizationRegistry.prototype.register: target and holdings must not be same",
        ));
    }
    if token != JsValue::Undefined && !vm.can_hold_weakly(&token) {
        return Err(
            vm.type_error("FinalizationRegistry.prototype.register: invalid unregisterToken")
        );
    }
    let cell = FinalizationCell {
        target: Some(target),
        holdings: holdings.clone(),
        token: (token != JsValue::Undefined).then_some(token),
    };
    if let Class::FinalizationRegistry(data) = &mut vm.heap.get_mut(id).unwrap().class {
        data.cells.push(cell);
    }
    vm.heap.adjust(1, holdings.string_units() as isize);
    Ok(JsValue::Undefined)
}

/** All matching registrations, including pending cleanup holdings, are removed atomically. */
fn unregister(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = registry(vm, &call.this, "unregister")?;
    let token = call.arg(0);
    if !vm.can_hold_weakly(&token) {
        return Err(
            vm.type_error("FinalizationRegistry.prototype.unregister: invalid unregisterToken")
        );
    }
    let mut removed = 0usize;
    let mut units = 0usize;
    if let Class::FinalizationRegistry(data) = &mut vm.heap.get_mut(id).unwrap().class {
        data.cells.retain(|cell| {
            let matches = cell.token.as_ref() == Some(&token);
            if matches {
                removed += 1;
                units += cell.holdings.string_units();
            }
            !matches
        });
    }
    vm.heap.adjust(-(removed as isize), -(units as isize));
    Ok(JsValue::Boolean(removed != 0))
}
