/** Intl time-zone names and offsets reuse the embedded IANA transition rules. */
use crate::v9::builtins::temporal::zone;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/** ICU 78.3's preferred names over the embedded IANA identifiers. */
const NAMES: &str = include_str!("zone_names.txt");
/** Static data is parsed once and never accepts page-supplied entries. */
static INDEX: OnceLock<BTreeMap<&'static str, &'static str>> = OnceLock::new();
/** Milliseconds in one minute. */
const MINUTE_MS: i32 = 60_000;
/** Nanoseconds in one millisecond. */
const NANOS_PER_MS: f64 = 1_000_000.0;
/** Milliseconds in one second. */
const SECOND_MS: f64 = 1_000.0;
/** Seconds per minute. */
const SECONDS_PER_MINUTE: i32 = 60;
/** Seconds per hour. */
const SECONDS_PER_HOUR: i32 = 3_600;

/** Read the checked-in name pairs without exposing storage to page scripts. */
fn names() -> &'static BTreeMap<&'static str, &'static str> {
    INDEX.get_or_init(|| {
        NAMES
            .lines()
            .filter(|s| !s.starts_with('#'))
            .filter_map(|s| s.split_once('\t'))
            .collect()
    })
}

/** Parse the ECMA-402 offset forms: ±HH, ±HHMM, or ±HH:MM. */
fn offset_minutes(name: &str) -> Option<i32> {
    if !name.is_ascii() {
        return None;
    }
    let sign = match name.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &name[1..];
    let (hour, minute) = match digits.len() {
        2 => (digits, "00"),
        4 => (&digits[..2], &digits[2..]),
        5 if digits.as_bytes().get(2) == Some(&b':') => (&digits[..2], &digits[3..]),
        _ => return None,
    };
    if !hour
        .bytes()
        .chain(minute.bytes())
        .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let (hour, minute) = (hour.parse::<i32>().ok()?, minute.parse::<i32>().ok()?);
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(sign * (hour * 60 + minute))
}

/** Keep a canonical fixed offset so resolution and later formatting agree. */
fn offset_name(minutes: i32) -> String {
    format!(
        "{}{:02}:{:02}",
        if minutes < 0 { '-' } else { '+' },
        minutes.abs() / 60,
        minutes.abs() % 60
    )
}

/** Resolve valid ICU names case-insensitively, or normalize an explicit offset. */
pub(super) fn canonical(name: &str) -> Option<String> {
    if name.starts_with(['+', '-']) {
        return offset_minutes(name).map(offset_name);
    }
    if ["utc", "gmt", "etc/utc", "etc/gmt"].contains(&name.to_ascii_lowercase().as_str()) {
        return Some("UTC".into());
    }
    names()
        .get(name.to_ascii_lowercase().as_str())
        .map(|name| (*name).into())
}

/** Resolve the actual offset at the formatted instant; unknown restored state fails closed. */
pub(super) fn offset_ms(name: &str, time: f64) -> Option<f64> {
    if name == "UTC" {
        return Some(0.0);
    }
    if let Some(minutes) = offset_minutes(name) {
        return Some(f64::from(minutes * MINUTE_MS));
    }
    let instant = (time * NANOS_PER_MS) as i128;
    Some(zone::lookup(name)?.offset_ns(instant) as f64 / NANOS_PER_MS)
}

/** Keep historic seconds in GMT labels rather than reporting a different offset. */
pub(super) fn label(offset_ms: f64) -> String {
    let seconds = (offset_ms / SECOND_MS) as i32;
    if seconds == 0 {
        return "UTC".into();
    }
    let magnitude = seconds.abs();
    let mut text = format!(
        "GMT{}{:02}:{:02}",
        if seconds < 0 { '-' } else { '+' },
        magnitude / SECONDS_PER_HOUR,
        magnitude % SECONDS_PER_HOUR / SECONDS_PER_MINUTE
    );
    if magnitude % SECONDS_PER_MINUTE != 0 {
        text.push_str(&format!(":{:02}", magnitude % SECONDS_PER_MINUTE));
    }
    text
}
