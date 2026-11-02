/*! Pure address presentation: what the field shows at rest and which security glyph sits beside it. */

/** A lock for HTTPS, an info mark for plain HTTP, and a magnifier before any page has loaded. */
pub fn security_symbol(url: &str) -> &'static str {
    if url.starts_with("https://") {
        "lock.fill"
    } else if url.starts_with("http://") {
        "info.circle"
    } else {
        "magnifyingglass"
    }
}
/** At rest the field drops the scheme and a bare trailing slash, as Oya and Safari do. */
pub fn display_address(url: &str) -> String {
    let short = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let short = short.strip_prefix("www.").unwrap_or(short);
    match short.strip_suffix('/') {
        Some(host) if !host.contains('/') => host.to_owned(),
        _ => short.to_owned(),
    }
}
