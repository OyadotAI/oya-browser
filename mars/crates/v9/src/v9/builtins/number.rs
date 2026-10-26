/** Number and Boolean: constructors, constants and formatting methods. */
use super::*;
use crate::v9::value::{number_to_string, radix_string};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{NativeFn, realm};

mod format;

/** Number exposes the same parser function identities as the corresponding global functions. */
pub(super) fn install_parser(b: &mut Builder, name: &'static str, function: ObjectId) {
    let ctor = b
        .globals
        .iter()
        .find_map(|(name, value)| match (*name, value) {
            ("Number", JsValue::Object(id)) => Some(*id),
            _ => None,
        });
    if let Some(ctor) = ctor {
        b.value(
            ctor,
            name,
            JsValue::Object(function),
            crate::v9::vm::heap::HIDDEN_ATTRS,
        );
    }
}

/** Number, Number.prototype, Boolean and Boolean.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.number_prototype = proto;
    let ctor = b.constructor("Number", 1, construct, proto, None);
    b.global("Number", ctor);
    for (name, value) in [
        ("EPSILON", f64::EPSILON),
        ("MAX_SAFE_INTEGER", 9_007_199_254_740_991.0),
        ("MIN_SAFE_INTEGER", -9_007_199_254_740_991.0),
        ("MAX_VALUE", f64::MAX),
        ("MIN_VALUE", 5e-324),
        ("NaN", f64::NAN),
        ("POSITIVE_INFINITY", f64::INFINITY),
        ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
    ] {
        b.value(ctor, name, JsValue::Number(value), 0);
    }
    let statics: [(&'static str, NativeFn); 4] = [
        ("isFinite", is_finite),
        ("isInteger", is_integer),
        ("isNaN", is_nan),
        ("isSafeInteger", is_safe_integer),
    ];
    for (name, function) in statics {
        b.method(ctor, name, 1, function);
    }
    let methods: [(&'static str, u32, NativeFn); 6] = [
        ("toFixed", 1, to_fixed),
        ("toPrecision", 1, to_precision),
        ("toExponential", 1, to_exponential),
        ("toString", 1, to_string),
        ("toLocaleString", 0, to_locale_string),
        ("valueOf", 0, value_of),
    ];
    for (name, length, function) in methods {
        b.method(proto, name, length, function);
    }
    let boolean_proto = b.object(Some(b.ids.object_prototype));
    b.ids.boolean_prototype = boolean_proto;
    let boolean = b.constructor("Boolean", 1, construct_boolean, boolean_proto, None);
    b.global("Boolean", boolean);
    b.method(boolean_proto, "toString", 0, boolean_to_string);
    b.method(boolean_proto, "valueOf", 0, boolean_value_of);
}

/** `Number(value)` and `new Number(value)`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    // Number(value) is ToNumeric then a BigInt becomes its nearest Number, as specified.
    let number = if call.args.is_empty() {
        0.0
    } else {
        match vm.to_numeric(&call.arg(0))? {
            JsValue::BigInt(big) => big as f64,
            JsValue::BigIntWide(big) => big.to_f64(),
            numeric => vm.to_number(&numeric)?,
        }
    };
    match &call.new_target {
        None => Ok(JsValue::Number(number)),
        Some(target) => {
            let proto = vm.prototype_for(target, realm().ids.number_prototype)?;
            Ok(JsValue::Object(
                vm.alloc(Object::new(Class::Number(number), Some(proto)))?,
            ))
        }
    }
}

/** Number.isFinite. */
fn is_finite(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(
        matches!(call.arg(0), JsValue::Number(n) if n.is_finite()),
    ))
}
/** Number.isInteger. */
fn is_integer(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(
        matches!(call.arg(0), JsValue::Number(n) if n.is_finite() && n.fract() == 0.0),
    ))
}
/** Number.isNaN. */
fn is_nan(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(
        matches!(call.arg(0), JsValue::Number(n) if n.is_nan()),
    ))
}
/** Number.isSafeInteger. */
fn is_safe_integer(_: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(
        matches!(call.arg(0), JsValue::Number(n) if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0),
    ))
}

/** The number a receiver holds. */
fn this_number(vm: &mut Vm, call: &Call) -> Res<f64> {
    match &call.this {
        JsValue::Number(value) => Ok(*value),
        // Number.prototype is itself a Number object whose [[NumberData]] is +0.
        JsValue::Object(id) if *id == realm().ids.number_prototype => Ok(0.0),
        JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
            Some(Class::Number(value)) => Ok(*value),
            _ => Err(vm.type_error("Number.prototype method called on incompatible receiver")),
        },
        _ => Err(vm.type_error("Number.prototype method called on incompatible receiver")),
    }
}

/** A fraction-digit or precision argument in a range. */
fn digits(vm: &mut Vm, call: &Call, low: f64, high: f64, name: &str) -> Res<usize> {
    let value = vm.to_integer(&call.arg(0))?;
    digit_range(vm, value, low, high, name)
}

/** Nonfinite exponential and precision receivers still convert their digit argument before bypassing its range check. */
fn digit_range(vm: &mut Vm, value: f64, low: f64, high: f64, name: &str) -> Res<usize> {
    if !(low..=high).contains(&value) {
        return Err(vm.range_error(&format!(
            "{name}() argument must be between {low} and {high}"
        )));
    }
    Ok(value as usize)
}

/** Fixed notation with JavaScript's round-half-up on exact ties. */
pub(in crate::v9) fn fixed(value: f64, fraction: usize) -> String {
    format::fixed(value, fraction)
}

/** Number.prototype.toFixed. */
fn to_fixed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_number(vm, &call)?;
    let fraction = digits(vm, &call, 0.0, 100.0, "toFixed")?;
    if !value.is_finite() || value.abs() >= 1e21 {
        return text(vm, &number_to_string(value));
    }
    let mut result = fixed(value, fraction);
    if value == 0.0 {
        result = result.trim_start_matches('-').to_string();
    }
    text(vm, &result)
}

/** JavaScript exponent text from Rust's `1.5e3` form. */
fn exponent_text(rust: &str) -> String {
    match rust.split_once('e') {
        Some((mantissa, exponent)) => {
            let sign = if exponent.starts_with('-') { "-" } else { "+" };
            format!("{mantissa}e{sign}{}", exponent.trim_start_matches('-'))
        }
        None => rust.to_string(),
    }
}

/** Number.prototype.toExponential. */
fn to_exponential(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_number(vm, &call)?;
    let fraction = vm.to_integer(&call.arg(0))?;
    if !value.is_finite() {
        return text(vm, &number_to_string(value));
    }
    let rust = if call.arg(0) == JsValue::Undefined {
        format!("{value:e}")
    } else {
        let fraction = digit_range(vm, fraction, 0.0, 100.0, "toExponential")?;
        format!("{value:.fraction$e}")
    };
    text(vm, &exponent_text(&rust))
}

/** Number.prototype.toPrecision. */
fn to_precision(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_number(vm, &call)?;
    if call.arg(0) == JsValue::Undefined {
        return text(vm, &number_to_string(value));
    }
    let precision = vm.to_integer(&call.arg(0))?;
    if !value.is_finite() {
        return text(vm, &number_to_string(value));
    }
    let precision = digit_range(vm, precision, 1.0, 100.0, "toPrecision")?;
    if value == 0.0 {
        let zeros = if precision > 1 {
            format!("0.{}", "0".repeat(precision - 1))
        } else {
            "0".into()
        };
        return text(vm, &zeros);
    }
    let rust = format!("{:.*e}", precision - 1, value);
    let exponent: i32 = rust
        .split_once('e')
        .and_then(|(_, e)| e.parse().ok())
        .unwrap_or(0);
    let result = if exponent < -6 || exponent >= precision as i32 {
        exponent_text(&rust)
    } else {
        let fraction = (precision as i32 - 1 - exponent).max(0) as usize;
        fixed(value, fraction)
    };
    text(vm, &result)
}

/** Number.prototype.toString(radix). */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_number(vm, &call)?;
    let radix = if call.arg(0) == JsValue::Undefined {
        10.0
    } else {
        vm.to_integer(&call.arg(0))?
    };
    if !(2.0..=36.0).contains(&radix) {
        return Err(vm.range_error("toString() radix must be between 2 and 36"));
    }
    text(vm, &radix_string(value, radix as u32))
}

/** Number.prototype.toLocaleString: en-US grouping, up to three fraction digits. */
fn to_locale_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_number(vm, &call)?;
    text(vm, &locale_number(value))
}

/** en-US number formatting. */
pub(in crate::v9) fn locale_number(value: f64) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".into()
        } else if value > 0.0 {
            "∞".into()
        } else {
            "-∞".into()
        };
    }
    let formatted = fixed(value.abs(), 3);
    let (integer, fraction) = formatted.split_once('.').unwrap_or((&formatted, ""));
    let fraction = fraction.trim_end_matches('0');
    let mut grouped = String::new();
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let sign = if value < 0.0 { "-" } else { "" };
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

/** Number.prototype.valueOf. */
fn value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    this_number(vm, &call).map(JsValue::Number)
}

/** `Boolean(value)` and `new Boolean(value)`. */
fn construct_boolean(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0).truthy();
    match &call.new_target {
        None => Ok(JsValue::Boolean(value)),
        Some(target) => {
            let proto = vm.prototype_for(target, realm().ids.boolean_prototype)?;
            Ok(JsValue::Object(
                vm.alloc(Object::new(Class::Boolean(value), Some(proto)))?,
            ))
        }
    }
}

/** The boolean a receiver holds. */
fn this_boolean(vm: &mut Vm, call: &Call) -> Res<bool> {
    match &call.this {
        JsValue::Boolean(value) => Ok(*value),
        // Boolean.prototype is itself a Boolean object whose [[BooleanData]] is false.
        JsValue::Object(id) if *id == realm().ids.boolean_prototype => Ok(false),
        JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
            Some(Class::Boolean(value)) => Ok(*value),
            _ => Err(vm.type_error("Boolean.prototype method called on incompatible receiver")),
        },
        _ => Err(vm.type_error("Boolean.prototype method called on incompatible receiver")),
    }
}

/** Boolean.prototype.toString. */
fn boolean_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = this_boolean(vm, &call)?;
    text(vm, if value { "true" } else { "false" })
}

/** Boolean.prototype.valueOf. */
fn boolean_value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    this_boolean(vm, &call).map(JsValue::Boolean)
}
