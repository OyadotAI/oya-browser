/** Iterator.from accepts iterables or direct iterators and wraps only objects outside Iterator's chain. */
use super::*;
use crate::v9::vm::data::{Class, IterState};
use crate::v9::vm::heap::{Object, Prop, Slot};
use crate::v9::vm::realm::realm;
use std::sync::OnceLock;

/** The immutable wrapper prototype is mapped to the active intrinsic bank when constructed. */
static PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();

/** The record is an internal slot and cannot be replaced through a JavaScript property name. */
fn record_key() -> Key {
    Key::private("valid-iterator-record")
}

/** Wrapper methods forward calls without reading iterator result properties. */
pub(super) fn install(builder: &mut Builder, constructor: ObjectId) {
    builder.method(constructor, "from", 1, from);
    let prototype = builder.object(Some(builder.ids.iterator_prototype));
    builder.method(prototype, "next", 0, next);
    builder.method(prototype, "return", 0, return_method);
    let _ = PROTOTYPE.set(prototype);
}

/** Flattenability permits primitive strings, but no other primitive receives iterator coercion. */
fn flattenable(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    if !matches!(value, JsValue::Object(_) | JsValue::String(_)) {
        return Err(vm.type_error("Iterator.from requires an object or string"));
    }
    let method = vm.get(&value, &Key::Sym(symbol::ITERATOR))?;
    let iterator = if method.nullish() {
        value
    } else {
        vm.call(method, value, Vec::new())?
    };
    if !matches!(iterator, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator method did not return an object"));
    }
    vm.pin(iterator.clone());
    Ok(iterator)
}

/** OrdinaryHasInstance uses the intrinsic prototype and observes actual proxy prototype traps. */
fn intrinsic_instance(vm: &mut Vm, iterator: &JsValue) -> Res<bool> {
    let mut current = iterator.object();
    for _ in 0..crate::v9::vm::property::CHAIN_STEPS {
        let Some(id) = current else {
            return Ok(false);
        };
        current = super::super::object_get_prototype(vm, id)?;
        if current == Some(realm().ids.iterator_prototype) {
            return Ok(true);
        }
        vm.charge(1)?;
    }
    Err(vm.range_error("Maximum prototype chain length exceeded"))
}

/** The next getter is captured once even when the original Iterator instance can be returned. */
fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let iterator = flattenable(vm, call.arg(0))?;
    let next = vm.get(&iterator, &Key::from("next"))?;
    vm.pin(next.clone());
    if intrinsic_instance(vm, &iterator)? {
        return Ok(iterator);
    }
    let record = vm.new_iterator(IterState::Record {
        iterator,
        next,
        done: false,
    })?;
    let prototype = vm.intrinsic_id(*PROTOTYPE.get().expect("iterator wrapper installed"));
    let wrapper = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap
        .put(wrapper, record_key(), Prop::data(JsValue::Object(record)));
    Ok(JsValue::Object(wrapper))
}

/** RequireInternalSlot refuses arbitrary objects and proxies around valid wrappers. */
fn record(vm: &mut Vm, value: &JsValue) -> Res<(JsValue, JsValue)> {
    let record = value
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| object.props.get(&record_key()))
        .and_then(|prop| match prop.slot {
            Slot::Data(JsValue::Object(id)) => Some(id),
            _ => None,
        });
    if let Some(IterState::Record { iterator, next, .. }) = record
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| match &object.class {
            Class::Iter(state) => Some(state.as_ref()),
            _ => None,
        })
    {
        return Ok((iterator.clone(), next.clone()));
    }
    Err(vm.type_error("Iterator wrapper method called on incompatible receiver"))
}

/** Wrapper next returns the original result, including its identity and deferred getters. */
fn next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (iterator, next) = record(vm, &call.this)?;
    vm.call(next, iterator, Vec::new())
}

/** Return is looked up afresh and forwards its raw result; absent return creates a done result. */
fn return_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (iterator, _) = record(vm, &call.this)?;
    let method = vm.get(&iterator, &Key::from("return"))?;
    if method.nullish() {
        return vm.iter_result(JsValue::Undefined, true);
    }
    vm.call(method, iterator, Vec::new())
}
