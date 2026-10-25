/*! Temporal's ISO 8601 calendar arithmetic and its string grammar, as pure functions over integers. */

/** Nanoseconds per day. */
pub const NS_DAY: i128 = 86_400_000_000_000;
/** The largest epoch nanoseconds an Instant may hold (10^8 days). */
pub const NS_MAX: i128 = 8_640_000_000_000_000_000_000;

/** Whether a proleptic Gregorian year is a leap year. */
pub fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/** Days in an ISO month (1-12). */
pub fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

/** Days since 1970-01-01 for an ISO date (month may be out of range; it balances). */
pub fn epoch_days(year: i64, month: i64, day: i64) -> i64 {
    let year = year + (month - 1).div_euclid(12);
    let month = (month - 1).rem_euclid(12) + 1;
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/** The ISO date of a day count since the epoch. */
pub fn from_epoch_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/** ISO day of week, Monday = 1. */
pub fn day_of_week(y: i64, m: i64, d: i64) -> i64 {
    (epoch_days(y, m, d) + 3).rem_euclid(7) + 1
}

/** Day of the year, 1-based. */
pub fn day_of_year(y: i64, m: i64, d: i64) -> i64 {
    epoch_days(y, m, d) - epoch_days(y, 1, 1) + 1
}

/** ISO week number and its week-year. */
pub fn week_of_year(y: i64, m: i64, d: i64) -> (i64, i64) {
    let doy = day_of_year(y, m, d);
    let dow = day_of_week(y, m, d);
    let week = (doy - dow + 10) / 7;
    if week < 1 {
        let prev = y - 1;
        return (weeks_in_year(prev), prev);
    }
    if week > weeks_in_year(y) {
        return (1, y + 1);
    }
    (week, y)
}

/** 52 or 53 ISO weeks. */
fn weeks_in_year(y: i64) -> i64 {
    let jan1 = day_of_week(y, 1, 1);
    if jan1 == 4 || (jan1 == 3 && is_leap(y)) {
        53
    } else {
        52
    }
}

/** Whether y-m-d is a real ISO date. */
pub fn valid_date(y: i64, m: i64, d: i64) -> bool {
    (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}

/** Whether a clock time is in range. */
pub fn valid_time(t: &[i64; 6]) -> bool {
    (0..24).contains(&t[0])
        && (0..60).contains(&t[1])
        && (0..60).contains(&t[2])
        && t[3..].iter().all(|v| (0..1000).contains(v))
}

/** Epoch nanoseconds of an ISO date-time read as UTC. */
pub fn epoch_ns(date: (i64, i64, i64), time: &[i64; 6]) -> i128 {
    i128::from(epoch_days(date.0, date.1, date.2)) * NS_DAY + time_ns(time)
}

/** Nanoseconds since midnight. */
pub fn time_ns(t: &[i64; 6]) -> i128 {
    let units = [
        3_600_000_000_000i128,
        60_000_000_000,
        1_000_000_000,
        1_000_000,
        1000,
        1,
    ];
    t.iter().zip(units).map(|(v, u)| i128::from(*v) * u).sum()
}

/** A day offset and the clock time of nanoseconds (any sign) since a midnight. */
pub fn balance_time(ns: i128) -> (i64, [i64; 6]) {
    let days = ns.div_euclid(NS_DAY) as i64;
    let mut rest = ns.rem_euclid(NS_DAY) as i64;
    let mut out = [0i64; 6];
    let units = [
        3_600_000_000_000i64,
        60_000_000_000,
        1_000_000_000,
        1_000_000,
        1000,
        1,
    ];
    for (slot, unit) in out.iter_mut().zip(units) {
        *slot = rest / unit;
        rest %= unit;
    }
    (days, out)
}

/** Whether a date-time is within Temporal's representable range. */
pub fn datetime_in_limits(date: (i64, i64, i64), time: &[i64; 6]) -> bool {
    if date.0.abs() > 300_000 {
        return false;
    }
    let ns = epoch_ns(date, time);
    ns > -NS_MAX - NS_DAY && ns < NS_MAX + NS_DAY
}

/** ISODateWithinLimits: the date at noon. */
pub fn date_in_limits(date: (i64, i64, i64)) -> bool {
    datetime_in_limits(date, &[12, 0, 0, 0, 0, 0])
}

/** ISOYearMonthWithinLimits. */
pub fn year_month_in_limits(y: i64, m: i64) -> bool {
    (-271_821..=275_760).contains(&y) && !(y == -271_821 && m < 4) && !(y == 275_760 && m > 9)
}

/** Balance a year and an out-of-range month. */
pub fn balance_year_month(y: i64, m: i64) -> (i64, i64) {
    (y + (m - 1).div_euclid(12), (m - 1).rem_euclid(12) + 1)
}

/** RegulateISODate: constrain clamps, reject returns None. */
pub fn regulate_date(y: i64, m: i64, d: i64, constrain: bool) -> Option<(i64, i64, i64)> {
    if constrain {
        let m = m.clamp(1, 12);
        return Some((y, m, d.clamp(1, days_in_month(y, m))));
    }
    valid_date(y, m, d).then_some((y, m, d))
}

/** RegulateTime. */
pub fn regulate_time(t: [i64; 6], constrain: bool) -> Option<[i64; 6]> {
    if !constrain {
        return valid_time(&t).then_some(t);
    }
    let max = [23, 59, 59, 999, 999, 999];
    let mut out = t;
    for (v, m) in out.iter_mut().zip(max) {
        *v = (*v).clamp(0, m);
    }
    Some(out)
}

/** AddISODate over years, months, weeks and days. */
pub fn add_date(date: (i64, i64, i64), d: [i64; 4], constrain: bool) -> Option<(i64, i64, i64)> {
    let (y, m) = balance_year_month(date.0 + d[0], date.1 + d[1]);
    let (y, m, day) = regulate_date(y, m, date.2, constrain)?;
    Some(from_epoch_days(epoch_days(y, m, day) + d[2] * 7 + d[3]))
}

/** Lexicographic date comparison. */
pub fn compare_date(a: (i64, i64, i64), b: (i64, i64, i64)) -> i64 {
    a.cmp(&b) as i64
}

/** CalendarDateUntil for ISO: years, months, weeks, days from one to two. */
pub fn date_until(one: (i64, i64, i64), two: (i64, i64, i64), largest: &str) -> [i64; 4] {
    let sign = -compare_date(one, two);
    if sign == 0 {
        return [0; 4];
    }
    let surpasses = |years: i64, months: i64| {
        let (y, m) = balance_year_month(one.0 + years, one.1 + months);
        sign * compare_date((y, m, one.2), two) > 0
    };
    let (mut years, mut months) = (0, 0);
    if largest == "year" {
        years = two.0 - one.0;
        while years != 0 && surpasses(years, 0) {
            years -= sign;
        }
    }
    if largest == "year" || largest == "month" {
        months = if largest == "month" {
            (two.0 - one.0) * 12 + two.1 - one.1
        } else {
            two.1 - one.1 + (two.0 - one.0 - years) * 12
        };
        while months != 0 && surpasses(years, months) {
            months -= sign;
        }
    }
    let (y, m) = balance_year_month(one.0 + years, one.1 + months);
    let (y, m, d) = regulate_date(y, m, one.2, true).unwrap_or(one);
    let mut days = epoch_days(two.0, two.1, two.2) - epoch_days(y, m, d);
    let mut weeks = 0;
    if largest == "week" {
        weeks = days / 7;
        days %= 7;
    }
    [years, months, weeks, days]
}

/** A parsed ISO 8601 string, before any type-specific checks. */
#[derive(Default, Debug, Clone)]
pub struct Parsed {
    pub date: Option<(i64, i64, i64)>,
    pub time: Option<[i64; 6]>,
    pub z: bool,
    /** The UTC offset in nanoseconds, and whether it had sub-minute precision. */
    pub offset: Option<(i128, bool)>,
    pub tz: Option<String>,
    pub calendar: Option<String>,
}

/** A byte cursor over the string. */
struct Cur<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cur<'_> {
    /** The next byte. */
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    /** Consume one byte if it is in `set`. */
    fn eat(&mut self, set: &[u8]) -> Option<u8> {
        let c = self.peek().filter(|c| set.contains(c))?;
        self.i += 1;
        Some(c)
    }
    /** Exactly n digits. */
    fn digits(&mut self, n: usize) -> Option<i64> {
        let part = self.s.get(self.i..self.i + n)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        self.i += n;
        Some(part.iter().fold(0, |a, c| a * 10 + i64::from(c - b'0')))
    }
    /** At the end. */
    fn done(&self) -> bool {
        self.i == self.s.len()
    }
}

/** DateYear: four digits or a sign and six. */
fn year(c: &mut Cur) -> Option<i64> {
    if let Some(sign) = c.eat(b"+-") {
        let v = c.digits(6)?;
        if sign == b'-' && v == 0 {
            return None;
        }
        return Some(if sign == b'-' { -v } else { v });
    }
    c.digits(4)
}

/** Fraction digits after a separator, as nanoseconds. */
fn fraction(c: &mut Cur) -> Option<Option<i64>> {
    if c.eat(b".,").is_none() {
        return Some(None);
    }
    let start = c.i;
    while c.peek().is_some_and(|b| b.is_ascii_digit()) {
        c.i += 1;
    }
    let n = c.i - start;
    if n == 0 || n > 9 {
        return None;
    }
    let mut v: i64 = c.s[start..c.i]
        .iter()
        .fold(0, |a, d| a * 10 + i64::from(d - b'0'));
    for _ in n..9 {
        v *= 10;
    }
    Some(Some(v))
}

/** HH[:MM[:SS[.f]]] or the basic form; returns the time. */
fn time(c: &mut Cur) -> Option<[i64; 6]> {
    let h = c.digits(2)?;
    let mut t = [h, 0, 0, 0, 0, 0];
    let ext = c.peek() == Some(b':');
    let sep = |c: &mut Cur| {
        if ext {
            c.eat(b":").is_some()
        } else {
            c.peek().is_some_and(|b| b.is_ascii_digit())
        }
    };
    if sep(c) {
        t[1] = c.digits(2)?;
        if sep(c) {
            t[2] = c.digits(2)?;
            if let Some(f) = fraction(c)? {
                t[3] = f / 1_000_000;
                t[4] = f / 1000 % 1000;
                t[5] = f % 1000;
            }
        }
    }
    if t[2] == 60 {
        t[2] = 59;
    }
    valid_time(&t).then_some(t)
}

/** A UTC offset: sign, hours, optional minutes, seconds and fraction. */
fn offset(c: &mut Cur) -> Option<(i128, bool)> {
    let sign = if c.eat(b"+-")? == b'-' { -1 } else { 1 };
    let h = c.digits(2)?;
    if h > 23 {
        return None;
    }
    let ext = c.peek() == Some(b':');
    let sep = |c: &mut Cur| {
        if ext {
            c.eat(b":").is_some()
        } else {
            c.peek().is_some_and(|b| b.is_ascii_digit())
        }
    };
    let (mut m, mut s, mut f, mut sub) = (0, 0, 0, false);
    if sep(c) {
        m = c.digits(2)?;
        if sep(c) {
            s = c.digits(2)?;
            f = fraction(c)?.unwrap_or(0);
            sub = true;
        }
    }
    if m > 59 || s > 59 {
        return None;
    }
    let ns =
        ((i128::from(h) * 60 + i128::from(m)) * 60 + i128::from(s)) * 1_000_000_000 + i128::from(f);
    Some((sign * ns, sub))
}

/** Bracketed annotations: an optional time zone first, then key=value pairs. */
fn annotations(c: &mut Cur, p: &mut Parsed) -> Option<()> {
    let mut first = true;
    let mut calendars = 0;
    let mut critical_cal = false;
    while c.eat(b"[").is_some() {
        let critical = c.eat(b"!").is_some();
        let start = c.i;
        while c.peek().is_some_and(|b| b != b']') {
            c.i += 1;
        }
        let body = std::str::from_utf8(&c.s[start..c.i]).ok()?;
        c.eat(b"]")?;
        if let Some((key, value)) = body.split_once('=') {
            let key_ok = key
                .bytes()
                .next()
                .is_some_and(|b| b.is_ascii_lowercase() || b == b'_')
                && key.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'
                });
            let value_ok = !value.is_empty()
                && value.split('-').all(|v| {
                    (3..=8).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_alphanumeric())
                });
            if !key_ok || !value_ok {
                return None;
            }
            if key == "u-ca" {
                calendars += 1;
                critical_cal |= critical;
                if p.calendar.is_none() {
                    p.calendar = Some(value.to_string());
                }
            } else if critical {
                return None;
            }
        } else {
            if !first || body.is_empty() || !tz_name(body) {
                return None;
            }
            p.tz = Some(body.to_string());
        }
        first = false;
    }
    if calendars > 1 && critical_cal {
        return None;
    }
    Some(())
}

/** A plausible time zone annotation: an IANA-style name or an offset. */
fn tz_name(s: &str) -> bool {
    if s.starts_with(['+', '-']) {
        let mut c = Cur {
            s: s.as_bytes(),
            i: 0,
        };
        return offset(&mut c).is_some_and(|(_, sub)| !sub) && c.done();
    }
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"_-+/.".contains(&b))
        && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/** Offset or Z after a time, then annotations, then the end. */
fn tail(c: &mut Cur, p: &mut Parsed) -> Option<()> {
    if c.eat(b"Zz").is_some() {
        p.z = true;
    } else if matches!(c.peek(), Some(b'+' | b'-')) {
        p.offset = Some(offset(c)?);
    }
    annotations(c, p)?;
    c.done().then_some(())
}

/** A full date, optionally with a time, offset and annotations. */
pub fn parse_datetime(s: &str) -> Option<Parsed> {
    let mut c = Cur {
        s: s.as_bytes(),
        i: 0,
    };
    let mut p = Parsed::default();
    let y = year(&mut c)?;
    let ext = c.eat(b"-").is_some();
    let m = c.digits(2)?;
    if ext {
        c.eat(b"-")?;
    }
    let d = c.digits(2)?;
    if !valid_date(y, m, d) {
        return None;
    }
    p.date = Some((y, m, d));
    if c.eat(b"Tt ").is_some() {
        p.time = Some(time(&mut c)?);
        tail(&mut c, &mut p)?;
    } else {
        annotations(&mut c, &mut p)?;
        c.done().then_some(())?;
    }
    Some(p)
}

/** A time alone, `T`-prefixed or not, with an optional offset and annotations. */
pub fn parse_time_only(s: &str) -> Option<Parsed> {
    let mut c = Cur {
        s: s.as_bytes(),
        i: 0,
    };
    let designator = c.eat(b"Tt").is_some();
    let mut p = Parsed {
        time: Some(time(&mut c)?),
        ..Parsed::default()
    };
    tail(&mut c, &mut p)?;
    if !designator && (parse_year_month(s).is_some() || parse_month_day(s).is_some()) {
        return None;
    }
    Some(p)
}

/** YYYY-MM or YYYYMM with annotations. */
pub fn parse_year_month(s: &str) -> Option<Parsed> {
    let mut c = Cur {
        s: s.as_bytes(),
        i: 0,
    };
    let y = year(&mut c)?;
    c.eat(b"-");
    let m = c.digits(2)?;
    if !(1..=12).contains(&m) {
        return None;
    }
    let mut p = Parsed {
        date: Some((y, m, 1)),
        ..Parsed::default()
    };
    annotations(&mut c, &mut p)?;
    c.done().then_some(p)
}

/** --MM-DD, MM-DD or MMDD with annotations. */
pub fn parse_month_day(s: &str) -> Option<Parsed> {
    let mut c = Cur {
        s: s.as_bytes(),
        i: 0,
    };
    if c.eat(b"-").is_some() {
        c.eat(b"-")?;
    }
    let m = c.digits(2)?;
    c.eat(b"-");
    let d = c.digits(2)?;
    if !valid_date(1972, m, d) {
        return None;
    }
    let mut p = Parsed {
        date: Some((1972, m, d)),
        ..Parsed::default()
    };
    annotations(&mut c, &mut p)?;
    c.done().then_some(p)
}

/** A year as Temporal prints it: four digits, or a sign and six. */
pub fn format_year(y: i64) -> String {
    if (0..=9999).contains(&y) {
        format!("{y:04}")
    } else {
        format!("{}{:06}", if y < 0 { '-' } else { '+' }, y.abs())
    }
}

/** HH:MM[:SS[.fraction]] for a precision: None = auto, Some(-1) = minute, Some(n) digits. */
pub fn format_time(t: &[i64; 6], precision: Option<i32>) -> String {
    let mut out = format!("{:02}:{:02}", t[0], t[1]);
    if precision == Some(-1) {
        return out;
    }
    out.push_str(&format!(":{:02}", t[2]));
    let frac = format!("{:09}", t[3] * 1_000_000 + t[4] * 1000 + t[5]);
    let frac = match precision {
        None => frac.trim_end_matches('0').to_string(),
        Some(n) => frac[..n as usize].to_string(),
    };
    if !frac.is_empty() {
        out.push('.');
        out.push_str(&frac);
    }
    out
}

/** ±HH:MM for an offset in nanoseconds rounded to minutes. */
pub fn format_offset(ns: i128) -> String {
    let sign = if ns < 0 { '-' } else { '+' };
    let minutes = (ns.abs() + 30_000_000_000) / 60_000_000_000;
    format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

/** Round an integer to a multiple of `inc` with a Temporal rounding mode. */
pub fn round_by(x: i128, inc: i128, mode: &str) -> i128 {
    let q = x.div_euclid(inc);
    let r = x.rem_euclid(inc);
    if r == 0 {
        return x;
    }
    let (floor, ceil) = (q * inc, (q + 1) * inc);
    let neg = x < 0;
    let half = (r * 2).cmp(&inc);
    let up = match mode {
        "ceil" => true,
        "floor" => false,
        "expand" => !neg,
        "trunc" => neg,
        "halfCeil" => half.is_ge(),
        "halfFloor" => half.is_gt(),
        "halfExpand" => {
            if neg {
                half.is_gt()
            } else {
                half.is_ge()
            }
        }
        "halfTrunc" => {
            if neg {
                half.is_ge()
            } else {
                half.is_gt()
            }
        }
        _ => half.is_gt() || (half.is_eq() && q % 2 != 0),
    };
    if up { ceil } else { floor }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_days_round_trip() {
        for days in [
            -1_000_000i64,
            -719_468,
            -1,
            0,
            1,
            59,
            365,
            11_016,
            2_932_896,
        ] {
            let (y, m, d) = from_epoch_days(days);
            assert_eq!(epoch_days(y, m, d), days);
        }
        assert_eq!(epoch_days(1970, 1, 1), 0);
        assert_eq!(from_epoch_days(19_000), (2022, 1, 8));
    }

    #[test]
    fn iso_weeks() {
        assert_eq!(week_of_year(2021, 1, 1), (53, 2020));
        assert_eq!(week_of_year(2019, 12, 30), (1, 2020));
        assert_eq!(day_of_week(1970, 1, 1), 4);
    }

    #[test]
    fn date_until_counts_months_then_days() {
        assert_eq!(
            date_until((2020, 1, 31), (2020, 3, 1), "month"),
            [0, 1, 0, 1]
        );
        assert_eq!(date_until((2019, 1, 1), (2021, 2, 3), "year"), [2, 1, 0, 2]);
        assert_eq!(
            date_until((2021, 2, 3), (2019, 1, 1), "year"),
            [-2, -1, 0, -2]
        );
        assert_eq!(
            date_until((2020, 1, 1), (2020, 1, 16), "week"),
            [0, 0, 2, 1]
        );
    }

    #[test]
    fn grammar() {
        let p = parse_datetime("2020-01-02T03:04:05.123456789+01:00[Europe/Paris][u-ca=iso8601]")
            .unwrap();
        assert_eq!(p.date, Some((2020, 1, 2)));
        assert_eq!(p.time, Some([3, 4, 5, 123, 456, 789]));
        assert_eq!(p.calendar.as_deref(), Some("iso8601"));
        assert!(parse_datetime("-000000-01-01").is_none());
        assert!(parse_datetime("2020-0102").is_none());
        assert!(parse_datetime("2020-01-01[!foo=bar]").is_none());
        assert!(parse_time_only("2021-12").is_none());
        assert!(parse_time_only("T2021-12").is_some());
        assert!(parse_time_only("12:30").is_some());
        assert!(parse_month_day("--12-25").is_some());
    }

    #[test]
    fn rounding_modes() {
        assert_eq!(round_by(15, 10, "halfEven"), 20);
        assert_eq!(round_by(25, 10, "halfEven"), 20);
        assert_eq!(round_by(-15, 10, "halfExpand"), -20);
        assert_eq!(round_by(-15, 10, "trunc"), -10);
        assert_eq!(format_time(&[1, 2, 3, 400, 0, 0], None), "01:02:03.4");
    }
}
