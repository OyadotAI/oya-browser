/** Lazy helpers read one input step at a time and preserve callback, flattening and close order. */
use super::*;
use state::{Mode, State};

/** Step errors mark completion; only callbacks and mapped flattenability failures close the outer iterator. */
pub(super) fn next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut state = State::load(vm, &call.this)?;
    if state.executing {
        return Err(vm.type_error("Generator is already running"));
    }
    if state.done {
        return vm.iter_result(JsValue::Undefined, true);
    }
    state.started = true;
    state.executing = true;
    state.save(vm);
    let result = advance(vm, &mut state);
    state.executing = false;
    state.done = !matches!(result, Ok(Some(_)));
    state.save(vm);
    match result {
        Ok(Some(value)) => {
            vm.pin(value.clone());
            vm.iter_result(value, false)
        }
        Ok(None) => vm.iter_result(JsValue::Undefined, true),
        Err(error) => Err(error),
    }
}

/** Return checks reentrancy before completion, including while an underlying return calls back. */
pub(super) fn return_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut state = State::load(vm, &call.this)?;
    if state.executing {
        return Err(vm.type_error("Generator is already running"));
    }
    if state.done {
        return vm.iter_result(JsValue::Undefined, true);
    }
    vm.pin(JsValue::Object(state.record));
    state.executing = state.started;
    state.done = true;
    state.save(vm);
    let result = close(vm, &state);
    state.executing = false;
    state.save(vm);
    result?;
    vm.iter_result(JsValue::Undefined, true)
}

/** Closing an active flatMap finishes its inner iterator before the outer record. */
fn close(vm: &mut Vm, state: &State) -> Res<()> {
    if matches!(state.mode, Mode::Zip | Mode::ZipKeyed) {
        return super::zip::close(vm, state);
    }
    if state.mode == Mode::Concat {
        return match state.inner {
            Some(inner) => vm.iter_close(inner),
            None => Ok(()),
        };
    }
    if let Some(inner) = state.inner
        && let Err(error) = vm.iter_close(inner)
    {
        return Err(close_error(vm, state.record, error));
    }
    vm.iter_close(state.record)
}

/** Infinite limits remain infinity; finite drop/take limits decrease before the next observable action. */
fn advance(vm: &mut Vm, state: &mut State) -> Res<Option<JsValue>> {
    if state.mode == Mode::Concat {
        return super::concat::advance(vm, state);
    }
    if matches!(state.mode, Mode::Zip | Mode::ZipKeyed) {
        return super::zip::next(vm, state);
    }
    loop {
        vm.charge(1)?;
        if let Some(inner) = state.inner {
            match vm.iter_step(inner) {
                Ok(Some(value)) => return Ok(Some(value)),
                Ok(None) => {
                    state.inner = None;
                    state.save(vm);
                }
                Err(error) => return Err(close_error(vm, state.record, error)),
            }
        }
        if state.mode == Mode::Take && state.remaining == 0.0 {
            vm.iter_close(state.record)?;
            return Ok(None);
        }
        let Some(value) = vm.iter_step(state.record)? else {
            return Ok(None);
        };
        match state.mode {
            Mode::Take => {
                state.remaining -= 1.0;
                return Ok(Some(value));
            }
            Mode::Drop if state.remaining > 0.0 => {
                state.remaining -= 1.0;
                state.save(vm);
            }
            Mode::Drop => return Ok(Some(value)),
            _ => {
                if let Some(value) = callback_step(vm, state, value)? {
                    return Ok(Some(value));
                }
            }
        }
    }
}

/** Mapper and predicate counters count examined values, including filter values that are not yielded. */
fn callback_step(vm: &mut Vm, state: &mut State, value: JsValue) -> Res<Option<JsValue>> {
    let result = vm.call(
        state.callback.clone(),
        JsValue::Undefined,
        vec![value.clone(), JsValue::Number(state.counter)],
    );
    state.counter += 1.0;
    state.save(vm);
    let mapped = match result {
        Ok(mapped) => mapped,
        Err(error) => return Err(close_error(vm, state.record, error)),
    };
    match state.mode {
        Mode::Map => Ok(Some(mapped)),
        Mode::Filter => Ok(mapped.truthy().then_some(value)),
        Mode::FlatMap => {
            state.inner = Some(match flattenable(vm, mapped) {
                Ok(inner) => inner,
                Err(error) => return Err(close_error(vm, state.record, error)),
            });
            state.save(vm);
            Ok(None)
        }
        _ => Err(crate::protocol::EngineError::invalid("Invalid iterator callback mode.").into()),
    }
}

/** flatMap rejects primitive strings while accepting objects with iteration or direct next methods. */
fn flattenable(vm: &mut Vm, value: JsValue) -> Res<ObjectId> {
    if !matches!(value, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator.flatMap mapper must return an object"));
    }
    vm.pin(value.clone());
    let method = vm.get(&value, &Key::Sym(symbol::ITERATOR))?;
    let iterator = if method.nullish() {
        value
    } else {
        vm.call(method, value, Vec::new())?
    };
    if !matches!(iterator, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator method did not return an object"));
    }
    super::direct(vm, &iterator)
}
