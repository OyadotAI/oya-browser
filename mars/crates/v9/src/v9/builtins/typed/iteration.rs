/** Typed-array traversal validates real backing storage and ignores shadowed length properties. */
use super::*;
use crate::v9::vm::data::{IterKind, IterState};

/** Override generic array algorithms that have typed-array receiver and length semantics. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    for (name, length, function) in [
        ("at", 1, at as NativeFn),
        ("find", 1, find),
        ("findIndex", 1, find_index),
        ("every", 1, every),
        ("some", 1, some),
        ("forEach", 1, for_each),
        ("findLast", 1, find_last),
        ("findLastIndex", 1, find_last_index),
        ("reduce", 1, reduce),
        ("reduceRight", 1, reduce_right),
        ("join", 1, join),
        ("fill", 1, fill),
        ("copyWithin", 2, copy_within),
        ("reverse", 0, reverse),
        ("entries", 0, entries),
        ("keys", 0, keys),
        ("values", 0, values),
    ] {
        let method = builder.method(prototype, name, length, function);
        if name == "values" {
            builder.put(
                prototype,
                Key::Sym(symbol::ITERATOR),
                Static::Value(JsValue::Object(method)),
                crate::v9::vm::heap::HIDDEN_ATTRS,
            );
        }
    }
}

/** The index is converted after the native receiver and attached buffer are checked. */
fn at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let relative = vm.to_integer(&call.arg(0))?;
    let index = if relative < 0.0 {
        view.length as f64 + relative
    } else {
        relative
    };
    if index < 0.0 || index >= view.length as f64 {
        return Ok(JsValue::Undefined);
    }
    Ok(load(vm, id, index as usize).unwrap_or(JsValue::Undefined))
}

/** Callback search visits the initially observed length, even if the backing later detaches. */
fn search(vm: &mut Vm, call: Call, reverse: bool, index_result: bool) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("callback is not a function"));
    }
    for step in 0..view.length {
        vm.charge(1)?;
        let index = if reverse {
            view.length - 1 - step
        } else {
            step
        };
        let value = load(vm, id, index).unwrap_or(JsValue::Undefined);
        let selected = vm.call(
            callback.clone(),
            call.arg(1),
            vec![
                value.clone(),
                JsValue::Number(index as f64),
                call.this.clone(),
            ],
        )?;
        if selected.truthy() {
            return Ok(if index_result {
                JsValue::Number(index as f64)
            } else {
                value
            });
        }
    }
    Ok(if index_result {
        JsValue::Number(-1.0)
    } else {
        JsValue::Undefined
    })
}
fn find(vm: &mut Vm, call: Call) -> Res<JsValue> {
    search(vm, call, false, false)
}
fn find_index(vm: &mut Vm, call: Call) -> Res<JsValue> {
    search(vm, call, false, true)
}
fn find_last(vm: &mut Vm, call: Call) -> Res<JsValue> {
    search(vm, call, true, false)
}
fn find_last_index(vm: &mut Vm, call: Call) -> Res<JsValue> {
    search(vm, call, true, true)
}

/** Callback traversal differs only in its short-circuit condition and result. */
#[derive(Clone, Copy, PartialEq)]
enum EachMode {
    Every,
    Some,
    ForEach,
}

/** Visit the original typed length, including values that become undefined after detachment. */
fn each(vm: &mut Vm, call: Call, mode: EachMode) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("callback is not a function"));
    }
    for index in 0..view.length {
        vm.charge(1)?;
        let value = load(vm, id, index).unwrap_or(JsValue::Undefined);
        let selected = vm.call(
            callback.clone(),
            call.arg(1),
            vec![value, JsValue::Number(index as f64), call.this.clone()],
        )?;
        if mode != EachMode::ForEach && selected.truthy() == (mode == EachMode::Some) {
            return Ok(JsValue::Boolean(mode == EachMode::Some));
        }
    }
    Ok(if mode == EachMode::ForEach {
        JsValue::Undefined
    } else {
        JsValue::Boolean(mode == EachMode::Every)
    })
}
fn every(vm: &mut Vm, call: Call) -> Res<JsValue> {
    each(vm, call, EachMode::Every)
}
fn some(vm: &mut Vm, call: Call) -> Res<JsValue> {
    each(vm, call, EachMode::Some)
}
fn for_each(vm: &mut Vm, call: Call) -> Res<JsValue> {
    each(vm, call, EachMode::ForEach)
}

/** Reduction uses the original typed length and performs no observable `length` lookup. */
fn reduce_inner(vm: &mut Vm, call: Call, reverse: bool) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("callback is not a function"));
    }
    let mut step = 0;
    let mut accumulator = if call.args.len() > 1 {
        call.arg(1)
    } else {
        if view.length == 0 {
            return Err(vm.type_error("Reduce of empty array with no initial value"));
        }
        step = 1;
        load(vm, id, if reverse { view.length - 1 } else { 0 }).unwrap_or(JsValue::Undefined)
    };
    while step < view.length {
        vm.charge(1)?;
        let index = if reverse {
            view.length - 1 - step
        } else {
            step
        };
        let value = load(vm, id, index).unwrap_or(JsValue::Undefined);
        vm.pin(accumulator.clone());
        accumulator = vm.call(
            callback.clone(),
            JsValue::Undefined,
            vec![
                accumulator,
                value,
                JsValue::Number(index as f64),
                call.this.clone(),
            ],
        )?;
        step += 1;
    }
    Ok(accumulator)
}
fn reduce(vm: &mut Vm, call: Call) -> Res<JsValue> {
    reduce_inner(vm, call, false)
}
fn reduce_right(vm: &mut Vm, call: Call) -> Res<JsValue> {
    reduce_inner(vm, call, true)
}

/** Native join keeps the generic algorithm's recursion guard, but uses the typed witness length. */
fn join(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, view) = this_typed(vm, &call)?;
    let separator = match call.arg(0) {
        JsValue::Undefined => JsString::from_utf8(",")?,
        value => vm.to_string(&value)?,
    };
    if vm.page.joining.contains(&call.this) {
        return Ok(JsValue::String(JsString::empty()));
    }
    vm.page.joining.push(call.this.clone());
    let result = super::super::array::join_values(vm, &call.this, view.length, &separator);
    vm.page.joining.pop();
    Ok(JsValue::String(result?))
}

/** Array iterator state retains the actual typed receiver and checks its live backing on each step. */
fn iterator(vm: &mut Vm, call: Call, kind: IterKind) -> Res<JsValue> {
    this_typed(vm, &call)?;
    vm.new_iterator(IterState::Array {
        object: call.this,
        index: 0,
        kind,
        done: false,
    })
    .map(JsValue::Object)
}
fn values(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterator(vm, call, IterKind::Values)
}
fn keys(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterator(vm, call, IterKind::Keys)
}
fn entries(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterator(vm, call, IterKind::Entries)
}

/** Fill coerces the value exactly once, then rechecks the backing after all argument conversions. */
fn fill(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = immutable::typed(vm, &call)?;
    let bytes = encode(vm, view.kind, &call.arg(0))?;
    let start = vm.relative_index(&call.arg(1), view.length, 0)?;
    let end = vm.relative_index(&call.arg(2), view.length, view.length)?;
    let (_, current) = immutable::typed(vm, &call)?;
    for index in start..end.min(current.length) {
        vm.charge(1)?;
        write_bytes(vm, &current, index * element_size(current.kind), &bytes)?;
    }
    Ok(JsValue::Object(id))
}

/** Reverse swaps raw element bytes over the witness length; the generic algorithm would read a
`length` property, which a typed array must never consult. */
fn reverse(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = immutable::typed(vm, &call)?;
    let size = element_size(view.kind);
    vm.charge(view.length)?;
    let bytes = read_range(vm, view.buffer, view.offset, view.length * size)
        .ok_or_else(|| vm.type_error("Typed array backing became invalid"))?;
    let reversed: Vec<u8> = bytes.chunks_exact(size).rev().flatten().copied().collect();
    if !write_range(vm, view.buffer, view.offset, &reversed) {
        return Err(vm.type_error("Typed array backing became invalid"));
    }
    Ok(JsValue::Object(id))
}

/** CopyWithin snapshots overlapping bytes only after rechecking the live view. */
fn copy_within(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = immutable::typed(vm, &call)?;
    let target = vm.relative_index(&call.arg(0), view.length, 0)?;
    let start = vm.relative_index(&call.arg(1), view.length, 0)?;
    let end = vm.relative_index(&call.arg(2), view.length, view.length)?;
    let (_, current) = immutable::typed(vm, &call)?;
    let count = end
        .saturating_sub(start)
        .min(current.length.saturating_sub(start))
        .min(current.length.saturating_sub(target));
    if count == 0 {
        return Ok(JsValue::Object(id));
    }
    vm.charge(count)?;
    let size = element_size(current.kind);
    let from = current.offset + start * size;
    let to = current.offset + target * size;
    let bytes = read_range(vm, current.buffer, from, count * size)
        .ok_or_else(|| vm.type_error("Typed array backing became invalid"))?;
    if !write_range(vm, current.buffer, to, &bytes) {
        return Err(vm.type_error("Typed array backing became invalid"));
    }
    Ok(JsValue::Object(id))
}
