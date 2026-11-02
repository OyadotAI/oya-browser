/** Desktop destination policy validates resolved addresses before exact socket connection. */
use crate::protocol::{EngineError, EngineResult};
use ::url::Url;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Parse once without accepting credentials, control injection or alternate protocols.
pub fn web_url(value: &str) -> EngineResult<Url> {
    if value.len() > crate::constants::URL_BYTES {
        return Err(EngineError::limit("URL bytes", crate::constants::URL_BYTES));
    }
    // Before the whitespace check: a serialized `data:` URL keeps its media type's spaces.
    if value
        .get(..DATA_SCHEME.len())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case(DATA_SCHEME))
    {
        return Err(inline_data());
    }
    if value.chars().any(|c| c.is_control() || c.is_whitespace()) || value.contains('\\') {
        return Err(denied());
    }
    let url = Url::parse(value).map_err(|_| denied())?;
    let authority = value
        .split_once("://")
        .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or_default())
        .unwrap_or_default();
    if !["http", "https"].contains(&url.scheme())
        || authority.contains('@')
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.port() == Some(0)
    {
        return Err(denied());
    }
    Ok(url)
}

/// Only the exact operator-owned fixture origin can reach loopback: a literal loopback address,
/// or a localhost name, which always means loopback (`local_hostname`).
pub fn fixture_granted(url: &Url, origins: &[String]) -> bool {
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    (host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()) || local_hostname(host))
        && origins
            .iter()
            .any(|origin| *origin == url.origin().ascii_serialization())
}

/// Chromium's `net::IsLocalHostname`: `localhost` and every `*.localhost` name, with or without
/// a trailing dot, case-insensitively. Chromium's host resolver answers these with loopback
/// itself and never asks DNS (RFC 6761 section 6.3), so no resolver can point them elsewhere,
/// and WPT's `www1.localhost` style subdomains reach the local test server.
pub fn local_hostname(host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    host == "localhost" || host.ends_with(".localhost")
}

/// Reject nonpublic, reserved and transition ranges, including IPv4-mapped forms.
pub fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(public_v4)
            .unwrap_or_else(|| public_v6(ip)),
    }
}

/** A conservative public IPv4 allow policy excludes special-use blocks. */
fn public_v4(ip: Ipv4Addr) -> bool {
    let value = u32::from(ip);
    let blocked = [
        (0x00000000, 8),
        (0x0a000000, 8),
        (0x64400000, 10),
        (0x7f000000, 8),
        (0xa9fe0000, 16),
        (0xac100000, 12),
        (0xc0000000, 24),
        (0xc0000200, 24),
        (0xc0586300, 24),
        (0xc0a80000, 16),
        (0xc6120000, 15),
        (0xc6336400, 24),
        (0xcb007100, 24),
        (0xe0000000, 3),
    ];
    !blocked
        .into_iter()
        .any(|(network, bits)| value >> (u32::BITS - bits) == network >> (u32::BITS - bits))
}

/** Only native global-unicast IPv6 is considered, excluding special 2001/documentation blocks. */
fn public_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    (segments[0] & 0xe000) == 0x2000
        && !((segments[0] == 0x2001 && (segments[1] < 0x0200 || segments[1] == 0x0db8))
            || segments[0] == 0x2002
            || segments[0] == 0x3ffe
            || (segments[0] == 0x3fff && (segments[1] & 0xf000) == 0))
}

/** The scheme prefix of an inline-content URL. */
const DATA_SCHEME: &str = "data:";

/** A `data:` URL names no destination at all: its bytes are inline. Meta's pages link
stylesheets and scripts that way. Refusing it as an egress denial made the page loader treat it
as an authority failure and abandon the whole navigation (facebook.com, instagram.com), so it is
refused as unsupported instead: still never fetched, but only that one resource is skipped. */
fn inline_data() -> EngineError {
    EngineError::unsupported("data: URLs carry their content inline and are not fetched.")
}

/// Egress denials preserve the current document and never trigger an alternate transport.
pub fn denied() -> EngineError {
    EngineError::new(
        "permission_denied",
        "Navigation blocked: only public HTTP/HTTPS destinations and the exact owned demo origin are allowed.",
        "Use a supported public URL or Open demo.",
    )
}
