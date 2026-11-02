/** Set union, intersection and differences implement set-like protocols with live callback ordering. */
use super::set_record::SetRecord;
use super::set_storage as storage;
use super::*;

/** Result selection controls membership, without exposing an intermediate Set to user code. */
enum Combination {
    Union,
    Intersection,
    Difference,
    Symmetric,
}

/** Register standard methods with their observable names and arity. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.method(prototype, "union", 1, |vm, call| {
        combine(vm, call, Combination::Union)
    });
    builder.method(prototype, "intersection", 1, |vm, call| {
        combine(vm, call, Combination::Intersection)
    });
    builder.method(prototype, "difference", 1, |vm, call| {
        combine(vm, call, Combination::Difference)
    });
    builder.method(prototype, "symmetricDifference", 1, |vm, call| {
        combine(vm, call, Combination::Symmetric)
    });
    super::set_predicates::install(builder, prototype);
}

/** Protocol getters run before copying the receiver or inspecting its current internal size. */
fn combine(vm: &mut Vm, call: Call, mode: Combination) -> Res<JsValue> {
    let receiver = storage::receiver(vm, &call.this)?;
    let other = SetRecord::read(vm, call.arg(0))?;
    let iterator = if matches!(mode, Combination::Union | Combination::Symmetric) {
        Some(other.iterator(vm)?)
    } else {
        None
    };
    let copy = !matches!(mode, Combination::Intersection);
    let output = storage::result(vm, copy.then_some(receiver))?;
    let shorter = storage::size(vm, receiver) as f64 <= other.size;
    if shorter && matches!(mode, Combination::Intersection | Combination::Difference) {
        from_receiver(vm, receiver, output, &other, &mode)?;
    } else {
        from_other(vm, receiver, output, &other, &mode, iterator)?;
    }
    Ok(JsValue::Object(output))
}

/** Difference scans its copied data; intersection scans the receiver's live data. */
fn from_receiver(
    vm: &mut Vm,
    receiver: ObjectId,
    output: ObjectId,
    other: &SetRecord,
    mode: &Combination,
) -> Res<()> {
    let source = if matches!(mode, Combination::Difference) {
        output
    } else {
        receiver
    };
    let mut cursor = 0;
    while let Some(value) = storage::next(vm, source, &mut cursor)? {
        if other.has(vm, value.clone())? {
            if matches!(mode, Combination::Intersection) {
                storage::insert(vm, output, value)?;
            } else {
                storage::remove(vm, output, &value);
            }
        }
    }
    Ok(())
}

/** Iterating keys observes mutations caused by keys/next, and normalizes minus zero through storage. */
fn from_other(
    vm: &mut Vm,
    receiver: ObjectId,
    output: ObjectId,
    other: &SetRecord,
    mode: &Combination,
    iterator: Option<ObjectId>,
) -> Res<()> {
    let iterator = match iterator {
        Some(iterator) => iterator,
        None => other.iterator(vm)?,
    };
    while let Some(value) = vm.iter_step(iterator)? {
        vm.charge(1)?;
        let present = storage::contains(vm, receiver, &value);
        match mode {
            Combination::Union => storage::insert(vm, output, value)?,
            Combination::Intersection if present => storage::insert(vm, output, value)?,
            Combination::Difference => storage::remove(vm, output, &value),
            Combination::Symmetric if present => storage::remove(vm, output, &value),
            Combination::Symmetric => storage::insert(vm, output, value)?,
            _ => {}
        }
    }
    Ok(())
}
