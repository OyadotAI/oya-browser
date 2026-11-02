/** Temporal (TC39 proposal, as V8 ships it over temporal_rs): the ISO 8601 calendar, UTC and fixed
offsets. Each Temporal object keeps its internal slots as integers in a host record. */
mod duration;
mod instant;
mod iso;
mod month;
mod plain;
pub(super) mod zone;
mod zone_rules;
mod zone_source;
mod zoned;

use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::convert::Hint;
use crate::v9::vm::data::{Class, HostData, IntlData};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{NativeFn, Static, string, symbol};
use std::sync::OnceLock;

/** The Temporal classes, in prototype-table order. */
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Date,
    Time,
    DateTime,
    YearMonth,
    MonthDay,
    Duration,
    Instant,
    Zoned,
}

/** Constructor names, in `Kind` order. */
const NAMES: [&str; 8] = [
    "PlainDate",
    "PlainTime",
    "PlainDateTime",
    "PlainYearMonth",
    "PlainMonthDay",
    "Duration",
    "Instant",
    "ZonedDateTime",
];

/** A Temporal object's slots: integers in a per-kind layout, and the time zone of a ZonedDateTime. */
#[derive(Clone, Debug)]
pub(super) struct T {
    pub kind: Kind,
    pub v: Vec<i128>,
    pub tz: String,
}

impl T {
    /** The ISO date fields. */
    pub fn date(&self) -> (i64, i64, i64) {
        (self.v[0] as i64, self.v[1] as i64, self.v[2] as i64)
    }
    /** The clock fields of a PlainTime or PlainDateTime. */
    pub fn time(&self) -> [i64; 6] {
        let at = if self.kind == Kind::Time { 0 } else { 3 };
        let mut out = [0; 6];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.v[at + i] as i64;
        }
        out
    }
}

/** The prototypes, recorded once at install so constructors can find them. */
static PROTOS: OnceLock<[ObjectId; 8]> = OnceLock::new();

/** A new Temporal object with `new_target`'s prototype, or the kind's own. */
pub(super) fn make(vm: &mut Vm, target: Option<&JsValue>, t: T) -> Res<JsValue> {
    let fallback = vm.intrinsic_id(PROTOS.get().expect("temporal installed")[t.kind as usize]);
    let proto = match target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => fallback,
    };
    let text: Vec<String> = t.v.iter().map(i128::to_string).collect();
    let mut options = std::collections::BTreeMap::new();
    options.insert("v".to_string(), text.join(","));
    let data = IntlData {
        kind: format!("Temporal.{}", NAMES[t.kind as usize]),
        locale: t.tz,
        options,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Intl(Box::new(data)))),
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** The Temporal slots behind a value, if it is a Temporal object. */
pub(super) fn slots(vm: &Vm, value: &JsValue) -> Option<T> {
    let JsValue::Object(id) = value else {
        return None;
    };
    let Some(Class::Host(host)) = vm.heap.get(*id).map(|o| &o.class) else {
        return None;
    };
    let HostData::Intl(data) = host.as_ref() else {
        return None;
    };
    let name = data.kind.strip_prefix("Temporal.")?;
    let kind = NAMES.iter().position(|n| *n == name)?;
    let v = data
        .options
        .get("v")?
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    Some(T {
        kind: KINDS[kind],
        v,
        tz: data.locale.clone(),
    })
}

/** Kinds by index. */
const KINDS: [Kind; 8] = [
    Kind::Date,
    Kind::Time,
    Kind::DateTime,
    Kind::YearMonth,
    Kind::MonthDay,
    Kind::Duration,
    Kind::Instant,
    Kind::Zoned,
];

/** `this` as a Temporal object of one kind, or the TypeError every method's branding check throws. */
pub(super) fn this_t(vm: &mut Vm, call: &Call, kind: Kind) -> Res<T> {
    match slots(vm, &call.this) {
        Some(t) if t.kind == kind => Ok(t),
        _ => Err(vm.type_error(&format!(
            "receiver is not a Temporal.{}",
            NAMES[kind as usize]
        ))),
    }
}

/** A Temporal argument of a kind, if it is one. */
pub(super) fn as_kind(vm: &Vm, value: &JsValue, kind: Kind) -> Option<T> {
    slots(vm, value).filter(|t| t.kind == kind)
}

/** ToIntegerWithTruncation: NaN and infinities are RangeErrors. */
pub(super) fn to_int(vm: &mut Vm, value: &JsValue) -> Res<i64> {
    let n = vm.to_number(value)?;
    if !n.is_finite() {
        return Err(vm.range_error("Temporal field must be a finite number"));
    }
    Ok(clamp_i64(n.trunc()))
}

/** A truncated float as i64, saturating far outside any valid field. */
fn clamp_i64(n: f64) -> i64 {
    n.clamp(-9.0e15, 9.0e15) as i64
}

/** ToPositiveIntegerWithTruncation. */
pub(super) fn to_positive(vm: &mut Vm, value: &JsValue) -> Res<i64> {
    let n = to_int(vm, value)?;
    if n <= 0 {
        return Err(vm.range_error("Temporal field must be positive"));
    }
    Ok(n)
}

/** GetOptionsObject: undefined means none; anything but an object is a TypeError. */
pub(super) fn options(vm: &mut Vm, value: &JsValue) -> Res<Option<ObjectId>> {
    match value {
        JsValue::Undefined => Ok(None),
        JsValue::Object(id) => Ok(Some(*id)),
        _ => Err(vm.type_error("options must be an object")),
    }
}

/** GetOption for a string-valued option among `allowed`. */
pub(super) fn string_option(
    vm: &mut Vm,
    opts: Option<ObjectId>,
    name: &str,
    allowed: &[&str],
    default: Option<&str>,
) -> Res<Option<String>> {
    let Some(id) = opts else {
        return Ok(default.map(str::to_string));
    };
    let value = vm.get(&JsValue::Object(id), &Key::from(name))?;
    if matches!(value, JsValue::Undefined) {
        return Ok(default.map(str::to_string));
    }
    let text = vm.to_text(&value)?;
    if !allowed.contains(&text.as_str()) {
        return Err(vm.range_error(&format!("{text} is not a valid value for {name}")));
    }
    Ok(Some(text))
}

/** GetTemporalOverflowOption: true for constrain. */
pub(super) fn overflow(vm: &mut Vm, opts: Option<ObjectId>) -> Res<bool> {
    let value = string_option(
        vm,
        opts,
        "overflow",
        &["constrain", "reject"],
        Some("constrain"),
    )?;
    Ok(value.as_deref() == Some("constrain"))
}

/** The rounding modes Temporal accepts. */
pub(super) const ROUNDING_MODES: [&str; 9] = [
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

/** A calendar argument: only the ISO 8601 calendar is supported. */
pub(super) fn calendar_arg(vm: &mut Vm, value: &JsValue) -> Res<()> {
    match value {
        JsValue::Undefined => Ok(()),
        JsValue::String(text) => check_calendar(vm, &text.lossy()),
        _ => Err(vm.type_error("calendar must be a string")),
    }
}

/** CanonicalizeCalendar for the ISO calendar. */
pub(super) fn check_calendar(vm: &mut Vm, id: &str) -> Res<()> {
    if id.eq_ignore_ascii_case("iso8601") {
        return Ok(());
    }
    Err(vm.range_error(&format!("unsupported calendar {id}")))
}

/** A string argument for `from`, or the TypeError non-strings get. */
pub(super) fn require_string(vm: &mut Vm, value: &JsValue) -> Res<String> {
    match value {
        JsValue::String(text) => Ok(text.lossy()),
        _ => Err(vm.type_error("expected a string or a Temporal object")),
    }
}

/** A RangeError for a string that is not ISO 8601 for this type. */
pub(super) fn bad_string(vm: &mut Vm, s: &str) -> Thrown {
    vm.range_error(&format!("invalid ISO 8601 string: {s}"))
}

/** A string value. */
pub(super) fn js(text: &str) -> Res<JsValue> {
    Ok(JsValue::String(JsString::from_utf8(text)?))
}

/** valueOf on every Temporal type: comparison must go through compare/equals. */
fn value_of(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    Err(vm.type_error("use compare() or equals() to compare Temporal values"))
}

/** A method table entry. */
pub(super) type Methods = &'static [(&'static str, u32, NativeFn)];

/** A Temporal class: constructor, statics, prototype methods and getters. */
#[allow(clippy::too_many_arguments)]
pub(super) fn class(
    b: &mut Builder,
    ns: ObjectId,
    kind: Kind,
    ctor_fn: NativeFn,
    length: u32,
    statics: Methods,
    methods: Methods,
    getters: &[(&'static str, NativeFn)],
) -> ObjectId {
    let name = NAMES[kind as usize];
    let proto = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor(name, length, ctor_fn, proto, None);
    b.alias(ns, name, ctor);
    for (method, len, f) in statics {
        b.method(ctor, method, *len, *f);
    }
    for (method, len, f) in methods {
        b.method(proto, method, *len, *f);
    }
    b.method(proto, "valueOf", 0, value_of);
    for (getter, f) in getters {
        b.builtin_accessor(proto, getter, Some(*f), None);
    }
    let tag = format!("Temporal.{name}");
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(&tag)),
        CONFIGURABLE,
    );
    proto
}

/** The Temporal namespace and every class on it. */
pub(super) fn install(b: &mut Builder) {
    let ns = b.object(Some(b.ids.object_prototype));
    b.global("Temporal", ns);
    b.put(
        ns,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Temporal")),
        CONFIGURABLE,
    );
    let mut protos = [0; 8];
    plain::install(b, ns, &mut protos);
    duration::install(b, ns, &mut protos);
    instant::install(b, ns, &mut protos);
    zoned::install(b, ns, &mut protos);
    month::install(b, ns, &mut protos);
    let _ = PROTOS.set(protos);
}

/** ToPrimitive for a monthCode field: it must come out a string. */
pub(super) fn to_month_code(vm: &mut Vm, value: &JsValue) -> Res<i64> {
    let prim = vm.to_primitive(value, Hint::String)?;
    let JsValue::String(text) = prim else {
        return Err(vm.type_error("monthCode must be a string"));
    };
    let text = text.lossy();
    let b = text.as_bytes();
    let well_formed =
        b.len() == 3 && b[0] == b'M' && b[1].is_ascii_digit() && b[2].is_ascii_digit();
    let month = if well_formed {
        i64::from(b[1] - b'0') * 10 + i64::from(b[2] - b'0')
    } else {
        0
    };
    if !(1..=12).contains(&month) {
        return Err(vm.range_error(&format!("invalid monthCode {text}")));
    }
    Ok(month)
}

/** The wall-clock date and time of a ZonedDateTime: epoch nanoseconds plus its offset. */
pub(super) fn zoned_parts(t: &T) -> ((i64, i64, i64), [i64; 6]) {
    instant::parts(t.v[0] + t.v.get(1).copied().unwrap_or(0))
}
