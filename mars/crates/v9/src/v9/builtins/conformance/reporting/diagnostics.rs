/** Scoring diagnostics inspect only ordinary data, leaving authored proxy traps and getters to the real reporter. */
use super::*;
use crate::v9::value::ObjectId;
use crate::v9::vm::data::{Class, FunctionKind};
use crate::v9::vm::heap::{Key, Slot};
use crate::v9::vm::realm::{Static, realm_for};

/** This is the existing diagnostic lookup depth, not a new traversal or language execution allowance. */
const DIAGNOSTIC_DEPTH: usize = 8;

/** Private file-assertion provenance and ordinary harness constructors are read without invoking exotic reflection. */
pub(super) fn is_assertion(vm: &mut Vm, value: &JsValue) -> bool {
    if value
        .object()
        .and_then(|id| vm.heap.get(id))
        .is_some_and(|object| {
            object
                .props
                .get(&Key::private("conformance-file-assertion"))
                .is_some()
        })
    {
        return true;
    }
    safe(vm, value) && vm.is_conformance_assertion(value)
}

/** A neutral fallback avoids inventing reflection calls merely to construct an error message. */
pub(super) fn describe(vm: &mut Vm, value: &JsValue) -> String {
    if safe(vm, value) {
        vm.describe(value)
    } else {
        "object".into()
    }
}

/** Ordinary data chains and constructor names are safe; opaque/Proxy chains retain their authored behavior elsewhere. */
fn safe(vm: &Vm, value: &JsValue) -> bool {
    let Some(id) = value.object() else {
        return true;
    };
    ordinary_chain(vm, id) && constructor(vm, id).is_none_or(|ctor| ordinary_chain(vm, ctor))
}

/** No bound-name materialization, proxy descriptor trap or host exotic accessor participates in classification. */
fn ordinary_chain(vm: &Vm, mut id: ObjectId) -> bool {
    for _ in 0..DIAGNOSTIC_DEPTH {
        if vm.heap.get(id).is_some_and(|object| match &object.class {
            Class::Proxy(_) | Class::Host(_) | Class::Namespace(_) => true,
            Class::Function(function) => matches!(function.kind, FunctionKind::Bound { .. }),
            _ => false,
        }) {
            return false;
        }
        match vm.proto_of(id) {
            Some(next) => id = next,
            None => return true,
        }
    }
    false
}

/** Constructor lookup reads inert own data slots only, matching the diagnostic helper without calling [[GetOwnProperty]]. */
fn constructor(vm: &Vm, mut id: ObjectId) -> Option<ObjectId> {
    let name = Key::constructor();
    for _ in 0..DIAGNOSTIC_DEPTH {
        if let Some(object) = vm.heap.get(id) {
            if let Some(prop) = object.props.get(&name) {
                return match &prop.slot {
                    Slot::Data(JsValue::Object(id)) => Some(*id),
                    _ => None,
                };
            }
        } else if let Some((_, property, _)) = realm_for(id)
            .object(id)
            .and_then(|object| object.get(&name))
        {
            return match property {
                Static::Value(JsValue::Object(id)) => Some(*id),
                _ => None,
            };
        }
        id = vm.proto_of(id)?;
    }
    None
}
