/** RFC 9111 freshness for a private cache: Cache-Control, Expires, Age and the 10% heuristic. */
use crate::network::constants::HEURISTIC_DIVISOR;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/** What a response's headers say about storing and reusing it. */
pub(super) struct Policy {
    /// `no-store`: never written to memory or disk.
    pub no_store: bool,
    /// `no-cache` (with or without a field list): stored, but revalidated before every use.
    pub no_cache: bool,
    /// Seconds the response stays fresh from its origin's point of view.
    pub lifetime: u64,
    /// Seconds old the response already was when it arrived (section 4.2.3).
    pub initial_age: u64,
}

/**
 * Reads the freshness rules from response headers received at `now`. This is a private cache,
 * so `private` responses are stored and `s-maxage`, a shared-cache directive, is ignored.
 * An invalid `max-age` or `Expires` means already stale, never fresh.
 */
pub(super) fn policy(headers: &BTreeMap<String, String>, now: SystemTime) -> Policy {
    let directives = directives(headers.get("cache-control").map_or("", String::as_str));
    let has = |name: &str| directives.iter().any(|(key, _)| key == name);
    let date = header_time(headers, "date")
        .filter(|date| *date <= now)
        .unwrap_or(now);
    let lifetime = if let Some((_, value)) = directives.iter().find(|(key, _)| key == "max-age") {
        value.parse().unwrap_or(0)
    } else if headers.contains_key("expires") {
        header_time(headers, "expires")
            .and_then(|expires| expires.duration_since(date).ok())
            .map_or(0, |duration| duration.as_secs())
    } else {
        header_time(headers, "last-modified")
            .and_then(|modified| date.duration_since(modified).ok())
            .map_or(0, |since| (since / HEURISTIC_DIVISOR).as_secs())
    };
    let apparent = now.duration_since(date).map_or(0, |age| age.as_secs());
    let age = headers
        .get("age")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    Policy {
        no_store: has("no-store"),
        no_cache: has("no-cache"),
        lifetime,
        initial_age: apparent.max(age),
    }
}

/** Whole seconds since the Unix epoch; the clock before 1970 counts as zero. */
pub(super) fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/**
 * Splits Cache-Control into lowercase names and unquoted values. A quoted list containing
 * commas (`no-cache="a, b"`) splits into extra unknown tokens, which are ignored, while the
 * leading directive still applies.
 */
fn directives(value: &str) -> Vec<(String, String)> {
    value
        .split(',')
        .filter_map(|part| {
            let (name, value) = part.split_once('=').unwrap_or((part, ""));
            let name = name.trim().to_ascii_lowercase();
            (!name.is_empty()).then(|| (name, value.trim().trim_matches('"').to_owned()))
        })
        .collect()
}

/** An HTTP-date header, or none when absent or malformed. */
fn header_time(headers: &BTreeMap<String, String>, name: &str) -> Option<SystemTime> {
    httpdate::parse_http_date(headers.get(name)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /** Builds headers from name/value pairs. */
    fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    /** A fixed receipt time so every rule is checked without the wall clock. */
    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }

    /** max-age wins over Expires, and s-maxage alone gives a private cache no lifetime. */
    #[test]
    fn max_age_wins_and_s_maxage_is_ignored() {
        let date = httpdate::fmt_http_date(now());
        let later = httpdate::fmt_http_date(now() + Duration::from_secs(900));
        let both = headers(&[
            ("cache-control", "private, max-age=60"),
            ("expires", &later),
            ("date", &date),
        ]);
        assert_eq!(policy(&both, now()).lifetime, 60);
        let shared = headers(&[("cache-control", "s-maxage=600")]);
        assert_eq!(policy(&shared, now()).lifetime, 0);
        let invalid = headers(&[("cache-control", "max-age=soon")]);
        assert_eq!(policy(&invalid, now()).lifetime, 0);
    }

    /** Expires counts from Date; a malformed Expires is already stale. */
    #[test]
    fn expires_counts_from_date_and_malformed_is_stale() {
        let date = httpdate::fmt_http_date(now());
        let later = httpdate::fmt_http_date(now() + Duration::from_secs(300));
        assert_eq!(
            policy(&headers(&[("expires", &later), ("date", &date)]), now()).lifetime,
            300
        );
        assert_eq!(
            policy(&headers(&[("expires", "0"), ("date", &date)]), now()).lifetime,
            0
        );
    }

    /** Heuristic freshness is a tenth of the time since Last-Modified. */
    #[test]
    fn heuristic_freshness_is_ten_percent_of_last_modified_age() {
        let date = httpdate::fmt_http_date(now());
        let modified = httpdate::fmt_http_date(now() - Duration::from_secs(1_000));
        let policy = policy(
            &headers(&[("last-modified", &modified), ("date", &date)]),
            now(),
        );
        assert_eq!(policy.lifetime, 100);
    }

    /** Age and an old Date both count toward the initial age; the larger wins. */
    #[test]
    fn initial_age_takes_the_larger_of_age_and_apparent_age() {
        let date = httpdate::fmt_http_date(now() - Duration::from_secs(40));
        assert_eq!(
            policy(&headers(&[("date", &date), ("age", "10")]), now()).initial_age,
            40
        );
        assert_eq!(policy(&headers(&[("age", "70")]), now()).initial_age, 70);
    }

    /** no-store and no-cache are recognized in any case and with field lists. */
    #[test]
    fn no_store_and_no_cache_are_recognized() {
        let policy = policy(
            &headers(&[("cache-control", "No-Store, no-cache=\"set-cookie, x\"")]),
            now(),
        );
        assert!(policy.no_store && policy.no_cache);
    }
}
