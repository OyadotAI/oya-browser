/** Eager iterator helpers consume direct records with observable callbacks and precise IteratorClose. */
use super::*;
use crate::v9::vm::Thrown;

/** Callback helpers share traversal while preserving their distinct early return values. */
#[derive(Clone, Copy)]
enum Mode {
    ForEach,
    Some,
    Every,
    Find,
}

/** Register each method on the intrinsic prototype, without consulting receiver constructors. */
pub(super) fn install(builder: &mut Builder, prototype: ObjectId) {
    builder.method(prototype, "toArray", 0, to_array);
    builder.method(prototype, "reduce", 1, reduce);
    builder.method(prototype, "forEach", 1, |vm, call| {
        visit(vm, call, Mode::ForEach)
    });
    builder.method(prototype, "some", 1, |vm, call| visit(vm, call, Mode::Some));
    builder.method(prototype, "every", 1, |vm, call| {
        visit(vm, call, Mode::Every)
    });
    builder.method(prototype, "find", 1, |vm, call| visit(vm, call, Mode::Find));
}

/** Argument validation closes the receiver before any next getter can run. */
fn callback_record(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator method called on non-object"));
    }
    if !vm.is_callable(&call.arg(0)) {
        let error = vm.type_error("Iterator callback must be callable");
        return Err(super::helpers::close_with(vm, &call.this, error));
    }
    super::helpers::direct(vm, &call.this)
}

/** Callback exceptions take precedence over script cleanup exceptions; cancellation remains fatal. */
fn close_error(vm: &mut Vm, record: ObjectId, error: Thrown) -> Thrown {
    match vm.iter_close(record) {
        Err(Thrown::Fatal(fatal)) => Thrown::Fatal(fatal),
        _ => error,
    }
}

/** Collect values without invoking an Array species, overridden iterator, or inherited numeric setter. */
fn to_array(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator.prototype.toArray called on non-object"));
    }
    let record = super::helpers::direct(vm, &call.this)?;
    let mut values = Vec::new();
    while let Some(value) = vm.iter_step(record)? {
        if values.len() >= vm.limits.array_length {
            return Err(crate::protocol::EngineError::limit(
                "V9 resident array elements",
                vm.limits.array_length,
            )
            .into());
        }
        vm.pin(value.clone());
        values.push(value);
        vm.charge(1)?;
    }
    vm.new_array(values).map(JsValue::Object)
}

/** Predicate results stop iteration only for the method's matching truth condition. */
fn visit(vm: &mut Vm, call: Call, mode: Mode) -> Res<JsValue> {
    let record = callback_record(vm, &call)?;
    let mut counter = 0.0;
    while let Some(value) = vm.iter_step(record)? {
        let result = vm.call(
            call.arg(0),
            JsValue::Undefined,
            vec![value.clone(), JsValue::Number(counter)],
        );
        let result = match result {
            Ok(result) => result,
            Err(error) => return Err(close_error(vm, record, error)),
        };
        if let Some(output) = selected(mode, result.truthy(), value) {
            vm.pin(output.clone());
            vm.iter_close(record)?;
            return Ok(output);
        }
        counter += 1.0;
        vm.charge(1)?;
    }
    Ok(match mode {
        Mode::Some => JsValue::Boolean(false),
        Mode::Every => JsValue::Boolean(true),
        _ => JsValue::Undefined,
    })
}

/** A successful find retains the examined value, while Boolean predicates return their decision. */
fn selected(mode: Mode, truthy: bool, value: JsValue) -> Option<JsValue> {
    match mode {
        Mode::Some if truthy => Some(JsValue::Boolean(true)),
        Mode::Every if !truthy => Some(JsValue::Boolean(false)),
        Mode::Find if truthy => Some(value),
        _ => None,
    }
}

/** Missing initialValue differs from explicitly supplied undefined, including the first callback index. */
fn reduce(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let record = callback_record(vm, &call)?;
    let (mut accumulator, mut counter) = if call.args.len() > 1 {
        (call.arg(1), 0.0)
    } else {
        (
            vm.iter_step(record)?
                .ok_or_else(|| vm.type_error("Reduce of empty iterator with no initial value"))?,
            1.0,
        )
    };
    let roots = vm.pins.len();
    loop {
        vm.pins.truncate(roots);
        vm.pin(accumulator.clone());
        let Some(value) = vm.iter_step(record)? else {
            return Ok(accumulator);
        };
        accumulator = match vm.call(
            call.arg(0),
            JsValue::Undefined,
            vec![accumulator, value, JsValue::Number(counter)],
        ) {
            Ok(value) => value,
            Err(error) => return Err(close_error(vm, record, error)),
        };
        counter += 1.0;
        vm.charge(1)?;
    }
}
