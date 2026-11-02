/**
 * `Intl.DisplayNames`, ported from V8's js-display-names.cc: the options are read and checked in
 * the spec's order, `of(code)` validates the code for its type and answers the English name from
 * the small embedded CLDR table, or the canonical code itself (`fallback: "code"`) when the
 * table has no entry.
 */
use super::locale::canonical_list;
use super::tag;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData, IntlData};
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, Key, Object};
use crate::v9::vm::realm::{Builder, Static, realm, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::collections::BTreeMap;

/** Options in reading order, with their allowed values; the first is the default. */
const OPTIONS: &[(&str, &[&str])] = &[
    ("localeMatcher", &["best fit", "lookup"]),
    ("style", &["long", "short", "narrow"]),
    (
        "type",
        &[
            "language",
            "region",
            "script",
            "currency",
            "calendar",
            "dateTimeField",
        ],
    ),
    ("fallback", &["code", "none"]),
    ("languageDisplay", &["dialect", "standard"]),
];

/** `dateTimeField` codes and their English names. */
const FIELDS: &[(&str, &str)] = &[
    ("era", "era"),
    ("year", "year"),
    ("quarter", "quarter"),
    ("month", "month"),
    ("weekOfYear", "week"),
    ("weekday", "day of the week"),
    ("day", "day"),
    ("dayPeriod", "AM/PM"),
    ("hour", "hour"),
    ("minute", "minute"),
    ("second", "second"),
    ("timeZoneName", "time zone"),
];

/** English display names (CLDR en), keyed by type then canonical code. */
const NAMES: &[(&str, &str, &str)] = &[
    ("language", "en", "English"),
    ("language", "en-US", "American English"),
    ("language", "en-GB", "British English"),
    ("language", "de", "German"),
    ("language", "fr", "French"),
    ("language", "es", "Spanish"),
    ("language", "it", "Italian"),
    ("language", "pt", "Portuguese"),
    ("language", "ja", "Japanese"),
    ("language", "zh", "Chinese"),
    ("language", "ko", "Korean"),
    ("language", "ru", "Russian"),
    ("language", "ar", "Arabic"),
    ("language", "hi", "Hindi"),
    ("language", "nl", "Dutch"),
    ("region", "US", "United States"),
    ("region", "GB", "United Kingdom"),
    ("region", "DE", "Germany"),
    ("region", "FR", "France"),
    ("region", "ES", "Spain"),
    ("region", "IT", "Italy"),
    ("region", "JP", "Japan"),
    ("region", "CN", "China"),
    ("region", "IN", "India"),
    ("region", "CA", "Canada"),
    ("region", "BR", "Brazil"),
    ("region", "419", "Latin America"),
    ("script", "Latn", "Latin"),
    ("script", "Cyrl", "Cyrillic"),
    ("script", "Arab", "Arabic"),
    ("script", "Hans", "Simplified Han"),
    ("script", "Hant", "Traditional Han"),
    ("currency", "USD", "US Dollar"),
    ("currency", "EUR", "Euro"),
    ("currency", "GBP", "British Pound"),
    ("currency", "JPY", "Japanese Yen"),
    ("currency", "CNY", "Chinese Yuan"),
    ("calendar", "gregory", "Gregorian Calendar"),
    ("calendar", "buddhist", "Buddhist Calendar"),
    ("calendar", "japanese", "Japanese Calendar"),
    ("calendar", "iso8601", "ISO-8601 Calendar"),
];

/** Install `Intl.DisplayNames`. */
pub(super) fn install(b: &mut Builder, intl: u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(proto, "of", 1, of);
    b.method(proto, "resolvedOptions", 0, resolved_options);
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Intl.DisplayNames")),
        CONFIGURABLE,
    );
    let ctor = b.constructor("DisplayNames", 2, construct, proto, None);
    b.method(ctor, "supportedLocalesOf", 1, super::supported_locales);
    b.value(intl, "DisplayNames", JsValue::Object(ctor), HIDDEN_ATTRS);
}

/** The resolved locale: the first requested tag without its `-u-` keywords, or the host default. */
pub(super) fn resolve_locale(list: &[String], default: &str) -> String {
    list.first()
        .and_then(|t| tag::parse(t))
        .map(|mut t| {
            t.attributes.clear();
            t.keywords.clear();
            tag::to_text(&t)
        })
        .unwrap_or_else(|| default.into())
}

/** GetOptionsObject: undefined gives an empty object, other non-objects a TypeError. */
pub(super) fn options_object(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    match value {
        JsValue::Undefined => Ok(JsValue::Object(vm.new_object()?)),
        JsValue::Object(_) => Ok(value),
        _ => Err(vm.type_error("Options must be an object")),
    }
}

/** GetOption with a list of allowed strings: the value, or None when undefined. */
pub(super) fn choice(
    vm: &mut Vm,
    options: &JsValue,
    name: &str,
    allowed: &[&str],
) -> Res<Option<String>> {
    let value = vm.get(options, &Key::from(name))?;
    if value == JsValue::Undefined {
        return Ok(None);
    }
    let text = vm.to_text(&value)?;
    if !allowed.contains(&text.as_str()) {
        return Err(vm.range_error(&format!("Value {text} out of range for option {name}")));
    }
    Ok(Some(text))
}

/** `new Intl.DisplayNames(locales, options)`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Constructor Intl.DisplayNames requires 'new'"));
    };
    let fallback_proto = match vm.get(&JsValue::Object(call.callee), &Key::prototype())? {
        JsValue::Object(id) => id,
        _ => realm().ids.object_prototype,
    };
    let proto = vm.prototype_for(&target, fallback_proto)?;
    let list = canonical_list(vm, &call.arg(0))?;
    let options = options_object(vm, call.arg(1))?;
    let mut kept = BTreeMap::new();
    for (name, allowed) in OPTIONS {
        let value = choice(vm, &options, name, allowed)?;
        if *name == "type" && value.is_none() {
            return Err(vm.type_error("Intl.DisplayNames requires a type option"));
        }
        let value = value.unwrap_or_else(|| allowed[0].to_string());
        kept.insert(name.to_string(), value);
    }
    let data = IntlData {
        kind: "DisplayNames".into(),
        locale: resolve_locale(&list, &vm.page.environment.locale),
        options: kept,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Intl(Box::new(data)))),
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** The DisplayNames behind `this`, or a TypeError. */
fn this_data(vm: &mut Vm, call: &Call) -> Res<IntlData> {
    match crate::v9::platform::host_data(vm, &call.this) {
        Some(HostData::Intl(data)) if data.kind == "DisplayNames" => Ok(*data),
        _ => Err(vm.type_error("Method called on incompatible receiver")),
    }
}

/** `resolvedOptions()`: locale, style, type, fallback and, for languages, languageDisplay. */
fn resolved_options(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = this_data(vm, &call)?;
    let id = vm.new_object()?;
    vm.put(id, "locale", JsValue::string(&data.locale)?)?;
    for name in ["style", "type", "fallback", "languageDisplay"] {
        if name == "languageDisplay" && data.options["type"] != "language" {
            continue;
        }
        vm.put(id, name, JsValue::string(&data.options[name])?)?;
    }
    Ok(JsValue::Object(id))
}

/** The canonical code for a type, or None when the code is not well formed. */
fn canonical_code(kind: &str, code: &str) -> Option<String> {
    match kind {
        "language" => {
            let parsed = tag::parse(code)?;
            let plain = parsed.attributes.is_empty()
                && parsed.keywords.is_empty()
                && !parsed.unicode
                && parsed.transform.is_none()
                && parsed.others.is_empty()
                && parsed.private.is_none();
            plain.then(|| tag::to_text(&tag::canonicalize_tag(parsed)))
        }
        "region" => tag::is_region(code).then(|| code.to_ascii_uppercase()),
        "script" => tag::is_script(code).then(|| {
            let lower = code.to_ascii_lowercase();
            lower[..1].to_ascii_uppercase() + &lower[1..]
        }),
        "currency" => (code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphabetic()))
            .then(|| code.to_ascii_uppercase()),
        "calendar" => tag::is_type(code).then(|| code.to_ascii_lowercase()),
        _ => FIELDS.iter().any(|(f, _)| *f == code).then(|| code.into()),
    }
}

/** `of(code)`: the English name, the code, or undefined with `fallback: "none"`. */
fn of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = this_data(vm, &call)?;
    let code = vm.to_text(&call.arg(0))?;
    let kind = data.options["type"].as_str();
    let Some(canonical) = canonical_code(kind, &code) else {
        return Err(vm.range_error(&format!(
            "invalid code for Intl.DisplayNames.prototype.of: {code}"
        )));
    };
    let name = if kind == "dateTimeField" {
        FIELDS
            .iter()
            .find(|(f, _)| *f == canonical)
            .map(|(_, n)| *n)
    } else {
        NAMES
            .iter()
            .find(|(k, c, _)| *k == kind && *c == canonical)
            .map(|(_, _, n)| *n)
    };
    match name {
        Some(name) => Ok(JsValue::string(name)?),
        None if data.options["fallback"] == "code" => Ok(JsValue::string(&canonical)?),
        None => Ok(JsValue::Undefined),
    }
}
