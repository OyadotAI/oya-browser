/** Array.from/of honor generic constructors and create data properties while observing iterable mutations between callbacks. */
use super::*;
use crate::v9::vm::property::Descriptor;

/** Constructor selection distinguishes an inaccessible intrinsic result from an authored constructor's observable target. */
fn construct(vm: &mut Vm, constructor: &JsValue, length: Option<usize>) -> Res<(JsValue, bool)> {
    let canonical = constructor.object() == Some(crate::v9::vm::realm::realm().ids.array);
    if builtin_subclassing(vm) && vm.is_constructor(constructor) {
        let args = length
            .map(|length| vec![JsValue::Number(length as f64)])
            .unwrap_or_default();
        let target = vm.construct(constructor.clone(), args, None)?;
        vm.pin(target.clone());
        return Ok((target, canonical && length.is_none()));
    }
    let target = vm.new_array_length(length.unwrap_or(0))?;
    Ok((JsValue::Object(target), length.is_none()))
}

/** Mappers are validated before source getters, and captured iterator methods are not re-read after construction. */
pub(super) fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mapping = !matches!(call.arg(1), JsValue::Undefined);
    if mapping && !vm.is_callable(&call.arg(1)) {
        return Err(vm.type_error("Array.from mapper must be callable"));
    }
    let source = call.arg(0);
    let method = vm.get(&source, &Key::Sym(symbol::ITERATOR))?;
    if !method.nullish() {
        vm.pin(method.clone());
        if !vm.is_callable(&method) {
            return Err(vm.type_error("Array.from iterator method is not callable"));
        }
        return iterable(vm, &call, source, method, mapping);
    }
    let source = JsValue::Object(vm.to_object(&source)?);
    vm.pin(source.clone());
    let length = vm.length_of(&source)?;
    let (target, _) = construct(vm, &call.this, Some(length))?;
    let roots = vm.pins.len();
    for index in 0..length {
        let value = vm.get(&source, &index_key(index)?)?;
        let value = map(vm, &call, value, index, mapping)?;
        put(vm, &target, index, value, false)?;
        vm.charge(1)?;
        vm.pins.truncate(roots);
    }
    vm.set(&target, key("length"), JsValue::Number(length as f64), true)?;
    Ok(target)
}

/** Native iterator stepping is retained only when the actual captured next identity matches its immutable intrinsic. */
fn iterable(
    vm: &mut Vm,
    call: &Call,
    source: JsValue,
    method: JsValue,
    mapping: bool,
) -> Res<JsValue> {
    let (target, fast) = construct(vm, &call.this, None)?;
    let record = vm.iterator_from_method(&source, method)?;
    let mut index = 0usize;
    let roots = vm.pins.len();
    while let Some(value) = vm.iter_step(record)? {
        let operation = map(vm, call, value, index, mapping)
            .and_then(|value| put(vm, &target, index, value, fast));
        if let Err(error) = operation {
            return Err(super::super::grouping::close_error(vm, record, error));
        }
        index += 1;
        vm.charge(1)?;
        vm.pins.truncate(roots);
    }
    vm.set(&target, key("length"), JsValue::Number(index as f64), true)?;
    Ok(target)
}

/** Callback invocation uses undefined this unless an explicit mapper receiver was supplied. */
fn map(vm: &mut Vm, call: &Call, value: JsValue, index: usize, mapping: bool) -> Res<JsValue> {
    if mapping {
        vm.call(
            call.arg(1),
            call.arg(2),
            vec![value, JsValue::Number(index as f64)],
        )
    } else {
        Ok(value)
    }
}

/** Only fresh, inaccessible intrinsic arrays bypass descriptor machinery; authored targets always observe DefineOwnProperty. */
fn put(vm: &mut Vm, target: &JsValue, index: usize, value: JsValue, fast: bool) -> Res<()> {
    vm.pin(value.clone());
    if fast {
        return vm.array_append(target, value);
    }
    let id = target
        .object()
        .ok_or_else(|| vm.type_error("Array constructor did not return an object"))?;
    if !vm.define(id, index_key(index)?, Descriptor::data(value))? {
        return Err(vm.type_error("Cannot define array element"));
    }
    Ok(())
}

/** of constructs with its exact argument count and sets length only after every indexed property succeeds. */
pub(super) fn of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (target, _) = construct(vm, &call.this, Some(call.args.len()))?;
    for (index, value) in call.args.iter().enumerate() {
        put(vm, &target, index, value.clone(), false)?;
        vm.charge(1)?;
    }
    vm.set(
        &target,
        key("length"),
        JsValue::Number(call.args.len() as f64),
        true,
    )?;
    Ok(target)
}
