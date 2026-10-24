/** Hermetic checks pin what the native address field shows at rest and which security glyph it uses. */
#[path = "../src/desktop/mars/address.rs"]
mod address;

use address::{display_address, security_symbol};

/// A site's root shows as its bare host, without scheme, www or trailing slash.
#[test]
fn root_address_shows_bare_host() {
    assert_eq!(display_address("https://www.example.com/"), "example.com");
    assert_eq!(
        display_address("http://127.0.0.1:60603/"),
        "127.0.0.1:60603"
    );
}

/// A deeper path keeps everything after the host so the person still sees where they are.
#[test]
fn path_address_keeps_path() {
    assert_eq!(
        display_address("https://example.com/docs/"),
        "example.com/docs/"
    );
    assert_eq!(
        display_address("https://example.com/a?b=1"),
        "example.com/a?b=1"
    );
}

/// Anything that is not HTTP(S) is shown exactly as committed.
#[test]
fn other_text_is_unchanged() {
    assert_eq!(display_address(""), "");
    assert_eq!(display_address("about:blank"), "about:blank");
}

/// HTTPS gets the lock, plain HTTP the info mark, and no page the magnifier.
#[test]
fn security_glyph_follows_scheme() {
    assert_eq!(security_symbol("https://example.com/"), "lock.fill");
    assert_eq!(security_symbol("http://example.com/"), "info.circle");
    assert_eq!(security_symbol(""), "magnifyingglass");
}
