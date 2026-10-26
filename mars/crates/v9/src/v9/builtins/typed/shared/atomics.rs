/** Integer Atomics use real lock-protected memory events; wait/notify coordinate actual indexed FIFO waiters. */
use super::*;
use crate::v9::vm::data::{ElementKind, WaitResult};
use std::time::Duration;
mod asynchronous;
pub(in crate::v9) use asynchronous::notify_shared;
pub(super) use asynchronous::{valid_function, valid_ticket};

/** Each callable has its specified arity and cannot be constructed. */
pub(super) fn install(builder: &mut Builder) {
    let object = builder.object(Some(builder.ids.object_prototype));
    builder.global("Atomics", object);
    tag(builder, object, "Atomics");
    for (name, length) in [
        ("add", 3),
        ("and", 3),
        ("compareExchange", 4),
        ("exchange", 3),
        ("load", 2),
        ("or", 3),
        ("store", 3),
        ("sub", 3),
        ("xor", 3),
    ] {
        builder.method(object, name, length, operation);
    }
    builder.method(object, "wait", 4, wait);
    builder.method(object, "notify", 3, notify);
    builder.method(object, "isLockFree", 1, is_lock_free);
    asynchronous::install(builder, object);
}

/** The receiver must be an attached, in-bounds integer typed array; waitable kinds are narrower. */
fn view(vm: &mut Vm, value: &JsValue, waitable: bool) -> Res<(ObjectId, ViewData)> {
    let candidate = value
        .object()
        .and_then(|id| view_of(vm, id).map(|view| (id, view)));
    candidate
        .filter(|(_, view)| {
            !view.invalid
                && if waitable {
                    matches!(view.kind, ElementKind::Int32 | ElementKind::BigInt64)
                } else {
                    matches!(
                        view.kind,
                        ElementKind::Int8
                            | ElementKind::Uint8
                            | ElementKind::Int16
                            | ElementKind::Uint16
                            | ElementKind::Int32
                            | ElementKind::Uint32
                            | ElementKind::BigInt64
                            | ElementKind::BigUint64
                    )
                }
        })
        .ok_or_else(|| vm.type_error("Atomics requires an integer typed array"))
}
/** Index coercion may detach or resize ordinary buffers, so validation is repeated before access. */
fn access(
    vm: &mut Vm,
    value: &JsValue,
    index: &JsValue,
    waitable: bool,
) -> Res<(ObjectId, ViewData, usize)> {
    let (_, initial) = view(vm, value, waitable)?;
    let index = to_index(vm, index, "atomic access index")?;
    if index >= initial.length {
        return Err(vm.range_error("Invalid atomic access index"));
    }
    let (id, view) = view(vm, value, waitable)?;
    if index >= view.length {
        return Err(vm.range_error("Invalid atomic access index"));
    }
    Ok((id, view, index))
}
/** Store returns ToIntegerOrInfinity/ToBigInt before element-width wrapping, as the specification requires. */
fn converted(vm: &mut Vm, kind: ElementKind, value: &JsValue) -> Res<JsValue> {
    if matches!(kind, ElementKind::BigInt64 | ElementKind::BigUint64) {
        super::super::super::bigint::to_bigint_full(vm, value).map(JsValue::bigint)
    } else {
        vm.to_integer(value).map(JsValue::Number)
    }
}
/** Values are converted before any memory lock is acquired, so user code never runs under it. */
fn operation(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = callee_name(&call);
    let (_, target, index) = access(vm, &call.arg(0), &call.arg(1), false)?;
    let operand = if name == "load" {
        if matches!(target.kind, ElementKind::BigInt64 | ElementKind::BigUint64) {
            JsValue::BigInt(0)
        } else {
            JsValue::Number(0.0)
        }
    } else {
        converted(vm, target.kind, &call.arg(2))?
    };
    let replacement = if name == "compareExchange" {
        converted(vm, target.kind, &call.arg(3))?
    } else {
        operand.clone()
    };
    let (_, target) = view(vm, &call.arg(0), false)?;
    if index >= target.length {
        return Err(vm.range_error("Invalid atomic access index"));
    }
    let bytes = encode(vm, target.kind, &operand)?;
    let replacement = encode(vm, target.kind, &replacement)?;
    let start = target.offset + index * element_size(target.kind);
    let size = element_size(target.kind);
    let update = |current: &mut [u8]| {
        let old = decode(target.kind, current, true);
        if name != "load" {
            let bits = update_bits(name, bits(current), bits(&bytes), bits(&replacement), size);
            current.copy_from_slice(&bits.to_le_bytes()[..size]);
        }
        old
    };
    let old = match vm
        .heap
        .get_mut(target.buffer)
        .map(|object| &mut object.class)
    {
        Some(Class::SharedArrayBuffer(buffer)) => {
            buffer.atomic(start, size, name != "load", update)
        }
        Some(Class::ArrayBuffer(buffer)) => buffer.0.get_mut(start..start + size).map(update),
        _ => None,
    }
    .ok_or_else(|| vm.type_error("Atomic backing storage is unavailable"))?;
    Ok(if name == "store" { operand } else { old })
}
/** Little-endian integer bits permit exact width-specific modular operations without floating-point loss. */
fn bits(bytes: &[u8]) -> u64 {
    let mut value = [0; 8];
    value[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(value)
}
/** Each RMW returns the old decoded element and stores the wrapped replacement in one event. */
fn update_bits(name: &str, old: u64, operand: u64, replacement: u64, size: usize) -> u64 {
    let mask = if size == 8 {
        u64::MAX
    } else {
        (1u64 << (size * 8)) - 1
    };
    (match name {
        "add" => old.wrapping_add(operand),
        "sub" => old.wrapping_sub(operand),
        "and" => old & operand,
        "or" => old | operand,
        "xor" => old ^ operand,
        "compareExchange" => {
            if old == operand {
                replacement
            } else {
                old
            }
        }
        _ => operand,
    }) & mask
}
/** Timeout is measured using real elapsed time; NaN and positive infinity mean unbounded JS waiting. */
fn timeout(vm: &mut Vm, value: &JsValue) -> Res<Option<Duration>> {
    let milliseconds = vm.to_number(value)?;
    if milliseconds.is_nan() || milliseconds == f64::INFINITY {
        return Ok(None);
    }
    Ok(Some(
        Duration::try_from_secs_f64(milliseconds.max(0.0) / 1000.0).unwrap_or(Duration::MAX),
    ))
}
/** Comparison/registration and notification use one store mutex, eliminating lost wakeups. */
fn wait(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, initial) = view(vm, &call.arg(0), true)?;
    if shared_buffer(vm, &JsValue::Object(initial.buffer)).is_none() {
        return Err(vm.type_error("Atomics.wait requires shared storage"));
    }
    let (_, target, index) = access(vm, &call.arg(0), &call.arg(1), true)?;
    let buffer = shared_buffer(vm, &JsValue::Object(target.buffer))
        .ok_or_else(|| vm.type_error("Atomics.wait requires shared storage"))?;
    let expected = converted(vm, target.kind, &call.arg(2))?;
    let expected = encode(vm, target.kind, &expected)?;
    let timeout = timeout(vm, &call.arg(3))?;
    let offset = target.offset + index * element_size(target.kind);
    let result = buffer
        .wait(offset, &expected, timeout, || vm.expired())
        .map_err(|()| crate::v9::platform::timeout())?;
    text(
        vm,
        match result {
            WaitResult::NotEqual => "not-equal",
            WaitResult::TimedOut => "timed-out",
            WaitResult::Ok => "ok",
        },
    )
}
/** Unshared waitable views return zero; no unrelated buffer or byte offset can consume a notification. */
fn notify(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, target, index) = access(vm, &call.arg(0), &call.arg(1), true)?;
    let count = match call.arg(2) {
        JsValue::Undefined => usize::MAX,
        value => vm.to_integer(&value)?.max(0.0).min(usize::MAX as f64) as usize,
    };
    let notified = match shared_buffer(vm, &JsValue::Object(target.buffer)) {
        Some(buffer) => notify_shared(
            vm,
            &buffer,
            target.offset + index * element_size(target.kind),
            count,
        )?,
        None => 0,
    };
    Ok(JsValue::Number(notified as f64))
}
/** ECMAScript mandates true for byte/halfword/word sizes; this mutex-backed implementation makes no 64-bit lock-free claim. */
fn is_lock_free(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let size = vm.to_integer(&call.arg(0))?;
    Ok(JsValue::Boolean(matches!(size, 1.0 | 2.0 | 4.0)))
}
