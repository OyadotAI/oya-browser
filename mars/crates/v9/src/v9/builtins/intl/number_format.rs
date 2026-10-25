/**
 * `Intl.NumberFormat` and `Intl.PluralRules`, ported from ECMA-402 and V8's js-number-format.cc:
 * options are read and checked in the spec's order (InitializeNumberFormat,
 * SetNumberFormatUnitOptions, SetNumberFormatDigitOptions), values are rounded exactly in
 * `decimal`, and the en-US patterns (sign, currency, percent, units, compact, scientific and
 * engineering) are assembled as typed parts so `format`, `formatToParts`, `formatRange` and
 * `formatRangeToParts` agree. Locale data is the small embedded en table below.
 */
use super::decimal::{self, Dec, Num, Raw, raw_fixed, raw_precision, unsigned_mode};
use super::display_names::resolve_locale;
use super::locale::canonical_list;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData, IntlData};
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, Key, Object};
use crate::v9::vm::realm::{Builder, Static, realm, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/** The native index of the bound `format` function. */
static BOUND_FORMAT: OnceLock<u32> = OnceLock::new();

/** Rounding increments ECMA-402 allows. */
const INCREMENTS: [u32; 15] = [
    1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
];
/** Rounding mode names. */
const MODES: [&str; 9] = [
    "ceil",
    "floor",
    "expand",
    "trunc",
    "halfCeil",
    "halfFloor",
    "halfExpand",
    "halfTrunc",
    "halfEven",
];
/** Units ECMA-402 sanctions, with en short, narrow and long (one, other) names. */
const UNITS: [(&str, &str, &str, &str, &str); 45] = [
    ("acre", " ac", "ac", "acre", "acres"),
    ("bit", " bit", "bit", "bit", "bits"),
    ("byte", " byte", "B", "byte", "bytes"),
    ("celsius", "°C", "°C", "degree Celsius", "degrees Celsius"),
    ("centimeter", " cm", "cm", "centimeter", "centimeters"),
    ("day", " days", "d", "day", "days"),
    ("degree", " deg", "°", "degree", "degrees"),
    (
        "fahrenheit",
        "°F",
        "°",
        "degree Fahrenheit",
        "degrees Fahrenheit",
    ),
    (
        "fluid-ounce",
        " fl oz",
        "fl oz",
        "fluid ounce",
        "fluid ounces",
    ),
    ("foot", " ft", "′", "foot", "feet"),
    ("gallon", " gal", "gal", "gallon", "gallons"),
    ("gigabit", " Gb", "Gb", "gigabit", "gigabits"),
    ("gigabyte", " GB", "GB", "gigabyte", "gigabytes"),
    ("gram", " g", "g", "gram", "grams"),
    ("hectare", " ha", "ha", "hectare", "hectares"),
    ("hour", " hr", "h", "hour", "hours"),
    ("inch", " in", "″", "inch", "inches"),
    ("kilobit", " kb", "kb", "kilobit", "kilobits"),
    ("kilobyte", " kB", "kB", "kilobyte", "kilobytes"),
    ("kilogram", " kg", "kg", "kilogram", "kilograms"),
    ("kilometer", " km", "km", "kilometer", "kilometers"),
    ("liter", " L", "L", "liter", "liters"),
    ("megabit", " Mb", "Mb", "megabit", "megabits"),
    ("megabyte", " MB", "MB", "megabyte", "megabytes"),
    ("meter", " m", "m", "meter", "meters"),
    ("microsecond", " μs", "μs", "microsecond", "microseconds"),
    ("mile", " mi", "mi", "mile", "miles"),
    (
        "mile-scandinavian",
        " smi",
        "smi",
        "mile-scandinavian",
        "miles-scandinavian",
    ),
    ("milliliter", " mL", "mL", "milliliter", "milliliters"),
    ("millimeter", " mm", "mm", "millimeter", "millimeters"),
    ("millisecond", " ms", "ms", "millisecond", "milliseconds"),
    ("minute", " min", "m", "minute", "minutes"),
    ("month", " mths", "m", "month", "months"),
    ("nanosecond", " ns", "ns", "nanosecond", "nanoseconds"),
    ("ounce", " oz", "oz", "ounce", "ounces"),
    ("percent", "%", "%", "percent", "percent"),
    ("petabyte", " PB", "PB", "petabyte", "petabytes"),
    ("pound", " lb", "#", "pound", "pounds"),
    ("second", " sec", "s", "second", "seconds"),
    ("stone", " st", "st", "stone", "stones"),
    ("terabit", " Tb", "Tb", "terabit", "terabits"),
    ("terabyte", " TB", "TB", "terabyte", "terabytes"),
    ("week", " wks", "w", "week", "weeks"),
    ("yard", " yd", "yd", "yard", "yards"),
    ("year", " yrs", "y", "year", "years"),
];
/** Short names a unit takes after "per" in a compound (`km/h`). */
const PER_SHORT: [(&str, &str); 3] = [("hour", "h"), ("second", "s"), ("minute", "min")];
/** en currency symbols, narrow symbols and names (one, other). */
const CURRENCIES: [(&str, &str, &str, &str, &str); 12] = [
    ("USD", "$", "$", "US dollar", "US dollars"),
    ("EUR", "€", "€", "euro", "euros"),
    ("GBP", "£", "£", "British pound", "British pounds"),
    ("JPY", "¥", "¥", "Japanese yen", "Japanese yen"),
    ("CAD", "CA$", "$", "Canadian dollar", "Canadian dollars"),
    ("AUD", "A$", "$", "Australian dollar", "Australian dollars"),
    ("CNY", "CN¥", "¥", "Chinese yuan", "Chinese yuan"),
    ("INR", "₹", "₹", "Indian rupee", "Indian rupees"),
    ("KRW", "₩", "₩", "South Korean won", "South Korean won"),
    ("MXN", "MX$", "$", "Mexican peso", "Mexican pesos"),
    ("CHF", "CHF", "CHF", "Swiss franc", "Swiss francs"),
    ("BRL", "R$", "R$", "Brazilian real", "Brazilian reals"),
];
/** Currencies with no minor unit (CLDR). */
const ZERO_DIGIT_CURRENCIES: [&str; 17] = [
    "BIF", "CLP", "DJF", "GNF", "ISK", "JPY", "KMF", "KRW", "PYG", "RWF", "UGX", "UYI", "VND",
    "VUV", "XAF", "XOF", "XPF",
];
/** Currencies with three minor digits (CLDR). */
const THREE_DIGIT_CURRENCIES: [&str; 7] = ["BHD", "IQD", "JOD", "KWD", "LYD", "OMR", "TND"];
/** en compact suffixes per exponent: short, long. */
const COMPACT: [(i64, &str, &str); 4] = [
    (3, "K", " thousand"),
    (6, "M", " million"),
    (9, "B", " billion"),
    (12, "T", " trillion"),
];
/** Largest exponent en compact notation has a name for. */
const COMPACT_MAX: i64 = 12;
/** Digits between group separators. */
const GROUP: usize = 3;
/** The resolved locale when none is requested. */
const DEFAULT_LOCALE: &str = "en-US";

/** Install `Intl.NumberFormat` and `Intl.PluralRules` on `intl`. */
pub(super) fn install(b: &mut Builder, intl: u32) {
    let _ = BOUND_FORMAT.set(b.native("", 1, bound_format));
    let proto = b.object(Some(b.ids.object_prototype));
    let getter = b.function("get format", 0, format_getter);
    b.put(
        proto,
        Key::from("format"),
        Static::Accessor(Some(getter), None),
        CONFIGURABLE,
    );
    b.method(proto, "formatToParts", 1, format_to_parts);
    b.method(proto, "formatRange", 2, format_range);
    b.method(proto, "formatRangeToParts", 2, format_range_to_parts);
    b.method(proto, "resolvedOptions", 0, resolved_options);
    tag(b, proto, "Intl.NumberFormat");
    let ctor = b.constructor("NumberFormat", 0, construct_number, proto, None);
    b.method(ctor, "supportedLocalesOf", 1, super::supported_locales);
    b.value(intl, "NumberFormat", JsValue::Object(ctor), HIDDEN_ATTRS);
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(proto, "select", 1, select);
    b.method(proto, "selectRange", 2, select_range);
    b.method(proto, "resolvedOptions", 0, plural_resolved_options);
    tag(b, proto, "Intl.PluralRules");
    let ctor = b.constructor("PluralRules", 0, construct_plural, proto, None);
    b.method(ctor, "supportedLocalesOf", 1, super::supported_locales);
    b.value(intl, "PluralRules", JsValue::Object(ctor), HIDDEN_ATTRS);
}

/** A configurable `Symbol.toStringTag`. */
fn tag(b: &mut Builder, target: u32, name: &'static str) {
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        CONFIGURABLE,
    );
}

/** CoerceOptionsToObject: undefined is an empty null-prototype object, others ToObject. */
fn coerce_options(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    if value == JsValue::Undefined {
        return Ok(JsValue::Object(vm.alloc(Object::new(Class::Plain, None))?));
    }
    Ok(JsValue::Object(vm.to_object(&value)?))
}

/** GetOption for a string: None when undefined, RangeError outside `allowed` (when given). */
fn get_string(vm: &mut Vm, options: &JsValue, name: &str, allowed: &[&str]) -> Res<Option<String>> {
    let value = vm.get(options, &Key::from(name))?;
    if value == JsValue::Undefined {
        return Ok(None);
    }
    let text = vm.to_text(&value)?;
    if !allowed.is_empty() && !allowed.contains(&text.as_str()) {
        return Err(vm.range_error(&format!("Value {text} out of range for option {name}")));
    }
    Ok(Some(text))
}

/** GetOption with a default, the first allowed value unless `fallback` says otherwise. */
fn choice(
    vm: &mut Vm,
    options: &JsValue,
    name: &str,
    allowed: &[&str],
    fallback: &str,
) -> Res<String> {
    Ok(get_string(vm, options, name, allowed)?.unwrap_or_else(|| fallback.into()))
}

/** DefaultNumberOption: undefined gives `fallback`, else an integer in range or RangeError. */
fn default_number(
    vm: &mut Vm,
    value: &JsValue,
    min: f64,
    max: f64,
    name: &str,
) -> Res<Option<f64>> {
    if *value == JsValue::Undefined {
        return Ok(None);
    }
    let number = vm.to_number(value)?;
    if number.is_nan() || number < min || number > max {
        return Err(vm.range_error(&format!("{name} value is out of range.")));
    }
    Ok(Some(number.floor()))
}

/** GetNumberOption. */
fn number_option(
    vm: &mut Vm,
    options: &JsValue,
    name: &str,
    min: f64,
    max: f64,
    fallback: f64,
) -> Res<f64> {
    let value = vm.get(options, &Key::from(name))?;
    Ok(default_number(vm, &value, min, max, name)?.unwrap_or(fallback))
}

/** Store a number option. */
fn put_num(map: &mut BTreeMap<String, String>, name: &str, value: f64) {
    map.insert(name.into(), format!("{value}"));
}

/** SetNumberFormatDigitOptions, into the kept options. */
fn digit_options(
    vm: &mut Vm,
    options: &JsValue,
    kept: &mut BTreeMap<String, String>,
    mnfd_default: f64,
    mut mxfd_default: f64,
    notation: &str,
) -> Res<()> {
    let mnid = number_option(vm, options, "minimumIntegerDigits", 1.0, 21.0, 1.0)?;
    let mnfd = vm.get(options, &Key::from("minimumFractionDigits"))?;
    let mxfd = vm.get(options, &Key::from("maximumFractionDigits"))?;
    let mnsd = vm.get(options, &Key::from("minimumSignificantDigits"))?;
    let mxsd = vm.get(options, &Key::from("maximumSignificantDigits"))?;
    put_num(kept, "minimumIntegerDigits", mnid);
    let increment = number_option(vm, options, "roundingIncrement", 1.0, 5000.0, 1.0)?;
    if !INCREMENTS.contains(&(increment as u32)) {
        return Err(vm.range_error("roundingIncrement value is out of range."));
    }
    let mode = choice(vm, options, "roundingMode", &MODES, "halfExpand")?;
    let priority = choice(
        vm,
        options,
        "roundingPriority",
        &["auto", "morePrecision", "lessPrecision"],
        "auto",
    )?;
    let tzd = choice(
        vm,
        options,
        "trailingZeroDisplay",
        &["auto", "stripIfInteger"],
        "auto",
    )?;
    if increment != 1.0 {
        mxfd_default = mnfd_default;
    }
    put_num(kept, "roundingIncrement", increment);
    kept.insert("roundingMode".into(), mode);
    kept.insert("trailingZeroDisplay".into(), tzd);
    let has_sd = mnsd != JsValue::Undefined || mxsd != JsValue::Undefined;
    let has_fd = mnfd != JsValue::Undefined || mxfd != JsValue::Undefined;
    let (mut need_sd, mut need_fd) = (true, true);
    if priority == "auto" {
        need_sd = has_sd;
        if need_sd || (!has_fd && notation == "compact") {
            need_fd = false;
        }
    }
    if need_sd {
        let (min, max) = if has_sd {
            let min =
                default_number(vm, &mnsd, 1.0, 21.0, "minimumSignificantDigits")?.unwrap_or(1.0);
            let max =
                default_number(vm, &mxsd, min, 21.0, "maximumSignificantDigits")?.unwrap_or(21.0);
            (min, max)
        } else {
            (1.0, 21.0)
        };
        put_num(kept, "minimumSignificantDigits", min);
        put_num(kept, "maximumSignificantDigits", max);
    }
    if need_fd {
        let (min, max) = if has_fd {
            let min = default_number(vm, &mnfd, 0.0, 100.0, "minimumFractionDigits")?;
            let max = default_number(vm, &mxfd, 0.0, 100.0, "maximumFractionDigits")?;
            match (min, max) {
                (None, Some(max)) => (mnfd_default.min(max), max),
                (Some(min), None) => (min, mxfd_default.max(min)),
                (Some(min), Some(max)) if min > max => {
                    return Err(vm.range_error("maximumFractionDigits value is out of range."));
                }
                (Some(min), Some(max)) => (min, max),
                (None, None) => (mnfd_default, mxfd_default),
            }
        } else {
            (mnfd_default, mxfd_default)
        };
        put_num(kept, "minimumFractionDigits", min);
        put_num(kept, "maximumFractionDigits", max);
    }
    let rounding_type = if !need_sd && !need_fd {
        put_num(kept, "minimumFractionDigits", 0.0);
        put_num(kept, "maximumFractionDigits", 0.0);
        put_num(kept, "minimumSignificantDigits", 1.0);
        put_num(kept, "maximumSignificantDigits", 2.0);
        kept.insert("roundingPriority".into(), "morePrecision".into());
        "morePrecision".to_string()
    } else if priority == "auto" {
        kept.insert("roundingPriority".into(), "auto".into());
        if has_sd {
            "significantDigits"
        } else {
            "fractionDigits"
        }
        .to_string()
    } else {
        kept.insert("roundingPriority".into(), priority.clone());
        priority
    };
    if increment != 1.0 {
        if rounding_type != "fractionDigits" {
            return Err(vm.type_error("roundingIncrement requires fraction digit rounding"));
        }
        if kept["maximumFractionDigits"] != kept["minimumFractionDigits"] {
            return Err(vm.range_error("roundingIncrement requires equal fraction digits"));
        }
    }
    kept.insert("@rounding".into(), rounding_type);
    Ok(())
}

/** IsWellFormedUnitIdentifier: a sanctioned unit or `a-per-b` of two. */
fn well_formed_unit(unit: &str) -> bool {
    let simple = |u: &str| UNITS.iter().any(|row| row.0 == u);
    simple(unit)
        || unit
            .split_once("-per-")
            .is_some_and(|(a, b)| simple(a) && simple(b))
}

/** SetNumberFormatUnitOptions, into the kept options. */
fn unit_options(vm: &mut Vm, options: &JsValue, kept: &mut BTreeMap<String, String>) -> Res<()> {
    let style = choice(
        vm,
        options,
        "style",
        &["decimal", "percent", "currency", "unit"],
        "decimal",
    )?;
    let currency = get_string(vm, options, "currency", &[])?;
    match &currency {
        None if style == "currency" => {
            return Err(vm.type_error("Currency code is required with currency style."));
        }
        Some(code) if !(code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphabetic())) => {
            return Err(vm.range_error(&format!("Invalid currency code : {code}")));
        }
        _ => {}
    }
    let display = choice(
        vm,
        options,
        "currencyDisplay",
        &["code", "symbol", "narrowSymbol", "name"],
        "symbol",
    )?;
    let sign = choice(
        vm,
        options,
        "currencySign",
        &["standard", "accounting"],
        "standard",
    )?;
    let unit = get_string(vm, options, "unit", &[])?;
    match &unit {
        None if style == "unit" => {
            return Err(vm.type_error("Unit is required with unit style."));
        }
        Some(unit) if !well_formed_unit(unit) => {
            return Err(vm.range_error(&format!("Invalid unit argument '{unit}'")));
        }
        _ => {}
    }
    let unit_display = choice(
        vm,
        options,
        "unitDisplay",
        &["short", "narrow", "long"],
        "short",
    )?;
    if style == "currency" {
        kept.insert(
            "currency".into(),
            currency.unwrap_or_default().to_ascii_uppercase(),
        );
        kept.insert("currencyDisplay".into(), display);
        kept.insert("currencySign".into(), sign);
    }
    if style == "unit" {
        kept.insert("unit".into(), unit.unwrap_or_default());
        kept.insert("unitDisplay".into(), unit_display);
    }
    kept.insert("style".into(), style);
    Ok(())
}

/** CurrencyDigits from the embedded CLDR table. */
fn currency_digits(code: &str) -> f64 {
    if ZERO_DIGIT_CURRENCIES.contains(&code) {
        0.0
    } else if THREE_DIGIT_CURRENCIES.contains(&code) {
        3.0
    } else {
        2.0
    }
}

/** The locale and the numbering system option, read first by both constructors. */
fn locale_and_matcher(vm: &mut Vm, call: &Call) -> Res<(String, JsValue)> {
    let list = canonical_list(vm, &call.arg(0))?;
    let options = coerce_options(vm, call.arg(1))?;
    choice(
        vm,
        &options,
        "localeMatcher",
        &["lookup", "best fit"],
        "best fit",
    )?;
    let locale = if list.is_empty() {
        DEFAULT_LOCALE.into()
    } else {
        resolve_locale(&list)
    };
    Ok((locale, options))
}

/** A numbering system type: 3-8 alphanumerics, dash separated. */
fn well_formed_type(text: &str) -> bool {
    text.split('-').all(|part| {
        (3..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/** `new Intl.NumberFormat(locales, options)`, also callable as a function. */
fn construct_number(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (locale, options) = locale_and_matcher(vm, &call)?;
    if let Some(system) = get_string(vm, &options, "numberingSystem", &[])?
        && !well_formed_type(&system)
    {
        return Err(vm.range_error(&format!("Invalid numberingSystem : {system}")));
    }
    let mut kept = BTreeMap::new();
    unit_options(vm, &options, &mut kept)?;
    let style = kept["style"].clone();
    let notation = choice(
        vm,
        &options,
        "notation",
        &["standard", "scientific", "engineering", "compact"],
        "standard",
    )?;
    let (mnfd, mxfd) = if style == "currency" && notation == "standard" {
        let digits = currency_digits(&kept["currency"]);
        (digits, digits)
    } else {
        (0.0, if style == "percent" { 0.0 } else { 3.0 })
    };
    digit_options(vm, &options, &mut kept, mnfd, mxfd, &notation)?;
    let compact = choice(vm, &options, "compactDisplay", &["short", "long"], "short")?;
    if notation == "compact" {
        kept.insert("compactDisplay".into(), compact);
    }
    let grouping_default = if notation == "compact" {
        "min2"
    } else {
        "auto"
    };
    let grouping = grouping_option(vm, &options, grouping_default)?;
    kept.insert("useGrouping".into(), grouping);
    kept.insert("notation".into(), notation);
    let sign = choice(
        vm,
        &options,
        "signDisplay",
        &["auto", "never", "always", "exceptZero", "negative"],
        "auto",
    )?;
    kept.insert("signDisplay".into(), sign);
    allocate(vm, &call, "NumberFormat", locale, kept)
}

/** GetBooleanOrStringNumberFormatOption for `useGrouping`; "false" stands for false. */
fn grouping_option(vm: &mut Vm, options: &JsValue, fallback: &str) -> Res<String> {
    let value = vm.get(options, &Key::from("useGrouping"))?;
    match value {
        JsValue::Undefined => Ok(fallback.into()),
        JsValue::Boolean(true) => Ok("always".into()),
        _ if !value.truthy() => Ok("false".into()),
        _ => {
            let text = vm.to_text(&value)?;
            match text.as_str() {
                "min2" | "auto" | "always" => Ok(text),
                "true" | "false" => Ok(fallback.into()),
                _ => {
                    Err(vm
                        .range_error(&format!("Value {text} out of range for option useGrouping")))
                }
            }
        }
    }
}

/** The formatter object, its prototype from NewTarget (or the constructor's own). */
fn allocate(
    vm: &mut Vm,
    call: &Call,
    kind: &str,
    locale: String,
    options: BTreeMap<String, String>,
) -> Res<JsValue> {
    let fallback = match vm.get(&JsValue::Object(call.callee), &Key::from("prototype"))? {
        JsValue::Object(id) => id,
        _ => realm().ids.object_prototype,
    };
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => fallback,
    };
    let data = IntlData {
        kind: kind.into(),
        locale,
        options,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Intl(Box::new(data)))),
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** The formatter of `kind` behind a value, or a TypeError. */
fn unwrap(vm: &mut Vm, value: &JsValue, kind: &str) -> Res<IntlData> {
    match crate::v9::platform::host_data(vm, value) {
        Some(HostData::Intl(data)) if data.kind == kind => Ok(*data),
        _ => Err(vm.type_error(&format!(
            "Method Intl.{kind} called on incompatible receiver"
        ))),
    }
}

/** The `format` getter: a function bound to this formatter. */
fn format_getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    unwrap(vm, &call.this, "NumberFormat")?;
    let index = *BOUND_FORMAT.get().expect("installed");
    Ok(JsValue::Object(
        vm.native_closure(index, vec![call.this.clone()])?,
    ))
}

/** The bound format function. */
fn bound_format(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = vm
        .slots(call.callee)
        .into_iter()
        .next()
        .unwrap_or(JsValue::Undefined);
    let data = unwrap(vm, &target, "NumberFormat")?;
    let value = to_intl_value(vm, &call.arg(0))?;
    let text: String = format_parts(&data.options, &value)
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    Ok(JsValue::string(&text)?)
}

/** ToIntlMathematicalValue: BigInts and numeric strings exactly, the rest through ToNumber. */
fn to_intl_value(vm: &mut Vm, value: &JsValue) -> Res<Num> {
    let primitive = vm.to_primitive(value, crate::v9::vm::convert::Hint::Number)?;
    match &primitive {
        JsValue::BigInt(big) => {
            let text = big.unsigned_abs().to_string();
            Ok(Num::Fin(
                *big < 0,
                decimal::parse_decimal(&text).unwrap_or_else(Dec::zero),
            ))
        }
        JsValue::String(_) => {
            let text = vm.to_text(&primitive)?;
            match decimal::from_string(&text) {
                Some(num) => Ok(num),
                None => Ok(decimal::from_f64(vm.to_number(&primitive)?)),
            }
        }
        _ => Ok(decimal::from_f64(vm.to_number(&primitive)?)),
    }
}

/** Formatted parts: (type, value). */
type Parts = Vec<(&'static str, String)>;

/** A kept option. */
fn opt<'a>(options: &'a BTreeMap<String, String>, name: &str) -> &'a str {
    options.get(name).map_or("", String::as_str)
}

/** A kept digit count. */
fn num(options: &BTreeMap<String, String>, name: &str) -> usize {
    opt(options, name).parse().unwrap_or(0)
}

/** FormatNumericToString on a magnitude: the rounded value with its digit strings. */
fn numeric_to_string(options: &BTreeMap<String, String>, x: &Dec, negative: bool) -> Raw {
    let mode = unsigned_mode(opt(options, "roundingMode"), negative);
    let increment = num(options, "roundingIncrement").max(1) as u32;
    let s = || {
        raw_precision(
            x,
            num(options, "minimumSignificantDigits"),
            num(options, "maximumSignificantDigits"),
            mode,
        )
    };
    let f = || {
        raw_fixed(
            x,
            num(options, "minimumFractionDigits"),
            num(options, "maximumFractionDigits"),
            increment,
            mode,
        )
    };
    let mut raw = match opt(options, "@rounding") {
        "significantDigits" => s(),
        "fractionDigits" => f(),
        priority => {
            let (sr, fr) = (s(), f());
            let fixed_more = sr.magnitude > fr.magnitude;
            let pick_fixed = if priority == "morePrecision" {
                fixed_more
            } else {
                !fixed_more
            };
            if pick_fixed { fr } else { sr }
        }
    };
    if opt(options, "trailingZeroDisplay") == "stripIfInteger" && raw.value.is_integer() {
        raw.frac.clear();
    }
    let min_int = num(options, "minimumIntegerDigits");
    while raw.int.len() < min_int {
        raw.int.insert(0, '0');
    }
    raw
}

/** ComputeExponentForMagnitude. */
fn exponent_for(options: &BTreeMap<String, String>, magnitude: i64) -> i64 {
    match opt(options, "notation") {
        "scientific" => magnitude,
        "engineering" => magnitude.div_euclid(3) * 3,
        "compact" if magnitude < 3 => 0,
        "compact" => (magnitude.div_euclid(3) * 3).min(COMPACT_MAX),
        _ => 0,
    }
}

/** ComputeExponent: the exponent, re-chosen when rounding reaches the next magnitude. */
fn compute_exponent(options: &BTreeMap<String, String>, x: &Dec, negative: bool) -> i64 {
    if x.is_zero() {
        return 0;
    }
    let magnitude = x.magnitude();
    let exponent = exponent_for(options, magnitude);
    let raw = numeric_to_string(options, &x.shift(-exponent), negative);
    if raw.value.is_zero() || raw.value.magnitude() == magnitude - exponent {
        return exponent;
    }
    exponent_for(options, magnitude + 1)
}

/** Digits with en grouping, as integer and group parts. */
fn integer_parts(options: &BTreeMap<String, String>, int: &str, out: &mut Parts) {
    let group = match opt(options, "useGrouping") {
        "false" => false,
        "min2" => int.len() > GROUP + 1,
        _ => int.len() > GROUP,
    };
    if !group {
        out.push(("integer", int.into()));
        return;
    }
    let head = int.len() % GROUP;
    let mut chunks: Vec<&str> = Vec::new();
    if head > 0 {
        chunks.push(&int[..head]);
    }
    chunks.extend(
        int.as_bytes()[head..]
            .chunks(GROUP)
            .map(|c| std::str::from_utf8(c).unwrap_or("")),
    );
    for (index, chunk) in chunks.into_iter().enumerate() {
        if index > 0 {
            out.push(("group", ",".into()));
        }
        out.push(("integer", chunk.into()));
    }
}

/** The number's own parts and whether the rounded value is zero; NaN and infinity included. */
fn number_parts(options: &BTreeMap<String, String>, value: &Num) -> (Parts, bool, bool) {
    let mut out = Parts::new();
    let (negative, x) = match value {
        Num::NaN => return (vec![("nan", "NaN".into())], false, false),
        Num::Inf(negative) => return (vec![("infinity", "∞".into())], false, *negative),
        Num::Fin(negative, x) => (*negative, x),
    };
    let x = if opt(options, "style") == "percent" {
        x.shift(2)
    } else {
        x.clone()
    };
    let exponent = compute_exponent(options, &x, negative);
    let raw = numeric_to_string(options, &x.shift(-exponent), negative);
    integer_parts(options, &raw.int, &mut out);
    if !raw.frac.is_empty() {
        out.push(("decimal", ".".into()));
        out.push(("fraction", raw.frac.clone()));
    }
    match opt(options, "notation") {
        "scientific" | "engineering" => {
            out.push(("exponentSeparator", "E".into()));
            if exponent < 0 {
                out.push(("exponentMinusSign", "-".into()));
            }
            out.push(("exponentInteger", exponent.abs().to_string()));
        }
        "compact" if exponent > 0 => {
            let long = opt(options, "compactDisplay") == "long";
            if let Some((_, short, long_name)) = COMPACT.iter().find(|row| row.0 == exponent) {
                if long {
                    out.push(("literal", " ".into()));
                    out.push(("compact", long_name.trim_start().into()));
                } else {
                    out.push(("compact", (*short).into()));
                }
            }
        }
        _ => {}
    }
    (out, raw.value.is_zero(), negative)
}

/** The sign a value shows: None, Some('-') or Some('+'). */
fn sign_for(
    options: &BTreeMap<String, String>,
    value: &Num,
    zero: bool,
    negative: bool,
) -> Option<char> {
    let nan = matches!(value, Num::NaN);
    match opt(options, "signDisplay") {
        "never" => None,
        "always" if negative => Some('-'),
        "always" => Some('+'),
        "exceptZero" if nan || zero => None,
        "exceptZero" if negative => Some('-'),
        "exceptZero" => Some('+'),
        "negative" if negative && !zero => Some('-'),
        "negative" => None,
        _ if negative => Some('-'),
        _ => None,
    }
}

/** Whether the formatted number is plural "one" in English (integer 1, no visible fraction). */
fn english_one(parts: &Parts) -> bool {
    let digits: String = parts
        .iter()
        .filter(|p| p.0 == "integer" || p.0 == "fraction")
        .map(|p| p.1.as_str())
        .collect();
    let fraction = parts.iter().any(|p| p.0 == "fraction");
    digits == "1" && !fraction
}

/** The unit's (short, narrow, one, other) names. */
fn unit_names(unit: &str) -> (String, String, String, String) {
    fn row(u: &str) -> (&str, &str, &str, &str, &str) {
        UNITS
            .iter()
            .find(|row| row.0 == u)
            .copied()
            .unwrap_or((u, u, u, u, u))
    }
    if let Some((a, b)) = unit.split_once("-per-") {
        let (top, bottom) = (row(a), row(b));
        let per = PER_SHORT
            .iter()
            .find(|p| p.0 == b)
            .map_or(bottom.2, |p| p.1);
        let short = format!("{}/{per}", top.1);
        let narrow = format!("{}/{per}", top.2);
        return (
            short,
            narrow,
            format!("{} per {}", top.3, bottom.3),
            format!("{} per {}", top.4, bottom.3),
        );
    }
    let r = row(unit);
    (r.1.into(), r.2.into(), r.3.into(), r.4.into())
}

/** Every part of a formatted value: sign, currency or unit affixes around the number. */
pub(super) fn format_parts(options: &BTreeMap<String, String>, value: &Num) -> Parts {
    let (number, zero, negative) = number_parts(options, value);
    let sign = sign_for(options, value, zero, negative);
    let style = opt(options, "style");
    let accounting =
        style == "currency" && opt(options, "currencySign") == "accounting" && sign == Some('-');
    let mut out = Parts::new();
    if accounting {
        out.push(("literal", "(".into()));
    } else if let Some(sign) = sign {
        out.push((
            if sign == '-' { "minusSign" } else { "plusSign" },
            sign.to_string(),
        ));
    }
    let one = english_one(&number);
    match style {
        "currency" => {
            let code = opt(options, "currency");
            let row = CURRENCIES.iter().find(|row| row.0 == code);
            match opt(options, "currencyDisplay") {
                "name" => {
                    out.extend(number);
                    let name = row.map_or(code, |r| if one { r.3 } else { r.4 });
                    out.push(("literal", " ".into()));
                    out.push(("currency", name.into()));
                }
                display => {
                    let symbol = match (display, row) {
                        ("code", _) | (_, None) => None,
                        ("narrowSymbol", Some(r)) => Some(r.2),
                        (_, Some(r)) => Some(r.1),
                    };
                    match symbol {
                        Some(symbol) if !symbol.bytes().all(|b| b.is_ascii_alphabetic()) => {
                            out.push(("currency", symbol.into()))
                        }
                        _ => {
                            out.push(("currency", symbol.unwrap_or(code).into()));
                            out.push(("literal", "\u{a0}".into()));
                        }
                    }
                    out.extend(number);
                }
            }
        }
        "percent" => {
            out.extend(number);
            out.push(("percentSign", "%".into()));
        }
        "unit" => {
            out.extend(number);
            let (short, narrow, singular, plural) = unit_names(opt(options, "unit"));
            match opt(options, "unitDisplay") {
                "long" => {
                    out.push(("literal", " ".into()));
                    out.push(("unit", if one { singular } else { plural }));
                }
                display => {
                    let text = if display == "narrow" { narrow } else { short };
                    let text = if one && text == " days" {
                        " day".into()
                    } else {
                        text
                    };
                    if let Some(rest) = text.strip_prefix(' ') {
                        out.push(("literal", " ".into()));
                        out.push(("unit", rest.into()));
                    } else {
                        out.push(("unit", text));
                    }
                }
            }
        }
        _ => out.extend(number),
    }
    if accounting {
        out.push(("literal", ")".into()));
    }
    out
}

/** Parts as `[{type, value}]`, with a `source` when given. */
fn parts_array(vm: &mut Vm, parts: &[(&str, String, Option<&str>)]) -> Res<JsValue> {
    let mut values = Vec::with_capacity(parts.len());
    for (kind, value, source) in parts {
        let part = vm.new_object()?;
        vm.put(part, "type", JsValue::string(kind)?)?;
        vm.put(part, "value", JsValue::string(value)?)?;
        if let Some(source) = source {
            vm.put(part, "source", JsValue::string(source)?)?;
        }
        values.push(JsValue::Object(part));
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** `formatToParts(value)`. */
fn format_to_parts(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "NumberFormat")?;
    let value = to_intl_value(vm, &call.arg(0))?;
    let parts: Vec<_> = format_parts(&data.options, &value)
        .into_iter()
        .map(|(k, v)| (k, v, None))
        .collect();
    parts_array(vm, &parts)
}

/** The two ends of a range, checked: undefined is a TypeError, NaN a RangeError. */
fn range_ends(vm: &mut Vm, call: &Call) -> Res<(Num, Num)> {
    if call.arg(0) == JsValue::Undefined || call.arg(1) == JsValue::Undefined {
        return Err(vm.type_error("start or end is undefined"));
    }
    let x = to_intl_value(vm, &call.arg(0))?;
    let y = to_intl_value(vm, &call.arg(1))?;
    if x == Num::NaN || y == Num::NaN {
        return Err(vm.range_error("Invalid range: NaN"));
    }
    Ok((x, y))
}

/** PartitionNumberRangePattern in en: "3–5", "$3 – $5", or "~5" when both ends read the same. */
fn range_parts(
    options: &BTreeMap<String, String>,
    x: &Num,
    y: &Num,
) -> Vec<(&'static str, String, Option<&'static str>)> {
    let start = format_parts(options, x);
    let end = format_parts(options, y);
    if start == end {
        let mut out = vec![("approximatelySign", "~".to_string(), Some("shared"))];
        let at = start
            .iter()
            .position(|p| !matches!(p.0, "minusSign" | "plusSign"))
            .unwrap_or(0);
        let mut parts: Vec<_> = start
            .into_iter()
            .map(|(k, v)| (k, v, Some("shared")))
            .collect();
        let tail = parts.split_off(at);
        out.splice(0..0, parts);
        out.extend(tail);
        return out;
    }
    let plain = |parts: &Parts| {
        parts
            .iter()
            .all(|p| matches!(p.0, "integer" | "group" | "decimal" | "fraction"))
    };
    let separator = if plain(&start) && plain(&end) {
        "–"
    } else {
        " – "
    };
    let mut out: Vec<_> = start
        .into_iter()
        .map(|(k, v)| (k, v, Some("startRange")))
        .collect();
    out.push(("literal", separator.into(), Some("shared")));
    out.extend(end.into_iter().map(|(k, v)| (k, v, Some("endRange"))));
    out
}

/** `formatRange(start, end)`. */
fn format_range(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "NumberFormat")?;
    let (x, y) = range_ends(vm, &call)?;
    let text: String = range_parts(&data.options, &x, &y)
        .into_iter()
        .map(|p| p.1)
        .collect();
    Ok(JsValue::string(&text)?)
}

/** `formatRangeToParts(start, end)`. */
fn format_range_to_parts(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "NumberFormat")?;
    let (x, y) = range_ends(vm, &call)?;
    let parts = range_parts(&data.options, &x, &y);
    parts_array(vm, &parts)
}

/** Put a kept option on a resolvedOptions object, typed. */
fn put_resolved(vm: &mut Vm, id: u32, options: &BTreeMap<String, String>, name: &str) -> Res<()> {
    let Some(value) = options.get(name) else {
        return Ok(());
    };
    let value = if name.ends_with("Digits") || name == "roundingIncrement" {
        JsValue::Number(value.parse().unwrap_or(0.0))
    } else if name == "useGrouping" && value == "false" {
        JsValue::Boolean(false)
    } else {
        JsValue::string(value)?
    };
    vm.put(id, name, value)
}

/** The digit options in resolvedOptions order, only those the rounding type uses. */
fn put_digit_options(vm: &mut Vm, id: u32, options: &BTreeMap<String, String>) -> Res<()> {
    put_resolved(vm, id, options, "minimumIntegerDigits")?;
    let rounding = opt(options, "@rounding");
    if rounding != "significantDigits" {
        put_resolved(vm, id, options, "minimumFractionDigits")?;
        put_resolved(vm, id, options, "maximumFractionDigits")?;
    }
    if rounding != "fractionDigits" {
        put_resolved(vm, id, options, "minimumSignificantDigits")?;
        put_resolved(vm, id, options, "maximumSignificantDigits")?;
    }
    Ok(())
}

/** `resolvedOptions()` in the spec's property order. */
fn resolved_options(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "NumberFormat")?;
    let id = vm.new_object()?;
    vm.put(id, "locale", JsValue::string(&data.locale)?)?;
    vm.put(id, "numberingSystem", JsValue::string("latn")?)?;
    for name in [
        "style",
        "currency",
        "currencyDisplay",
        "currencySign",
        "unit",
        "unitDisplay",
    ] {
        put_resolved(vm, id, &data.options, name)?;
    }
    put_digit_options(vm, id, &data.options)?;
    for name in [
        "useGrouping",
        "notation",
        "compactDisplay",
        "signDisplay",
        "roundingIncrement",
        "roundingMode",
        "roundingPriority",
        "trailingZeroDisplay",
    ] {
        put_resolved(vm, id, &data.options, name)?;
    }
    Ok(JsValue::Object(id))
}

/** `new Intl.PluralRules(locales, options)`; `new` is required. */
fn construct_plural(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Constructor Intl.PluralRules requires 'new'"));
    }
    let (locale, options) = locale_and_matcher(vm, &call)?;
    let mut kept = BTreeMap::new();
    let kind = choice(vm, &options, "type", &["cardinal", "ordinal"], "cardinal")?;
    kept.insert("type".into(), kind);
    let notation = choice(
        vm,
        &options,
        "notation",
        &["standard", "scientific", "engineering", "compact"],
        "standard",
    )?;
    let compact = choice(vm, &options, "compactDisplay", &["short", "long"], "short")?;
    if notation == "compact" {
        kept.insert("compactDisplay".into(), compact);
    }
    digit_options(vm, &options, &mut kept, 0.0, 3.0, &notation)?;
    kept.insert("notation".into(), notation);
    kept.insert("style".into(), "decimal".into());
    kept.insert("useGrouping".into(), "false".into());
    allocate(vm, &call, "PluralRules", locale, kept)
}

/** The en plural category of a value under the rules' digit options. */
fn category(options: &BTreeMap<String, String>, value: &Num) -> &'static str {
    let Num::Fin(_, x) = value else {
        return "other";
    };
    let (parts, _, _) = number_parts(options, &Num::Fin(false, x.clone()));
    if opt(options, "type") != "ordinal" {
        return if english_one(&parts) { "one" } else { "other" };
    }
    let int: String = parts
        .iter()
        .filter(|p| p.0 == "integer")
        .map(|p| p.1.as_str())
        .collect();
    let tail: u32 = int[int.len().saturating_sub(2)..].parse().unwrap_or(0);
    match (tail % 10, tail) {
        (1, t) if t != 11 => "one",
        (2, t) if t != 12 => "two",
        (3, t) if t != 13 => "few",
        _ => "other",
    }
}

/** `select(n)`. */
fn select(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "PluralRules")?;
    let value = decimal::from_f64(vm.to_number(&call.arg(0))?);
    Ok(JsValue::string(category(&data.options, &value))?)
}

/** `selectRange(x, y)`: en takes the end's category. */
fn select_range(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "PluralRules")?;
    if call.arg(0) == JsValue::Undefined || call.arg(1) == JsValue::Undefined {
        return Err(vm.type_error("start or end is undefined"));
    }
    let x = vm.to_number(&call.arg(0))?;
    let y = vm.to_number(&call.arg(1))?;
    if x.is_nan() || y.is_nan() {
        return Err(vm.range_error("Invalid range: NaN"));
    }
    Ok(JsValue::string(category(
        &data.options,
        &decimal::from_f64(y),
    ))?)
}

/** PluralRules `resolvedOptions()`. */
fn plural_resolved_options(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = unwrap(vm, &call.this, "PluralRules")?;
    let id = vm.new_object()?;
    vm.put(id, "locale", JsValue::string(&data.locale)?)?;
    put_resolved(vm, id, &data.options, "type")?;
    put_resolved(vm, id, &data.options, "notation")?;
    put_resolved(vm, id, &data.options, "compactDisplay")?;
    put_digit_options(vm, id, &data.options)?;
    let names: &[&str] = if opt(&data.options, "type") == "ordinal" {
        &["one", "two", "few", "other"]
    } else {
        &["one", "other"]
    };
    let values = names
        .iter()
        .map(|n| JsValue::string(n))
        .collect::<Result<Vec<_>, _>>()?;
    let categories = vm.new_array(values)?;
    vm.put(id, "pluralCategories", JsValue::Object(categories))?;
    for name in [
        "roundingIncrement",
        "roundingMode",
        "roundingPriority",
        "trailingZeroDisplay",
    ] {
        put_resolved(vm, id, &data.options, name)?;
    }
    Ok(JsValue::Object(id))
}
