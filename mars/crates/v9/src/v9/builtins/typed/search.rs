/** Typed-array searches use an actual branded view and its captured internal length, not array-like properties. */
use super::*;

/** ValidateTypedArray precedes argument coercion; later resize or detachment affects each indexed read. */
pub(super) fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    if call.args.is_empty() || view.length == 0 {
        return Ok(JsValue::Boolean(false));
    }
    let start = vm.relative_index(&call.arg(1), view.length, 0)?;
    let searched = call.arg(0);
    for index in start..view.length {
        vm.charge(1)?;
        if load(vm, id, index)
            .unwrap_or(JsValue::Undefined)
            .same_value_zero(&searched)
        {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}
