/** Iterator.concat captures method identities in argument order and opens only the currently demanded input. */
use super::*;
use state::State;

/** Arguments must be objects with callable iterator methods; no iterator is opened during construction. */
pub(super) fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut inputs = Vec::new();
    for value in call.args {
        if !matches!(value, JsValue::Object(_)) {
            return Err(vm.type_error("Iterator.concat called on non-object"));
        }
        let method = vm.get(&value, &Key::Sym(symbol::ITERATOR))?;
        if !vm.is_callable(&method) {
            return Err(vm.type_error("Iterator.concat argument is not iterable"));
        }
        vm.pin(method.clone());
        inputs.push(value.clone());
        inputs.push(method);
    }
    let record = vm.new_array(inputs)?;
    let prototype = vm.intrinsic_id(*PROTOTYPE.get().expect("iterator helper installed"));
    State::create(vm, record, Mode::Concat, JsValue::Undefined, 0.0, prototype)
}

/** An inner step failure closes that active iterator; exhausted and unopened inputs are never closed. */
pub(super) fn advance(vm: &mut Vm, state: &mut State) -> Res<Option<JsValue>> {
    loop {
        vm.charge(1)?;
        if let Some(inner) = state.inner {
            match vm.iter_step(inner) {
                Ok(Some(value)) => return Ok(Some(value)),
                Ok(None) => {
                    state.inner = None;
                    state.save(vm);
                }
                Err(error) => return Err(close_error(vm, inner, error)),
            }
        }
        if !open(vm, state)? {
            return Ok(None);
        }
    }
}

/** Call the captured method only when its input becomes current, and then capture next exactly once. */
fn open(vm: &mut Vm, state: &mut State) -> Res<bool> {
    let source = JsValue::Object(state.record);
    if state.counter >= vm.length_of(&source)? as f64 {
        return Ok(false);
    }
    let value = vm.get(&source, &Key::from(state.counter.to_string().as_str()))?;
    let method = vm.get(
        &source,
        &Key::from((state.counter + 1.0).to_string().as_str()),
    )?;
    state.counter += 2.0;
    state.save(vm);
    let iterator = vm.call(method, value, Vec::new())?;
    if !matches!(iterator, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator method did not return an object"));
    }
    state.inner = Some(direct(vm, &iterator)?);
    state.save(vm);
    Ok(true)
}
