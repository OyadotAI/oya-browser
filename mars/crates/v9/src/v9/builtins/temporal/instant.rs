/** Temporal.Instant and Temporal.Now: exact time as BigInt nanoseconds, formatted in UTC or a fixed
offset, and the clock read from the page's virtual time. */
use super::duration::{self, Dur, UNIT_NS};
use super::iso::{self, NS_DAY, NS_MAX};
use super::plain;
use super::zone;
use super::*;

/** A new Instant, or the RangeError outside the representable range. */
pub(super) fn new_instant(vm: &mut Vm, target: Option<&JsValue>, ns: i128) -> Res<JsValue> {
    if ns.abs() > NS_MAX {
        return Err(vm.range_error("instant outside the supported range"));
    }
    make(
        vm,
        target,
        T {
            kind: Kind::Instant,
            v: vec![ns],
            tz: String::new(),
        },
    )
}

/** ToTemporalInstant: an Instant, a ZonedDateTime, or a string with Z or an offset. */
fn to_instant(vm: &mut Vm, item: &JsValue) -> Res<i128> {
    if let Some(t) = slots(vm, item).filter(|t| matches!(t.kind, Kind::Instant | Kind::Zoned)) {
        return Ok(t.v[0]);
    }
    let item = if matches!(item, JsValue::Object(_)) {
        vm.to_primitive(item, Hint::String)?
    } else {
        item.clone()
    };
    let s = require_string(vm, &item)?;
    let p = iso::parse_datetime(&s).filter(|p| p.time.is_some() && (p.z || p.offset.is_some()));
    let p = p.ok_or_else(|| bad_string(vm, &s))?;
    check_calendar(vm, p.calendar.as_deref().unwrap_or("iso8601"))?;
    let offset = if p.z { 0 } else { p.offset.map_or(0, |o| o.0) };
    let ns = iso::epoch_ns(p.date.unwrap_or_default(), &p.time.unwrap_or_default()) - offset;
    if ns.abs() > NS_MAX {
        return Err(vm.range_error("instant outside the supported range"));
    }
    Ok(ns)
}

/** `new Temporal.Instant(epochNanoseconds)`. */
fn ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Temporal.Instant requires 'new'"));
    };
    let ns = super::super::bigint::to_bigint_checked(vm, &call.arg(0))?;
    new_instant(vm, Some(&target), ns)
}

/** Instant.from. */
fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let ns = to_instant(vm, &call.arg(0))?;
    new_instant(vm, None, ns)
}

/** Instant.fromEpochMilliseconds. */
fn from_ms(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let n = vm.to_number(&call.arg(0))?;
    if !n.is_finite() || n.trunc() != n {
        return Err(vm.range_error("epoch milliseconds must be an integer"));
    }
    new_instant(vm, None, (n as i128).saturating_mul(1_000_000))
}

/** Instant.fromEpochNanoseconds. */
fn from_ns(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let ns = super::super::bigint::to_bigint_checked(vm, &call.arg(0))?;
    new_instant(vm, None, ns)
}

/** Instant.compare. */
fn compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = to_instant(vm, &call.arg(0))?;
    let b = to_instant(vm, &call.arg(1))?;
    Ok(JsValue::Number(a.cmp(&b) as i32 as f64))
}

/** Instant.prototype.equals. */
fn equals(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let other = to_instant(vm, &call.arg(0))?;
    Ok(JsValue::Boolean(t.v[0] == other))
}

/** Instant.prototype.epochMilliseconds. */
fn epoch_ms(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    Ok(JsValue::Number(t.v[0].div_euclid(1_000_000) as f64))
}

/** Instant.prototype.epochNanoseconds. */
fn epoch_ns(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    Ok(JsValue::BigInt(t.v[0]))
}

/** Instant add and subtract: only time units. */
fn add_sub(vm: &mut Vm, call: Call, sign: i128) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let d: Dur = duration::to_duration(vm, &call.arg(0))?;
    if d[..4].iter().any(|v| *v != 0) {
        return Err(vm.range_error("Instant arithmetic takes only time units"));
    }
    new_instant(vm, None, t.v[0] + sign * duration::time_ns(&d))
}

/** Instant.prototype.add. */
fn add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    add_sub(vm, call, 1)
}

/** Instant.prototype.subtract. */
fn subtract(vm: &mut Vm, call: Call) -> Res<JsValue> {
    add_sub(vm, call, -1)
}

/** DifferenceTemporalInstant. */
fn diff(vm: &mut Vm, call: Call, since: bool) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let other = to_instant(vm, &call.arg(0))?;
    let s = duration::difference_settings(vm, &call.arg(1), since, 4, 9, 6)?;
    let ns = duration::round_ns(other - t.v[0], &s);
    duration::new_signed(vm, duration::balance(ns, s.largest), since)
}

/** Instant.prototype.until. */
fn until(vm: &mut Vm, call: Call) -> Res<JsValue> {
    diff(vm, call, false)
}

/** Instant.prototype.since. */
fn since(vm: &mut Vm, call: Call) -> Res<JsValue> {
    diff(vm, call, true)
}

/** Instant.prototype.round: increments must divide a solar day. */
fn round(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let (inc, mode) = duration::round_to(vm, &call.arg(0), 4, false)?;
    if NS_DAY % inc != 0 {
        return Err(vm.range_error("roundingIncrement must divide a day"));
    }
    new_instant(vm, None, iso::round_by(t.v[0], inc, &mode))
}

/** ToTemporalTimeZoneIdentifier: the canonical id and the offset in force at epoch nanoseconds `ns`. */
pub(super) fn time_zone(vm: &mut Vm, value: &JsValue, ns: i128) -> Res<(String, i128)> {
    if let Some(t) = slots(vm, value).filter(|t| t.kind == Kind::Zoned) {
        return Ok(named(&t.tz, ns).unwrap_or((t.tz.clone(), t.v[1])));
    }
    let JsValue::String(text) = value else {
        return Err(vm.type_error("time zone must be a string"));
    };
    let s = text.lossy();
    named(&s, ns)
        .or_else(|| zone_id(&s))
        .ok_or_else(|| vm.range_error(&format!("unsupported time zone {s}")))
}

/** A shipped IANA zone's canonical id and its offset at `ns`; None for anything that is not a shipped zone. */
fn named(s: &str, ns: i128) -> Option<(String, i128)> {
    let zone = zone::lookup(s)?;
    Some((zone.id().to_string(), zone.offset_ns(ns)))
}

/** Instant.prototype.toZonedDateTimeISO: the same instant, read in a zone's rules at that instant. */
fn to_zoned(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let (id, offset) = time_zone(vm, &call.arg(0), t.v[0])?;
    make(
        vm,
        None,
        T {
            kind: Kind::Zoned,
            v: vec![t.v[0], offset],
            tz: id,
        },
    )
}

/** A time zone string: UTC, an offset, or an ISO string carrying one. */
fn zone_id(s: &str) -> Option<(String, i128)> {
    if s.eq_ignore_ascii_case("utc")
        || s.eq_ignore_ascii_case("etc/utc")
        || s.eq_ignore_ascii_case("etc/gmt")
    {
        return Some(("UTC".into(), 0));
    }
    if s.starts_with(['+', '-']) {
        let p = iso::parse_time_only(&format!("00:00{s}"))?;
        let (ns, sub) = p.offset?;
        return (!sub).then(|| (iso::format_offset(ns), ns));
    }
    let p = iso::parse_datetime(s)?;
    if let Some(tz) = p.tz {
        return zone_id(&tz);
    }
    if p.z {
        return Some(("UTC".into(), 0));
    }
    p.offset
        .filter(|o| !o.1)
        .map(|(ns, _)| (iso::format_offset(ns), ns))
}

/** Instant.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let o = options(vm, &call.arg(0))?;
    let p = duration::precision_options(vm, o)?;
    let tz = match o {
        Some(id) => vm.get(&JsValue::Object(id), &Key::from("timeZone"))?,
        None => JsValue::Undefined,
    };
    let zone = if matches!(tz, JsValue::Undefined) {
        None
    } else {
        Some(time_zone(vm, &tz, t.v[0])?)
    };
    let ns = iso::round_by(t.v[0], p.increment, &p.mode);
    if ns.abs() > NS_MAX {
        return Err(vm.range_error("rounded instant is out of range"));
    }
    let offset = zone.as_ref().map_or(0, |z| z.1);
    let (date, time) = parts(ns + offset);
    let suffix = match zone {
        None => "Z".to_string(),
        Some(_) => iso::format_offset(offset),
    };
    js(&format!(
        "{}T{}{suffix}",
        plain::format_date(date),
        iso::format_time(&time, p.digits)
    ))
}

/** The ISO date and time of epoch nanoseconds in UTC. */
pub(super) fn parts(ns: i128) -> ((i64, i64, i64), [i64; 6]) {
    let (days, time) = iso::balance_time(ns);
    (iso::from_epoch_days(days), time)
}

/** Instant toJSON and toLocaleString. */
fn to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Instant)?;
    let (date, time) = parts(t.v[0]);
    js(&format!(
        "{}T{}Z",
        plain::format_date(date),
        iso::format_time(&time, None)
    ))
}

/** The current time in epoch nanoseconds. */
fn now_ns(vm: &Vm) -> i128 {
    (super::super::date::now_ms(vm) as i128) * UNIT_NS[7]
}

/** Temporal.Now.instant. */
fn now_instant(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    let ns = now_ns(vm);
    new_instant(vm, None, ns)
}

/** Temporal.Now.timeZoneId: the host zone, which V9 reports as UTC. */
fn now_zone(_vm: &mut Vm, _call: Call) -> Res<JsValue> {
    js("UTC")
}

/** The current local date-time in an optional zone. */
fn now_local(vm: &mut Vm, call: &Call) -> Res<((i64, i64, i64), [i64; 6])> {
    let ns = now_ns(vm);
    let offset = match call.arg(0) {
        JsValue::Undefined => 0,
        v => time_zone(vm, &v, ns)?.1,
    };
    Ok(parts(ns + offset))
}

/** Temporal.Now.plainDateTimeISO. */
fn now_datetime(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (d, t) = now_local(vm, &call)?;
    plain::new_datetime(vm, None, d, t)
}

/** Temporal.Now.plainDateISO. */
fn now_date(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (d, _) = now_local(vm, &call)?;
    plain::new_date(vm, None, d)
}

/** Temporal.Now.plainTimeISO. */
fn now_time(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, t) = now_local(vm, &call)?;
    plain::new_time(vm, None, t)
}

/** Install Temporal.Instant and Temporal.Now. */
pub(super) fn install(b: &mut Builder, ns: ObjectId, protos: &mut [ObjectId; 8]) {
    protos[Kind::Instant as usize] = class(
        b,
        ns,
        Kind::Instant,
        ctor,
        1,
        &[
            ("from", 1, from),
            ("fromEpochMilliseconds", 1, from_ms),
            ("fromEpochNanoseconds", 1, from_ns),
            ("compare", 2, compare),
        ],
        &[
            ("add", 1, add),
            ("subtract", 1, subtract),
            ("until", 1, until),
            ("since", 1, since),
            ("round", 1, round),
            ("equals", 1, equals),
            ("toString", 0, to_string),
            ("toZonedDateTimeISO", 1, to_zoned),
            ("toLocaleString", 0, to_json),
            ("toJSON", 0, to_json),
        ],
        &[
            ("epochMilliseconds", epoch_ms),
            ("epochNanoseconds", epoch_ns),
        ],
    );
    let now = b.object(Some(b.ids.object_prototype));
    b.alias(ns, "Now", now);
    b.put(
        now,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Temporal.Now")),
        CONFIGURABLE,
    );
    b.method(now, "instant", 0, now_instant);
    b.method(now, "timeZoneId", 0, now_zone);
    b.method(now, "plainDateTimeISO", 0, now_datetime);
    b.method(now, "plainDateISO", 0, now_date);
    b.method(now, "plainTimeISO", 0, now_time);
}
