/** Canvas solid colors preserve alpha across assignments, drawing and serialized getters. */
const OPAQUE: u8 = u8::MAX;
const RGB_CHANNELS: usize = 3;
const RGBA_CHANNELS: usize = 4;
const HEX_SHORT: usize = 3;
const HEX_SHORT_ALPHA: usize = 4;
const HEX_LONG: usize = 6;
const HEX_LONG_ALPHA: usize = 8;

/** Invalid syntax returns no color so an assignment preserves the previous drawing state. */
pub(super) fn parse(text: &str) -> Option<[u8; RGBA_CHANNELS]> {
    let text = text.trim().to_ascii_lowercase();
    match text.as_str() {
        "black" => Some([0, 0, 0, OPAQUE]),
        "white" => Some([OPAQUE, OPAQUE, OPAQUE, OPAQUE]),
        "red" => Some([OPAQUE, 0, 0, OPAQUE]),
        "green" => Some([0, 128, 0, OPAQUE]),
        "blue" => Some([0, 0, OPAQUE, OPAQUE]),
        "transparent" => Some([0, 0, 0, 0]),
        _ if text.starts_with('#') => hex(&text[1..]),
        _ => functional(&text),
    }
}

/** Expand shorthand before decoding, admitting only ASCII hexadecimal digits. */
fn hex(text: &str) -> Option<[u8; RGBA_CHANNELS]> {
    if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let text = match text.len() {
        HEX_SHORT | HEX_SHORT_ALPHA => text.chars().flat_map(|c| [c, c]).collect::<String>(),
        HEX_LONG | HEX_LONG_ALPHA => text.to_string(),
        _ => return None,
    };
    let mut color = [OPAQUE; RGBA_CHANNELS];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        color[index] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(color)
}

/** Both legacy comma forms and modern space/slash forms share the same component conversions. */
fn functional(text: &str) -> Option<[u8; RGBA_CHANNELS]> {
    let body = text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let (channels, alpha) = if body.contains(',') {
        commas(body)?
    } else {
        spaces(body)?
    };
    let percent = channels[0].ends_with('%');
    if body.contains(',') && channels.iter().any(|v| v.ends_with('%') != percent) {
        return None;
    }
    let alpha = match alpha {
        Some(value) => alpha_component(value)?,
        None => OPAQUE,
    };
    Some([
        component(channels[0])?,
        component(channels[1])?,
        component(channels[2])?,
        alpha,
    ])
}

/** Legacy syntax never accepts a slash or embedded component whitespace. */
fn commas(body: &str) -> Option<([&str; RGB_CHANNELS], Option<&str>)> {
    if body.contains('/') {
        return None;
    }
    let parts: Vec<_> = body.split(',').map(str::trim).collect();
    if !(RGB_CHANNELS..=RGBA_CHANNELS).contains(&parts.len()) {
        return None;
    }
    Some((
        [parts[0], parts[1], parts[2]],
        parts.get(RGB_CHANNELS).copied(),
    ))
}

/** Modern syntax separates optional alpha from exactly three color components. */
fn spaces(body: &str) -> Option<([&str; RGB_CHANNELS], Option<&str>)> {
    let mut split = body.split('/');
    let parts: Vec<_> = split.next()?.split_ascii_whitespace().collect();
    let alpha = split.next().map(str::trim);
    if split.next().is_some() || parts.len() != RGB_CHANNELS {
        return None;
    }
    Some(([parts[0], parts[1], parts[2]], alpha))
}

/** Finite CSS numbers are clamped after percentage conversion, not wrapped as integer channels. */
fn component(text: &str) -> Option<u8> {
    number(text, f64::from(OPAQUE)).map(|n| n.clamp(0.0, f64::from(OPAQUE)).round() as u8)
}

/** Alpha accepts fractions or percentages and maps them to the canonical stored byte. */
fn alpha_component(text: &str) -> Option<u8> {
    number(text, 1.0).map(|n| (n.clamp(0.0, 1.0) * f64::from(OPAQUE)).round() as u8)
}

/** Reject infinities and NaN rather than allowing parser extensions into CSS values. */
fn number(text: &str, percent_scale: f64) -> Option<f64> {
    let percent = text.ends_with('%');
    let text = text.strip_suffix('%').unwrap_or(text);
    let value = text.parse::<f64>().ok()?;
    value.is_finite().then_some(if percent {
        value * percent_scale / 100.0
    } else {
        value
    })
}

/** Three fractional decimal places round-trip every stored alpha byte. */
pub(super) fn serialize([r, g, b, a]: [u8; RGBA_CHANNELS]) -> String {
    if a == OPAQUE {
        return format!("#{r:02x}{g:02x}{b:02x}");
    }
    let alpha = format!("{:.3}", f64::from(a) / f64::from(OPAQUE));
    let alpha = alpha.trim_end_matches('0').trim_end_matches('.');
    format!("rgba({r}, {g}, {b}, {alpha})")
}
