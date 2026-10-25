/**
 * Intl Locale Info (`getCalendars`, `getCollations`, `getHourCycles`, `getNumberingSystems`,
 * `getTextInfo`, `getTimeZones`, `getWeekInfo`), as V8's js-locale.cc answers them: the tag's
 * own keyword wins, otherwise the maximized region's CLDR preference from the small embedded
 * tables below; regions outside them get CLDR's world (001) defaults.
 */
use super::locale::locale_of;
use super::tag::{self, Tag};
use crate::v9::platform::native_name;
use crate::v9::value::JsValue;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};

/** The methods, all dispatched through `info`. */
const METHODS: &[&str] = &[
    "getCalendars",
    "getCollations",
    "getHourCycles",
    "getNumberingSystems",
    "getTextInfo",
    "getTimeZones",
    "getWeekInfo",
];

/** Regions whose preferred hour cycle is h12 (CLDR timeData, the common ones). */
const H12_REGIONS: &[&str] = &[
    "US", "CA", "AU", "NZ", "IN", "PH", "EG", "SA", "PK", "MX", "CO", "KR", "TW", "BD",
];

/** Calendars CLDR prefers per region beyond the Gregorian default. */
const CALENDARS: &[(&str, &[&str])] = &[
    ("TH", &["buddhist", "gregory"]),
    ("JP", &["gregory", "japanese"]),
    ("TW", &["gregory", "roc", "chinese"]),
    (
        "SA",
        &["islamic-umalqura", "gregory", "islamic", "islamic-rgsa"],
    ),
    (
        "IR",
        &[
            "persian",
            "gregory",
            "islamic",
            "islamic-civil",
            "islamic-tbla",
        ],
    ),
    (
        "IL",
        &[
            "gregory",
            "hebrew",
            "islamic",
            "islamic-civil",
            "islamic-tbla",
        ],
    ),
];

/** Scripts written right to left (CLDR scriptMetadata). */
const RTL_SCRIPTS: &[&str] = &[
    "Arab", "Hebr", "Syrc", "Thaa", "Nkoo", "Adlm", "Rohg", "Mand", "Samr", "Mend", "Hung", "Phnx",
];

/** IANA zones per region (CLDR / tzdb zone.tab, the embedded regions). */
const ZONES: &[(&str, &[&str])] = &[
    ("GB", &["Europe/London"]),
    ("DE", &["Europe/Berlin", "Europe/Busingen"]),
    ("FR", &["Europe/Paris"]),
    ("ES", &["Africa/Ceuta", "Atlantic/Canary", "Europe/Madrid"]),
    ("IT", &["Europe/Rome"]),
    ("JP", &["Asia/Tokyo"]),
    ("CN", &["Asia/Shanghai", "Asia/Urumqi"]),
    ("IN", &["Asia/Kolkata"]),
    ("EG", &["Africa/Cairo"]),
    ("RU", &["Europe/Moscow"]),
    ("BR", &["America/Sao_Paulo"]),
    ("AU", &["Australia/Sydney"]),
    (
        "US",
        &[
            "America/Chicago",
            "America/Denver",
            "America/Los_Angeles",
            "America/New_York",
            "Pacific/Honolulu",
        ],
    ),
];

/** Regions whose week starts on Sunday with one minimal day (CLDR weekData). */
const SUNDAY_FIRST: &[&str] = &[
    "US", "CA", "JP", "BR", "IN", "IL", "KR", "MX", "PH", "TW", "TH", "SA", "ZA", "AU",
];

/** Install the methods on `Intl.Locale.prototype`. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    for name in METHODS {
        b.method(proto, name, 0, info);
    }
}

/** A string array. */
fn strings(vm: &mut Vm, items: &[&str]) -> Res<JsValue> {
    let values = items
        .iter()
        .map(|s| JsValue::string(s))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** The region a preference reads: the tag's own, or the likely one. */
fn region(tag: &Tag) -> String {
    tag.region
        .clone()
        .or_else(|| tag::maximize(tag).region)
        .unwrap_or_default()
        .to_ascii_uppercase()
}

/** A keyword's value on the tag. */
fn keyword<'a>(tag: &'a Tag, key: &str) -> Option<&'a str> {
    tag.keywords
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/** Every info method, dispatched on its name. */
fn info(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(tag) = locale_of(vm, &call.this).and_then(|t| tag::parse(&t)) else {
        return Err(vm.type_error("Method Intl.Locale.prototype called on incompatible receiver"));
    };
    let region = region(&tag);
    match native_name(&call) {
        "getCalendars" => match keyword(&tag, "ca") {
            Some(ca) => strings(vm, &[ca]),
            None => {
                let found = CALENDARS.iter().find(|(r, _)| *r == region);
                strings(vm, found.map_or(&["gregory"][..], |(_, list)| list))
            }
        },
        "getCollations" => match keyword(&tag, "co").filter(|c| !c.is_empty()) {
            Some(co) => strings(vm, &[co]),
            None => strings(vm, &["emoji", "eor"]),
        },
        "getHourCycles" => match keyword(&tag, "hc") {
            Some(hc) => strings(vm, &[hc]),
            None if H12_REGIONS.contains(&region.as_str()) => strings(vm, &["h12"]),
            None => strings(vm, &["h23"]),
        },
        "getNumberingSystems" => match keyword(&tag, "nu").filter(|n| !n.is_empty()) {
            Some(nu) => strings(vm, &[nu]),
            None => strings(vm, &["latn"]),
        },
        "getTimeZones" => {
            if tag.region.is_none() {
                return Ok(JsValue::Undefined);
            }
            let found = ZONES.iter().find(|(r, _)| *r == region);
            strings(vm, found.map_or(&["Etc/UTC"][..], |(_, list)| list))
        }
        "getTextInfo" => text_info(vm, &tag),
        _ => week_info(vm, &tag, &region),
    }
}

/** `{direction}` from the maximized script. */
fn text_info(vm: &mut Vm, tag: &Tag) -> Res<JsValue> {
    let max = tag::maximize(tag);
    let script = max.script.map(|s| {
        let mut chars = s.chars();
        chars.next().map_or(String::new(), |c| {
            c.to_ascii_uppercase().to_string() + chars.as_str()
        })
    });
    let rtl = script.is_some_and(|s| RTL_SCRIPTS.contains(&s.as_str()));
    let id = vm.new_object()?;
    vm.put(
        id,
        "direction",
        JsValue::string(if rtl { "rtl" } else { "ltr" })?,
    )?;
    Ok(JsValue::Object(id))
}

/** `{firstDay, weekend, minimalDays}`, with `fw` overriding the first day. */
fn week_info(vm: &mut Vm, tag: &Tag, region: &str) -> Res<JsValue> {
    let sunday = SUNDAY_FIRST.contains(&region);
    let days = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    let first = keyword(tag, "fw")
        .and_then(|fw| days.iter().position(|d| *d == fw))
        .map_or(if sunday { 7 } else { 1 }, |i| i + 1);
    let minimal = if sunday || region.is_empty() { 1 } else { 4 };
    let id = vm.new_object()?;
    vm.put(id, "firstDay", JsValue::Number(first as f64))?;
    let weekend = vm.new_array(vec![JsValue::Number(6.0), JsValue::Number(7.0)])?;
    vm.put(id, "weekend", JsValue::Object(weekend))?;
    vm.put(id, "minimalDays", JsValue::Number(f64::from(minimal)))?;
    Ok(JsValue::Object(id))
}
