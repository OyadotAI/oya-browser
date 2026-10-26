/** Native integer callbacks validate primitive values before truncation and use real fixed-width arithmetic. */
use super::*;
/** Safe-number bounds are the primary helper's deliberate clamping bounds for 64-bit values. */
const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
/** Float endpoints for int64/uint64 are exclusive because their maxima round up in binary64. */
pub(super) fn in_range(value: f64, signed: bool, bits: u32) -> bool {
    if !value.is_finite() {
        return false;
    }
    match (signed, bits) {
        (true, 32) => (i32::MIN as f64..=i32::MAX as f64).contains(&value),
        (false, 32) => (0.0..=u32::MAX as f64).contains(&value),
        (true, 64) => value >= i64::MIN as f64 && value < -(i64::MIN as f64),
        (false, 64) => value >= 0.0 && value < 18_446_744_073_709_551_616.0,
        _ => false,
    }
}
/** The primitive type test never calls Number(), valueOf or Symbol.toPrimitive. */
fn number_argument(vm: &mut Vm, value: JsValue, index: usize, signed: bool) -> Res<u64> {
    let ordinal = if index == 0 { "first" } else { "second" };
    let capital = if index == 0 { "First" } else { "Second" };
    let kind = if signed { "int64_t" } else { "uint64_t" };
    let JsValue::Number(value) = value else {
        return Err(error(
            vm,
            &format!("Did not get a number as {ordinal} parameter."),
        ));
    };
    if !in_range(value, signed, 64) {
        return Err(error(
            vm,
            &format!("{capital} number is out of {kind} range."),
        ));
    }
    Ok(if signed {
        (value as i64) as u64
    } else {
        value as u64
    })
}
/** BigInt native APIs consume the low 64 bits, including mathematical modulo for negative wide values. */
fn bigint_argument(vm: &mut Vm, value: JsValue, index: usize) -> Res<u64> {
    let ordinal = if index == 0 { "first" } else { "second" };
    value
        .bigint_value()
        .map(|value| value.low_u64())
        .ok_or_else(|| error(vm, &format!("Did not get a BigInt as {ordinal} parameter.")))
}
/** Native count and argument order remain observable even when a later type/range check throws. */
fn sum(vm: &mut Vm, call: Call, signed: bool, bigint: bool) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    if call.args.len() != 2 {
        return Err(error(vm, "Invalid number of arguments, expected two."));
    }
    let mut values = [0; 2];
    for index in 0..2 {
        values[index] = if bigint {
            bigint_argument(vm, call.arg(index), index)?
        } else {
            number_argument(vm, call.arg(index), index, signed)?
        };
    }
    let result = values[0].wrapping_add(values[1]);
    Ok(if bigint {
        JsValue::BigInt(if signed {
            (result as i64) as i128
        } else {
            result as i128
        })
    } else {
        JsValue::Number(if signed {
            (result as i64) as f64
        } else {
            result as f64
        })
    })
}
pub(super) fn sum_signed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    sum(vm, call, true, false)
}
pub(super) fn sum_unsigned(vm: &mut Vm, call: Call) -> Res<JsValue> {
    sum(vm, call, false, false)
}
pub(super) fn sum_signed_bigint(vm: &mut Vm, call: Call) -> Res<JsValue> {
    sum(vm, call, true, true)
}
pub(super) fn sum_unsigned_bigint(vm: &mut Vm, call: Call) -> Res<JsValue> {
    sum(vm, call, false, true)
}
/** Exact native integer comparison is done after validation; no lossy decimal or JS coercion is involved. */
fn integer(value: f64, signed: bool, bits: u32) -> u64 {
    match (signed, bits) {
        (true, 32) => (value as i32) as u64,
        (false, 32) => (value as u32) as u64,
        (true, 64) => (value as i64) as u64,
        _ => value as u64,
    }
}
/** The slow enforce-range callback defaults nonnumeric slots to zero without coercing them. */
fn enforce(vm: &mut Vm, call: Call, signed: bool, bits: u32) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    let real = if let JsValue::Number(value) = call.arg(0) {
        value
    } else {
        0.0
    };
    if !in_range(real, signed, bits) {
        return Err(error(vm, "Argument out of range."));
    }
    let checked = if let JsValue::Number(value) = call.arg(1) {
        if in_range(value, signed, bits) {
            integer(value, signed, bits)
        } else {
            0
        }
    } else {
        0
    };
    Ok(JsValue::Boolean(integer(real, signed, bits) == checked))
}
/** Slow clamp callback uses primary safe-integer saturation and its real primitive-Boolean in-range switch. */
fn clamp(vm: &mut Vm, call: Call, signed: bool, bits: u32) -> Res<JsValue> {
    state::enter(vm, &call.this)?;
    let real = if let JsValue::Number(value) = call.arg(1) {
        value
    } else {
        0.0
    };
    let checked = if let JsValue::Number(value) = call.arg(2) {
        value
    } else {
        f64::MAX
    };
    let range = call.arg(0) == JsValue::Boolean(true)
        && in_range(real, signed, bits)
        && in_range(checked, signed, bits);
    if !range {
        if real.is_nan() || checked.is_nan() {
            return Ok(JsValue::Number(0.0));
        }
        let (min, max) = match (signed, bits) {
            (true, 32) => (i32::MIN as f64, i32::MAX as f64),
            (false, 32) => (0.0, u32::MAX as f64),
            (true, 64) => (-MAX_SAFE, MAX_SAFE),
            _ => (0.0, MAX_SAFE),
        };
        return Ok(JsValue::Number(real.clamp(min, max).trunc() + 0.0));
    }
    let mut checked = checked.trunc() + 0.0;
    if !vm.page.environment.conformance_fuzzing && real != checked && !real.is_nan() {
        if (real - checked).abs() > 0.5 {
            return Err(crate::protocol::EngineError::new(
                "fast_api_check_failed",
                "FastCAPI native clamp invariant failed.",
                "The native test helper's precondition did not hold.",
            )
            .into());
        }
        if checked % 2.0 != 0.0 {
            checked += 1.0
        }
    }
    Ok(JsValue::Number(checked))
}
/** Type-specific entrypoints correspond to real primary callback instantiations, never an ABI counter simulation. */
macro_rules! callbacks {($($name:ident:$function:ident($signed:literal,$bits:literal)),*$(,)?)=>{$(#[doc="Run this native integer specialization through the actual slow callback."]pub(super) fn $name(vm:&mut Vm,call:Call)->Res<JsValue>{$function(vm,call,$signed,$bits)})*};}
callbacks! {enforce_i32:enforce(true,32),enforce_u32:enforce(false,32),enforce_i64:enforce(true,64),enforce_u64:enforce(false,64),clamp_i32:clamp(true,32),clamp_u32:clamp(false,32),clamp_i64:clamp(true,64),clamp_u64:clamp(false,64)}
