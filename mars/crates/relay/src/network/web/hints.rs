/**
 * High-entropy User-Agent client hints. A site asks for them with `Accept-CH` on a navigation
 * response, and Chrome then sends them on that origin's later HTTPS requests. Google asks for
 * arch, bitness, full-version-list, platform-version, model, wow64 and form-factors; Mars ignored
 * the header, so its search request differed from Chrome's in exactly the request Google answered
 * with its "unusual traffic" page (mars/docs/D1-fingerprint.md).
 */
use ::url::Url;
use mars_core::constants::{CHROME_FULL_VERSION, MACOS_PLATFORM_VERSION};
use std::sync::Mutex;

/** Origins whose preferences are kept; the oldest is forgotten past this. */
const ORIGINS: usize = 256;

/** The hints Mars answers, in the order Chrome sends them after the three low-entropy ones. */
const HINTS: [&str; 8] = [
    "sec-ch-ua-arch",
    "sec-ch-ua-bitness",
    "sec-ch-ua-form-factors",
    "sec-ch-ua-full-version",
    "sec-ch-ua-full-version-list",
    "sec-ch-ua-model",
    "sec-ch-ua-platform-version",
    "sec-ch-ua-wow64",
];

/** Each origin's accepted hints as bits over HINTS, oldest first. ponytail: a linear list of at
most ORIGINS entries; a map with an order index if this ever grows. Process-wide, like the
memory cache, so it lasts as long as the network service. */
static ACCEPTED: Mutex<Vec<(String, u8)>> = Mutex::new(Vec::new());

/** Records an origin's `Accept-CH`. Each header replaces the origin's earlier choice, as in
Chrome, and one naming none of HINTS clears it. */
pub(super) fn remember(url: &Url, accept_ch: &str) {
    let wanted = HINTS
        .iter()
        .enumerate()
        .filter(|(_, hint)| named(accept_ch, hint))
        .fold(0u8, |bits, (index, _)| bits | 1 << index);
    let origin = url.origin().ascii_serialization();
    let mut accepted = ACCEPTED.lock().unwrap_or_else(|poison| poison.into_inner());
    accepted.retain(|(known, _)| *known != origin);
    if wanted != 0 {
        accepted.push((origin, wanted));
    }
    if accepted.len() > ORIGINS {
        accepted.remove(0);
    }
}

/** Whether a comma-separated `Accept-CH` list names `hint`, ignoring case and spaces. */
fn named(list: &str, hint: &str) -> bool {
    list.split(',')
        .any(|token| token.trim().eq_ignore_ascii_case(hint))
}

/** The hint fields this request's origin asked for, in Chrome's order, with Chrome's values. */
pub(super) fn high_entropy(url: &Url) -> Vec<(&'static str, String)> {
    let origin = url.origin().ascii_serialization();
    let accepted = ACCEPTED.lock().unwrap_or_else(|poison| poison.into_inner());
    let Some(&(_, bits)) = accepted.iter().find(|(known, _)| *known == origin) else {
        return Vec::new();
    };
    HINTS
        .iter()
        .enumerate()
        .filter(|(index, _)| bits & 1 << index != 0)
        .map(|(_, hint)| (*hint, value(hint)))
        .collect()
}

/** Chrome 152 on a Mac: the structured-header value of one hint. */
fn value(hint: &str) -> String {
    let full = CHROME_FULL_VERSION;
    match hint {
        "sec-ch-ua-arch" => quoted(if cfg!(target_arch = "aarch64") {
            "arm"
        } else {
            "x86"
        }),
        "sec-ch-ua-bitness" => quoted("64"),
        "sec-ch-ua-form-factors" => quoted("Desktop"),
        "sec-ch-ua-full-version" => quoted(full),
        "sec-ch-ua-full-version-list" => {
            format!(r#""Not?A_Brand";v="24.0.0.0", "Chromium";v="{full}", "Mars";v="{full}""#)
        }
        "sec-ch-ua-model" => quoted(""),
        "sec-ch-ua-platform-version" => quoted(MACOS_PLATFORM_VERSION),
        _ => "?0".into(),
    }
}

/** A structured-header string. */
fn quoted(text: &str) -> String {
    format!("\"{text}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Hints go only to the origin that asked, in Chrome's order, and a new header replaces them. */
    #[test]
    fn an_origin_gets_the_hints_it_asked_for() {
        let asked = Url::parse("https://hints-a.test/search").unwrap();
        let other = Url::parse("https://hints-b.test/").unwrap();
        remember(
            &asked,
            "Sec-CH-UA-Platform-Version, sec-ch-ua-arch,sec-ch-ua-unknown",
        );
        let names: Vec<_> = high_entropy(&asked)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["sec-ch-ua-arch", "sec-ch-ua-platform-version"]);
        assert!(high_entropy(&other).is_empty());
        remember(&asked, "");
        assert!(high_entropy(&asked).is_empty());
    }

    /** The full version list carries the same brands, in the same order, as Sec-CH-UA. */
    #[test]
    fn the_full_version_list_matches_the_brands() {
        let list = value("sec-ch-ua-full-version-list");
        assert!(list.starts_with(r#""Not?A_Brand";v="24.0.0.0""#));
        assert!(list.contains(r#""Not?A_Brand";v="24.0.0.0""#));
        assert!(list.ends_with(&format!(r#""Mars";v="{CHROME_FULL_VERSION}""#)));
    }

    /** The full version's major is the major the User-Agent and Sec-CH-UA present. */
    #[test]
    fn the_full_version_matches_the_presented_major() {
        let major = CHROME_FULL_VERSION.split('.').next().unwrap();
        assert!(super::super::fields::CHROME_ENGINE.starts_with(&format!("Chrome/{major}.")));
    }
}
