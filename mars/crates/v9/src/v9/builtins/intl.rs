/**
 * Intl, in the one locale the browser presents (en-US with Latin digits): NumberFormat,
 * DateTimeFormat, PluralRules, RelativeTimeFormat, ListFormat, Collator and Segmenter with the
 * options real pages pass. Pages such as YouTube construct formatters while loading and fail
 * outright when `Intl` is absent, so a small faithful English implementation beats none; other
 * locales format as English, which is what the persona reports anyway.
 */
use super::date::parts;
use crate::v9::platform::native_name;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData, IntlData};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Vm};
use std::collections::BTreeMap;

mod bound;
#[cfg(feature = "conformance")]
mod break_cursor;
#[cfg(feature = "conformance")]
mod break_iterator;
mod date_receiver;
mod decimal;
mod display_names;
mod locale;
/** CanonicalizeLocaleList, shared with String.prototype.toLocale*Case. */
pub(in crate::v9) use locale::canonical_list;
mod locale_info;
mod number_format;
mod segmentation;
mod tag;
mod time_zone;

/** The formatter kinds, each a constructor on `Intl`. */
const KINDS: [&str; 7] = [
    "NumberFormat",
    "DateTimeFormat",
    "PluralRules",
    "RelativeTimeFormat",
    "ListFormat",
    "Collator",
    "Segmenter",
];

/** Options a constructor keeps, read from its options argument when present. */
const OPTIONS: [&str; 34] = [
    "style",
    "currency",
    "currencyDisplay",
    "minimumIntegerDigits",
    "minimumFractionDigits",
    "maximumFractionDigits",
    "minimumSignificantDigits",
    "maximumSignificantDigits",
    "useGrouping",
    "notation",
    "compactDisplay",
    "signDisplay",
    "unit",
    "unitDisplay",
    "year",
    "month",
    "day",
    "weekday",
    "hour",
    "minute",
    "second",
    "hour12",
    "hourCycle",
    "timeZone",
    "timeZoneName",
    "dateStyle",
    "timeStyle",
    "era",
    "type",
    "numeric",
    "sensitivity",
    "ignorePunctuation",
    "granularity",
    "localeMatcher",
];

/** Argument counts ECMA-402 gives formatter methods. */
const NO_ARGUMENTS: u32 = 0;
/** One argument. */
const ONE_ARGUMENT: u32 = 1;
/** Two arguments. */
const TWO_ARGUMENTS: u32 = 2;
/** The locale every formatter resolves to. */
/** Fraction digits a plain number keeps at most. */
const DEFAULT_FRACTION_DIGITS: usize = 3;
/** Fraction digits a currency amount keeps. */
const CURRENCY_FRACTION_DIGITS: usize = 2;
/** Digits between grouping separators. */
const GROUP: usize = 3;
/** Compact notation thresholds and their English suffixes. */
const COMPACT: [(f64, &str); 4] = [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")];
/** Month names, long and short. */
const MONTHS: [(&str, &str); 12] = [
    ("January", "Jan"),
    ("February", "Feb"),
    ("March", "Mar"),
    ("April", "Apr"),
    ("May", "May"),
    ("June", "Jun"),
    ("July", "Jul"),
    ("August", "Aug"),
    ("September", "Sep"),
    ("October", "Oct"),
    ("November", "Nov"),
    ("December", "Dec"),
];
/** Weekday names, long and short, Sunday first as `Date` counts them. */
const WEEKDAYS: [(&str, &str); 7] = [
    ("Sunday", "Sun"),
    ("Monday", "Mon"),
    ("Tuesday", "Tue"),
    ("Wednesday", "Wed"),
    ("Thursday", "Thu"),
    ("Friday", "Fri"),
    ("Saturday", "Sat"),
];

/** Spec length of a formatter method: resolvedOptions takes none, the range methods and relative-time formatting take two. */
fn method_length(kind: &str, name: &str) -> u32 {
    match name {
        "resolvedOptions" => NO_ARGUMENTS,
        "formatRange" | "selectRange" => TWO_ARGUMENTS,
        "format" | "formatToParts" if kind == "RelativeTimeFormat" => TWO_ARGUMENTS,
        _ => ONE_ARGUMENT,
    }
}

/** Install `Intl` and its constructors. */
pub(super) fn install(b: &mut Builder) {
    let intl = b.object(Some(b.ids.object_prototype));
    b.global("Intl", intl);
    b.method(intl, "getCanonicalLocales", 1, canonical_locales);
    b.method(intl, "supportedValuesOf", 1, supported_values);
    locale::install(b, intl);
    display_names::install(b, intl);
    number_format::install(b, intl);
    to_string_tag(b, intl, "Intl");
    #[cfg(feature = "conformance")]
    break_iterator::install(b, intl);
    for kind in KINDS {
        if kind == "NumberFormat" || kind == "PluralRules" {
            continue;
        }
        let proto = b.object(Some(b.ids.object_prototype));
        for name in [
            "format",
            "formatToParts",
            "formatRange",
            "resolvedOptions",
            "select",
            "selectRange",
            "segment",
        ] {
            if name == "format" && kind == "DateTimeFormat" {
                continue;
            }
            b.method(
                proto,
                name,
                method_length(kind, name),
                if kind == "DateTimeFormat" {
                    date_receiver::method
                } else {
                    method
                },
            );
        }
        bound::install(b, proto, kind);
        to_string_tag(
            b,
            proto,
            INTL_TAGS[KINDS.iter().position(|k| *k == kind).unwrap_or(0)],
        );
        let ctor = b.constructor(kind, 0, construct, proto, None);
        b.method(ctor, "supportedLocalesOf", 1, supported_locales);
        b.value(
            intl,
            kind,
            JsValue::Object(ctor),
            crate::v9::vm::heap::HIDDEN_ATTRS,
        );
    }
}

/** `Symbol.toStringTag` names per formatter kind, in `KINDS` order. */
const INTL_TAGS: [&str; 7] = [
    "Intl.NumberFormat",
    "Intl.DateTimeFormat",
    "Intl.PluralRules",
    "Intl.RelativeTimeFormat",
    "Intl.ListFormat",
    "Intl.Collator",
    "Intl.Segmenter",
];

/** A configurable, non-writable `Symbol.toStringTag` value. */
fn to_string_tag(b: &mut Builder, target: u32, name: &'static str) {
    use crate::v9::vm::realm::{Static, string, symbol};
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        crate::v9::vm::heap::CONFIGURABLE,
    );
}

/** `Intl.getCanonicalLocales(locales)`: the requested tags, canonical and deduplicated. */
fn canonical_locales(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let locales = locale::canonical_list(vm, &call.arg(0))?;
    let values = locales
        .iter()
        .map(|locale| JsValue::string(locale))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** `Intl.supportedValuesOf(key)`: the one value of each kind this locale has. */
fn supported_values(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key = vm.to_text(&call.arg(0))?;
    let value = match key.as_str() {
        "calendar" => "gregory",
        "currency" => "USD",
        "numberingSystem" => "latn",
        "timeZone" => "UTC",
        "unit" => "second",
        "collation" => "emoji",
        _ => return Err(vm.range_error(&format!("Invalid key : {key}"))),
    };
    let values = if value.is_empty() {
        Vec::new()
    } else {
        vec![JsValue::string(value)?]
    };
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** `supportedLocalesOf(locales)`: every requested tag, since all format as English. */
pub(super) fn supported_locales(vm: &mut Vm, call: Call) -> Res<JsValue> {
    canonical_locales(vm, call)
}

/** The locale tags an argument names: a string, an array of strings, or none. */
fn locale_list(vm: &mut Vm, value: &JsValue) -> Res<Vec<String>> {
    match value {
        JsValue::Undefined | JsValue::Null => Ok(Vec::new()),
        JsValue::Object(id) => {
            let length = vm.get(value, &Key::from("length"))?;
            let count = vm.to_integer(&length)?.max(0.0) as usize;
            let mut out = Vec::with_capacity(count.min(16));
            for index in 0..count.min(16) {
                let item = vm.get(
                    &JsValue::Object(*id),
                    &Key::from(index.to_string().as_str()),
                )?;
                out.push(vm.to_text(&item)?);
            }
            Ok(out)
        }
        _ => Ok(vec![vm.to_text(value)?]),
    }
}

/** One constructor option as text. A timeZone must name a zone, so `0` or `true` throws a
 * RangeError as V8 does instead of formatting in an unknown zone. */
fn option_text(vm: &mut Vm, name: &str, value: &JsValue) -> Res<String> {
    let text = vm.to_text(value)?;
    if name == "timeZone" {
        return time_zone::canonical(&text)
            .ok_or_else(|| vm.range_error(&format!("Invalid time zone specified: {text}")));
    }
    Ok(text)
}

/** `new Intl.X(locales, options)`, also callable without `new`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    // The constructor's own `name` says which formatter this is; natives on constructors have
    // no name entry of their own.
    let name = vm.get(&JsValue::Object(call.callee), &Key::from("name"))?;
    let kind = vm.to_text(&name)?;
    let locale = locale::canonical_list(vm, &call.arg(0))?
        .into_iter()
        .next()
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| vm.page.environment.locale.clone());
    let init = call.arg(1);
    let mut options = BTreeMap::new();
    if matches!(init, JsValue::Object(_)) {
        for name in OPTIONS {
            if name == "timeZone" && kind != "DateTimeFormat" {
                continue;
            }
            let value = vm.get(&init, &Key::from(name))?;
            if value != JsValue::Undefined {
                options.insert(name.to_string(), option_text(vm, name, &value)?);
            }
        }
    }
    let prototype = vm.get(&JsValue::Object(call.callee), &Key::from("prototype"))?;
    let proto = match prototype {
        JsValue::Object(id) => id,
        _ => realm().ids.object_prototype,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Intl(Box::new(IntlData {
            kind,
            locale,
            options,
        })))),
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** The formatter behind `this`. */
fn data(vm: &mut Vm, call: &Call) -> Res<IntlData> {
    match crate::v9::platform::host_data(vm, &call.this) {
        Some(HostData::Intl(data)) => Ok(*data),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** Every formatter method, dispatched on the method name and the formatter kind. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = data(vm, &call)?;
    let name = native_name(&call);
    match (name, data.kind.as_str()) {
        ("resolvedOptions", _) => resolved_options(vm, &data),
        ("format", "NumberFormat") => {
            let value = vm.to_number(&call.arg(0))?;
            text(vm, &format_number(value, &data.options))
        }
        ("formatToParts", "NumberFormat") => {
            let value = vm.to_number(&call.arg(0))?;
            number_parts(vm, &format_number(value, &data.options))
        }
        ("formatToParts", "DateTimeFormat") => {
            let time = time_value(vm, &call.arg(0))?;
            let formatted = format_date(vm, time, &data.options)?;
            single_part(vm, "literal", &formatted)
        }
        ("formatRange", "DateTimeFormat") => {
            let from = time_value(vm, &call.arg(0))?;
            let to = time_value(vm, &call.arg(1))?;
            let joined = format!(
                "{} \u{2013} {}",
                format_date(vm, from, &data.options)?,
                format_date(vm, to, &data.options)?
            );
            text(vm, &joined)
        }
        ("select", "PluralRules") => {
            let value = vm.to_number(&call.arg(0))?;
            text(vm, plural_category(value, &data.options))
        }
        ("format", "RelativeTimeFormat") => {
            let value = vm.to_number(&call.arg(0))?;
            let unit = vm.to_text(&call.arg(1))?;
            text(vm, &format_relative(value, &unit, &data.options))
        }
        ("formatToParts", "RelativeTimeFormat") => {
            let value = vm.to_number(&call.arg(0))?;
            let unit = vm.to_text(&call.arg(1))?;
            let formatted = format_relative(value, &unit, &data.options);
            single_part(vm, "literal", &formatted)
        }
        ("format", "ListFormat") => {
            let items = locale_list(vm, &call.arg(0))?;
            text(vm, &format_list(&items, &data.options))
        }
        ("segment", "Segmenter") => {
            let input = vm.to_text(&call.arg(0))?;
            segments(vm, &input, &data.options)
        }
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** A string result. */
fn text(vm: &mut Vm, value: &str) -> Res<JsValue> {
    let _ = vm;
    JsValue::string(value).map_err(Into::into)
}

/** `resolvedOptions()`: the locale, Latin digits, the Gregorian calendar and the options kept. */
fn resolved_options(vm: &mut Vm, data: &IntlData) -> Res<JsValue> {
    let id = vm.new_object()?;
    vm.put(id, "locale", JsValue::string(&data.locale)?)?;
    vm.put(id, "numberingSystem", JsValue::string("latn")?)?;
    if data.kind == "DateTimeFormat" {
        vm.put(id, "calendar", JsValue::string("gregory")?)?;
        let zone = data.options.get("timeZone").map_or("UTC", String::as_str);
        vm.put(id, "timeZone", JsValue::string(zone)?)?;
    }
    for (name, value) in &data.options {
        if name == "timeZone" {
            continue;
        }
        vm.put(id, name, option_value(name, value)?)?;
    }
    Ok(JsValue::Object(id))
}

/** Options come back typed: digit counts as numbers, flags as booleans, the rest as strings. */
fn option_value(name: &str, value: &str) -> Res<JsValue> {
    if name.ends_with("Digits") {
        return Ok(JsValue::Number(value.parse().unwrap_or(0.0)));
    }
    if ["useGrouping", "hour12", "numeric", "ignorePunctuation"].contains(&name)
        && (value == "true" || value == "false")
    {
        return Ok(JsValue::Boolean(value == "true"));
    }
    JsValue::string(value).map_err(Into::into)
}

/** `[{type, value}]` for a whole formatted string. */
fn single_part(vm: &mut Vm, kind: &str, value: &str) -> Res<JsValue> {
    let part = vm.new_object()?;
    vm.put(part, "type", JsValue::string(kind)?)?;
    vm.put(part, "value", JsValue::string(value)?)?;
    Ok(JsValue::Object(vm.new_array(vec![JsValue::Object(part)])?))
}

/** The parts of a formatted number: integer, group, decimal, fraction, sign and symbols. */
fn number_parts(vm: &mut Vm, formatted: &str) -> Res<JsValue> {
    let mut parts = Vec::new();
    let mut run = String::new();
    let mut kind = "";
    let flush = |vm: &mut Vm, parts: &mut Vec<JsValue>, kind: &str, run: &mut String| -> Res<()> {
        if run.is_empty() {
            return Ok(());
        }
        let part = vm.new_object()?;
        vm.put(part, "type", JsValue::string(kind)?)?;
        vm.put(part, "value", JsValue::string(run)?)?;
        parts.push(JsValue::Object(part));
        run.clear();
        Ok(())
    };
    let mut seen_decimal = false;
    for c in formatted.chars() {
        let next = match c {
            '0'..='9' if seen_decimal => "fraction",
            '0'..='9' => "integer",
            ',' => "group",
            '.' => "decimal",
            '-' => "minusSign",
            '+' => "plusSign",
            '%' => "percentSign",
            '$' | '€' | '£' | '¥' => "currency",
            'K' | 'M' | 'B' | 'T' => "compact",
            _ => "literal",
        };
        if next == "decimal" {
            seen_decimal = true;
        }
        if next != kind {
            flush(vm, &mut parts, kind, &mut run)?;
            kind = next;
        }
        run.push(c);
    }
    flush(vm, &mut parts, kind, &mut run)?;
    Ok(JsValue::Object(vm.new_array(parts)?))
}

/** An option as a digit count. */
fn digits(options: &BTreeMap<String, String>, name: &str) -> Option<usize> {
    options.get(name).and_then(|value| value.parse().ok())
}

/** Format a number the way en-US does: grouping, fraction bounds, percent, currency, compact. */
pub(in crate::v9) fn format_number(value: f64, options: &BTreeMap<String, String>) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    let style = options.get("style").map_or("decimal", String::as_str);
    let mut value = if style == "percent" {
        value * 100.0
    } else {
        value
    };
    if value.is_infinite() {
        return if value < 0.0 {
            "-∞".into()
        } else {
            "∞".into()
        };
    }
    let negative = value < 0.0;
    value = value.abs();
    let compact = options
        .get("notation")
        .is_some_and(|notation| notation == "compact");
    let mut suffix = String::new();
    let currency_digits = (style == "currency").then_some(CURRENCY_FRACTION_DIGITS);
    let mut minimum = digits(options, "minimumFractionDigits")
        .or(currency_digits)
        .unwrap_or(0);
    let mut maximum = digits(options, "maximumFractionDigits")
        .or(currency_digits)
        .unwrap_or(DEFAULT_FRACTION_DIGITS)
        .max(minimum);
    if compact
        && let Some((threshold, label)) = COMPACT.iter().find(|(threshold, _)| value >= *threshold)
    {
        value /= threshold;
        suffix = (*label).into();
        minimum = 0;
        maximum = if value < 10.0 { 1 } else { 0 };
    }
    let rounded = format!("{value:.maximum$}");
    let (integer, fraction) = rounded.split_once('.').unwrap_or((&rounded, ""));
    let mut fraction = fraction.to_string();
    while fraction.len() > minimum && fraction.ends_with('0') {
        fraction.pop();
    }
    let mut integer = integer.to_string();
    let minimum_integer = digits(options, "minimumIntegerDigits").unwrap_or(1);
    while integer.len() < minimum_integer {
        integer.insert(0, '0');
    }
    if options
        .get("useGrouping")
        .is_none_or(|grouping| grouping != "false")
    {
        integer = grouped(&integer);
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    } else if options
        .get("signDisplay")
        .is_some_and(|sign| sign == "always" || sign == "exceptZero" && value != 0.0)
    {
        out.push('+');
    }
    if style == "currency" {
        out.push_str(currency_symbol(
            options.get("currency").map_or("USD", String::as_str),
        ));
    }
    out.push_str(&integer);
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(&fraction);
    }
    out.push_str(&suffix);
    if style == "percent" {
        out.push('%');
    }
    if style == "unit"
        && let Some(unit) = options.get("unit")
    {
        out.push(' ');
        out.push_str(unit);
    }
    out
}

/** Digits with a comma every three from the right. */
fn grouped(integer: &str) -> String {
    let digits: Vec<char> = integer.chars().collect();
    let mut out = String::with_capacity(digits.len() + digits.len() / GROUP);
    for (index, digit) in digits.iter().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(GROUP) {
            out.push(',');
        }
        out.push(*digit);
    }
    out
}

/** The symbol en-US shows for a currency code. */
fn currency_symbol(code: &str) -> &str {
    match code.to_ascii_uppercase().as_str() {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "INR" => "₹",
        _ => code,
    }
}

/** English cardinal or ordinal plural category. */
fn plural_category(value: f64, options: &BTreeMap<String, String>) -> &'static str {
    let ordinal = options.get("type").is_some_and(|kind| kind == "ordinal");
    if !ordinal {
        return if value == 1.0 { "one" } else { "other" };
    }
    let tens = (value.abs() as u64) % 100;
    match (tens % 10, tens) {
        (1, t) if t != 11 => "one",
        (2, t) if t != 12 => "two",
        (3, t) if t != 13 => "few",
        _ => "other",
    }
}

/** "in 3 days", "2 hours ago", and with `numeric: "auto"` "yesterday", "now", "next week". */
fn format_relative(value: f64, unit: &str, options: &BTreeMap<String, String>) -> String {
    let unit = unit.trim_end_matches('s');
    let auto = options
        .get("numeric")
        .is_some_and(|numeric| numeric == "auto");
    if auto {
        let special = match (unit, value as i64) {
            ("day", 0) => Some("today"),
            ("day", 1) => Some("tomorrow"),
            ("day", -1) => Some("yesterday"),
            ("second", 0) => Some("now"),
            (_, 0) => None,
            (_, 1) => None,
            (_, -1) => None,
            _ => None,
        };
        if let Some(special) = special {
            return special.into();
        }
        if value == 0.0 {
            return format!("this {unit}");
        }
        if value == 1.0 {
            return format!("next {unit}");
        }
        if value == -1.0 {
            return format!("last {unit}");
        }
    }
    let count = format_number(value.abs(), &BTreeMap::new());
    let plural = if value.abs() == 1.0 { "" } else { "s" };
    if value < 0.0 || (value == 0.0 && value.is_sign_negative()) {
        format!("{count} {unit}{plural} ago")
    } else {
        format!("in {count} {unit}{plural}")
    }
}

/** "a, b, and c", "a or b", or "a, b, c" for units. */
fn format_list(items: &[String], options: &BTreeMap<String, String>) -> String {
    let word = match options.get("type").map(String::as_str) {
        Some("disjunction") => "or",
        Some("unit") => "",
        _ => "and",
    };
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] if !word.is_empty() => format!("{a} {word} {b}"),
        _ => {
            let (last, rest) = items.split_last().expect("two or more");
            let head = rest.join(", ");
            if word.is_empty() {
                format!("{head}, {last}")
            } else {
                format!("{head}, {word} {last}")
            }
        }
    }
}

/** Order two strings: case-insensitively unless the sensitivity asks for case, and with
embedded numbers compared by value when `numeric` is on. */
fn collate(a: &str, b: &str, options: &BTreeMap<String, String>) -> f64 {
    let case_sensitive = options
        .get("sensitivity")
        .is_some_and(|sensitivity| sensitivity == "case" || sensitivity == "variant");
    let numeric = options
        .get("numeric")
        .is_some_and(|numeric| numeric == "true");
    let key = |text: &str| -> Vec<(u64, String)> {
        let folded = if case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };
        if !numeric {
            return vec![(0, folded)];
        }
        let mut parts = Vec::new();
        let mut run = String::new();
        for c in folded.chars() {
            if c.is_ascii_digit()
                != run
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_ascii_digit())
                && !run.is_empty()
            {
                parts.push(std::mem::take(&mut run));
            }
            run.push(c);
        }
        if !run.is_empty() {
            parts.push(run);
        }
        parts
            .into_iter()
            .map(|part| (part.parse::<u64>().unwrap_or(0), part))
            .collect()
    };
    match key(a).cmp(&key(b)) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    }
}

/** `segment(input)`: an array of `{segment, index, input, isWordLike}` by grapheme (one scalar
each), word (runs of letters and digits, and each other character) or sentence. */
fn segments(vm: &mut Vm, input: &str, options: &BTreeMap<String, String>) -> Res<JsValue> {
    let granularity = options
        .get("granularity")
        .map_or("grapheme", String::as_str);
    let pieces = segmentation::split(input, granularity);
    let mut out = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let id = vm.new_object()?;
        vm.put(id, "segment", JsValue::string(&piece.text)?)?;
        vm.put(id, "index", JsValue::Number(piece.start as f64))?;
        vm.put(id, "input", JsValue::string(input)?)?;
        if granularity == "word" {
            vm.put(id, "isWordLike", JsValue::Boolean(piece.wordy))?;
        }
        out.push(JsValue::Object(id));
    }
    Ok(JsValue::Object(vm.new_array(out)?))
}

/** The time value of a Date, a number, or now when absent. */
fn time_value(vm: &mut Vm, value: &JsValue) -> Res<f64> {
    match value {
        JsValue::Undefined => Ok(super::date::now_ms(vm)),
        JsValue::Object(id) => match vm.heap.get(*id).map(|object| &object.class) {
            Some(Class::Date(time)) => Ok(*time),
            _ => vm.to_number(value),
        },
        _ => vm.to_number(value),
    }
}

/** Format a date the way en-US does for the fields the options name; "M/D/YYYY" by default. */
pub(in crate::v9) fn format_date(
    vm: &mut Vm,
    time: f64,
    options: &BTreeMap<String, String>,
) -> Res<String> {
    if time.is_nan() {
        return Ok("Invalid Date".into());
    }
    let zone = options.get("timeZone").map_or("UTC", String::as_str);
    let offset = time_zone::offset_ms(zone, time)
        .ok_or_else(|| vm.range_error("Invalid stored time zone."))?;
    let p = parts(time + offset);
    let get = |name: &str| options.get(name).map(String::as_str);
    let date_style = get("dateStyle");
    let time_style = get("timeStyle");
    let (year, month, day, weekday) = match date_style {
        Some("full") => (Some("numeric"), Some("long"), Some("numeric"), Some("long")),
        Some("long") => (Some("numeric"), Some("long"), Some("numeric"), None),
        Some("medium") => (Some("numeric"), Some("short"), Some("numeric"), None),
        Some("short") => (Some("2-digit"), Some("numeric"), Some("numeric"), None),
        _ => (get("year"), get("month"), get("day"), get("weekday")),
    };
    let (hour, minute, second) = match time_style {
        Some("short") => (Some("numeric"), Some("2-digit"), None),
        Some(_) => (Some("numeric"), Some("2-digit"), Some("2-digit")),
        None => (get("hour"), get("minute"), get("second")),
    };
    let any_date = year.is_some() || month.is_some() || day.is_some() || weekday.is_some();
    let any_time = hour.is_some() || minute.is_some() || second.is_some();
    let (year, month, day) = if any_date || any_time {
        (year, month, day)
    } else {
        (Some("numeric"), Some("numeric"), Some("numeric"))
    };
    let month_index = (p.month as usize).min(11);
    let year_text = year.map(|style| {
        if style == "2-digit" {
            format!("{:02}", (p.year as i64).rem_euclid(100))
        } else {
            format!("{}", p.year as i64)
        }
    });
    let day_text = day.map(|style| {
        if style == "2-digit" {
            format!("{:02}", p.date as i64)
        } else {
            format!("{}", p.date as i64)
        }
    });
    let weekday_text = weekday.map(|style| match style {
        "long" => WEEKDAYS[p.day as usize % 7].0.to_string(),
        "narrow" => WEEKDAYS[p.day as usize % 7].1[..1].to_string(),
        _ => WEEKDAYS[p.day as usize % 7].1.to_string(),
    });
    let mut date_text = match month {
        Some("long") | Some("short") | Some("narrow") => {
            let name = match month {
                Some("long") => MONTHS[month_index].0.to_string(),
                Some("narrow") => MONTHS[month_index].1[..1].to_string(),
                _ => MONTHS[month_index].1.to_string(),
            };
            let mut text = name;
            if let Some(day) = &day_text {
                text.push(' ');
                text.push_str(day);
            }
            if let Some(year) = &year_text {
                text.push_str(if day_text.is_some() { ", " } else { " " });
                text.push_str(year);
            }
            text
        }
        Some(style) => {
            let month_text = if style == "2-digit" {
                format!("{:02}", month_index + 1)
            } else {
                format!("{}", month_index + 1)
            };
            [Some(month_text), day_text.clone(), year_text.clone()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("/")
        }
        None => [day_text.clone(), year_text.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
    };
    if let Some(weekday) = weekday_text {
        date_text = if date_text.is_empty() {
            weekday
        } else {
            format!("{weekday}, {date_text}")
        };
    }
    let zone_name = get("timeZoneName").is_some() || matches!(time_style, Some("long" | "full"));
    let time_text = (any_time || zone_name).then(|| {
        let twelve = get("hour12").is_none_or(|flag| flag != "false")
            && get("hourCycle").is_none_or(|cycle| cycle == "h12" || cycle == "h11");
        let hours = p.hours as i64;
        let mut text = if hour.is_some() {
            if twelve {
                format!("{}", if hours % 12 == 0 { 12 } else { hours % 12 })
            } else {
                format!("{hours:02}")
            }
        } else {
            String::new()
        };
        if minute.is_some() {
            if !text.is_empty() {
                text.push(':');
            }
            text.push_str(&format!("{:02}", p.minutes as i64));
        }
        if second.is_some() {
            text.push(':');
            text.push_str(&format!("{:02}", p.seconds as i64));
        }
        if hour.is_some() && twelve {
            text.push_str(if hours < 12 { " AM" } else { " PM" });
        }
        if zone_name {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&time_zone::label(offset));
        }
        text
    });
    Ok(match (date_text.is_empty(), time_text) {
        (false, Some(time)) => format!("{date_text}, {time}"),
        (true, Some(time)) => time,
        _ => date_text,
    })
}
