/** TypedArray.from/of construct their actual receiver, validate its native result and preserve source observation order. */
use super::*;
use crate::v9::vm::data::IterState;

/** Constructors may be subclasses, proxies or user functions returning a genuine typed array. */
fn create(vm: &mut Vm, constructor: JsValue, length: usize) -> Res<JsValue> {
    let target = vm.construct(constructor, vec![JsValue::Number(length as f64)], None)?;
    vm.pin(target.clone());
    let view = target
        .object()
        .and_then(|id| view_of(vm, id))
        .filter(|view| view.kind != ElementKind::DataView && !view.invalid);
    if view.is_none_or(|view| view.length < length) {
        return Err(
            vm.type_error("TypedArray constructor returned an invalid or short typed array")
        );
    }
    Ok(target)
}

/** The iterable branch consumes values before construction; the array-like branch reads each value afterwards. */
pub(super) fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_constructor(&call.this) {
        return Err(vm.type_error("TypedArray.from receiver is not a constructor"));
    }
    let mapper = call.arg(1);
    let mapping = !matches!(mapper, JsValue::Undefined);
    if mapping && !vm.is_callable(&mapper) {
        return Err(vm.type_error("TypedArray.from mapper is not callable"));
    }
    let source = call.arg(0);
    let method = vm.get(&source, &Key::Sym(symbol::ITERATOR))?;
    if !method.nullish() {
        return from_iterable(vm, &call, source, method, mapping);
    }
    let source = JsValue::Object(vm.to_object(&source)?);
    vm.pin(source.clone());
    let length = vm.length_of(&source)?;
    let target = create(vm, call.this.clone(), length)?;
    for index in 0..length {
        let value = vm.get(&source, &key(&index.to_string()))?;
        assign(vm, &call, &target, value, index, mapping)?;
        vm.charge(1)?;
    }
    Ok(target)
}

/** The captured iterator method is invoked once; it is never re-read through GetIterator. */
fn from_iterable(
    vm: &mut Vm,
    call: &Call,
    source: JsValue,
    method: JsValue,
    mapping: bool,
) -> Res<JsValue> {
    let iterator = vm.call(method, source, Vec::new())?;
    if !matches!(iterator, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator result is not an object"));
    }
    vm.pin(iterator.clone());
    let next = vm.get(&iterator, &key("next"))?;
    let record = vm.new_iterator(IterState::Record {
        iterator,
        next,
        done: false,
    })?;
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
    let target = create(vm, call.this.clone(), values.len())?;
    for (index, value) in values.into_iter().enumerate() {
        assign(vm, call, &target, value, index, mapping)?;
        vm.charge(1)?;
    }
    Ok(target)
}

/** Mapping occurs immediately before each strict element Set, including after a preceding map detached the target. */
fn assign(
    vm: &mut Vm,
    call: &Call,
    target: &JsValue,
    value: JsValue,
    index: usize,
    mapping: bool,
) -> Res<()> {
    let value = if mapping {
        vm.call(
            call.arg(1),
            call.arg(2),
            vec![value, JsValue::Number(index as f64)],
        )?
    } else {
        value
    };
    vm.set(target, key(&index.to_string()), value, true)
}

/** of always performs Construct(C, length), then numeric element sets without consulting a prototype or name. */
pub(super) fn of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_constructor(&call.this) {
        return Err(vm.type_error("TypedArray.of receiver is not a constructor"));
    }
    let target = create(vm, call.this.clone(), call.args.len())?;
    for (index, value) in call.args.iter().enumerate() {
        vm.set(&target, key(&index.to_string()), value.clone(), true)?;
        vm.charge(1)?;
    }
    Ok(target)
}
