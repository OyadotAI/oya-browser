/**
 * `Intl.Locale` and ECMA-402's CanonicalizeLocaleList, ported from V8's js-locale.cc and
 * intl-objects.cc: the constructor applies the options over the tag in spec order, the getters
 * read the canonical tag back, and `maximize` / `minimize` use the embedded likely-subtag table.
 */
use super::tag::{self, Tag};
use crate::v9::platform::native_name;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData, IntlData};
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, Key, Object};
use crate::v9::vm::realm::{Builder, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};

/** The getters on `Intl.Locale.prototype`. */
const GETTERS: &[(&str, &str)] = &[
    ("baseName", "get baseName"),
    ("calendar", "get calendar"),
    ("caseFirst", "get caseFirst"),
    ("collation", "get collation"),
    ("firstDayOfWeek", "get firstDayOfWeek"),
    ("hourCycle", "get hourCycle"),
    ("numeric", "get numeric"),
    ("numberingSystem", "get numberingSystem"),
    ("language", "get language"),
    ("script", "get script"),
    ("region", "get region"),
    ("variants", "get variants"),
];

/** Keyword options in the order the constructor reads them, with their extension key. */
const KEYWORD_OPTIONS: &[(&str, &str)] = &[
    ("calendar", "ca"),
    ("collation", "co"),
    ("firstDayOfWeek", "fw"),
    ("hourCycle", "hc"),
    ("caseFirst", "kf"),
    ("numeric", "kn"),
    ("numberingSystem", "nu"),
];

/** Weekday numbers `firstDayOfWeek` accepts, as their `fw` values (0 and 7 are Sunday). */
const WEEKDAYS: [&str; 8] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/** Install `Intl.Locale` on the `Intl` object. */
pub(super) fn install(b: &mut Builder, intl: u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    for (name, label) in GETTERS {
        let get = b.function(label, 0, getter);
        b.put(
            proto,
            Key::from(name),
            Static::Accessor(Some(get), None),
            CONFIGURABLE,
        );
    }
    b.method(proto, "toString", 0, to_string);
    b.method(proto, "maximize", 0, maximize);
    b.method(proto, "minimize", 0, minimize);
    super::locale_info::install(b, proto);
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Intl.Locale")),
        CONFIGURABLE,
    );
    let ctor = b.constructor("Locale", 1, construct, proto, None);
    b.value(intl, "Locale", JsValue::Object(ctor), HIDDEN_ATTRS);
}

/** The canonical tag of an `Intl.Locale` object, or None for any other value. */
pub(super) fn locale_of(vm: &Vm, value: &JsValue) -> Option<String> {
    match crate::v9::platform::host_data(vm, value) {
        Some(HostData::Intl(data)) if data.kind == "Locale" => Some(data.locale),
        _ => None,
    }
}

/** ECMA-402 CanonicalizeLocaleList: strings and Locale objects, validated, canonical, deduplicated. */
pub(super) fn canonical_list(vm: &mut Vm, locales: &JsValue) -> Res<Vec<String>> {
    if *locales == JsValue::Undefined {
        return Ok(Vec::new());
    }
    let object = if matches!(locales, JsValue::String(_)) || locale_of(vm, locales).is_some() {
        vm.new_array(vec![locales.clone()])?
    } else {
        vm.to_object(locales)?
    };
    let length = vm.get(&JsValue::Object(object), &Key::from("length"))?;
    let length = vm.to_integer(&length)?.clamp(0.0, 9_007_199_254_740_991.0) as u64;
    let mut seen = Vec::new();
    for index in 0..length {
        let key = Key::from(index.to_string().as_str());
        if !vm.has_property(object, &key)? {
            continue;
        }
        let value = vm.get(&JsValue::Object(object), &key)?;
        let canonical = canonical_one(vm, &value)?;
        if !seen.contains(&canonical) {
            seen.push(canonical);
        }
    }
    Ok(seen)
}

/** One list element: a Locale's tag, or a string checked and canonicalized. */
fn canonical_one(vm: &mut Vm, value: &JsValue) -> Res<String> {
    if !matches!(value, JsValue::String(_) | JsValue::Object(_)) {
        return Err(vm.type_error("Language ID should be string or object."));
    }
    if let Some(tag) = locale_of(vm, value) {
        return Ok(tag);
    }
    let text = vm.to_text(value)?;
    tag::canonicalize(&text)
        .ok_or_else(|| vm.range_error(&format!("Incorrect locale information provided: {text}")))
}

/** GetOption(options, name, string): the option as text, or None when undefined. */
fn string_option(vm: &mut Vm, options: &JsValue, name: &str) -> Res<Option<String>> {
    let value = vm.get(options, &Key::from(name))?;
    if value == JsValue::Undefined {
        return Ok(None);
    }
    vm.to_text(&value).map(Some)
}

/** A RangeError naming the option. */
fn invalid(vm: &mut Vm, name: &str, value: &str) -> crate::v9::vm::Thrown {
    vm.range_error(&format!(
        "Value {value} out of range for Intl.Locale options property {name}"
    ))
}

/** ApplyOptionsToTag: language, script, region and variants over the parsed tag. */
fn apply_base(vm: &mut Vm, options: &JsValue, tag: &mut Tag) -> Res<()> {
    let language = string_option(vm, options, "language")?;
    let script = string_option(vm, options, "script")?;
    let region = string_option(vm, options, "region")?;
    let variants = string_option(vm, options, "variants")?;
    type Check<'a> = (&'a str, &'a Option<String>, fn(&str) -> bool);
    let checks: [Check; 3] = [
        ("language", &language, tag::is_language),
        ("script", &script, tag::is_script),
        ("region", &region, tag::is_region),
    ];
    for (name, value, valid) in checks {
        if let Some(value) = value
            && !valid(value)
        {
            return Err(invalid(vm, name, value));
        }
    }
    if let Some(v) = &variants {
        let list: Vec<String> = v.split('-').map(str::to_ascii_lowercase).collect();
        let unique = list.iter().enumerate().all(|(i, x)| !list[..i].contains(x));
        if !list.iter().all(|x| tag::is_variant(x)) || !unique {
            return Err(invalid(vm, "variants", v));
        }
        tag.variants = list;
    }
    *tag = tag::canonicalize_tag(std::mem::take(tag));
    if let Some(language) = language {
        tag.language = language.to_ascii_lowercase();
    }
    if let Some(script) = script {
        tag.script = Some(script.to_ascii_lowercase());
    }
    if let Some(region) = region {
        tag.region = Some(region.to_ascii_lowercase());
    }
    Ok(())
}

/** The keyword options (calendar through numberingSystem), validated, as extension values. */
fn keyword_values(vm: &mut Vm, options: &JsValue) -> Res<Vec<(&'static str, String)>> {
    let mut out = Vec::new();
    for (name, key) in KEYWORD_OPTIONS {
        let value = if *name == "numeric" {
            let raw = vm.get(options, &Key::from("numeric"))?;
            (raw != JsValue::Undefined).then(|| raw.truthy().to_string())
        } else {
            string_option(vm, options, name)?
        };
        let Some(mut value) = value else { continue };
        if *name == "firstDayOfWeek"
            && let Ok(day) = value.parse::<usize>()
            && value.len() == 1
            && day < WEEKDAYS.len()
        {
            value = WEEKDAYS[day].into();
        }
        let ok = match *name {
            "hourCycle" => ["h11", "h12", "h23", "h24"].contains(&value.as_str()),
            "caseFirst" => ["upper", "lower", "false"].contains(&value.as_str()),
            "numeric" => true,
            _ => tag::is_type(&value),
        };
        if !ok {
            return Err(invalid(vm, name, &value));
        }
        out.push((*key, value.to_ascii_lowercase()));
    }
    Ok(out)
}

/** `new Intl.Locale(tag, options)`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Constructor Intl.Locale requires 'new'"));
    };
    let fallback = match vm.get(&JsValue::Object(call.callee), &Key::from("prototype"))? {
        JsValue::Object(id) => id,
        _ => crate::v9::vm::realm::realm().ids.object_prototype,
    };
    let proto = vm.prototype_for(&target, fallback)?;
    let input = call.arg(0);
    if !matches!(input, JsValue::String(_) | JsValue::Object(_)) {
        return Err(
            vm.type_error("First argument to Intl.Locale constructor can't be empty or missing")
        );
    }
    let text = match locale_of(vm, &input) {
        Some(tag) => tag,
        None => vm.to_text(&input)?,
    };
    let options = match call.arg(1) {
        JsValue::Undefined => JsValue::Object(vm.new_object()?),
        other => JsValue::Object(vm.to_object(&other)?),
    };
    let Some(mut tag) = tag::parse(&text) else {
        return Err(vm.range_error(&format!("Incorrect locale information provided: {text}")));
    };
    apply_base(vm, &options, &mut tag)?;
    for (key, value) in keyword_values(vm, &options)? {
        tag.unicode = true;
        match tag.keywords.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => tag.keywords.push((key.into(), value)),
        }
    }
    let locale = tag::to_text(&tag::canonicalize_tag(tag));
    let data = IntlData {
        kind: "Locale".into(),
        locale,
        options: Default::default(),
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Intl(Box::new(data)))),
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** The parsed tag behind `this`, or a TypeError. */
fn this_tag(vm: &mut Vm, call: &Call) -> Res<Tag> {
    match locale_of(vm, &call.this).and_then(|text| tag::parse(&text)) {
        Some(tag) => Ok(tag),
        None => Err(vm.type_error("Method Intl.Locale.prototype called on incompatible receiver")),
    }
}

/** Text or undefined. */
fn optional(value: Option<String>) -> Res<JsValue> {
    match value {
        Some(text) => Ok(JsValue::string(&text)?),
        None => Ok(JsValue::Undefined),
    }
}

/** Every `Intl.Locale.prototype` getter, dispatched on its name. */
fn getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let tag = this_tag(vm, &call)?;
    let name = native_name(&call).trim_start_matches("get ");
    let keyword = |key: &str| {
        tag.keywords
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    let base = tag::base_name(&tag);
    let pieces: Vec<&str> = base.split('-').collect();
    match name {
        "baseName" => optional(Some(base.clone())),
        "language" => optional(Some(tag.language.clone())),
        "script" => optional(tag.script.as_ref().map(|_| pieces[1].to_string())),
        "region" => optional(tag.region.as_ref().map(|r| r.to_ascii_uppercase())),
        "variants" => optional((!tag.variants.is_empty()).then(|| tag.variants.join("-"))),
        "numeric" => Ok(JsValue::Boolean(
            keyword("kn").is_some_and(|v| v.is_empty() || v == "true"),
        )),
        _ => {
            let key = KEYWORD_OPTIONS
                .iter()
                .find(|(n, _)| *n == name)
                .map_or("", |(_, k)| *k);
            optional(keyword(key))
        }
    }
}

/** `toString()`: the canonical tag. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let tag = this_tag(vm, &call)?;
    Ok(JsValue::string(&tag::to_text(&tag))?)
}

/** A new Locale object for a tag, with this realm's prototype. */
fn new_locale(vm: &mut Vm, call: &Call, tag: &Tag) -> Res<JsValue> {
    let proto = call.this.object().and_then(|id| vm.heap.get(id)?.proto);
    let data = IntlData {
        kind: "Locale".into(),
        locale: tag::to_text(tag),
        options: Default::default(),
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Intl(Box::new(data)))),
        proto,
    ))?;
    Ok(JsValue::Object(id))
}

/** `maximize()`: likely subtags added. */
fn maximize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let tag = this_tag(vm, &call)?;
    new_locale(vm, &call, &tag::maximize(&tag))
}

/** `minimize()`: likely subtags removed. */
fn minimize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let tag = this_tag(vm, &call)?;
    new_locale(vm, &call, &tag::minimize(&tag))
}
