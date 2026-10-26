/** Math: the constants and functions; random draws from a seeded, serialized generator. */
use super::*;
use crate::v9::vm::heap::CONFIGURABLE;
use crate::v9::vm::realm::{NativeFn, Static, string, symbol};

mod exact_sum;
mod stable;
mod sum;

/** The Math namespace. */
pub(super) fn install(b: &mut Builder) {
    let math = b.object(Some(b.ids.object_prototype));
    b.global("Math", math);
    b.put(
        math,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Math")),
        CONFIGURABLE,
    );
    for (name, value) in [
        ("E", std::f64::consts::E),
        ("LN10", std::f64::consts::LN_10),
        ("LN2", std::f64::consts::LN_2),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("PI", std::f64::consts::PI),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
        ("SQRT2", std::f64::consts::SQRT_2),
    ] {
        b.value(math, name, JsValue::Number(value), 0);
    }
    let unary: [(&'static str, NativeFn); 26] = [
        ("abs", abs),
        ("acos", acos),
        ("acosh", acosh),
        ("asin", asin),
        ("asinh", asinh),
        ("atan", atan),
        ("atanh", atanh),
        ("cbrt", cbrt),
        ("ceil", ceil),
        ("clz32", clz32),
        ("cos", cos),
        ("cosh", cosh),
        ("exp", exp),
        ("expm1", expm1),
        ("floor", floor),
        ("fround", fround),
        ("log", log),
        ("log1p", log1p),
        ("log10", log10),
        ("log2", log2),
        ("round", round),
        ("sign", sign),
        ("sin", sin),
        ("sinh", sinh),
        ("sqrt", sqrt),
        ("tan", tan),
    ];
    for (name, function) in unary {
        b.method(math, name, 1, function);
    }
    b.method(math, "f16round", 1, f16round);
    b.method(math, "tanh", 1, tanh);
    b.method(math, "trunc", 1, trunc);
    b.method(math, "atan2", 2, atan2);
    b.method(math, "pow", 2, pow);
    b.method(math, "imul", 2, imul);
    b.method(math, "max", 2, max);
    b.method(math, "min", 2, min);
    b.method(math, "hypot", 2, hypot);
    b.method(math, "random", 0, random);
    b.method(math, "sumPrecise", 1, sum::sum_precise);
}

/** Define a unary Math function over f64. */
macro_rules! unary {
    ($($name:ident => $body:expr;)*) => {
        $(
            /** A Math function of one number. */
            fn $name(vm: &mut Vm, call: Call) -> Res<JsValue> {
                let x = arg_number(vm, &call, 0)?;
                let f: fn(f64) -> f64 = $body;
                Ok(JsValue::Number(f(x)))
            }
        )*
    };
}

unary! {
    abs => f64::abs;
    acos => f64::acos;
    acosh => f64::acosh;
    asin => f64::asin;
    asinh => f64::asinh;
    atan => f64::atan;
    atanh => stable::atanh;
    cbrt => f64::cbrt;
    ceil => f64::ceil;
    cos => f64::cos;
    cosh => f64::cosh;
    exp => f64::exp;
    expm1 => f64::exp_m1;
    floor => f64::floor;
    fround => |x| f64::from(x as f32);
    log => f64::ln;
    log1p => f64::ln_1p;
    log10 => f64::log10;
    log2 => f64::log2;
    round => stable::round;
    sign => |x| if x.is_nan() || x == 0.0 { x } else { x.signum() };
    sin => f64::sin;
    sinh => f64::sinh;
    sqrt => f64::sqrt;
    tan => f64::tan;
    tanh => f64::tanh;
    trunc => f64::trunc;
    clz32 => |x| f64::from(crate::v9::value::to_uint32(x).leading_zeros());
}

/** Math.atan2. */
fn atan2(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let y = arg_number(vm, &call, 0)?;
    let x = arg_number(vm, &call, 1)?;
    Ok(JsValue::Number(y.atan2(x)))
}

/** Math.pow. */
fn pow(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let base = arg_number(vm, &call, 0)?;
    let exponent = arg_number(vm, &call, 1)?;
    Ok(JsValue::Number(crate::v9::vm::run::js_power(
        base, exponent,
    )))
}

/** Math.imul. */
fn imul(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = crate::v9::value::to_int32(arg_number(vm, &call, 0)?);
    let b = crate::v9::value::to_int32(arg_number(vm, &call, 1)?);
    Ok(JsValue::Number(f64::from(a.wrapping_mul(b))))
}

/** Math.max. */
fn max(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut result = f64::NEG_INFINITY;
    for value in call.args {
        let number = vm.to_number(value)?;
        if number.is_nan() || result.is_nan() {
            result = f64::NAN;
        } else if number > result || number == 0.0 && result == 0.0 && result.is_sign_negative() {
            result = number;
        }
    }
    Ok(JsValue::Number(result))
}

/** Math.min. */
fn min(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut result = f64::INFINITY;
    for value in call.args {
        let number = vm.to_number(value)?;
        if number.is_nan() || result.is_nan() {
            result = f64::NAN;
        } else if number < result || number == 0.0 && result == 0.0 && number.is_sign_negative() {
            result = number;
        }
    }
    Ok(JsValue::Number(result))
}

/** Scaled compensated summation avoids overflow and underflow while preserving conversion order. */
fn hypot(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let numbers = call
        .args
        .iter()
        .map(|value| vm.to_number(value))
        .collect::<Res<Vec<_>>>()?;
    Ok(JsValue::Number(stable::hypot(&numbers)))
}

/** Math.random: xorshift64* over state the page serializes, seeded by the host. */
fn random(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let mut state = vm.page.state.random;
    if state == 0 {
        state = RANDOM_FALLBACK_SEED;
    }
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    vm.page.state.random = state;
    let bits = state.wrapping_mul(RANDOM_MULTIPLIER) >> RANDOM_SHIFT;
    Ok(JsValue::Number(bits as f64 / RANDOM_SCALE))
}

/** Seed used when the host supplied zero. */
const RANDOM_FALLBACK_SEED: u64 = 0x9e37_79b9_7f4a_7c15;
/** xorshift64* output multiplier. */
const RANDOM_MULTIPLIER: u64 = 0x2545_f491_4f6c_dd1d;
/** Keep 53 bits for an exact double in [0, 1). */
const RANDOM_SHIFT: u32 = 11;
/** 2^53. */
const RANDOM_SCALE: f64 = 9_007_199_254_740_992.0;

/** Round directly from binary64 to binary16, avoiding a second rounding through binary32. */
fn f16round(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let number = arg_number(vm, &call, 0)?;
    Ok(JsValue::Number(crate::v9::value::decode_f16(
        crate::v9::value::encode_f16(number),
    )))
}
