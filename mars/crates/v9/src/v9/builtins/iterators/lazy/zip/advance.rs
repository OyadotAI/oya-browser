/** Joint helper stepping tracks open inputs, ordered strict probes and reverse cleanup on abrupt completion. */
use super::*;

/** A row observes inputs in ascending order and never yields a row after the last input is exhausted. */
pub(in crate::v9::builtins::iterators::lazy) fn next(
    vm: &mut Vm,
    state: &mut State,
) -> Res<Option<JsValue>> {
    let mut records = values(vm, state.record)?;
    if records.iter().all(|record| record.object().is_none()) {
        return Ok(None);
    }
    let payload = state
        .callback
        .object()
        .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid joint iterator payload."))?;
    let padding = values(
        vm,
        get(vm, payload, "padding")?
            .object()
            .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid joint padding."))?,
    )?;
    if padding.len() != records.len() {
        return Err(crate::protocol::EngineError::invalid("Invalid joint padding length.").into());
    }
    let mut output = Vec::with_capacity(records.len());
    for (index, pad) in padding.iter().enumerate() {
        match item(vm, state, &mut records, index)? {
            Some(value) => {
                vm.pin(value.clone());
                output.push(value)
            }
            None if state.remaining == LONGEST => output.push(pad.clone()),
            None => return Ok(None),
        }
    }
    if records.iter().all(|record| record.object().is_none()) {
        return Ok(None);
    }
    finish(vm, state, payload, output).map(Some)
}

/** Remove a failed input before closing the remaining live inputs. */
fn item(vm: &mut Vm, state: &State, records: &mut [JsValue], index: usize) -> Res<Option<JsValue>> {
    let Some(id) = records[index].object() else {
        return Ok(None);
    };
    match vm.iter_step(id) {
        Ok(Some(value)) => Ok(Some(value)),
        Ok(None) => {
            remove(vm, state, records, index);
            ended(vm, state, records, index)?;
            Ok(None)
        }
        Err(error) => {
            remove(vm, state, records, index);
            Err(close_error_all(vm, state, error))
        }
    }
}

/** Shortest closes immediately; strict checks later done states without observing their value getters. */
fn ended(vm: &mut Vm, state: &State, records: &mut [JsValue], index: usize) -> Res<()> {
    if state.remaining == SHORTEST {
        return close(vm, state);
    }
    if state.remaining != STRICT {
        return Ok(());
    }
    if index != 0 {
        let error = vm.type_error("Iterator.zip iterators have different lengths");
        return Err(close_error_all(vm, state, error));
    }
    for later in 1..records.len() {
        let id = records[later].object().ok_or_else(|| {
            crate::protocol::EngineError::invalid("Invalid strict joint iterator state.")
        })?;
        match done_only(vm, id) {
            Ok(true) => remove(vm, state, records, later),
            Ok(false) => {
                let error = vm.type_error("Iterator.zip iterators have different lengths");
                return Err(close_error_all(vm, state, error));
            }
            Err(error) => {
                remove(vm, state, records, later);
                return Err(close_error_all(vm, state, error));
            }
        }
    }
    Ok(())
}

/** The strict completion probe uses IteratorStep, which must not read a non-done result's value. */
fn done_only(vm: &mut Vm, id: ObjectId) -> Res<bool> {
    let (iterator, next) = match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::Iter(state)) => match state.as_ref() {
            IterState::Record { iterator, next, .. } => (iterator.clone(), next.clone()),
            _ => {
                return Err(crate::protocol::EngineError::invalid(
                    "Invalid strict iterator record.",
                )
                .into());
            }
        },
        _ => {
            return Err(
                crate::protocol::EngineError::invalid("Invalid strict iterator record.").into(),
            );
        }
    };
    let result = vm.call(next, iterator, Vec::new())?;
    if !matches!(result, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator result is not an object"));
    }
    Ok(vm.get(&result, &Key::from("done"))?.truthy())
}

/** State changes precede callbacks, so nested return observes an already removed input. */
fn remove(vm: &mut Vm, state: &State, records: &mut [JsValue], index: usize) {
    records[index] = JsValue::Undefined;
    clear(vm, state.record, index);
}
/** Reverse cleanup keeps the original script exception and promotes only fatal cancellation. */
fn close_error_all(vm: &mut Vm, state: &State, error: Thrown) -> Thrown {
    match close(vm, state) {
        Err(Thrown::Fatal(fatal)) => Thrown::Fatal(fatal),
        _ => error,
    }
}

/** Keyed results have null prototypes and complete data properties, including symbol-named keys. */
fn finish(vm: &mut Vm, state: &State, payload: ObjectId, output: Vec<JsValue>) -> Res<JsValue> {
    if state.mode == Mode::Zip {
        return vm.new_array(output).map(JsValue::Object);
    }
    let keys = values(
        vm,
        get(vm, payload, "keys")?
            .object()
            .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid joint keys."))?,
    )?;
    if keys.len() != output.len() {
        return Err(crate::protocol::EngineError::invalid("Invalid joint keys length.").into());
    }
    let object = vm.alloc(crate::v9::vm::heap::Object::new(Class::Plain, None))?;
    for (key, value) in keys.iter().zip(output) {
        let key = vm.to_key(key)?;
        vm.heap.put(object, key, Prop::data(value));
    }
    Ok(JsValue::Object(object))
}
