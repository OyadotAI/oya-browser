/** Real slow callbacks preserve native primitive-only numeric conversion and explicit missing ABI capabilities. */
use super::*;
/** Slow AddAll ignores nonnumeric integer slots and uses NaN for either missing floating-point slot. */
pub(super) fn add_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    let mut sum = 0.0;
    for index in 0..6 {
        if let Some(value) = call.arg(index).number_value() {
            sum += match index {
                0 => crate::v9::value::to_int32(value) as f64,
                1 => crate::v9::value::to_uint32(value) as f64,
                2 | 3 => (value as i64) as f64,
                _ => value,
            };
        } else if index >= 4 {
            sum += f64::NAN
        }
    }
    Ok(JsValue::Number(sum))
}
/** Integer overload fallback accepts the six slots, and the final pair only when both extra slots exist. */
pub(super) fn add_ints(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    let mut sum = 0.0;
    for index in 0..6 {
        if let Some(value) = call.arg(index).number_value() {
            sum += if index < 3 {
                crate::v9::value::to_int32(value) as f64
            } else {
                crate::v9::value::to_uint32(value) as f64
            };
        }
    }
    if call.args.len() > 7
        && call.arg(6).number_value().is_some()
        && call.arg(7).number_value().is_some()
    {
        for index in [6, 7] {
            if let Some(value) = call.arg(index).number_value() {
                sum += crate::v9::value::to_uint32(value) as f64;
            }
        }
    }
    Ok(JsValue::Number(sum))
}
/** Fuzzing deliberately returns a null pointer in the primary native helper; no fake external object is created. */
pub(super) fn pointer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    if vm.page.environment.conformance_fuzzing {
        Ok(JsValue::Null)
    } else {
        Err(vm.unsupported_error("FastCAPI external native pointer values"))
    }
}
pub(super) fn null_pointer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    Ok(JsValue::Null)
}
/** Without a native External representation only genuine null pointers can cross this callback boundary. */
pub(super) fn compare_pointers(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    if call.args.len() != 2 {
        return Err(error(vm, "Invalid number of arguments, expected two."));
    }
    if call.arg(0) != JsValue::Null {
        return Err(error(vm, "Did not get an external as first parameter."));
    }
    if call.arg(1) != JsValue::Null {
        return Err(error(vm, "Did not get an external as second parameter."));
    }
    Ok(JsValue::Boolean(true))
}
pub(super) fn pass_pointer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    if call.args.len() != 1 {
        return Err(error(vm, "Invalid number of arguments, expected one."));
    }
    if call.arg(0) == JsValue::Null {
        Ok(JsValue::Null)
    } else {
        Err(error(vm, "Did not get an external."))
    }
}
pub(super) fn assert_external(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::receiver(vm, &call.this)?;
    Err(error(vm, "Did not get an external."))
}
/** This is the real nonoptimized callback's exception; no fast call is claimed or counted. */
pub(super) fn throw(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    Err(error(vm, "Exception from slow callback"))
}
/** Pinned d8 deliberately throws when the memory-only fast callback reaches its slow fallback. */
pub(super) fn memory(vm: &mut Vm, call: Call) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    Err(error(vm, "should be unreachable from wasm"))
}
