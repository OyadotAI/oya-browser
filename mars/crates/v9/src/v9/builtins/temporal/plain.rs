/** Temporal.PlainDate, PlainTime and PlainDateTime over the ISO 8601 calendar: constructors, `from`
with its property-bag and string paths, getters, arithmetic, differences, rounding and formatting,
reading options in the spec's order. */
use super::duration::{self, Dur};
use super::iso::{self, NS_DAY};
use super::*;

/** Every calendar and clock field a property bag may carry, in the alphabetical read order. */
const FIELDS: [&str; 10] = [
    "day",
    "hour",
    "microsecond",
    "millisecond",
    "minute",
    "month",
    "monthCode",
    "nanosecond",
    "second",
    "year",
];
pub(super) const DAY: usize = 0;
pub(super) const MONTH: usize = 5;
pub(super) const MONTH_CODE: usize = 6;
pub(super) const YEAR: usize = 9;
/** Clock fields as indexes into FIELDS, in hour..nanosecond order. */
const CLOCK: [usize; 6] = [1, 4, 8, 3, 2, 7];
const DATE_F: &[usize] = &[0, 5, 6, 9];
const TIME_F: &[usize] = &[1, 2, 3, 4, 7, 8];
const ALL_F: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

/** A property bag's fields, read and converted in order. */
pub(super) type Fields = [Option<i64>; 10];

/** Read the named fields of a bag, converting each as it is read. */
pub(super) fn read_fields(vm: &mut Vm, bag: &JsValue, which: &[usize]) -> Res<Fields> {
    let mut out = [None; 10];
    for &i in which {
        let value = vm.get(bag, &Key::from(FIELDS[i]))?;
        if matches!(value, JsValue::Undefined) {
            continue;
        }
        out[i] = Some(match i {
            DAY | MONTH => to_positive(vm, &value)?,
            MONTH_CODE => to_month_code(vm, &value)?,
            _ => to_int(vm, &value)?,
        });
    }
    Ok(out)
}

/** The bag's clock fields, missing ones zero. */
fn clock_of(f: &Fields, base: [i64; 6]) -> [i64; 6] {
    let mut t = base;
    for (slot, &i) in t.iter_mut().zip(CLOCK.iter()) {
        if let Some(v) = f[i] {
            *slot = v;
        }
    }
    t
}

/** The month of a bag: month and monthCode must agree. */
pub(super) fn month_of(vm: &mut Vm, f: &Fields) -> Res<Option<i64>> {
    match (f[MONTH], f[MONTH_CODE]) {
        (Some(a), Some(b)) if a != b => Err(vm.range_error("month and monthCode disagree")),
        (a, b) => Ok(a.or(b)),
    }
}

/** Required date fields of a full bag, or the TypeError for a missing one. */
fn require_date(vm: &mut Vm, f: &Fields) -> Res<()> {
    if f[YEAR].is_none() || f[DAY].is_none() || (f[MONTH].is_none() && f[MONTH_CODE].is_none()) {
        return Err(vm.type_error("year, month and day are required"));
    }
    Ok(())
}

/** CalendarDateFromFields for ISO, after the overflow option is known. */
fn date_from_fields(vm: &mut Vm, f: &Fields, constrain: bool) -> Res<(i64, i64, i64)> {
    let month = month_of(vm, f)?.unwrap_or(1);
    let (y, d) = (f[YEAR].unwrap_or(0), f[DAY].unwrap_or(1));
    let date = iso::regulate_date(y, month, d, constrain)
        .ok_or_else(|| vm.range_error("date out of range"))?;
    check_date(vm, date)
}

/** ISODateWithinLimits as a RangeError. */
pub(super) fn check_date(vm: &mut Vm, date: (i64, i64, i64)) -> Res<(i64, i64, i64)> {
    if !iso::date_in_limits(date) {
        return Err(vm.range_error("date outside the supported range"));
    }
    Ok(date)
}

/** ISODateTimeWithinLimits as a RangeError. */
pub(super) fn check_datetime(vm: &mut Vm, date: (i64, i64, i64), time: &[i64; 6]) -> Res<()> {
    if !iso::datetime_in_limits(date, time) {
        return Err(vm.range_error("date-time outside the supported range"));
    }
    Ok(())
}

/** GetTemporalCalendarIdentifierWithISODefault on a bag. */
pub(super) fn bag_calendar(vm: &mut Vm, bag: &JsValue) -> Res<()> {
    let value = vm.get(bag, &Key::from("calendar"))?;
    to_calendar(vm, &value)
}

/** ToTemporalCalendarIdentifier: a Temporal object's calendar, or an id or ISO string naming one. */
pub(super) fn to_calendar(vm: &mut Vm, value: &JsValue) -> Res<()> {
    if matches!(value, JsValue::Undefined)
        || slots(vm, value).is_some_and(|t| t.kind != Kind::Duration && t.kind != Kind::Instant)
    {
        return Ok(());
    }
    let JsValue::String(text) = value else {
        return Err(vm.type_error("calendar must be a string"));
    };
    let text = text.lossy();
    if text.eq_ignore_ascii_case("iso8601") {
        return Ok(());
    }
    let parsed = iso::parse_datetime(&text)
        .or_else(|| iso::parse_time_only(&text))
        .or_else(|| iso::parse_year_month(&text))
        .or_else(|| iso::parse_month_day(&text));
    match parsed {
        Some(p) => check_calendar(vm, p.calendar.as_deref().unwrap_or("iso8601")),
        None => Err(vm.range_error(&format!("invalid calendar {text}"))),
    }
}

/** Reject a parsed string's unsupported calendar annotation. */
pub(super) fn parsed_calendar(vm: &mut Vm, p: &iso::Parsed) -> Res<()> {
    check_calendar(vm, p.calendar.as_deref().unwrap_or("iso8601"))
}

/** Read the overflow option of an options argument. */
pub(super) fn read_overflow(vm: &mut Vm, opts: &JsValue) -> Res<bool> {
    let o = options(vm, opts)?;
    overflow(vm, o)
}

/** ToTemporalDate: the ISO date of a PlainDate, PlainDateTime, bag or string. */
pub(super) fn to_date(vm: &mut Vm, item: &JsValue, opts: &JsValue) -> Res<(i64, i64, i64)> {
    if let JsValue::Object(_) = item {
        if let Some(t) = slots(vm, item) {
            if matches!(t.kind, Kind::Date | Kind::DateTime) {
                read_overflow(vm, opts)?;
                return Ok(t.date());
            }
            if t.kind == Kind::Zoned {
                read_overflow(vm, opts)?;
                return Ok(super::zoned_parts(&t).0);
            }
        }
        bag_calendar(vm, item)?;
        let f = read_fields(vm, item, DATE_F)?;
        require_date(vm, &f)?;
        let constrain = read_overflow(vm, opts)?;
        return date_from_fields(vm, &f, constrain);
    }
    let s = require_string(vm, item)?;
    let p = iso::parse_datetime(&s)
        .filter(|p| !p.z)
        .ok_or_else(|| bad_string(vm, &s))?;
    parsed_calendar(vm, &p)?;
    let date = check_date(vm, p.date.unwrap_or_default())?;
    read_overflow(vm, opts)?;
    Ok(date)
}

/** ToTemporalTime: the clock time of a PlainTime, PlainDateTime, bag or string. */
pub(super) fn to_time(vm: &mut Vm, item: &JsValue, opts: &JsValue) -> Res<[i64; 6]> {
    if let JsValue::Object(_) = item {
        if let Some(t) = slots(vm, item) {
            if matches!(t.kind, Kind::Time | Kind::DateTime) {
                read_overflow(vm, opts)?;
                return Ok(t.time());
            }
            if t.kind == Kind::Zoned {
                read_overflow(vm, opts)?;
                return Ok(super::zoned_parts(&t).1);
            }
        }
        let f = read_fields(vm, item, TIME_F)?;
        if TIME_F.iter().all(|&i| f[i].is_none()) {
            return Err(vm.type_error("a time needs at least one time field"));
        }
        let constrain = read_overflow(vm, opts)?;
        return iso::regulate_time(clock_of(&f, [0; 6]), constrain)
            .ok_or_else(|| vm.range_error("time out of range"));
    }
    let s = require_string(vm, item)?;
    let time = parse_time_string(vm, &s)?;
    read_overflow(vm, opts)?;
    Ok(time)
}

/** ParseTemporalTimeString: a time alone or a date-time with a time, never with Z. */
fn parse_time_string(vm: &mut Vm, s: &str) -> Res<[i64; 6]> {
    let p = iso::parse_time_only(s).or_else(|| iso::parse_datetime(s).filter(|p| p.time.is_some()));
    let p = p.filter(|p| !p.z).ok_or_else(|| bad_string(vm, s))?;
    parsed_calendar(vm, &p)?;
    Ok(p.time.unwrap_or_default())
}

/** ToTemporalDateTime: a PlainDateTime's slots from any accepted input. */
pub(super) fn to_datetime(
    vm: &mut Vm,
    item: &JsValue,
    opts: &JsValue,
) -> Res<((i64, i64, i64), [i64; 6])> {
    if let JsValue::Object(_) = item {
        if let Some(t) = slots(vm, item) {
            match t.kind {
                Kind::DateTime => {
                    read_overflow(vm, opts)?;
                    return Ok((t.date(), t.time()));
                }
                Kind::Date => {
                    read_overflow(vm, opts)?;
                    return Ok((t.date(), [0; 6]));
                }
                Kind::Zoned => {
                    read_overflow(vm, opts)?;
                    return Ok(super::zoned_parts(&t));
                }
                _ => {}
            }
        }
        bag_calendar(vm, item)?;
        let f = read_fields(vm, item, ALL_F)?;
        require_date(vm, &f)?;
        let constrain = read_overflow(vm, opts)?;
        return datetime_from_fields(vm, &f, constrain);
    }
    let s = require_string(vm, item)?;
    let p = iso::parse_datetime(&s)
        .filter(|p| !p.z)
        .ok_or_else(|| bad_string(vm, &s))?;
    parsed_calendar(vm, &p)?;
    let (date, time) = (p.date.unwrap_or_default(), p.time.unwrap_or_default());
    check_datetime(vm, date, &time)?;
    read_overflow(vm, opts)?;
    Ok((date, time))
}

/** InterpretTemporalDateTimeFields for ISO. */
fn datetime_from_fields(
    vm: &mut Vm,
    f: &Fields,
    constrain: bool,
) -> Res<((i64, i64, i64), [i64; 6])> {
    let month = month_of(vm, f)?.unwrap_or(1);
    let (y, d) = (f[YEAR].unwrap_or(0), f[DAY].unwrap_or(1));
    let date = iso::regulate_date(y, month, d, constrain)
        .ok_or_else(|| vm.range_error("date out of range"))?;
    let time = iso::regulate_time(clock_of(f, [0; 6]), constrain)
        .ok_or_else(|| vm.range_error("time out of range"))?;
    check_datetime(vm, date, &time)?;
    Ok((date, time))
}

/** A new PlainDate. */
pub(super) fn new_date(vm: &mut Vm, target: Option<&JsValue>, d: (i64, i64, i64)) -> Res<JsValue> {
    make(
        vm,
        target,
        T {
            kind: Kind::Date,
            v: vec![d.0.into(), d.1.into(), d.2.into()],
            tz: String::new(),
        },
    )
}

/** A new PlainTime. */
pub(super) fn new_time(vm: &mut Vm, target: Option<&JsValue>, t: [i64; 6]) -> Res<JsValue> {
    make(
        vm,
        target,
        T {
            kind: Kind::Time,
            v: t.iter().map(|v| i128::from(*v)).collect(),
            tz: String::new(),
        },
    )
}

/** A new PlainDateTime. */
pub(super) fn new_datetime(
    vm: &mut Vm,
    target: Option<&JsValue>,
    d: (i64, i64, i64),
    t: [i64; 6],
) -> Res<JsValue> {
    let mut v = vec![d.0.into(), d.1.into(), d.2.into()];
    v.extend(t.iter().map(|x| i128::from(*x)));
    make(
        vm,
        target,
        T {
            kind: Kind::DateTime,
            v,
            tz: String::new(),
        },
    )
}

/** The new.target of a constructor call, or the TypeError a plain call gets. */
pub(super) fn need_new<'a>(vm: &mut Vm, call: &'a Call) -> Res<&'a JsValue> {
    call.new_target
        .as_ref()
        .ok_or_else(|| vm.type_error("Temporal constructors require 'new'"))
}

/** Integer arguments from `from` onwards, undefined as zero. */
fn int_args(vm: &mut Vm, call: &Call, from: usize, n: usize) -> Res<Vec<i64>> {
    (from..from + n)
        .map(|i| match call.arg(i) {
            JsValue::Undefined => Ok(0),
            v => to_int(vm, &v),
        })
        .collect()
}

/** `new Temporal.PlainDate(y, m, d, calendar)`. */
fn date_ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = need_new(vm, &call)?.clone();
    let a = (0..3)
        .map(|i| to_int(vm, &call.arg(i)))
        .collect::<Res<Vec<_>>>()?;
    calendar_arg(vm, &call.arg(3))?;
    if !iso::valid_date(a[0], a[1], a[2]) {
        return Err(vm.range_error("invalid ISO date"));
    }
    let d = check_date(vm, (a[0], a[1], a[2]))?;
    new_date(vm, Some(&target), d)
}

/** `new Temporal.PlainTime(h, m, s, ms, us, ns)`. */
fn time_ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = need_new(vm, &call)?.clone();
    let a = int_args(vm, &call, 0, 6)?;
    let t = [a[0], a[1], a[2], a[3], a[4], a[5]];
    if !iso::valid_time(&t) {
        return Err(vm.range_error("invalid time"));
    }
    new_time(vm, Some(&target), t)
}

/** `new Temporal.PlainDateTime(y, m, d, h, mi, s, ms, us, ns, calendar)`. */
fn datetime_ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = need_new(vm, &call)?.clone();
    let d = (0..3)
        .map(|i| to_int(vm, &call.arg(i)))
        .collect::<Res<Vec<_>>>()?;
    let a = int_args(vm, &call, 3, 6)?;
    calendar_arg(vm, &call.arg(9))?;
    let t = [a[0], a[1], a[2], a[3], a[4], a[5]];
    if !iso::valid_date(d[0], d[1], d[2]) || !iso::valid_time(&t) {
        return Err(vm.range_error("invalid ISO date-time"));
    }
    check_datetime(vm, (d[0], d[1], d[2]), &t)?;
    new_datetime(vm, Some(&target), (d[0], d[1], d[2]), t)
}

/** PlainDate.from. */
fn date_from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let d = to_date(vm, &call.arg(0), &call.arg(1))?;
    new_date(vm, None, d)
}

/** PlainTime.from. */
fn time_from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = to_time(vm, &call.arg(0), &call.arg(1))?;
    new_time(vm, None, t)
}

/** PlainDateTime.from. */
fn datetime_from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (d, t) = to_datetime(vm, &call.arg(0), &call.arg(1))?;
    new_datetime(vm, None, d, t)
}

/** A comparison result as a Number. */
pub(super) fn ordering(o: std::cmp::Ordering) -> Res<JsValue> {
    Ok(JsValue::Number(o as i32 as f64))
}

/** PlainDate.compare. */
fn date_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = to_date(vm, &call.arg(0), &JsValue::Undefined)?;
    let b = to_date(vm, &call.arg(1), &JsValue::Undefined)?;
    ordering(a.cmp(&b))
}

/** PlainTime.compare. */
fn time_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = to_time(vm, &call.arg(0), &JsValue::Undefined)?;
    let b = to_time(vm, &call.arg(1), &JsValue::Undefined)?;
    ordering(a.cmp(&b))
}

/** PlainDateTime.compare. */
fn datetime_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = to_datetime(vm, &call.arg(0), &JsValue::Undefined)?;
    let b = to_datetime(vm, &call.arg(1), &JsValue::Undefined)?;
    ordering(a.cmp(&b))
}

/** `equals` on the three plain types. */
fn date_equals(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    let other = to_date(vm, &call.arg(0), &JsValue::Undefined)?;
    Ok(JsValue::Boolean(t.date() == other))
}

/** PlainTime.prototype.equals. */
fn time_equals(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    let other = to_time(vm, &call.arg(0), &JsValue::Undefined)?;
    Ok(JsValue::Boolean(t.time() == other))
}

/** PlainDateTime.prototype.equals. */
fn datetime_equals(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let other = to_datetime(vm, &call.arg(0), &JsValue::Undefined)?;
    Ok(JsValue::Boolean((t.date(), t.time()) == other))
}

/** A field getter shared by every plain type. */
pub(super) fn field(vm: &mut Vm, call: &Call, kind: Kind, name: &str) -> Res<JsValue> {
    let t = this_t(vm, call, kind)?;
    let (y, m, d) = if matches!(kind, Kind::Time) {
        (1972, 1, 1)
    } else {
        t.date()
    };
    let n = match name {
        "calendarId" => return js("iso8601"),
        "era" | "eraYear" => return Ok(JsValue::Undefined),
        "monthCode" => return js(&format!("M{m:02}")),
        "inLeapYear" => return Ok(JsValue::Boolean(iso::is_leap(y))),
        "year" => y,
        "month" => m,
        "day" => d,
        "dayOfWeek" => iso::day_of_week(y, m, d),
        "dayOfYear" => iso::day_of_year(y, m, d),
        "weekOfYear" => iso::week_of_year(y, m, d).0,
        "yearOfWeek" => iso::week_of_year(y, m, d).1,
        "daysInWeek" => 7,
        "daysInMonth" => iso::days_in_month(y, m),
        "daysInYear" => 365 + i64::from(iso::is_leap(y)),
        "monthsInYear" => 12,
        _ => {
            let i = TIME_NAMES.iter().position(|n| *n == name).unwrap_or(0);
            t.time()[i]
        }
    };
    Ok(JsValue::Number(n as f64))
}

/** Clock field names in slot order. */
const TIME_NAMES: [&str; 6] = [
    "hour",
    "minute",
    "second",
    "millisecond",
    "microsecond",
    "nanosecond",
];

/** Generate getter functions and their table. */
macro_rules! getters {
    ($table:ident, $kind:expr, [$($name:literal => $f:ident),* $(,)?]) => {
        $(fn $f(vm: &mut Vm, call: Call) -> Res<JsValue> { field(vm, &call, $kind, $name) })*
        const $table: &[(&str, NativeFn)] = &[$(($name, $f)),*];
    };
}

getters!(DATE_GETTERS, Kind::Date, [
    "calendarId" => d_cal, "era" => d_era, "eraYear" => d_era_year, "year" => d_year, "month" => d_month,
    "monthCode" => d_month_code, "day" => d_day, "dayOfWeek" => d_dow, "dayOfYear" => d_doy,
    "weekOfYear" => d_woy, "yearOfWeek" => d_yow, "daysInWeek" => d_diw, "daysInMonth" => d_dim,
    "daysInYear" => d_diy, "monthsInYear" => d_miy, "inLeapYear" => d_leap,
]);
getters!(TIME_GETTERS, Kind::Time, [
    "hour" => t_hour, "minute" => t_minute, "second" => t_second, "millisecond" => t_ms,
    "microsecond" => t_us, "nanosecond" => t_ns,
]);
getters!(DT_GETTERS, Kind::DateTime, [
    "calendarId" => dt_cal, "era" => dt_era, "eraYear" => dt_era_year, "year" => dt_year, "month" => dt_month,
    "monthCode" => dt_month_code, "day" => dt_day, "dayOfWeek" => dt_dow, "dayOfYear" => dt_doy,
    "weekOfYear" => dt_woy, "yearOfWeek" => dt_yow, "daysInWeek" => dt_diw, "daysInMonth" => dt_dim,
    "daysInYear" => dt_diy, "monthsInYear" => dt_miy, "inLeapYear" => dt_leap,
    "hour" => dt_hour, "minute" => dt_minute, "second" => dt_second, "millisecond" => dt_ms,
    "microsecond" => dt_us, "nanosecond" => dt_ns,
]);

/** IsPartialTemporalObject: a plain bag without calendar or timeZone. */
pub(super) fn partial_bag(vm: &mut Vm, value: &JsValue) -> Res<()> {
    if !matches!(value, JsValue::Object(_)) || slots(vm, value).is_some() {
        return Err(vm.type_error("expected a property bag"));
    }
    for key in ["calendar", "timeZone"] {
        if !matches!(vm.get(value, &Key::from(key))?, JsValue::Undefined) {
            return Err(vm.type_error(&format!("{key} is not allowed here")));
        }
    }
    Ok(())
}

/** Merge a partial bag over existing fields; the TypeError when it names none. */
fn merge(vm: &mut Vm, base: Fields, f: Fields, which: &[usize]) -> Res<Fields> {
    if which.iter().all(|&i| f[i].is_none()) {
        return Err(vm.type_error("no recognized fields"));
    }
    let mut out = base;
    if f[MONTH].is_some() || f[MONTH_CODE].is_some() {
        out[MONTH] = f[MONTH];
        out[MONTH_CODE] = f[MONTH_CODE];
    }
    for &i in which {
        if i != MONTH && i != MONTH_CODE && f[i].is_some() {
            out[i] = f[i];
        }
    }
    Ok(out)
}

/** Fields of an existing date-time. */
fn fields_of(date: (i64, i64, i64), time: [i64; 6]) -> Fields {
    let mut f = [None; 10];
    f[YEAR] = Some(date.0);
    f[MONTH] = Some(date.1);
    f[DAY] = Some(date.2);
    for (v, &i) in time.iter().zip(CLOCK.iter()) {
        f[i] = Some(*v);
    }
    f
}

/** PlainDate.prototype.with. */
fn date_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    let bag = call.arg(0);
    partial_bag(vm, &bag)?;
    let f = read_fields(vm, &bag, DATE_F)?;
    let f = merge(vm, fields_of(t.date(), [0; 6]), f, DATE_F)?;
    let constrain = read_overflow(vm, &call.arg(1))?;
    let d = date_from_fields(vm, &f, constrain)?;
    new_date(vm, None, d)
}

/** PlainTime.prototype.with. */
fn time_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    let bag = call.arg(0);
    partial_bag(vm, &bag)?;
    let f = read_fields(vm, &bag, TIME_F)?;
    let f = merge(vm, fields_of((1972, 1, 1), t.time()), f, TIME_F)?;
    let constrain = read_overflow(vm, &call.arg(1))?;
    let time = iso::regulate_time(clock_of(&f, [0; 6]), constrain)
        .ok_or_else(|| vm.range_error("time out of range"))?;
    new_time(vm, None, time)
}

/** PlainDateTime.prototype.with. */
fn datetime_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let bag = call.arg(0);
    partial_bag(vm, &bag)?;
    let f = read_fields(vm, &bag, ALL_F)?;
    let f = merge(vm, fields_of(t.date(), t.time()), f, ALL_F)?;
    let constrain = read_overflow(vm, &call.arg(1))?;
    let (d, time) = datetime_from_fields(vm, &f, constrain)?;
    new_datetime(vm, None, d, time)
}

/** AddTime: a clock time plus nanoseconds, with the day carry. */
fn add_time(time: &[i64; 6], ns: i128) -> (i64, [i64; 6]) {
    iso::balance_time(iso::time_ns(time) + ns)
}

/** The duration argument of add/subtract, negated for subtract. */
pub(super) fn duration_arg(vm: &mut Vm, call: &Call, sign: i128) -> Res<Dur> {
    let d = duration::to_duration(vm, &call.arg(0))?;
    Ok(d.map(|v| v * sign))
}

/** AddDurationToDate for ISO. */
fn add_to_date(
    vm: &mut Vm,
    date: (i64, i64, i64),
    d: &Dur,
    extra_days: i128,
    constrain: bool,
) -> Res<(i64, i64, i64)> {
    let days = d[3] + extra_days + duration::time_ns(d) / NS_DAY;
    let big = [d[0], d[1], d[2], days];
    if big.iter().any(|v| v.abs() > i128::from(i32::MAX) * 4) {
        return Err(vm.range_error("date out of range"));
    }
    let parts = [big[0] as i64, big[1] as i64, big[2] as i64, big[3] as i64];
    let out =
        iso::add_date(date, parts, constrain).ok_or_else(|| vm.range_error("date out of range"))?;
    check_date(vm, out)
}

/** PlainDate add and subtract. */
fn date_add_sub(vm: &mut Vm, call: Call, sign: i128) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    let d = duration_arg(vm, &call, sign)?;
    let constrain = read_overflow(vm, &call.arg(1))?;
    let out = add_to_date(vm, t.date(), &d, 0, constrain)?;
    new_date(vm, None, out)
}

/** PlainDate.prototype.add. */
fn date_add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    date_add_sub(vm, call, 1)
}

/** PlainDate.prototype.subtract. */
fn date_subtract(vm: &mut Vm, call: Call) -> Res<JsValue> {
    date_add_sub(vm, call, -1)
}

/** PlainTime add and subtract: date units are ignored. */
fn time_add_sub(vm: &mut Vm, call: Call, sign: i128) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    let d = duration_arg(vm, &call, sign)?;
    let (_, time) = add_time(&t.time(), duration::time_ns(&d));
    new_time(vm, None, time)
}

/** PlainTime.prototype.add. */
fn time_add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    time_add_sub(vm, call, 1)
}

/** PlainTime.prototype.subtract. */
fn time_subtract(vm: &mut Vm, call: Call) -> Res<JsValue> {
    time_add_sub(vm, call, -1)
}

/** PlainDateTime add and subtract: the time first, its carry into the date. */
fn datetime_add_sub(vm: &mut Vm, call: Call, sign: i128) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let d = duration_arg(vm, &call, sign)?;
    let constrain = read_overflow(vm, &call.arg(1))?;
    let (carry, time) = add_time(&t.time(), duration::time_ns(&d) % NS_DAY);
    let date = add_to_date(vm, t.date(), &d, i128::from(carry), constrain)?;
    check_datetime(vm, date, &time)?;
    new_datetime(vm, None, date, time)
}

/** PlainDateTime.prototype.add. */
fn datetime_add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    datetime_add_sub(vm, call, 1)
}

/** PlainDateTime.prototype.subtract. */
fn datetime_subtract(vm: &mut Vm, call: Call) -> Res<JsValue> {
    datetime_add_sub(vm, call, -1)
}

/** DifferenceTemporalPlainTime. */
fn time_diff(vm: &mut Vm, call: Call, since: bool) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    let other = to_time(vm, &call.arg(0), &JsValue::Undefined)?;
    let s = duration::difference_settings(vm, &call.arg(1), since, 4, 9, 4)?;
    let ns = iso::time_ns(&other) - iso::time_ns(&t.time());
    let ns = duration::round_ns(ns, &s);
    let d = duration::balance(ns, s.largest);
    duration::new_signed(vm, d, since)
}

/** PlainTime.prototype.until. */
fn time_until(vm: &mut Vm, call: Call) -> Res<JsValue> {
    time_diff(vm, call, false)
}

/** PlainTime.prototype.since. */
fn time_since(vm: &mut Vm, call: Call) -> Res<JsValue> {
    time_diff(vm, call, true)
}

/** DifferenceISODateTime then rounding, for PlainDate (no time) and PlainDateTime. */
fn datetime_diff_core(
    vm: &mut Vm,
    one: ((i64, i64, i64), [i64; 6]),
    two: ((i64, i64, i64), [i64; 6]),
    s: &duration::Settings,
) -> Res<Dur> {
    let mut time = iso::time_ns(&two.1) - iso::time_ns(&one.1);
    let time_sign = time.signum() as i64;
    let date_sign = iso::compare_date(one.0, two.0);
    let mut adjusted = two.0;
    if time_sign == date_sign && time_sign != 0 {
        adjusted =
            iso::from_epoch_days(iso::epoch_days(adjusted.0, adjusted.1, adjusted.2) + time_sign);
        time -= i128::from(time_sign) * NS_DAY;
    }
    let date_largest = duration::UNITS[s.largest.min(3)];
    let mut dd = iso::date_until(one.0, adjusted, date_largest);
    if s.largest > 3 {
        time += i128::from(dd[3]) * NS_DAY;
        dd[3] = 0;
    }
    let _ = vm;
    Ok(duration::round_relative(
        dd,
        time,
        s,
        one.0,
        iso::epoch_ns(two.0, &two.1),
    ))
}

/** DifferenceTemporalPlainDate. */
fn date_diff(vm: &mut Vm, call: Call, since: bool) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    let other = to_date(vm, &call.arg(0), &JsValue::Undefined)?;
    let s = duration::difference_settings(vm, &call.arg(1), since, 0, 3, 3)?;
    let d = datetime_diff_core(vm, (t.date(), [0; 6]), (other, [0; 6]), &s)?;
    duration::new_signed(vm, d, since)
}

/** PlainDate.prototype.until. */
fn date_until(vm: &mut Vm, call: Call) -> Res<JsValue> {
    date_diff(vm, call, false)
}

/** PlainDate.prototype.since. */
fn date_since(vm: &mut Vm, call: Call) -> Res<JsValue> {
    date_diff(vm, call, true)
}

/** DifferenceTemporalPlainDateTime. */
fn datetime_diff(vm: &mut Vm, call: Call, since: bool) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let other = to_datetime(vm, &call.arg(0), &JsValue::Undefined)?;
    let s = duration::difference_settings(vm, &call.arg(1), since, 0, 9, 3)?;
    let d = datetime_diff_core(vm, (t.date(), t.time()), other, &s)?;
    duration::new_signed(vm, d, since)
}

/** PlainDateTime.prototype.until. */
fn datetime_until(vm: &mut Vm, call: Call) -> Res<JsValue> {
    datetime_diff(vm, call, false)
}

/** PlainDateTime.prototype.since. */
fn datetime_since(vm: &mut Vm, call: Call) -> Res<JsValue> {
    datetime_diff(vm, call, true)
}

/** PlainTime.prototype.round. */
fn time_round(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    let r = duration::round_to(vm, &call.arg(0), 4, false)?;
    let ns = iso::round_by(iso::time_ns(&t.time()), r.0, &r.1);
    new_time(vm, None, iso::balance_time(ns).1)
}

/** PlainDateTime.prototype.round. */
fn datetime_round(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let r = duration::round_to(vm, &call.arg(0), 3, true)?;
    let ns = iso::round_by(iso::time_ns(&t.time()), r.0, &r.1);
    let (carry, time) = iso::balance_time(ns);
    let (y, m, d) = t.date();
    let date = iso::from_epoch_days(iso::epoch_days(y, m, d) + carry);
    check_datetime(vm, date, &time)?;
    new_datetime(vm, None, date, time)
}

/** The calendar annotation for a calendarName option. */
pub(super) fn calendar_suffix(show: &str) -> &'static str {
    match show {
        "always" => "[u-ca=iso8601]",
        "critical" => "[!u-ca=iso8601]",
        _ => "",
    }
}

/** GetTemporalShowCalendarNameOption. */
pub(super) fn show_calendar(vm: &mut Vm, o: Option<ObjectId>) -> Res<String> {
    let v = string_option(
        vm,
        o,
        "calendarName",
        &["auto", "always", "never", "critical"],
        Some("auto"),
    )?;
    Ok(v.unwrap_or_default())
}

/** An ISO date as text. */
pub(super) fn format_date(d: (i64, i64, i64)) -> String {
    format!("{}-{:02}-{:02}", iso::format_year(d.0), d.1, d.2)
}

/** PlainDate.prototype.toString. */
fn date_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    let o = options(vm, &call.arg(0))?;
    let show = show_calendar(vm, o)?;
    js(&format!(
        "{}{}",
        format_date(t.date()),
        calendar_suffix(&show)
    ))
}

/** PlainDate toJSON and toLocaleString. */
fn date_to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    js(&format_date(t.date()))
}

/** PlainTime.prototype.toString. */
fn time_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    let o = options(vm, &call.arg(0))?;
    let p = duration::precision_options(vm, o)?;
    let ns = iso::round_by(iso::time_ns(&t.time()), p.increment, &p.mode);
    js(&iso::format_time(&iso::balance_time(ns).1, p.digits))
}

/** PlainTime toJSON and toLocaleString. */
fn time_to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Time)?;
    js(&iso::format_time(&t.time(), None))
}

/** PlainDateTime.prototype.toString. */
fn datetime_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let o = options(vm, &call.arg(0))?;
    let show = show_calendar(vm, o)?;
    let p = duration::precision_options(vm, o)?;
    let ns = iso::round_by(iso::time_ns(&t.time()), p.increment, &p.mode);
    let (carry, time) = iso::balance_time(ns);
    let (y, m, d) = t.date();
    let date = iso::from_epoch_days(iso::epoch_days(y, m, d) + carry);
    check_datetime(vm, date, &time)?;
    js(&format!(
        "{}T{}{}",
        format_date(date),
        iso::format_time(&time, p.digits),
        calendar_suffix(&show)
    ))
}

/** PlainDateTime toJSON and toLocaleString. */
fn datetime_to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    js(&format!(
        "{}T{}",
        format_date(t.date()),
        iso::format_time(&t.time(), None)
    ))
}

/** PlainDate.prototype.toPlainDateTime. */
fn date_to_datetime(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    let time = match call.arg(0) {
        JsValue::Undefined => [0; 6],
        v => to_time(vm, &v, &JsValue::Undefined)?,
    };
    check_datetime(vm, t.date(), &time)?;
    new_datetime(vm, None, t.date(), time)
}

/** PlainDateTime.prototype.toPlainDate. */
fn datetime_to_date(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    new_date(vm, None, t.date())
}

/** PlainDateTime.prototype.toPlainTime. */
fn datetime_to_time(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    new_time(vm, None, t.time())
}

/** PlainDateTime.prototype.withPlainTime. */
fn datetime_with_time(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    let time = match call.arg(0) {
        JsValue::Undefined => [0; 6],
        v => to_time(vm, &v, &JsValue::Undefined)?,
    };
    check_datetime(vm, t.date(), &time)?;
    new_datetime(vm, None, t.date(), time)
}

/** withCalendar on PlainDate. */
fn date_with_calendar(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Date)?;
    calendar_required(vm, &call.arg(0))?;
    new_date(vm, None, t.date())
}

/** withCalendar on PlainDateTime. */
fn datetime_with_calendar(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::DateTime)?;
    calendar_required(vm, &call.arg(0))?;
    new_datetime(vm, None, t.date(), t.time())
}

/** A required calendar argument. */
fn calendar_required(vm: &mut Vm, value: &JsValue) -> Res<()> {
    if matches!(value, JsValue::Undefined) {
        return Err(vm.type_error("a calendar is required"));
    }
    to_calendar(vm, value)
}

/** Install the three plain classes. */
pub(super) fn install(b: &mut Builder, ns: ObjectId, protos: &mut [ObjectId; 8]) {
    protos[Kind::Date as usize] = class(
        b,
        ns,
        Kind::Date,
        date_ctor,
        3,
        &[("from", 1, date_from), ("compare", 2, date_compare)],
        &[
            ("add", 1, date_add),
            ("subtract", 1, date_subtract),
            ("with", 1, date_with),
            ("withCalendar", 1, date_with_calendar),
            ("until", 1, date_until),
            ("since", 1, date_since),
            ("equals", 1, date_equals),
            ("toPlainDateTime", 0, date_to_datetime),
            ("toString", 0, date_to_string),
            ("toLocaleString", 0, date_to_json),
            ("toJSON", 0, date_to_json),
        ],
        DATE_GETTERS,
    );
    protos[Kind::Time as usize] = class(
        b,
        ns,
        Kind::Time,
        time_ctor,
        0,
        &[("from", 1, time_from), ("compare", 2, time_compare)],
        &[
            ("add", 1, time_add),
            ("subtract", 1, time_subtract),
            ("with", 1, time_with),
            ("until", 1, time_until),
            ("since", 1, time_since),
            ("round", 1, time_round),
            ("equals", 1, time_equals),
            ("toString", 0, time_to_string),
            ("toLocaleString", 0, time_to_json),
            ("toJSON", 0, time_to_json),
        ],
        TIME_GETTERS,
    );
    protos[Kind::DateTime as usize] = class(
        b,
        ns,
        Kind::DateTime,
        datetime_ctor,
        3,
        &[("from", 1, datetime_from), ("compare", 2, datetime_compare)],
        &[
            ("with", 1, datetime_with),
            ("withPlainTime", 0, datetime_with_time),
            ("withCalendar", 1, datetime_with_calendar),
            ("add", 1, datetime_add),
            ("subtract", 1, datetime_subtract),
            ("until", 1, datetime_until),
            ("since", 1, datetime_since),
            ("round", 1, datetime_round),
            ("equals", 1, datetime_equals),
            ("toString", 0, datetime_to_string),
            ("toLocaleString", 0, datetime_to_json),
            ("toJSON", 0, datetime_to_json),
            ("toPlainDate", 0, datetime_to_date),
            ("toPlainTime", 0, datetime_to_time),
        ],
        DT_GETTERS,
    );
}
