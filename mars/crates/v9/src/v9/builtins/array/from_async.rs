/** Array.fromAsync (ES2026 23.1.2.2): the async closure the spec starts with AsyncFunctionStart,
written as a chain of promise reactions. Each await is PromiseResolve plus PerformPromiseThen
on two native closures that carry the loop state in their slots, so every iteration is one
microtask, as in V8's ArrayFromAsync builtins. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{PromiseState, ReactionHandler};
use crate::v9::vm::property::Descriptor;

/** Loop state slots carried by every continuation. */
const PROMISE: usize = 0;
const TARGET: usize = 1;
const ITERATOR: usize = 2;
const NEXT: usize = 3;
const MAPPER: usize = 4;
const THIS_ARG: usize = 5;
const INDEX: usize = 6;
/** The array-like's length; undefined while iterating. */
const LENGTH: usize = 7;
const ITEMS: usize = 8;
/** The error AsyncIteratorClose rethrows once the iterator's return() settles. */
const ERROR: usize = 9;
const SLOTS: usize = 10;

/** The continuation natives, registered once at install. */
struct Natives {
    next_done: u32,
    mapped: u32,
    close_on_reject: u32,
    reject: u32,
    closed: u32,
    value: u32,
    value_mapped: u32,
}

static NATIVES: std::sync::OnceLock<Natives> = std::sync::OnceLock::new();

/** The registered continuation natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("Array.fromAsync natives installed")
}

/** Install Array.fromAsync on the constructor and register its continuations. */
pub(super) fn install(b: &mut Builder, ctor: ObjectId) {
    b.method(ctor, "fromAsync", 1, from_async);
    let _ = NATIVES.set(Natives {
        next_done: b.native("", 1, on_next),
        mapped: b.native("", 1, on_mapped),
        close_on_reject: b.native("", 1, on_map_rejected),
        reject: b.native("", 1, on_rejected),
        closed: b.native("", 1, on_closed),
        value: b.native("", 1, on_value),
        value_mapped: b.native("", 1, on_value_mapped),
    });
}

/** Array.fromAsync(asyncItems, mapfn, thisArg): returns a promise; errors reject it. */
fn from_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    if let Err(error) = start(vm, &call, promise) {
        reject_with(vm, promise, error)?;
    }
    Ok(JsValue::Object(promise))
}

/** Reject the result promise with a thrown value; engine errors propagate. */
fn reject_with(vm: &mut Vm, promise: ObjectId, error: Thrown) -> Res<()> {
    match error {
        Thrown::Value(reason) => vm.settle(promise, PromiseState::Rejected(reason)),
        fatal => Err(fatal),
    }
}

/** The synchronous part of the closure: check mapfn, open an iterator or read the array-like. */
fn start(vm: &mut Vm, call: &Call, promise: ObjectId) -> Res<()> {
    let (items, mapper) = (call.arg(0), call.arg(1));
    if mapper != JsValue::Undefined && !vm.is_callable(&mapper) {
        return Err(vm.type_error("Array.fromAsync: mapper is not a function"));
    }
    let mut state = vec![JsValue::Undefined; SLOTS];
    state[PROMISE] = JsValue::Object(promise);
    state[MAPPER] = mapper;
    state[THIS_ARG] = call.arg(2);
    state[INDEX] = JsValue::Number(0.0);
    let ctor = call.this.clone();
    if let Some((iterator, next)) = vm.async_iterator_parts(&items)? {
        state[TARGET] = create_target(vm, ctor, None)?;
        state[ITERATOR] = iterator;
        state[NEXT] = next;
        return step_next(vm, state);
    }
    let array_like = JsValue::Object(vm.to_object(&items)?);
    let length = vm.length_of(&array_like)?;
    state[TARGET] = create_target(vm, ctor, Some(length))?;
    state[LENGTH] = JsValue::Number(length as f64);
    state[ITEMS] = array_like;
    step_value(vm, state)
}

/** `Construct(C, «len»)` when `this` is a constructor, else ArrayCreate(len). */
fn create_target(vm: &mut Vm, ctor: JsValue, length: Option<usize>) -> Res<JsValue> {
    let target = if builtin_subclassing(vm) && vm.is_constructor(&ctor) {
        let args = length
            .map(|n| JsValue::Number(n as f64))
            .into_iter()
            .collect();
        vm.construct(ctor, args, None)?
    } else {
        let length = length.unwrap_or(0);
        if length as f64 > f64::from(u32::MAX) {
            return Err(vm.range_error("Invalid array length"));
        }
        JsValue::Object(vm.new_array_length(length)?)
    };
    vm.pin(target.clone());
    Ok(target)
}

/** Await(value), continuing with `fulfilled` or `rejected` over a copy of the state. */
fn await_then(
    vm: &mut Vm,
    value: JsValue,
    fulfilled: u32,
    rejected: u32,
    state: Vec<JsValue>,
) -> Res<()> {
    let promise = vm.promise_resolve(value)?;
    let on_fulfilled = JsValue::Object(vm.native_closure(fulfilled, state.clone())?);
    vm.pin(on_fulfilled.clone());
    let on_rejected = JsValue::Object(vm.native_closure(rejected, state)?);
    vm.pin(on_rejected.clone());
    vm.perform_then(
        promise,
        ReactionHandler::Function(on_fulfilled),
        ReactionHandler::Function(on_rejected),
        None,
    )
}

/** Run a continuation: a thrown error rejects the result promise. */
fn continue_with(
    vm: &mut Vm,
    call: &Call,
    step: fn(&mut Vm, Vec<JsValue>, JsValue) -> Res<()>,
) -> Res<JsValue> {
    let state = vm.slots(call.callee);
    let Some(JsValue::Object(promise)) = state.get(PROMISE).cloned() else {
        return Ok(JsValue::Undefined);
    };
    if let Err(error) = step(vm, state, call.arg(0)) {
        reject_with(vm, promise, error)?;
    }
    Ok(JsValue::Undefined)
}

/** The loop index. */
fn index(state: &[JsValue]) -> f64 {
    match state[INDEX] {
        JsValue::Number(n) => n,
        _ => 0.0,
    }
}

/** Set `length` on the target and resolve the result promise with it. */
fn finish(vm: &mut Vm, state: &[JsValue], length: f64) -> Res<()> {
    let target = state[TARGET].clone();
    vm.set(&target, key("length"), JsValue::Number(length), true)?;
    let JsValue::Object(promise) = state[PROMISE] else {
        return Ok(());
    };
    vm.resolve_promise(promise, target)
}

/** CreateDataPropertyOrThrow(A, Pk, value). */
fn define_index(vm: &mut Vm, state: &[JsValue], value: JsValue) -> Res<()> {
    let JsValue::Object(target) = state[TARGET] else {
        return Err(vm.type_error("Array.fromAsync target is not an object"));
    };
    let key = index_key(index(state) as usize)?;
    if !vm.define(target, key, Descriptor::data(value))? {
        return Err(vm.type_error("Array.fromAsync: cannot define array element"));
    }
    Ok(())
}

/** Iterator step: call next() and await its result. */
fn step_next(vm: &mut Vm, state: Vec<JsValue>) -> Res<()> {
    let (next, iterator) = (state[NEXT].clone(), state[ITERATOR].clone());
    let result = vm.call(next, iterator, Vec::new())?;
    let (fulfilled, rejected) = (natives().next_done, natives().reject);
    await_then(vm, result, fulfilled, rejected, state)
}

/** The awaited next() result: finish when done, else map (closing on failure) or store. */
fn on_next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, |vm, state, result| {
        if !matches!(result, JsValue::Object(_)) {
            return Err(vm.type_error("Iterator result is not an object"));
        }
        if vm.get(&result, &key("done"))?.truthy() {
            return finish(vm, &state, index(&state));
        }
        let value = vm.get(&result, &key("value"))?;
        if state[MAPPER] == JsValue::Undefined {
            return store_and_continue(vm, state, value);
        }
        let args = vec![value, JsValue::Number(index(&state))];
        let mapped = match vm.call(state[MAPPER].clone(), state[THIS_ARG].clone(), args) {
            Ok(mapped) => mapped,
            Err(error) => return close(vm, state, error),
        };
        let (fulfilled, rejected) = (natives().mapped, natives().close_on_reject);
        match await_then(vm, mapped, fulfilled, rejected, state.clone()) {
            Err(error) => close(vm, state, error),
            ok => ok,
        }
    })
}

/** The awaited mapped value. */
fn on_mapped(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, store_and_continue)
}

/** Store an iterated value (closing the iterator if that fails) and take the next step. */
fn store_and_continue(vm: &mut Vm, mut state: Vec<JsValue>, value: JsValue) -> Res<()> {
    if let Err(error) = define_index(vm, &state, value) {
        return close(vm, state, error);
    }
    state[INDEX] = JsValue::Number(index(&state) + 1.0);
    step_next(vm, state)
}

/** The mapped value's await rejected: close the iterator with that reason. */
fn on_map_rejected(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, |vm, state, reason| {
        close(vm, state, Thrown::Value(reason))
    })
}

/** An awaited next() rejected: reject without closing. */
fn on_rejected(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, |_, _, reason| Err(Thrown::Value(reason)))
}

/** AsyncIteratorClose with a throw completion: call return(), await it, then rethrow `error`. */
fn close(vm: &mut Vm, mut state: Vec<JsValue>, error: Thrown) -> Res<()> {
    let Thrown::Value(reason) = error else {
        return Err(error);
    };
    let iterator = state[ITERATOR].clone();
    let inner = match vm.get_method(&iterator, &key("return")) {
        Ok(Some(method)) => vm.call(method, iterator, Vec::new()),
        Ok(None) => return Err(Thrown::Value(reason)),
        Err(other) => Err(other),
    };
    let inner = match inner {
        Ok(inner) => inner,
        Err(Thrown::Value(_)) => return Err(Thrown::Value(reason)),
        Err(fatal) => return Err(fatal),
    };
    state[ERROR] = reason.clone();
    let closed = natives().closed;
    match await_then(vm, inner, closed, closed, state) {
        Err(Thrown::Value(_)) => Err(Thrown::Value(reason)),
        other => other,
    }
}

/** The iterator's return() settled: reject with the error that closed it. */
fn on_closed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, |_, state, _| {
        Err(Thrown::Value(state[ERROR].clone()))
    })
}

/** Array-like step: finish at the length, else read element k and await it. */
fn step_value(vm: &mut Vm, state: Vec<JsValue>) -> Res<()> {
    let length = match state[LENGTH] {
        JsValue::Number(n) => n,
        _ => 0.0,
    };
    let k = index(&state);
    if k >= length {
        return finish(vm, &state, length);
    }
    let value = get_index(vm, &state[ITEMS].clone(), k as usize)?;
    let (fulfilled, rejected) = (natives().value, natives().reject);
    await_then(vm, value, fulfilled, rejected, state)
}

/** The awaited element: map and await it, or store it. */
fn on_value(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, |vm, state, value| {
        if state[MAPPER] == JsValue::Undefined {
            return store_value(vm, state, value);
        }
        let args = vec![value, JsValue::Number(index(&state))];
        let mapped = vm.call(state[MAPPER].clone(), state[THIS_ARG].clone(), args)?;
        let (fulfilled, rejected) = (natives().value_mapped, natives().reject);
        await_then(vm, mapped, fulfilled, rejected, state)
    })
}

/** The awaited mapped element. */
fn on_value_mapped(vm: &mut Vm, call: Call) -> Res<JsValue> {
    continue_with(vm, &call, store_value)
}

/** Store an array-like element and take the next step. */
fn store_value(vm: &mut Vm, mut state: Vec<JsValue>, value: JsValue) -> Res<()> {
    define_index(vm, &state, value)?;
    state[INDEX] = JsValue::Number(index(&state) + 1.0);
    step_value(vm, state)
}
