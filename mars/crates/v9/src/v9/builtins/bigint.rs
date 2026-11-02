/** BigInt conversions and methods preserve exact bounded arbitrary-width integer values. */
use super::*;
use crate::v9::value::{BigIntParseError, BigInteger};
use crate::v9::vm::convert::Hint;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::CONFIGURABLE;
use crate::v9::vm::realm::{Static, string, symbol};

/** Radix bounds Number and BigInt toString accept. */
const RADIX: std::ops::RangeInclusive<u32> = 2..=36;

/** BigInt and BigInt.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.bigint_prototype = proto;
    let ctor = b.constructor("BigInt", 1, construct, proto, None);
    b.global("BigInt", ctor);
    b.method(ctor, "asUintN", 2, as_uint);
    b.method(ctor, "asIntN", 2, as_int);
    b.method(proto, "toString", 0, to_string);
    b.method(proto, "toLocaleString", 0, to_string);
    b.method(proto, "valueOf", 0, value_of);
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("BigInt")),
        CONFIGURABLE,
    );
}

/** `BigInt(value)`: integers, numeric text and booleans convert; `new BigInt` throws. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_some() {
        return Err(vm.type_error("BigInt is not a constructor"));
    }
    let primitive = vm.to_primitive(&call.arg(0), Hint::Number)?;
    if let Some(number) = primitive.number_value() {
        if number.trunc() != number || !number.is_finite() {
            let text = vm.to_text(&primitive)?;
            return Err(vm.range_error(&format!(
                "The number {text} cannot be converted to a BigInt because it is not an integer"
            )));
        }
        return BigInteger::from_f64(number)
            .map(JsValue::bigint)
            .ok_or_else(|| vm.range_error("Maximum BigInt size exceeded"));
    }
    to_bigint_full(vm, &primitive).map(JsValue::bigint)
}

/** Generic ToBigInt retains every mathematical bit and applies authored primitive conversion once. */
pub(in crate::v9) fn to_bigint_full(vm: &mut Vm, value: &JsValue) -> Res<BigInteger> {
    match vm.to_primitive(value, Hint::Number)? {
        JsValue::BigInt(value) => Ok(BigInteger::from_i128(value)),
        JsValue::BigIntWide(value) => Ok(value),
        JsValue::Boolean(flag) => Ok(BigInteger::from_i128(i128::from(flag))),
        JsValue::String(text) => {
            let text = text.lossy();
            vm.charge(BigInteger::parse_work(&text))?;
            BigInteger::parse(&text).map_err(|error| match error {
                BigIntParseError::Syntax => {
                    vm.syntax_error(&format!("Cannot convert {text} to a BigInt"))
                }
                BigIntParseError::Limit => vm.range_error("Maximum BigInt size exceeded"),
            })
        }
        other => {
            let text = vm.callee_text(&other);
            Err(vm.type_error(&format!("Cannot convert {text} to a BigInt")))
        }
    }
}
/** Integer64 element stores use modulo 2^64; this adapter is never a generic range-checking conversion. */
pub(in crate::v9) fn to_bigint(vm: &mut Vm, value: &JsValue) -> Res<i128> {
    let value = to_bigint_full(vm, value)?;
    Ok(value
        .to_i128()
        .unwrap_or_else(|| i128::from(value.low_u64() as i64)))
}
/** Consumers with an actual signed-i128 domain reject larger mathematical values rather than wrapping them. */
pub(in crate::v9) fn to_bigint_checked(vm: &mut Vm, value: &JsValue) -> Res<i128> {
    to_bigint_full(vm, value)?
        .to_i128()
        .ok_or_else(|| vm.range_error("BigInt is outside the supported range"))
}

/** ToIndex for asIntN/asUintN's bit count. */
fn bits(vm: &mut Vm, call: &Call) -> Res<u64> {
    let bits = vm.to_integer(&call.arg(0))?;
    if !(0.0..=9_007_199_254_740_991.0).contains(&bits) {
        return Err(vm.range_error("Invalid value: not (convertible to) a safe integer"));
    }
    Ok(bits as u64)
}

/** BigInt.asUintN. */
fn as_uint(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bits = bits(vm, &call)?;
    let value = to_bigint_full(vm, &call.arg(1))?;
    value
        .unsigned_n(bits)
        .map(JsValue::bigint)
        .ok_or_else(|| vm.range_error("Maximum BigInt size exceeded"))
}

/** BigInt.asIntN. */
fn as_int(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bits = bits(vm, &call)?;
    let value = to_bigint_full(vm, &call.arg(1))?;
    Ok(JsValue::bigint(value.signed_n(bits)))
}

/** The BigInt a receiver holds. */
fn this_bigint(vm: &mut Vm, call: &Call) -> Res<BigInteger> {
    match &call.this {
        JsValue::BigInt(value) => Ok(BigInteger::from_i128(*value)),
        JsValue::BigIntWide(value) => Ok(value.clone()),
        JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
            Some(Class::BigInt(value)) => Ok(BigInteger::from_i128(*value)),
            Some(Class::BigIntWide(value)) => Ok(value.clone()),
            _ => Err(vm.type_error("BigInt.prototype method called on incompatible receiver")),
        },
        _ => Err(vm.type_error("BigInt.prototype method called on incompatible receiver")),
    }
}

/** BigInt.prototype.toString(radix). */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_bigint(vm, &call)?;
    let radix = match call.arg(0) {
        JsValue::Undefined => 10,
        radix => vm.to_integer(&radix)? as i64,
    };
    let Some(radix) = u32::try_from(radix)
        .ok()
        .filter(|radix| RADIX.contains(radix))
    else {
        return Err(vm.range_error("toString() radix must be between 2 and 36"));
    };
    vm.charge(value.format_work(radix))?;
    text(vm, &value.to_radix(radix))
}

/** BigInt.prototype.valueOf. */
fn value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::bigint(this_bigint(vm, &call)?))
}
