/** Disposal drains captured records in reverse order and resumes asynchronous cleanup through real promise jobs. */
use super::*;

/** A callback is invoked with exactly the receiver or argument shape captured at registration time. */
fn invoke(vm: &mut Vm, value: JsValue, method: JsValue, flags: u32) -> Res<JsValue> {
    if flags & ARGUMENT_FLAG != 0 {
        vm.call(method, JsValue::Undefined, vec![value])
    } else if flags & DEFER_FLAG != 0 {
        vm.call(method, JsValue::Undefined, Vec::new())
    } else {
        vm.call(method, value, Vec::new())
    }
}
/** Synchronous resources ignore return values and continue disposing after ordinary script exceptions. */
pub(super) fn synchronous(vm: &mut Vm, id: ObjectId) -> Res<()> {
    while let Some((value, method, flags)) = state::peek(vm, id)? {
        let roots = vm.pins.len();
        vm.pin(value.clone());
        vm.pin(method.clone());
        state::pop(vm, id)?;
        if flags & ASYNC_FLAG != 0 {
            return Err(crate::protocol::EngineError::invalid(
                "Async resource in synchronous disposal.",
            )
            .into());
        }
        match invoke(vm, value, method, flags) {
            Ok(_) => {}
            Err(Thrown::Value(error)) => state::failure(vm, id, error)?,
            Err(fatal) => return Err(fatal),
        }
        vm.charge(1)?;
        vm.pins.truncate(roots);
    }
    state::finish(vm, id)
}
/** Async null records defer one await until needed, rather than imposing a tick for every null resource. */
pub(super) fn step(vm: &mut Vm, id: ObjectId) -> Res<Option<JsValue>> {
    while let Some((value, method, flags)) = state::peek(vm, id)? {
        if flags & ASYNC_FLAG == 0
            && state::boolean(vm, id, "needs-await")?
            && !state::boolean(vm, id, "has-awaited")?
        {
            state::put(vm, id, "needs-await", JsValue::Boolean(false));
            return vm
                .promise_resolve(JsValue::Undefined)
                .map(|promise| Some(JsValue::Object(promise)));
        }
        let roots = vm.pins.len();
        vm.pin(value.clone());
        vm.pin(method.clone());
        state::pop(vm, id)?;
        if matches!(method, JsValue::Undefined) {
            state::put(vm, id, "needs-await", JsValue::Boolean(true));
            vm.pins.truncate(roots);
            continue;
        }
        match invoke(vm, value, method, flags) {
            Ok(value) if flags & ASYNC_FLAG != 0 => {
                state::put(vm, id, "has-awaited", JsValue::Boolean(true));
                match vm.promise_resolve(value) {
                    Ok(promise) => return Ok(Some(JsValue::Object(promise))),
                    Err(Thrown::Value(error)) => state::failure(vm, id, error)?,
                    Err(fatal) => return Err(fatal),
                }
            }
            Ok(_) => {}
            Err(Thrown::Value(error)) => state::failure(vm, id, error)?,
            Err(fatal) => return Err(fatal),
        }
        vm.charge(1)?;
        vm.pins.truncate(roots);
    }
    if state::boolean(vm, id, "needs-await")? && !state::boolean(vm, id, "has-awaited")? {
        state::put(vm, id, "has-awaited", JsValue::Boolean(true));
        return vm
            .promise_resolve(JsValue::Undefined)
            .map(|promise| Some(JsValue::Object(promise)));
    }
    state::finish(vm, id)?;
    Ok(None)
}

/** Public asynchronous stacks use the same single-step state machine with an outer result promise. */
pub(super) fn asynchronous(vm: &mut Vm, id: ObjectId, outer: ObjectId) -> Res<()> {
    match step(vm, id) {
        Ok(Some(JsValue::Object(promise))) => wait(vm, id, outer, promise),
        Ok(Some(_)) => {
            Err(crate::protocol::EngineError::invalid("Invalid disposal await value.").into())
        }
        Ok(None) => vm.resolve_promise(outer, JsValue::Undefined),
        Err(Thrown::Value(error)) => vm.settle(outer, PromiseState::Rejected(error)),
        Err(fatal) => Err(fatal),
    }
}

/** Native reactions use PerformPromiseThen directly and do not add a second PromiseResolve boundary. */
fn wait(vm: &mut Vm, id: ObjectId, outer: ObjectId, promise: ObjectId) -> Res<()> {
    let natives = NATIVES.get().expect("disposal installed");
    let slots = vec![JsValue::Object(id), JsValue::Object(outer)];
    let fulfilled = JsValue::Object(vm.native_closure(natives.fulfilled, slots.clone())?);
    let rejected = JsValue::Object(vm.native_closure(natives.rejected, slots)?);
    vm.perform_then(
        promise,
        ReactionHandler::Function(fulfilled),
        ReactionHandler::Function(rejected),
        None,
    )
}
/** Fulfillment continues with the next captured resource. */
pub(super) fn fulfilled(vm: &mut Vm, call: Call) -> Res<JsValue> {
    resume(vm, call, false)
}
/** Rejection records a suppression error before continuing disposal. */
pub(super) fn rejected(vm: &mut Vm, call: Call) -> Res<JsValue> {
    resume(vm, call, true)
}
/** Both continuation closures carry only rooted, validated native state and their outer promise. */
fn resume(vm: &mut Vm, call: Call, rejected: bool) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let id = slots
        .first()
        .and_then(JsValue::object)
        .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid disposal continuation."))?;
    let promise = slots
        .get(1)
        .and_then(JsValue::object)
        .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid disposal promise."))?;
    if rejected {
        state::failure(vm, id, call.arg(0))?;
    }
    asynchronous(vm, id, promise)?;
    Ok(JsValue::Undefined)
}
