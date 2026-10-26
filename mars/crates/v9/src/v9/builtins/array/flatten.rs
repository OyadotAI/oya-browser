/** Flat and flatMap interleave source reads, callbacks and species definitions without a temporary array. */
use super::*;

/** Nesting shares the language recursion bound so cyclic infinity-depth inputs fail catchably. */
fn nested(
    vm: &mut Vm,
    result: ObjectId,
    source: &JsValue,
    depth: f64,
    next: &mut usize,
) -> Res<()> {
    vm.deeper(|vm| {
        let length = vm.length_of(source)?;
        elements(vm, result, source, length, depth, None, next)
    })
}

/** A mapper runs only at the outer level and its returned array is flattened before the next call. */
fn elements(
    vm: &mut Vm,
    result: ObjectId,
    source: &JsValue,
    length: usize,
    depth: f64,
    mapper: Option<(&JsValue, &JsValue)>,
    next: &mut usize,
) -> Res<()> {
    for index in 0..length {
        vm.charge(1)?;
        if !has_index(vm, source, index)? {
            continue;
        }
        let mut value = get_index(vm, source, index)?;
        if let Some((function, receiver)) = mapper {
            value = vm.call(
                function.clone(),
                receiver.clone(),
                vec![value, JsValue::Number(index as f64), source.clone()],
            )?;
        }
        vm.pin(value.clone());
        if depth > 0.0 && is_array_checked(vm, &value)? {
            nested(vm, result, &value, depth - 1.0, next)?;
        } else {
            safe_growth(vm, *next, 1)?;
            species::define(vm, result, *next, value)?;
            *next += 1;
        }
    }
    Ok(())
}

/** flat reads length and depth before creating the species result and copying indexed properties. */
pub(super) fn flat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (source, length) = receiver(vm, &call, "Array.prototype.flat")?;
    let depth = if call.arg(0) == JsValue::Undefined {
        1.0
    } else {
        vm.to_integer(&call.arg(0))?
    };
    let result = species::create(vm, &source, 0)?;
    elements(vm, result, &source, length, depth, None, &mut 0)?;
    Ok(JsValue::Object(result))
}

/** flatMap validates its callback once and creates exactly one species result. */
pub(super) fn flat_map(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (source, length) = receiver(vm, &call, "Array.prototype.flatMap")?;
    let mapper = callback(vm, &call, "Array.prototype.flatMap")?;
    let receiver = call.arg(1);
    let result = species::create(vm, &source, 0)?;
    elements(
        vm,
        result,
        &source,
        length,
        1.0,
        Some((&mapper, &receiver)),
        &mut 0,
    )?;
    Ok(JsValue::Object(result))
}
