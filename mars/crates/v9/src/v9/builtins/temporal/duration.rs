/** Temporal.Duration and the unit, rounding and difference options every Temporal type shares:
validity, the ISO 8601 duration grammar, balancing and formatting. */
use super::iso::{self, NS_DAY};
use super::*;

/** A duration's ten fields, years to nanoseconds. */
pub(super) type Dur = [i128; 10];

/** Unit names, largest first; plurals add an `s`. */
pub(super) const UNITS: [&str; 10] = [
    "year",
    "month",
    "week",
    "day",
    "hour",
    "minute",
    "second",
    "millisecond",
    "microsecond",
    "nanosecond",
];

/** Duration field names, in slot order. */
const FIELD_NAMES: [&str; 10] = [
    "years",
    "months",
    "weeks",
    "days",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
    "microseconds",
    "nanoseconds",
];

/** Nanoseconds per unit for day and the time units (index 3 onwards). */
pub(super) const UNIT_NS: [i128; 10] = [
    0,
    0,
    0,
    NS_DAY,
    3_600_000_000_000,
    60_000_000_000,
    1_000_000_000,
    1_000_000,
    1000,
    1,
];

/** Nanoseconds in the time part of a duration, days excluded. */
pub(super) fn time_ns(d: &Dur) -> i128 {
    (4..10).map(|i| d[i] * UNIT_NS[i]).sum()
}

/** The sign of a duration. */
fn sign_of(d: &Dur) -> i128 {
    d.iter().find(|v| **v != 0).map_or(0, |v| v.signum())
}

/** IsValidDuration. */
fn valid(d: &Dur) -> bool {
    let s = sign_of(d);
    if d.iter().any(|v| v.signum() != 0 && v.signum() != s) {
        return false;
    }
    if d[..3].iter().any(|v| v.abs() >= 1 << 32) {
        return false;
    }
    let total = d[3].checked_mul(NS_DAY).and_then(|days| {
        (4..10).try_fold(days, |a, i| {
            d[i].checked_mul(UNIT_NS[i]).and_then(|x| a.checked_add(x))
        })
    });
    total.is_some_and(|t| t.abs() < (1i128 << 53) * 1_000_000_000)
}

/** A new Duration, or the RangeError for an invalid one. */
pub(super) fn new_duration(vm: &mut Vm, target: Option<&JsValue>, d: Dur) -> Res<JsValue> {
    if !valid(&d) {
        return Err(vm.range_error("invalid duration"));
    }
    make(
        vm,
        target,
        T {
            kind: Kind::Duration,
            v: d.to_vec(),
            tz: String::new(),
        },
    )
}

/** A difference result, negated for `since`. */
pub(super) fn new_signed(vm: &mut Vm, d: Dur, since: bool) -> Res<JsValue> {
    let d = if since { d.map(|v| -v) } else { d };
    new_duration(vm, None, d)
}

/** The slots of a Duration value. */
fn dur(t: &T) -> Dur {
    let mut d = [0; 10];
    d.copy_from_slice(&t.v[..10]);
    d
}

/** ToIntegerIfIntegral. */
fn to_integral(vm: &mut Vm, value: &JsValue) -> Res<i128> {
    let n = vm.to_number(value)?;
    if !n.is_finite() || n.trunc() != n {
        return Err(vm.range_error("duration fields must be integers"));
    }
    Ok(n as i128)
}

/** ToTemporalDuration: a Duration, a string, or a partial bag. */
pub(super) fn to_duration(vm: &mut Vm, item: &JsValue) -> Res<Dur> {
    if let Some(t) = as_kind(vm, item, Kind::Duration) {
        return Ok(dur(&t));
    }
    let JsValue::Object(_) = item else {
        let s = require_string(vm, item)?;
        let d = parse(&s).ok_or_else(|| bad_string(vm, &s))?;
        if !valid(&d) {
            return Err(vm.range_error("invalid duration"));
        }
        return Ok(d);
    };
    let partial = partial(vm, item)?;
    let d = partial.map(|v| v.unwrap_or(0));
    if !valid(&d) {
        return Err(vm.range_error("invalid duration"));
    }
    Ok(d)
}

/** ToTemporalPartialDurationRecord: fields in alphabetical order, at least one. */
fn partial(vm: &mut Vm, bag: &JsValue) -> Res<[Option<i128>; 10]> {
    let mut order: Vec<usize> = (0..10).collect();
    order.sort_by_key(|i| FIELD_NAMES[*i]);
    let mut out = [None; 10];
    for i in order {
        let value = vm.get(bag, &Key::from(FIELD_NAMES[i]))?;
        if !matches!(value, JsValue::Undefined) {
            out[i] = Some(to_integral(vm, &value)?);
        }
    }
    if out.iter().all(Option::is_none) {
        return Err(vm.type_error("a duration needs at least one field"));
    }
    Ok(out)
}

/** ParseTemporalDurationString. */
pub(super) fn parse(s: &str) -> Option<Dur> {
    let b = s.as_bytes();
    let mut i = 0;
    let sign = match b.first() {
        Some(b'-') => {
            i += 1;
            -1
        }
        Some(b'+') => {
            i += 1;
            1
        }
        _ => 1,
    };
    if !matches!(b.get(i), Some(b'P' | b'p')) {
        return None;
    }
    i += 1;
    let mut d = [0i128; 10];
    let (mut in_time, mut any, mut last, mut frac_seen, mut time_any) =
        (false, false, -1i32, false, false);
    while i < b.len() {
        if matches!(b[i], b'T' | b't') {
            if in_time {
                return None;
            }
            in_time = true;
            i += 1;
            continue;
        }
        if frac_seen {
            return None;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        let whole: i128 = s[start..i].parse::<i128>().ok().unwrap_or(i128::MAX / 4);
        let mut frac: Option<i128> = None;
        if i < b.len() && matches!(b[i], b'.' | b',') {
            i += 1;
            let fs = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n = i - fs;
            if n == 0 || n > 9 {
                return None;
            }
            frac = Some(s[fs..i].parse::<i128>().ok()? * 10i128.pow(9 - n as u32));
        }
        let unit = (*b.get(i)?).to_ascii_uppercase();
        i += 1;
        let idx: i32 = match (in_time, unit) {
            (false, b'Y') => 0,
            (false, b'M') => 1,
            (false, b'W') => 2,
            (false, b'D') => 3,
            (true, b'H') => 4,
            (true, b'M') => 5,
            (true, b'S') => 6,
            _ => return None,
        };
        if idx <= last || (frac.is_some() && idx < 4) {
            return None;
        }
        last = idx;
        any = true;
        time_any |= in_time;
        d[idx as usize] = whole.min(i128::MAX / 4);
        if let Some(f) = frac {
            frac_seen = true;
            let ns = f * UNIT_NS[idx as usize] / 1_000_000_000;
            spread(&mut d, idx as usize + 1, ns);
        }
    }
    if !any || (in_time && !time_any) {
        return None;
    }
    Some(d.map(|v| v * sign))
}

/** Spread fractional nanoseconds over the units below a fractional one. */
fn spread(d: &mut Dur, from: usize, mut ns: i128) {
    for i in from..10 {
        d[i] += ns / UNIT_NS[i];
        ns %= UNIT_NS[i];
    }
}

/** `new Temporal.Duration(...)`. */
fn ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Temporal.Duration requires 'new'"));
    };
    let mut d = [0; 10];
    for (i, slot) in d.iter_mut().enumerate() {
        let v = call.arg(i);
        if !matches!(v, JsValue::Undefined) {
            *slot = to_integral(vm, &v)?;
        }
    }
    new_duration(vm, Some(&target), d)
}

/** Duration.from. */
fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let d = to_duration(vm, &call.arg(0))?;
    new_duration(vm, None, d)
}

/** Duration.compare, without a relativeTo calendar. */
fn compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let a = to_duration(vm, &call.arg(0))?;
    let b = to_duration(vm, &call.arg(1))?;
    let o = options(vm, &call.arg(2))?;
    if let Some(id) = o {
        vm.get(&JsValue::Object(id), &Key::from("relativeTo"))?;
    }
    if a == b {
        return Ok(JsValue::Number(0.0));
    }
    if a[..3].iter().chain(b[..3].iter()).any(|v| *v != 0) {
        return Err(vm.range_error("comparing calendar units needs relativeTo"));
    }
    let total = |d: &Dur| d[3] * NS_DAY + time_ns(d);
    Ok(JsValue::Number(total(&a).cmp(&total(&b)) as i32 as f64))
}

/** A field getter. */
fn get_field(vm: &mut Vm, call: &Call, i: usize) -> Res<JsValue> {
    let t = this_t(vm, call, Kind::Duration)?;
    Ok(JsValue::Number(dur(&t)[i] as f64))
}

/** Generate the field getters. */
macro_rules! field_getters {
    ($($i:literal => $f:ident),*) => {
        $(fn $f(vm: &mut Vm, call: Call) -> Res<JsValue> { get_field(vm, &call, $i) })*
    };
}
field_getters!(0 => g_years, 1 => g_months, 2 => g_weeks, 3 => g_days, 4 => g_hours, 5 => g_minutes,
    6 => g_seconds, 7 => g_ms, 8 => g_us, 9 => g_ns);

/** Duration.prototype.sign. */
fn g_sign(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    Ok(JsValue::Number(sign_of(&dur(&t)) as f64))
}

/** Duration.prototype.blank. */
fn g_blank(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    Ok(JsValue::Boolean(sign_of(&dur(&t)) == 0))
}

/** Duration.prototype.negated. */
fn negated(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    new_duration(vm, None, dur(&t).map(|v| -v))
}

/** Duration.prototype.abs. */
fn abs(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    new_duration(vm, None, dur(&t).map(i128::abs))
}

/** Duration.prototype.with. */
fn with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    let bag = call.arg(0);
    if !matches!(bag, JsValue::Object(_)) {
        return Err(vm.type_error("expected a property bag"));
    }
    let p = partial(vm, &bag)?;
    let mut d = dur(&t);
    for (slot, v) in d.iter_mut().zip(p) {
        if let Some(v) = v {
            *slot = v;
        }
    }
    new_duration(vm, None, d)
}

/** Duration add and subtract without relativeTo: calendar units are a RangeError. */
fn add_sub(vm: &mut Vm, call: Call, sign: i128) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    let other = to_duration(vm, &call.arg(0))?;
    let a = dur(&t);
    if a[..3].iter().chain(other[..3].iter()).any(|v| *v != 0) {
        return Err(vm.range_error("adding calendar units needs relativeTo"));
    }
    let total = |d: &Dur| d[3] * NS_DAY + time_ns(d);
    let sum = total(&a) + sign * total(&other);
    let largest = largest_of(&a).min(largest_of(&other));
    new_duration(vm, None, balance(sum, largest))
}

/** The largest non-zero unit, nanosecond for a zero duration. */
fn largest_of(d: &Dur) -> usize {
    d.iter().position(|v| *v != 0).unwrap_or(9)
}

/** Duration.prototype.add. */
fn add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    add_sub(vm, call, 1)
}

/** Duration.prototype.subtract. */
fn subtract(vm: &mut Vm, call: Call) -> Res<JsValue> {
    add_sub(vm, call, -1)
}

/** BalanceTimeDuration: nanoseconds as days (when largest allows) and time fields. */
pub(super) fn balance(ns: i128, largest: usize) -> Dur {
    let mut d = [0; 10];
    let mut rest = ns;
    for i in largest.max(3)..10 {
        if i == 3 && largest > 3 {
            continue;
        }
        d[i] = rest / UNIT_NS[i];
        rest %= UNIT_NS[i];
    }
    d
}

/** TemporalDurationToString for a precision. */
fn format(d: &Dur, digits: Option<i32>) -> String {
    let sign = sign_of(d);
    let a = d.map(i128::abs);
    let mut date = String::new();
    for (i, u) in ["Y", "M", "W", "D"].iter().enumerate() {
        if a[i] != 0 {
            date.push_str(&format!("{}{u}", a[i]));
        }
    }
    let sub = a[6] * 1_000_000_000 + a[7] * 1_000_000 + a[8] * 1000 + a[9];
    let (secs, frac) = (sub / 1_000_000_000, sub % 1_000_000_000);
    let mut time = String::new();
    for (i, u) in [(4, "H"), (5, "M")] {
        if a[i] != 0 {
            time.push_str(&format!("{}{u}", a[i]));
        }
    }
    if sub != 0 || (date.is_empty() && time.is_empty()) || digits.is_some() {
        let mut f = format!("{frac:09}");
        match digits {
            None => f = f.trim_end_matches('0').to_string(),
            Some(n) => f.truncate(n as usize),
        }
        let dot = if f.is_empty() {
            String::new()
        } else {
            format!(".{f}")
        };
        time.push_str(&format!("{secs}{dot}S"));
    }
    let mut out = String::from(if sign < 0 { "-P" } else { "P" });
    out.push_str(&date);
    if !time.is_empty() {
        out.push('T');
        out.push_str(&time);
    }
    out
}

/** Duration.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    let o = options(vm, &call.arg(0))?;
    let p = precision_options(vm, o)?;
    if p.digits == Some(-1) {
        return Err(vm.range_error("smallestUnit must be second or smaller"));
    }
    let mut d = dur(&t);
    if p.increment != 1 {
        let largest = largest_of(&d).max(4);
        let total = time_ns(&d);
        let rounded = iso::round_by(total, p.increment, &p.mode);
        let b = balance(rounded, largest);
        d[4..].copy_from_slice(&b[4..]);
        if !valid(&d) {
            return Err(vm.range_error("rounded duration is out of range"));
        }
    }
    js(&format(&d, p.digits))
}

/** Duration toJSON and toLocaleString. */
fn to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_t(vm, &call, Kind::Duration)?;
    js(&format(&dur(&t), None))
}

/** The parsed unit of an option: its index, or None when undefined. */
fn unit_option(vm: &mut Vm, o: Option<ObjectId>, name: &str, auto: bool) -> Res<Option<usize>> {
    let Some(id) = o else { return Ok(None) };
    let value = vm.get(&JsValue::Object(id), &Key::from(name))?;
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    let text = vm.to_text(&value)?;
    if auto && text == "auto" {
        return Ok(Some(usize::MAX));
    }
    let singular = text.strip_suffix('s').unwrap_or(&text);
    UNITS
        .iter()
        .position(|u| *u == singular)
        .map(Some)
        .ok_or_else(|| vm.range_error(&format!("{text} is not a valid value for {name}")))
}

/** GetRoundingIncrementOption. */
fn increment_option(vm: &mut Vm, o: Option<ObjectId>) -> Res<i128> {
    let Some(id) = o else { return Ok(1) };
    let value = vm.get(&JsValue::Object(id), &Key::from("roundingIncrement"))?;
    if matches!(value, JsValue::Undefined) {
        return Ok(1);
    }
    let n = vm.to_number(&value)?;
    if !n.is_finite() || !(1.0..=1e9).contains(&n.trunc()) {
        return Err(vm.range_error("roundingIncrement is out of range"));
    }
    Ok(n.trunc() as i128)
}

/** GetRoundingModeOption. */
fn mode_option(vm: &mut Vm, o: Option<ObjectId>, default: &str) -> Res<String> {
    Ok(string_option(vm, o, "roundingMode", &ROUNDING_MODES, Some(default))?.unwrap_or_default())
}

/** NegateRoundingMode, for `since`. */
fn negate_mode(mode: &str) -> String {
    match mode {
        "ceil" => "floor",
        "floor" => "ceil",
        "halfCeil" => "halfFloor",
        "halfFloor" => "halfCeil",
        m => m,
    }
    .to_string()
}

/** MaximumTemporalDurationRoundingIncrement, None for calendar units. */
fn max_increment(unit: usize) -> Option<i128> {
    match unit {
        4 => Some(24),
        5 | 6 => Some(60),
        7..=9 => Some(1000),
        _ => None,
    }
}

/** ValidateTemporalRoundingIncrement. */
fn check_increment(vm: &mut Vm, inc: i128, max: i128, inclusive: bool) -> Res<()> {
    let limit = if inclusive { max } else { max - 1 };
    if inc > limit || max % inc != 0 {
        return Err(vm.range_error("roundingIncrement does not divide its unit"));
    }
    Ok(())
}

/** The settings of a difference operation. */
pub(super) struct Settings {
    pub largest: usize,
    pub smallest: usize,
    pub increment: i128,
    pub mode: String,
}

/** GetDifferenceSettings: unit indexes lo..=hi are allowed; default_largest when auto. */
pub(super) fn difference_settings(
    vm: &mut Vm,
    opts: &JsValue,
    since: bool,
    lo: usize,
    hi: usize,
    default_largest: usize,
) -> Res<Settings> {
    let o = options(vm, opts)?;
    let largest = unit_option(vm, o, "largestUnit", true)?;
    let increment = increment_option(vm, o)?;
    let mode = mode_option(vm, o, "trunc")?;
    let smallest = unit_option(vm, o, "smallestUnit", false)?.unwrap_or(hi);
    let ok = |u: usize| (lo..=hi).contains(&u);
    if largest.is_some_and(|l| l != usize::MAX && !ok(l)) || !ok(smallest) {
        return Err(vm.range_error("unit not allowed here"));
    }
    let largest = match largest {
        None | Some(usize::MAX) => default_largest.min(smallest),
        Some(l) => l,
    };
    if largest > smallest {
        return Err(vm.range_error("largestUnit is smaller than smallestUnit"));
    }
    if let Some(max) = max_increment(smallest) {
        check_increment(vm, increment, max, false)?;
    }
    if smallest < 3 && increment > 1 && largest < smallest {
        return Err(vm.range_error("roundingIncrement needs largestUnit equal to smallestUnit"));
    }
    let mode = if since { negate_mode(&mode) } else { mode };
    Ok(Settings {
        largest,
        smallest,
        increment,
        mode,
    })
}

/** Round a time difference to the settings' increment. */
pub(super) fn round_ns(ns: i128, s: &Settings) -> i128 {
    iso::round_by(ns, s.increment * UNIT_NS[s.smallest], &s.mode)
}

/** RoundRelativeDuration for ISO dates: days and time units, and calendar units by progress. */
pub(super) fn round_relative(
    dd: [i64; 4],
    time: i128,
    s: &Settings,
    start: (i64, i64, i64),
    dest_ns: i128,
) -> Dur {
    let mut out = [0i128; 10];
    for i in 0..4 {
        out[i] = i128::from(dd[i]);
    }
    if s.smallest >= 3 {
        let unit = s.increment * UNIT_NS[s.smallest];
        if s.largest > 3 {
            let b = balance(iso::round_by(time, unit, &s.mode), s.largest);
            out[4..].copy_from_slice(&b[4..]);
            return out;
        }
        let total = iso::round_by(out[3] * NS_DAY + time, unit, &s.mode);
        let b = balance(total % NS_DAY, 4);
        out[3] = total / NS_DAY;
        out[4..].copy_from_slice(&b[4..]);
        return out;
    }
    nudge_calendar(out, s, start, dest_ns)
}

/** NudgeToCalendarUnit: round the smallest calendar unit by how far the destination lies into it. */
fn nudge_calendar(mut d: Dur, s: &Settings, start: (i64, i64, i64), dest_ns: i128) -> Dur {
    let sign = if dest_ns < iso::epoch_ns(start, &[0; 6]) {
        -1
    } else {
        1
    };
    let u = s.smallest;
    if u == 2 {
        d[2] += d[3] / 7;
    }
    let r1 = d[u] / s.increment * s.increment;
    let r2 = r1 + s.increment * sign;
    let at = |n: i128| {
        let mut parts = [0i64; 4];
        for (i, p) in parts.iter_mut().enumerate().take(u) {
            *p = d[i] as i64;
        }
        parts[u] = n as i64;
        let date = iso::add_date(start, parts, true).unwrap_or(start);
        iso::epoch_ns(date, &[0; 6])
    };
    let (a, b) = (at(r1), at(r2));
    let den = (b - a).abs().max(1);
    let num = (dest_ns - a).abs();
    let k = r1.abs() / s.increment;
    let m = iso::round_by(sign * (k * den + num), den, &s.mode) / den;
    d[u] = m * s.increment;
    for slot in d.iter_mut().skip(u + 1) {
        *slot = 0;
    }
    d
}

/** The precision options of toString: digits shown, rounding increment and mode. */
pub(super) struct Precision {
    pub digits: Option<i32>,
    pub increment: i128,
    pub mode: String,
}

/** Read fractionalSecondDigits, roundingMode and smallestUnit, in that order. */
pub(super) fn precision_options(vm: &mut Vm, o: Option<ObjectId>) -> Res<Precision> {
    let digits = digits_option(vm, o)?;
    let mode = mode_option(vm, o, "trunc")?;
    let smallest = unit_option(vm, o, "smallestUnit", false)?;
    let (digits, increment) = match smallest {
        Some(5) => (Some(-1), UNIT_NS[5]),
        Some(6) => (Some(0), UNIT_NS[6]),
        Some(7) => (Some(3), UNIT_NS[7]),
        Some(8) => (Some(6), UNIT_NS[8]),
        Some(9) => (Some(9), 1),
        Some(_) => return Err(vm.range_error("smallestUnit must be minute or smaller")),
        None => (digits, digits.map_or(1, |n| 10i128.pow(9 - n as u32))),
    };
    Ok(Precision {
        digits,
        increment,
        mode,
    })
}

/** GetTemporalFractionalSecondDigitsOption: None for auto. */
fn digits_option(vm: &mut Vm, o: Option<ObjectId>) -> Res<Option<i32>> {
    let Some(id) = o else { return Ok(None) };
    let value = vm.get(&JsValue::Object(id), &Key::from("fractionalSecondDigits"))?;
    match value {
        JsValue::Undefined => Ok(None),
        JsValue::Number(n) => {
            if !n.is_finite() || !(0.0..10.0).contains(&n.floor()) {
                return Err(vm.range_error("fractionalSecondDigits is out of range"));
            }
            Ok(Some(n.floor() as i32))
        }
        v => {
            let text = vm.to_text(&v)?;
            if text != "auto" {
                return Err(vm.range_error("fractionalSecondDigits must be auto or 0-9"));
            }
            Ok(None)
        }
    }
}

/** The `round` options of PlainTime and PlainDateTime: increment in ns and the mode. */
pub(super) fn round_to(
    vm: &mut Vm,
    value: &JsValue,
    lo: usize,
    day_ok: bool,
) -> Res<(i128, String)> {
    let o = match value {
        JsValue::Undefined => return Err(vm.type_error("round requires options")),
        JsValue::String(text) => {
            let id = vm.new_object()?;
            vm.set(
                &JsValue::Object(id),
                Key::from("smallestUnit"),
                JsValue::String(text.clone()),
                true,
            )?;
            Some(id)
        }
        v => options(vm, v)?,
    };
    let increment = increment_option(vm, o)?;
    let mode = mode_option(vm, o, "halfExpand")?;
    let smallest = unit_option(vm, o, "smallestUnit", false)?
        .ok_or_else(|| vm.range_error("smallestUnit is required"))?;
    if smallest < lo || (smallest == 3 && !day_ok) {
        return Err(vm.range_error("smallestUnit not allowed here"));
    }
    match max_increment(smallest) {
        Some(max) => check_increment(vm, increment, max, false)?,
        None => check_increment(vm, increment, 1, true)?,
    }
    Ok((increment * UNIT_NS[smallest], mode))
}

/** Install Temporal.Duration. */
pub(super) fn install(b: &mut Builder, ns: ObjectId, protos: &mut [ObjectId; 8]) {
    protos[Kind::Duration as usize] = class(
        b,
        ns,
        Kind::Duration,
        ctor,
        0,
        &[("from", 1, from), ("compare", 2, compare)],
        &[
            ("with", 1, with),
            ("negated", 0, negated),
            ("abs", 0, abs),
            ("add", 1, add),
            ("subtract", 1, subtract),
            ("toString", 0, to_string),
            ("toJSON", 0, to_json),
            ("toLocaleString", 0, to_json),
        ],
        &[
            ("years", g_years),
            ("months", g_months),
            ("weeks", g_weeks),
            ("days", g_days),
            ("hours", g_hours),
            ("minutes", g_minutes),
            ("seconds", g_seconds),
            ("milliseconds", g_ms),
            ("microseconds", g_us),
            ("nanoseconds", g_ns),
            ("sign", g_sign),
            ("blank", g_blank),
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_grammar() {
        assert_eq!(
            parse("P1Y2M3W4DT5H6M7.5S"),
            Some([1, 2, 3, 4, 5, 6, 7, 500, 0, 0])
        );
        assert_eq!(parse("-PT1.5H"), Some([0, 0, 0, 0, -1, -30, 0, 0, 0, 0]));
        assert!(parse("P").is_none());
        assert!(parse("PT").is_none());
        assert!(parse("P1DT").is_none());
        assert!(parse("PT1.5H2M").is_none());
    }

    #[test]
    fn formatting() {
        assert_eq!(format(&[0; 10], None), "PT0S");
        assert_eq!(format(&[1, 0, 0, 0, 0, 0, 0, 1500, 0, 0], None), "P1YT1.5S");
        assert_eq!(format(&[0, 0, 0, -1, 0, 0, 0, 0, 0, 0], None), "-P1D");
    }
}
