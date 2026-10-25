/**
 * The global Iterator constructor and the eager %Iterator.prototype% methods (includes, join),
 * ported from V8's builtins-iterator-gen / iterator-helpers semantics: the receiver is read
 * directly (GetIteratorDirect), and argument validation failures close it before `next` is read.
 */
use super::*;
use crate::v9::vm::data::IterState;
use crate::v9::vm::heap::Object;
use crate::v9::vm::{Thrown, realm::realm};

/** Largest skippedElements count includes accepts: Number.MAX_SAFE_INTEGER. */
const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

/** Iterator, Iterator.prototype.includes and Iterator.prototype.join. */
pub(super) fn install(b: &mut Builder, iterator: ObjectId) {
    let ctor = b.constructor("Iterator", 0, construct, iterator, None);
    b.global("Iterator", ctor);
    b.method(iterator, "includes", 1, includes);
    b.method(iterator, "join", 1, join);
}

/** `new Iterator()` throws; only subclasses construct (NewTarget is not Iterator itself). */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = match call.new_target.clone() {
        Some(JsValue::Object(id)) if id != call.callee => JsValue::Object(id),
        _ => return Err(vm.type_error("Abstract class Iterator not directly constructable")),
    };
    let proto = vm.prototype_for(&target, realm().ids.iterator_prototype)?;
    let id = vm.alloc(Object::new(crate::v9::vm::data::Class::Plain, Some(proto)))?;
    Ok(JsValue::Object(id))
}

/** The receiver, which must be an object. */
fn receiver(vm: &mut Vm, call: &Call, name: &str) -> Res<JsValue> {
    match &call.this {
        JsValue::Object(_) => Ok(call.this.clone()),
        _ => Err(vm.type_error(&format!("Iterator.prototype.{name} called on non-object"))),
    }
}

/** IteratorClose(O, throw error): call `return` if present, then rethrow the original error. */
fn close_with(vm: &mut Vm, object: &JsValue, error: Thrown) -> Thrown {
    if let Ok(method) = vm.get(object, &Key::from("return"))
        && vm.is_callable(&method)
    {
        let _ = vm.call(method, object.clone(), Vec::new());
    }
    error
}

/** GetIteratorDirect: an iterator record over the receiver and its `next`, read once. */
fn direct(vm: &mut Vm, object: &JsValue) -> Res<ObjectId> {
    let next = vm.get(object, &Key::from("next"))?;
    vm.new_iterator(IterState::Record {
        iterator: object.clone(),
        next,
        done: false,
    })
}

/** The skippedElements count: undefined is 0, +Infinity skips all, other non-integers throw. */
fn to_skip(vm: &mut Vm, value: &JsValue) -> Res<f64> {
    let number = match value {
        JsValue::Undefined => return Ok(0.0),
        JsValue::Number(number) => *number,
        _ => return Err(vm.type_error("skippedElements must be a number")),
    };
    if number == f64::INFINITY {
        return Ok(number);
    }
    if number.is_nan() || (number.is_finite() && number.trunc() != number) {
        return Err(vm.type_error("skippedElements must be an integer"));
    }
    if !(0.0..=MAX_SAFE).contains(&number) {
        return Err(vm.range_error("skippedElements is out of range"));
    }
    Ok(number)
}

/** Iterator.prototype.includes(searchElement, skippedElements), with SameValueZero. */
fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let object = receiver(vm, &call, "includes")?;
    let skip = match to_skip(vm, &call.arg(1)) {
        Ok(skip) => skip,
        Err(error) => return Err(close_with(vm, &object, error)),
    };
    let record = direct(vm, &object)?;
    let target = call.arg(0);
    let mut skipped = 0.0;
    while let Some(value) = vm.iter_step(record)? {
        if skipped < skip {
            skipped += 1.0;
        } else if value.same_value_zero(&target) {
            vm.iter_close(record)?;
            return Ok(JsValue::Boolean(true));
        }
        vm.charge(1)?;
    }
    Ok(JsValue::Boolean(false))
}

/** Iterator.prototype.join(separator): like Array join, closing the iterator on a throw. */
fn join(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let object = receiver(vm, &call, "join")?;
    let separator = match call.arg(0) {
        JsValue::Undefined => JsString::from_utf8(",")?,
        value => match vm.to_string(&value) {
            Ok(text) => text,
            Err(error) => return Err(close_with(vm, &object, error)),
        },
    };
    let record = direct(vm, &object)?;
    let mut units: Vec<u16> = Vec::new();
    let mut first = true;
    while let Some(value) = vm.iter_step(record)? {
        if !first {
            units.extend_from_slice(separator.as_units());
        }
        first = false;
        if !value.nullish() {
            match vm.to_string(&value) {
                Ok(text) => units.extend_from_slice(text.as_units()),
                Err(error) => return Err(close_with(vm, &object, error)),
            }
        }
        if units.len() > crate::constants::JS_STRING_UNITS {
            let error = vm.range_error("Invalid string length");
            return Err(close_with(vm, &object, error));
        }
        vm.charge(1)?;
    }
    Ok(JsValue::String(JsString::from_units(units)?))
}
