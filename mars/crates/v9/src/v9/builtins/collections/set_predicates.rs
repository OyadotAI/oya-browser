/** Set relationship predicates choose the smaller traversal and close key iterators on early exit. */
use super::set_record::SetRecord;
use super::set_storage as storage;
use super::*;

/** The relationship being checked controls both size shortcuts and successful membership. */
#[derive(Clone, Copy)]
enum Relation {
    Subset,
    Superset,
    Disjoint,
}

/** Predicate methods have one argument and run against a Set internal slot. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.method(prototype, "isSubsetOf", 1, |vm, call| {
        test(vm, call, Relation::Subset)
    });
    builder.method(prototype, "isSupersetOf", 1, |vm, call| {
        test(vm, call, Relation::Superset)
    });
    builder.method(prototype, "isDisjointFrom", 1, |vm, call| {
        test(vm, call, Relation::Disjoint)
    });
}

/** Even impossible size relationships validate both argument methods before returning. */
fn test(vm: &mut Vm, call: Call, relation: Relation) -> Res<JsValue> {
    let receiver = storage::receiver(vm, &call.this)?;
    let other = SetRecord::read(vm, call.arg(0))?;
    let size = storage::size(vm, receiver) as f64;
    if matches!(relation, Relation::Subset) && size > other.size
        || matches!(relation, Relation::Superset) && size < other.size
    {
        return Ok(JsValue::Boolean(false));
    }
    let iterate_receiver = matches!(relation, Relation::Subset)
        || matches!(relation, Relation::Disjoint) && size <= other.size;
    let value = if iterate_receiver {
        from_receiver(vm, receiver, &other, relation)?
    } else {
        from_other(vm, receiver, &other, relation)?
    };
    Ok(JsValue::Boolean(value))
}

/** Receiver traversal remains live while has callbacks delete and append entries. */
fn from_receiver(
    vm: &mut Vm,
    receiver: ObjectId,
    other: &SetRecord,
    relation: Relation,
) -> Res<bool> {
    let mut cursor = 0;
    while let Some(value) = storage::next(vm, receiver, &mut cursor)? {
        let present = other.has(vm, value)?;
        if present == matches!(relation, Relation::Disjoint) {
            return Ok(false);
        }
    }
    Ok(true)
}

/** Early returns use normal IteratorClose, so malformed return methods and thrown errors propagate. */
fn from_other(vm: &mut Vm, receiver: ObjectId, other: &SetRecord, relation: Relation) -> Res<bool> {
    let iterator = other.iterator(vm)?;
    while let Some(value) = vm.iter_step(iterator)? {
        vm.charge(1)?;
        let present = storage::contains(vm, receiver, &value);
        if present == matches!(relation, Relation::Disjoint) {
            vm.iter_close(iterator)?;
            return Ok(false);
        }
    }
    Ok(true)
}
