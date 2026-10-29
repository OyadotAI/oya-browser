/** Date over the page virtual clock and validated host timezone transitions or a fixed offset. */
mod legacy;
mod timezone;
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{NativeFn, realm, symbol};
pub(in crate::v9) use timezone::validate as validate_timezone;

/** Milliseconds per day. */
const DAY: f64 = 86_400_000.0;
/** Milliseconds per hour. */
const HOUR: f64 = 3_600_000.0;
/** Milliseconds per minute. */
const MINUTE: f64 = 60_000.0;
/** Milliseconds per second. */
const SECOND: f64 = 1000.0;
/** The largest time value. */
const MAX_TIME: f64 = 8.64e15;
/** Annex B legacy setters interpret integer years zero through 99 relative to 1900. */
const LEGACY_YEAR_OFFSET: f64 = 1900.0;
/** Largest legacy short year before ordinary full-year interpretation applies. */
const LEGACY_YEAR_LAST: f64 = 99.0;
/** Day names. */
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
/** Month names. */
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/** Date and Date.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.date_prototype = proto;
    let ctor = b.constructor("Date", 7, construct, proto, None);
    b.global("Date", ctor);
    b.method(ctor, "now", 0, now);
    b.method(ctor, "parse", 1, parse);
    b.method(ctor, "UTC", 7, utc);
    let getters: [(&'static str, NativeFn); 20] = [
        ("getTime", get_time),
        ("valueOf", get_time),
        ("getFullYear", |vm, call| field(vm, call, Field::Year, true)),
        ("getMonth", |vm, call| field(vm, call, Field::Month, true)),
        ("getDate", |vm, call| field(vm, call, Field::Date, true)),
        ("getDay", |vm, call| field(vm, call, Field::Day, true)),
        ("getHours", |vm, call| field(vm, call, Field::Hours, true)),
        ("getMinutes", |vm, call| {
            field(vm, call, Field::Minutes, true)
        }),
        ("getSeconds", |vm, call| {
            field(vm, call, Field::Seconds, true)
        }),
        ("getMilliseconds", |vm, call| {
            field(vm, call, Field::Millis, true)
        }),
        ("getUTCFullYear", |vm, call| {
            field(vm, call, Field::Year, false)
        }),
        ("getUTCMonth", |vm, call| {
            field(vm, call, Field::Month, false)
        }),
        ("getUTCDate", |vm, call| field(vm, call, Field::Date, false)),
        ("getUTCDay", |vm, call| field(vm, call, Field::Day, false)),
        ("getUTCHours", |vm, call| {
            field(vm, call, Field::Hours, false)
        }),
        ("getUTCMinutes", |vm, call| {
            field(vm, call, Field::Minutes, false)
        }),
        ("getUTCSeconds", |vm, call| {
            field(vm, call, Field::Seconds, false)
        }),
        ("getUTCMilliseconds", |vm, call| {
            field(vm, call, Field::Millis, false)
        }),
        ("getTimezoneOffset", timezone_offset),
        ("getYear", |vm, call| {
            let year = field(vm, call, Field::Year, true)?;
            Ok(match year {
                #[cfg(feature = "conformance")]
                JsValue::Smi(year) => JsValue::Number(year - 1900.0),
                JsValue::Number(year) => JsValue::Number(year - 1900.0),
                other => other,
            })
        }),
    ];
    for (name, function) in getters {
        b.method(proto, name, 0, function);
    }
    let setters: [(&'static str, u32, NativeFn); 15] = [
        ("setTime", 1, set_time),
        ("setMilliseconds", 1, |vm, call| {
            set(vm, call, Field::Millis, true)
        }),
        ("setSeconds", 2, |vm, call| {
            set(vm, call, Field::Seconds, true)
        }),
        ("setMinutes", 3, |vm, call| {
            set(vm, call, Field::Minutes, true)
        }),
        ("setHours", 4, |vm, call| set(vm, call, Field::Hours, true)),
        ("setDate", 1, |vm, call| set(vm, call, Field::Date, true)),
        ("setMonth", 2, |vm, call| set(vm, call, Field::Month, true)),
        ("setFullYear", 3, |vm, call| {
            set(vm, call, Field::Year, true)
        }),
        ("setUTCMilliseconds", 1, |vm, call| {
            set(vm, call, Field::Millis, false)
        }),
        ("setUTCSeconds", 2, |vm, call| {
            set(vm, call, Field::Seconds, false)
        }),
        ("setUTCMinutes", 3, |vm, call| {
            set(vm, call, Field::Minutes, false)
        }),
        ("setUTCHours", 4, |vm, call| {
            set(vm, call, Field::Hours, false)
        }),
        ("setUTCDate", 1, |vm, call| {
            set(vm, call, Field::Date, false)
        }),
        ("setUTCMonth", 2, |vm, call| {
            set(vm, call, Field::Month, false)
        }),
        ("setUTCFullYear", 3, |vm, call| {
            set(vm, call, Field::Year, false)
        }),
    ];
    for (name, length, function) in setters {
        b.method(proto, name, length, function);
    }
    b.method(proto, "setYear", 1, set_legacy_year);
    let utc_string = b.method(proto, "toUTCString", 0, to_utc_string);
    b.value(
        proto,
        "toGMTString",
        JsValue::Object(utc_string),
        crate::v9::vm::heap::WRITABLE | crate::v9::vm::heap::CONFIGURABLE,
    );
    let formats: [(&'static str, NativeFn); 8] = [
        ("toISOString", to_iso),
        ("toJSON", to_json),
        ("toString", to_string),
        ("toDateString", to_date_string),
        ("toTimeString", to_time_string),
        ("toLocaleString", to_locale_string),
        ("toLocaleDateString", to_locale_date_string),
        ("toLocaleTimeString", to_locale_time_string),
    ];
    for (name, function) in formats {
        b.method(proto, name, 0, function);
    }
    b.symbol_method(
        proto,
        symbol::TO_PRIMITIVE,
        "[Symbol.toPrimitive]",
        1,
        to_primitive,
    );
}

/** The page's virtual time in milliseconds. */
pub(in crate::v9) fn now_ms(vm: &Vm) -> f64 {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 {
        return d8_now_ms();
    }
    (vm.page.state.epoch + vm.page.state.clock).floor()
}

/** D8 reads a process-wide monotonic clock so busy loops advance without wall-clock jumps. */
#[cfg(feature = "conformance")]
fn d8_now_ms() -> f64 {
    let (epoch, elapsed) = d8_time_origin_and_elapsed();
    (epoch + elapsed).floor()
}

/** One process anchor keeps shell Date and performance time coherent across isolated workers. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn d8_time_origin_and_elapsed() -> (f64, f64) {
    static CLOCK: std::sync::OnceLock<(std::time::Instant, u64)> = std::sync::OnceLock::new();
    let (anchor, epoch) = CLOCK.get_or_init(|| {
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        (std::time::Instant::now(), epoch)
    });
    (*epoch as f64, anchor.elapsed().as_secs_f64() * 1000.0)
}

/** The local timezone offset in milliseconds (UTC plus this is local). */
pub(super) fn local_offset(vm: &Vm, time: f64) -> f64 {
    timezone::offset(&vm.page.environment, time)
}

/** Days since the epoch for a civil date (proleptic Gregorian). */
fn days_from_civil(year: f64, month: f64, day: f64) -> f64 {
    let year = year + (month / 12.0).floor();
    let month = month.rem_euclid(12.0) + 1.0;
    let (y, m) = if month <= 2.0 {
        (year - 1.0, month + 9.0)
    } else {
        (year, month - 3.0)
    };
    let era = (y / 400.0).floor();
    let yoe = y - era * 400.0;
    let doy = ((153.0 * m + 2.0) / 5.0).floor() + day - 1.0;
    let doe = yoe * 365.0 + (yoe / 4.0).floor() - (yoe / 100.0).floor() + doy;
    era * 146_097.0 + doe - 719_468.0
}

/** Civil year, month (0-based) and day for days since the epoch. */
fn civil_from_days(days: f64) -> (f64, f64, f64) {
    let z = days + 719_468.0;
    let era = (z / 146_097.0).floor();
    let doe = z - era * 146_097.0;
    let yoe = ((doe - (doe / 1460.0).floor() + (doe / 36_524.0).floor()
        - (doe / 146_096.0).floor())
        / 365.0)
        .floor();
    let doy = doe - (365.0 * yoe + (yoe / 4.0).floor() - (yoe / 100.0).floor());
    let mp = ((5.0 * doy + 2.0) / 153.0).floor();
    let day = doy - ((153.0 * mp + 2.0) / 5.0).floor() + 1.0;
    let month = if mp < 10.0 { mp + 3.0 } else { mp - 9.0 };
    let year = yoe + era * 400.0 + if month <= 2.0 { 1.0 } else { 0.0 };
    (year, month - 1.0, day)
}

/** MakeTime + MakeDay + MakeDate. */
fn make_date(
    year: f64,
    month: f64,
    day: f64,
    hours: f64,
    minutes: f64,
    seconds: f64,
    millis: f64,
) -> f64 {
    let parts = [year, month, day, hours, minutes, seconds, millis];
    if parts.iter().any(|part| !part.is_finite()) {
        return f64::NAN;
    }
    let days = days_from_civil(year.trunc(), month.trunc(), 1.0) + day.trunc() - 1.0;
    days * DAY
        + hours.trunc() * HOUR
        + minutes.trunc() * MINUTE
        + seconds.trunc() * SECOND
        + millis.trunc()
}

/** TimeClip. */
fn clip(time: f64) -> f64 {
    if !time.is_finite() || time.abs() > MAX_TIME {
        f64::NAN
    } else {
        time.trunc() + 0.0
    }
}

/** Broken-down fields of a time value. */
pub(super) struct Parts {
    pub year: f64,
    pub month: f64,
    pub date: f64,
    pub day: f64,
    pub hours: f64,
    pub minutes: f64,
    pub seconds: f64,
    pub millis: f64,
}

/** Split a time value. */
pub(super) fn parts(time: f64) -> Parts {
    let days = (time / DAY).floor();
    let within = time - days * DAY;
    let (year, month, date) = civil_from_days(days);
    Parts {
        year,
        month,
        date,
        day: (days + 4.0).rem_euclid(7.0),
        hours: (within / HOUR).floor(),
        minutes: (within / MINUTE).floor() % 60.0,
        seconds: (within / SECOND).floor() % 60.0,
        millis: within % SECOND,
    }
}

/** `Date(...)`: a string when called, an object with `new`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        let time = now_ms(vm);
        return text(vm, &format_local(vm, time));
    };
    let time = match call.args.len() {
        0 => now_ms(vm),
        1 => {
            let value = call.arg(0);
            let primitive = match &value {
                JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
                    Some(Class::Date(time)) => JsValue::Number(*time),
                    _ => vm.to_primitive(&value, crate::v9::vm::convert::Hint::Default)?,
                },
                other => other.clone(),
            };
            match primitive {
                JsValue::String(text) => parse_date(vm, &text.lossy()),
                other => vm.to_number(&other)?,
            }
        }
        _ => {
            let local = components(vm, &call)?;
            timezone::utc(&vm.page.environment, local)
        }
    };
    let proto = vm.prototype_for(target, realm().ids.date_prototype)?;
    let id = vm.alloc(Object::new(Class::Date(clip(time)), Some(proto)))?;
    Ok(JsValue::Object(id))
}

/** Year, month, ... arguments to a time value (two-digit years mean 19xx). */
fn components(vm: &mut Vm, call: &Call) -> Res<f64> {
    let mut values = [f64::NAN, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    for (index, slot) in values.iter_mut().enumerate() {
        if index < call.args.len() {
            *slot = vm.to_number(&call.args[index])?;
        }
    }
    let year = values[0];
    if year.is_finite() && (0.0..=99.0).contains(&year.trunc()) {
        values[0] = 1900.0 + year.trunc();
    }
    Ok(make_date(
        values[0], values[1], values[2], values[3], values[4], values[5], values[6],
    ))
}

/** Date.now. */
fn now(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Number(now_ms(vm)))
}

/** Date.parse. */
fn parse(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?.lossy();
    Ok(JsValue::Number(clip(parse_date(vm, &text))))
}

/** Date.UTC. */
fn utc(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Number(clip(components(vm, &call)?)))
}

/** Parse ISO, RFC 2822 and common English date strings. */
fn parse_date(vm: &Vm, text: &str) -> f64 {
    let text = text.trim_matches(crate::v9::source::whitespace);
    if text.starts_with("-000000") {
        return f64::NAN;
    }
    if let Some(time) = parse_iso(text) {
        return match time {
            (time, true) => time,
            (time, false) => timezone::utc(&vm.page.environment, time),
        };
    }
    legacy::parse(text).map_or(f64::NAN, |(time, utc)| {
        if utc {
            time
        } else {
            timezone::utc(&vm.page.environment, time)
        }
    })
}

/** `YYYY-MM-DDTHH:mm:ss.sssZ` and its shorter forms; date-only forms are UTC. */
fn parse_iso(text: &str) -> Option<(f64, bool)> {
    let bytes = text.as_bytes();
    let number = |range: std::ops::Range<usize>| -> Option<f64> {
        let slice = text.get(range)?;
        slice
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| slice.parse().ok())
            .flatten()
    };
    let (year, rest) = if bytes.first().is_some_and(|b| *b == b'+' || *b == b'-') {
        let sign = if bytes[0] == b'-' { -1.0 } else { 1.0 };
        (sign * number(1..7)?, 7)
    } else {
        (number(0..4)?, 4)
    };
    let mut month = 1.0;
    let mut day = 1.0;
    let mut index = rest;
    if bytes.get(index) == Some(&b'-') {
        month = number(index + 1..index + 3)?;
        index += 3;
        if bytes.get(index) == Some(&b'-') {
            day = number(index + 1..index + 3)?;
            index += 3;
        }
    }
    if !(1.0..=12.0).contains(&month) || !(1.0..=31.0).contains(&day) {
        return None;
    }
    if index == bytes.len()
        || (index + 1 == bytes.len() && matches!(bytes.get(index), Some(b'Z' | b'z')))
    {
        return Some((make_date(year, month - 1.0, day, 0.0, 0.0, 0.0, 0.0), true));
    }
    if !matches!(bytes.get(index), Some(b'T' | b't' | b' ')) {
        return None;
    }
    let hours = number(index + 1..index + 3)?;
    if bytes.get(index + 3) != Some(&b':') {
        return None;
    }
    let minutes = number(index + 4..index + 6)?;
    index += 6;
    let mut seconds = 0.0;
    let mut millis = 0.0;
    if bytes.get(index) == Some(&b':') {
        seconds = number(index + 1..index + 3)?;
        index += 3;
        if bytes.get(index) == Some(&b'.') {
            let start = index + 1;
            let mut end = start;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            if end == start {
                return None;
            }
            let digits = (end - start).min(3);
            let value = bytes[start..start + digits]
                .iter()
                .fold(0u32, |value, digit| value * 10 + u32::from(*digit - b'0'));
            millis = f64::from(value * 10u32.pow((3 - digits) as u32));
            index = end;
        }
    }
    if hours > 24.0
        || minutes > 59.0
        || seconds > 59.0
        || hours == 24.0 && (minutes != 0.0 || seconds != 0.0 || millis != 0.0)
    {
        return None;
    }
    let base = make_date(year, month - 1.0, day, hours, minutes, seconds, millis);
    match bytes.get(index) {
        None => Some((base, false)),
        Some(b'Z' | b'z') if index + 1 == bytes.len() => Some((base, true)),
        Some(sign @ (b'+' | b'-')) => {
            let offset_hours = number(index + 1..index + 3)?;
            let minute_start = if bytes.get(index + 3) == Some(&b':') {
                index + 4
            } else {
                index + 3
            };
            let offset_minutes = number(minute_start..minute_start + 2)?;
            if offset_hours > 23.0 || offset_minutes > 59.0 || minute_start + 2 != bytes.len() {
                return None;
            }
            let offset = (offset_hours * HOUR + offset_minutes * MINUTE)
                * if *sign == b'+' { 1.0 } else { -1.0 };
            Some((base - offset, true))
        }
        _ => None,
    }
}

/** The time value a receiver holds. */
fn this_time(vm: &mut Vm, call: &Call) -> Res<f64> {
    if let JsValue::Object(id) = &call.this
        && let Some(Class::Date(time)) = vm.heap.get(*id).map(|o| &o.class)
    {
        return Ok(*time);
    }
    let name = super::function::native_source_name(vm, &JsValue::Object(call.callee));
    if name.starts_with("get") || name == "valueOf" {
        return Err(vm.type_error("this is not a Date object."));
    }
    let receiver = super::function::diagnostic_receiver(vm, &call.this)?;
    Err(vm.type_error(&format!(
        "Method Date.prototype.{name} called on incompatible receiver {receiver}"
    )))
}

/** Replace a receiver's time value. */
fn store(vm: &mut Vm, call: &Call, time: f64) -> Res<JsValue> {
    let time = clip(time);
    if let JsValue::Object(id) = &call.this
        && let Some(Class::Date(slot)) = vm.heap.get_mut(*id).map(|o| &mut o.class)
    {
        *slot = time;
    }
    Ok(JsValue::Number(time))
}

/** Date.prototype.getTime and valueOf. */
fn get_time(vm: &mut Vm, call: Call) -> Res<JsValue> {
    this_time(vm, &call).map(JsValue::Number)
}

/** Which broken-down field. */
#[derive(Clone, Copy)]
enum Field {
    Year,
    Month,
    Date,
    Day,
    Hours,
    Minutes,
    Seconds,
    Millis,
}

/** A getter for one field, local or UTC. */
fn field(vm: &mut Vm, call: Call, which: Field, local: bool) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    if time.is_nan() {
        return Ok(JsValue::Number(f64::NAN));
    }
    let time = if local {
        time + local_offset(vm, time)
    } else {
        time
    };
    let p = parts(time);
    Ok(JsValue::Number(match which {
        Field::Year => p.year,
        Field::Month => p.month,
        Field::Date => p.date,
        Field::Day => p.day,
        Field::Hours => p.hours,
        Field::Minutes => p.minutes,
        Field::Seconds => p.seconds,
        Field::Millis => p.millis,
    }))
}

/** getTimezoneOffset: minutes UTC is ahead of local. */
fn timezone_offset(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    Ok(JsValue::Number(-local_offset(vm, time) / MINUTE + 0.0))
}

/** setTime. */
fn set_time(vm: &mut Vm, call: Call) -> Res<JsValue> {
    this_time(vm, &call)?;
    let time = arg_number(vm, &call, 0)?;
    store(vm, &call, time)
}

/** Legacy setYear captures the original date before coercion and resets invalid dates from local +0. */
fn set_legacy_year(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    let year = arg_number(vm, &call, 0)?;
    if year.is_nan() {
        return store(vm, &call, f64::NAN);
    }
    let base = if time.is_nan() {
        0.0
    } else {
        time + local_offset(vm, time)
    };
    let parts = parts(base);
    let year = if (0.0..=LEGACY_YEAR_LAST).contains(&year.trunc()) {
        year.trunc() + LEGACY_YEAR_OFFSET
    } else {
        year
    };
    let local = make_date(
        year,
        parts.month,
        parts.date,
        parts.hours,
        parts.minutes,
        parts.seconds,
        parts.millis,
    );
    let utc = timezone::utc(&vm.page.environment, local);
    store(vm, &call, utc)
}

/** The field setters: the field and the ones after it come from the arguments. */
fn set(vm: &mut Vm, call: Call, which: Field, local: bool) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    let offset = if local { local_offset(vm, time) } else { 0.0 };
    let base = if time.is_nan() && matches!(which, Field::Year) {
        0.0
    } else {
        time + offset
    };
    let p = parts(base);
    let mut fields = [
        p.year, p.month, p.date, p.hours, p.minutes, p.seconds, p.millis,
    ];
    let start = match which {
        Field::Year => 0,
        Field::Month => 1,
        Field::Date | Field::Day => 2,
        Field::Hours => 3,
        Field::Minutes => 4,
        Field::Seconds => 5,
        Field::Millis => 6,
    };
    let max = match which {
        Field::Year | Field::Minutes => 3,
        Field::Month | Field::Seconds => 2,
        Field::Hours => 4,
        Field::Date | Field::Day | Field::Millis => 1,
    };
    for index in 0..call.args.len().clamp(1, max) {
        fields[start + index] = vm.to_number(&call.arg(index))?;
    }
    // An invalid date stays as it is (the arguments were still converted, which may have
    // changed it); only setFullYear starts over from +0.
    if time.is_nan() && !matches!(which, Field::Year) {
        return Ok(JsValue::Number(f64::NAN));
    }
    let local_time = make_date(
        fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
    );
    store(
        vm,
        &call,
        if local {
            timezone::utc(&vm.page.environment, local_time)
        } else {
            local_time
        },
    )
}

/** The ISO form, used by toISOString and toJSON. */
pub(in crate::v9) fn format_iso(time: f64) -> String {
    let p = parts(time);
    let year = if (0.0..=9999.0).contains(&p.year) {
        format!("{:04}", p.year as i64)
    } else {
        format!(
            "{}{:06}",
            if p.year < 0.0 { '-' } else { '+' },
            p.year.abs() as i64
        )
    };
    format!(
        "{year}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        p.month as i64 + 1,
        p.date as i64,
        p.hours as i64,
        p.minutes as i64,
        p.seconds as i64,
        p.millis as i64
    )
}

/** Date.prototype.toISOString. */
fn to_iso(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    if time.is_nan() {
        return Err(vm.range_error("Invalid time value"));
    }
    text(vm, &format_iso(time))
}

/** Date.prototype.toJSON. */
fn to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = vm.to_primitive(&call.this, crate::v9::vm::convert::Hint::Number)?;
    if time.number_value().is_some_and(|n| !n.is_finite()) {
        return Ok(JsValue::Null);
    }
    let method = vm.get(&call.this, &key("toISOString"))?;
    vm.call(method, call.this.clone(), Vec::new())
}

/** The zone suffix such as `GMT+0100 (Coordinated Universal Time)`. */
fn zone(vm: &Vm, time: f64) -> String {
    let minutes = local_offset(vm, time) / MINUTE;
    let sign = if minutes < 0.0 { '-' } else { '+' };
    let minutes = minutes.abs() as i64;
    let name = if let Some(name) = timezone::display_name(&vm.page.environment, time) {
        name.to_string()
    } else if minutes == 0 {
        "Coordinated Universal Time".to_string()
    } else {
        "Local Time".to_string()
    };
    format!("GMT{sign}{:02}{:02} ({name})", minutes / 60, minutes % 60)
}

/** The year as DateString prints it: the magnitude padded to four digits, then a `-` for years
before 1 BCE. Rust's `{:04}` counts the sign in the width, giving `-001` where the spec has `-0001`. */
fn year_text(year: f64) -> String {
    let sign = if year < 0.0 { "-" } else { "" };
    format!("{sign}{:04}", year.abs() as i64)
}

/** The date portion: `Tue Jan 01 2020`. */
fn date_part(p: &Parts) -> String {
    format!(
        "{} {} {:02} {}",
        DAYS[p.day as usize],
        MONTHS[p.month as usize],
        p.date as i64,
        year_text(p.year)
    )
}

/** The time portion: `10:00:00`. */
fn time_part(p: &Parts) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        p.hours as i64, p.minutes as i64, p.seconds as i64
    )
}

/** Date.prototype.toString form. */
fn format_local(vm: &Vm, time: f64) -> String {
    if time.is_nan() {
        return "Invalid Date".into();
    }
    let p = parts(time + local_offset(vm, time));
    format!("{} {} {}", date_part(&p), time_part(&p), zone(vm, time))
}

/** Date.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    let formatted = format_local(vm, time);
    text(vm, &formatted)
}

/** Date.prototype.toDateString. */
fn to_date_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    if time.is_nan() {
        return text(vm, "Invalid Date");
    }
    let p = parts(time + local_offset(vm, time));
    text(vm, &date_part(&p))
}

/** Date.prototype.toTimeString. */
fn to_time_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    if time.is_nan() {
        return text(vm, "Invalid Date");
    }
    let p = parts(time + local_offset(vm, time));
    let zone = zone(vm, time);
    text(vm, &format!("{} {zone}", time_part(&p)))
}

/** Date.prototype.toUTCString. */
fn to_utc_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    if time.is_nan() {
        return text(vm, "Invalid Date");
    }
    let p = parts(time);
    text(
        vm,
        &format!(
            "{}, {:02} {} {} {} GMT",
            DAYS[p.day as usize],
            p.date as i64,
            MONTHS[p.month as usize],
            year_text(p.year),
            time_part(&p)
        ),
    )
}

/** en-US `1/2/2020, 3:04:05 PM`. */
fn locale(vm: &Vm, time: f64, date: bool, clock: bool) -> String {
    if time.is_nan() {
        return "Invalid Date".into();
    }
    let p = parts(time + local_offset(vm, time));
    let day = format!("{}/{}/{}", p.month as i64 + 1, p.date as i64, p.year as i64);
    let hour = p.hours as i64 % 12;
    let clock_text = format!(
        "{}:{:02}:{:02} {}",
        if hour == 0 { 12 } else { hour },
        p.minutes as i64,
        p.seconds as i64,
        if p.hours < 12.0 { "AM" } else { "PM" }
    );
    match (date, clock) {
        (true, true) => format!("{day}, {clock_text}"),
        (true, false) => day,
        _ => clock_text,
    }
}

/** Date.prototype.toLocaleString. */
fn to_locale_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    check_locale_options(vm, &call, time)?;
    let formatted = locale(vm, time, true, true);
    text(vm, &formatted)
}

/** Date.prototype.toLocaleDateString. */
fn to_locale_date_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    check_locale_options(vm, &call, time)?;
    let formatted = locale(vm, time, true, false);
    text(vm, &formatted)
}

/** Date.prototype.toLocaleTimeString. */
fn to_locale_time_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let time = this_time(vm, &call)?;
    check_locale_options(vm, &call, time)?;
    let formatted = locale(vm, time, false, true);
    text(vm, &formatted)
}

/** ToDateTimeOptions coerces any options value other than undefined to an object, so null throws a TypeError; an invalid date skips this and returns "Invalid Date". */
fn check_locale_options(vm: &mut Vm, call: &Call, time: f64) -> Res<()> {
    if time.is_nan() || call.arg(1) == JsValue::Undefined {
        return Ok(());
    }
    vm.to_object(&call.arg(1))?;
    Ok(())
}

/** Date.prototype[Symbol.toPrimitive]: "number" gives the time, otherwise the string. */
fn to_primitive(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Err(vm.type_error("Date.prototype[Symbol.toPrimitive] called on non-object"));
    }
    let hint = match call.arg(0) {
        JsValue::String(hint) => hint.lossy(),
        _ => String::new(),
    };
    let order = match hint.as_str() {
        "number" => ["valueOf", "toString"],
        "string" | "default" => ["toString", "valueOf"],
        _ => return Err(vm.type_error("Invalid hint")),
    };
    // OrdinaryToPrimitive: the first callable method whose result is not an object wins.
    for name in order {
        let method = vm.get(&call.this, &key(name))?;
        if vm.is_callable(&method) {
            let result = vm.call(method, call.this.clone(), Vec::new())?;
            if !matches!(result, JsValue::Object(_)) {
                return Ok(result);
            }
        }
    }
    Err(vm.type_error("Cannot convert object to primitive value"))
}
