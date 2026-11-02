/** Temporal.PlainYearMonth and Temporal.PlainMonthDay over the ISO 8601 calendar. A year-month keeps
day 1 as its reference day and a month-day keeps 1972 (a leap year) as its reference year. */
use super::duration;
use super::iso;
use super::plain::{self, DAY, Fields, MONTH, MONTH_CODE, YEAR};
use super::*;

/** Fields a year-month bag carries, alphabetically. */
const YM_F: &[usize] = &[MONTH, MONTH_CODE, YEAR];
/** Fields a month-day bag carries, alphabetically. */
const MD_F: &[usize] = &[DAY, MONTH, MONTH_CODE, YEAR];
/** The reference year of a month-day. */
const REF_YEAR: i64 = 1972;

/** ISOYearMonthWithinLimits as a RangeError. */
fn check_ym(vm: &mut Vm, y: i64, m: i64) -> Res<()> {
    if !iso::year_month_in_limits(y, m) {
        return Err(vm.range_error("year-month outside the supported range"));
    }
    Ok(())
}

/** A new PlainYearMonth. */
fn new_ym(vm: &mut Vm, target: Option<&JsValue>, y: i64, m: i64, day: i64) -> Res<JsValue> {
    make(
        vm,
        target,
        T {
            kind: Kind::YearMonth,
            v: vec![y.into(), m.into(), day.into()],
            tz: String::new(),
        },
    )
}

/** A new PlainMonthDay. */
fn new_md(vm: &mut Vm, target: Option<&JsValue>, year: i64, m: i64, d: i64) -> Res<JsValue> {
    make(
        vm,
        target,
        T {
            kind: Kind::MonthDay,
            v: vec![year.into(), m.into(), d.into()],
            tz: String::new(),
        },
    )
}

/** ToTemporalYearMonth: the year and month of any accepted input. */
fn to_ym(vm: &mut Vm, item: &JsValue, opts: &JsValue) -> Res<(i64, i64)> {
    if let JsValue::Object(_) = item {
        if let Some(t) = as_kind(vm, item, Kind::YearMonth) {
            plain::read_overflow(vm, opts)?;
            return Ok((t.date().0, t.date().1));
        }
        plain::bag_calendar(vm, item)?;
        let f = plain::read_fields(vm, item, YM_F)?;
        return ym_from_fields(vm, &f, opts);
    }
    let s = require_string(vm, item)?;
    let p = iso::parse_year_month(&s)
        .or_else(|| iso::parse_datetime(&s))
        .filter(|p| !p.z);
    let p = p.ok_or_else(|| bad_string(vm, &s))?;
    plain::parsed_calendar(vm, &p)?;
    let (y, m, _) = p.date.unwrap_or_default();
    check_ym(vm, y, m)?;
    plain::read_overflow(vm, opts)?;
    Ok((y, m))
}

/** CalendarYearMonthFromFields for ISO. */
fn ym_from_fields(vm: &mut Vm, f: &Fields, opts: &JsValue) -> Res<(i64, i64)> {
    if f[YEAR].is_none() || (f[MONTH].is_none() && f[MONTH_CODE].is_none()) {
        return Err(vm.type_error("year and month are required"));
    }
    let constrain = plain::read_overflow(vm, opts)?;
    let month = plain::month_of(vm, f)?.unwrap_or(1);
    let y = f[YEAR].unwrap_or(0);
    let (y, m, _) = iso::regulate_date(y, month, 1, constrain)
        .ok_or_else(|| vm.range_error("month out of range"))?;
    check_ym(vm, y, m)?;
    Ok((y, m))
}

/** ToTemporalMonthDay: the month and day of any accepted input. */
fn to_md(vm: &mut Vm, item: &JsValue, opts: &JsValue) -> Res<(i64, i64)> {
    if let JsValue::Object(_) = item {
        if let Some(t) = as_kind(vm, item, Kind::MonthDay) {
            plain::read_overflow(vm, opts)?;
            return Ok((t.date().1, t.date().2));
        }
        plain::bag_calendar(vm, item)?;
        let f = plain::read_fields(vm, item, MD_F)?;
        return md_from_fields(vm, &f, opts);
    }
    let s = require_string(vm, item)?;
    let p = iso::parse_month_day(&s)
        .or_else(|| iso::parse_datetime(&s))
        .filter(|p| !p.z);
    let p = p.ok_or_else(|| bad_string(vm, &s))?;
    plain::parsed_calendar(vm, &p)?;
    let (y, m, d) = p.date.unwrap_or_default();
    plain::check_date(vm, (y, m, d))?;
    plain::read_overflow(vm, opts)?;
    Ok((m, d))
}

/** CalendarMonthDayFromFields for ISO: a bare month needs a year to resolve against. */
fn md_from_fields(vm: &mut Vm, f: &Fields, opts: &JsValue) -> Res<(i64, i64)> {
    let missing_month = f[MONTH].is_none() && f[MONTH_CODE].is_none();
    let bare_month = f[MONTH].is_some() && f[MONTH_CODE].is_none() && f[YEAR].is_none();
    if f[DAY].is_none() || missing_month || bare_month {
        return Err(vm.type_error("month and day are required"));
    }
    let constrain = plain::read_overflow(vm, opts)?;
    let month = plain::month_of(vm, f)?.unwrap_or(1);
    let year = f[YEAR].unwrap_or(REF_YEAR);
    let day = f[DAY].unwrap_or(1);
    let (_, m, d) = iso::regulate_date(year, month, day, constrain)
        .ok_or_else(|| vm.range_error("date out of range"))?;
    Ok((m, d))
}

/** `new Temporal.PlainYearMonth(y, m, calendar, referenceDay)`. */
fn ym_ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = plain::need_new(vm, &call)?.clone();
    let y = to_int(vm, &call.arg(0))?;
    let m = to_int(vm, &call.arg(1))?;
    calendar_arg(vm, &call.arg(2))?;
    let day = match call.arg(3) {
        JsValue::Undefined => 1,
        v => to_int(vm, &v)?,
    };
    if !iso::valid_date(y, m, day) {
        return Err(vm.range_error("invalid ISO date"));
    }
    check_ym(vm, y, m)?;
    new_ym(vm, Some(&target), y, m, day)
}

/** `new Temporal.PlainMonthDay(m, d, calendar, referenceYear)`. */
fn md_ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = plain::need_new(vm, &call)?.clone();
    let m = to_int(vm, &call.arg(0))?;
    let d = to_int(vm, &call.arg(1))?;
    calendar_arg(vm, &call.arg(2))?;
    let year = match call.arg(3) {
        JsValue::Undefined => REF_YEAR,
        v => to_int(vm, &v)?,
    };
    if !iso::valid_date(year, m, d) {
        return Err(vm.range_error("invalid ISO date"));
    }
    plain::check_date(vm, (year, m, d))?;
    new_md(vm, Some(&target), year, m, d)
}

/** PlainYearMonth.from. */
fn ym_from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (y, m) = to_ym(vm, &call.arg(0), &call.arg(1))?;
    new_ym(vm, None, y, m, 1)
}

/** PlainMonthDay.from. */
fn md_from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (m, d) = to_md(vm, &call.arg(0), &call.arg(1))?;
    new_md(vm, None, REF_YEAR, m, d)
}

/** PlainYearMonth.compare. */
fn ym_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = to_ym(vm, &call.arg(0), &JsValue::Undefined)?;
    let b = to_ym(vm, &call.arg(1), &JsValue::Undefined)?;
    plain::ordering(a.cmp(&b))
}

/** PlainYearMonth.prototype.equals. */
fn ym_equals(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    let other = to_ym(vm, &call.arg(0), &JsValue::Undefined)?;
    Ok(JsValue::Boolean(
        (t.date().0, t.date().1) == other && t.date().2 == 1,
    ))
}

/** PlainMonthDay.prototype.equals. */
fn md_equals(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::MonthDay)?;
    let other = to_md(vm, &call.arg(0), &JsValue::Undefined)?;
    Ok(JsValue::Boolean(
        (t.date().1, t.date().2) == other && t.date().0 == REF_YEAR,
    ))
}

/** Merge a partial bag's year-month or month-day fields over the receiver's. */
fn with_fields(vm: &mut Vm, bag: &JsValue, base: Fields, which: &[usize]) -> Res<Fields> {
    plain::partial_bag(vm, bag)?;
    let f = plain::read_fields(vm, bag, which)?;
    if which.iter().all(|&i| f[i].is_none()) {
        return Err(vm.type_error("no recognized fields"));
    }
    let mut out = base;
    if f[MONTH].is_some() || f[MONTH_CODE].is_some() {
        out[MONTH] = f[MONTH];
        out[MONTH_CODE] = f[MONTH_CODE];
    }
    for i in [DAY, YEAR] {
        if f[i].is_some() {
            out[i] = f[i];
        }
    }
    Ok(out)
}

/** PlainYearMonth.prototype.with. */
fn ym_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    let mut base = [None; 10];
    base[YEAR] = Some(t.date().0);
    base[MONTH_CODE] = Some(t.date().1);
    let f = with_fields(vm, &call.arg(0), base, YM_F)?;
    let (y, m) = ym_from_fields(vm, &f, &call.arg(1))?;
    new_ym(vm, None, y, m, 1)
}

/** PlainMonthDay.prototype.with. */
fn md_with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::MonthDay)?;
    let mut base = [None; 10];
    base[MONTH_CODE] = Some(t.date().1);
    base[DAY] = Some(t.date().2);
    let f = with_fields(vm, &call.arg(0), base, MD_F)?;
    let (m, d) = md_from_fields(vm, &f, &call.arg(1))?;
    new_md(vm, None, REF_YEAR, m, d)
}

/** PlainYearMonth add and subtract: years and months move it; other units must be zero-signed. */
fn ym_add_sub(vm: &mut Vm, call: Call, sign: i128) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    let d = plain::duration_arg(vm, &call, sign)?;
    plain::read_overflow(vm, &call.arg(1))?;
    let (y0, m0, _) = t.date();
    let months = i128::from(y0) * 12 + i128::from(m0 - 1) + d[0] * 12 + d[1];
    let (y, m) = (months.div_euclid(12), months.rem_euclid(12) + 1);
    if y.abs() > 300_000 {
        return Err(vm.range_error("year-month outside the supported range"));
    }
    check_ym(vm, y as i64, m as i64)?;
    new_ym(vm, None, y as i64, m as i64, 1)
}

/** PlainYearMonth.prototype.add. */
fn ym_add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    ym_add_sub(vm, call, 1)
}

/** PlainYearMonth.prototype.subtract. */
fn ym_subtract(vm: &mut Vm, call: Call) -> Res<JsValue> {
    ym_add_sub(vm, call, -1)
}

/** DifferenceTemporalPlainYearMonth. */
fn ym_diff(vm: &mut Vm, call: Call, since: bool) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    let (y2, m2) = to_ym(vm, &call.arg(0), &JsValue::Undefined)?;
    let s = duration::difference_settings(vm, &call.arg(1), since, 0, 1, 0)?;
    let one = (t.date().0, t.date().1, 1);
    let two = (y2, m2, 1);
    let dd = iso::date_until(one, two, duration::UNITS[s.largest]);
    let d = duration::round_relative(
        [dd[0], dd[1], 0, 0],
        0,
        &s,
        one,
        iso::epoch_ns(two, &[0; 6]),
    );
    duration::new_signed(vm, d, since)
}

/** PlainYearMonth.prototype.until. */
fn ym_until(vm: &mut Vm, call: Call) -> Res<JsValue> {
    ym_diff(vm, call, false)
}

/** PlainYearMonth.prototype.since. */
fn ym_since(vm: &mut Vm, call: Call) -> Res<JsValue> {
    ym_diff(vm, call, true)
}

/** PlainYearMonth.prototype.toString. */
fn ym_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    let o = options(vm, &call.arg(0))?;
    let show = plain::show_calendar(vm, o)?;
    js(&ym_text(&t, &show))
}

/** A year-month as text: the reference day appears only with a calendar annotation. */
fn ym_text(t: &T, show: &str) -> String {
    let (y, m, d) = t.date();
    match show {
        "always" | "critical" => format!(
            "{}{}",
            plain::format_date((y, m, d)),
            plain::calendar_suffix(show)
        ),
        _ => format!("{}-{m:02}", iso::format_year(y)),
    }
}

/** PlainYearMonth toJSON and toLocaleString. */
fn ym_to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    js(&ym_text(&t, "auto"))
}

/** A month-day as text: the reference year appears only with a calendar annotation. */
fn md_text(t: &T, show: &str) -> String {
    let (y, m, d) = t.date();
    match show {
        "always" | "critical" => format!(
            "{}{}",
            plain::format_date((y, m, d)),
            plain::calendar_suffix(show)
        ),
        _ => format!("{m:02}-{d:02}"),
    }
}

/** PlainMonthDay.prototype.toString. */
fn md_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::MonthDay)?;
    let o = options(vm, &call.arg(0))?;
    let show = plain::show_calendar(vm, o)?;
    js(&md_text(&t, &show))
}

/** PlainMonthDay toJSON and toLocaleString. */
fn md_to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::MonthDay)?;
    js(&md_text(&t, "auto"))
}

/** Read one required field of a toPlainDate argument. */
fn required_field(vm: &mut Vm, item: &JsValue, which: usize) -> Res<i64> {
    if !matches!(item, JsValue::Object(_)) {
        return Err(vm.type_error("toPlainDate requires an object"));
    }
    let f = plain::read_fields(vm, item, &[which])?;
    f[which].ok_or_else(|| vm.type_error("a required field is missing"))
}

/** PlainYearMonth.prototype.toPlainDate. */
fn ym_to_date(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::YearMonth)?;
    let day = required_field(vm, &call.arg(0), DAY)?;
    let (y, m, _) = t.date();
    let date = iso::regulate_date(y, m, day, true).unwrap_or((y, m, 1));
    let date = plain::check_date(vm, date)?;
    plain::new_date(vm, None, date)
}

/** PlainMonthDay.prototype.toPlainDate. */
fn md_to_date(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::MonthDay)?;
    let year = required_field(vm, &call.arg(0), YEAR)?;
    let (_, m, d) = t.date();
    let date = iso::regulate_date(year, m, d, true).unwrap_or((year, m, 1));
    let date = plain::check_date(vm, date)?;
    plain::new_date(vm, None, date)
}

/** Generate getter functions and their table. */
macro_rules! getters {
    ($table:ident, $kind:expr, [$($name:literal => $f:ident),* $(,)?]) => {
        $(fn $f(vm: &mut Vm, call: Call) -> Res<JsValue> { plain::field(vm, &call, $kind, $name) })*
        const $table: &[(&str, NativeFn)] = &[$(($name, $f)),*];
    };
}

getters!(YM_GETTERS, Kind::YearMonth, [
    "calendarId" => ym_cal, "era" => ym_era, "eraYear" => ym_era_year, "year" => ym_year,
    "month" => ym_month, "monthCode" => ym_month_code, "daysInYear" => ym_diy, "daysInMonth" => ym_dim,
    "monthsInYear" => ym_miy, "inLeapYear" => ym_leap,
]);
getters!(MD_GETTERS, Kind::MonthDay, [
    "calendarId" => md_cal, "monthCode" => md_month_code, "day" => md_day,
]);

/** Install the two partial-date classes. */
pub(super) fn install(b: &mut Builder, ns: ObjectId, protos: &mut [ObjectId; 8]) {
    protos[Kind::YearMonth as usize] = class(
        b,
        ns,
        Kind::YearMonth,
        ym_ctor,
        2,
        &[("from", 1, ym_from), ("compare", 2, ym_compare)],
        &[
            ("with", 1, ym_with),
            ("add", 1, ym_add),
            ("subtract", 1, ym_subtract),
            ("until", 1, ym_until),
            ("since", 1, ym_since),
            ("equals", 1, ym_equals),
            ("toString", 0, ym_to_string),
            ("toLocaleString", 0, ym_to_json),
            ("toJSON", 0, ym_to_json),
            ("toPlainDate", 1, ym_to_date),
        ],
        YM_GETTERS,
    );
    protos[Kind::MonthDay as usize] = class(
        b,
        ns,
        Kind::MonthDay,
        md_ctor,
        2,
        &[("from", 1, md_from)],
        &[
            ("with", 1, md_with),
            ("equals", 1, md_equals),
            ("toString", 0, md_to_string),
            ("toLocaleString", 0, md_to_json),
            ("toJSON", 0, md_to_json),
            ("toPlainDate", 1, md_to_date),
        ],
        MD_GETTERS,
    );
}
