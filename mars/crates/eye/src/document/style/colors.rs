/** CSS color syntax resolves to one painter literal: a palette name, `transparent`, `#rrggbb` or `#rrggbbaa`. */
use super::unsupported;
use crate::protocol::EngineResult;
/** Names the painter already understands are kept verbatim so existing literals stay stable. */
const PALETTE: [&str; 10] = [
    "black",
    "white",
    "red",
    "green",
    "blue",
    "gray",
    "grey",
    "transparent",
    "navy",
    "teal",
];
/** The CSS named colors as 0xrrggbb. */
const NAMED: [(&str, u32); 148] = [
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];
/** Channel maximum. */
const CHANNEL: f64 = 255.0;
/** Degrees in one hue turn. */
const TURN: f64 = 360.0;
/** Alpha channel position in RGBA. */
const ALPHA: usize = 3;
/** Any supported CSS color becomes one painter literal; anything else is unsupported. */
pub(super) fn parse(source: &str) -> EngineResult<String> {
    let value = source.trim().to_ascii_lowercase();
    if PALETTE.contains(&value.as_str()) {
        return Ok(value);
    }
    if let Some((_, rgb)) = NAMED.iter().find(|(name, _)| *name == value) {
        let [_, r, g, b] = rgb.to_be_bytes();
        return Ok(hex([r, g, b, u8::MAX]));
    }
    if let Some(digits) = value.strip_prefix('#') {
        return hash(digits);
    }
    let (name, arguments) = value
        .strip_suffix(')')
        .and_then(|value| value.split_once('('))
        .ok_or_else(unsupported)?;
    let parts = arguments_of(arguments)?;
    let rgba = match name.trim() {
        "rgb" | "rgba" => rgb(&parts)?,
        "hsl" | "hsla" => hsl(&parts)?,
        "oklch" => oklab(&parts, true)?,
        "oklab" => oklab(&parts, false)?,
        _ => return Err(unsupported()),
    };
    Ok(hex(rgba))
}
/** A stored value is a literal the painter and validators accept without reparsing CSS. */
pub(super) fn literal(value: &str) -> bool {
    PALETTE.contains(&value)
        || value.strip_prefix('#').is_some_and(|digits| {
            [3, 6, 8].contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_hexdigit())
        })
}
/** Opaque colors keep the six-digit form; translucent ones add an alpha byte; zero alpha is transparent. */
fn hex([r, g, b, a]: [u8; 4]) -> String {
    match a {
        0 => "transparent".into(),
        u8::MAX => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
    }
}
/** Three and six digits are already literals; four and eight carry alpha. */
fn hash(digits: &str) -> EngineResult<String> {
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(unsupported());
    }
    let expanded: String = match digits.len() {
        3 | 6 => return Ok(format!("#{digits}")),
        4 => digits.chars().flat_map(|c| [c, c]).collect(),
        8 => digits.into(),
        _ => return Err(unsupported()),
    };
    let byte = |index: usize| u8::from_str_radix(&expanded[index..index + 2], 16).unwrap_or(0);
    Ok(hex([byte(0), byte(2), byte(4), byte(6)]))
}
/** Legacy comma syntax and modern space syntax with an optional `/ alpha` both give four parts. */
fn arguments_of(source: &str) -> EngineResult<Vec<&str>> {
    let (channels, alpha) = match source.split_once('/') {
        Some((channels, alpha)) => (channels, Some(alpha.trim())),
        None => (source, None),
    };
    let mut parts: Vec<&str> = if channels.contains(',') {
        channels.split(',').map(str::trim).collect()
    } else {
        channels.split_whitespace().collect()
    };
    parts.extend(alpha);
    if !(3..=4).contains(&parts.len()) || parts.iter().any(|part| part.is_empty()) {
        return Err(unsupported());
    }
    Ok(parts)
}
/** A number or percentage scaled so 100% equals `full`; `none` is zero. */
fn component(source: &str, full: f64) -> EngineResult<f64> {
    if source == "none" {
        return Ok(0.0);
    }
    let value = match source.strip_suffix('%') {
        Some(percent) => percent.parse::<f64>().map(|p| p / 100.0 * full),
        None => source.parse::<f64>(),
    }
    .ok()
    .filter(|value| value.is_finite())
    .ok_or_else(unsupported)?;
    Ok(value)
}
/** Optional alpha defaults to opaque and clamps into 0..1. */
fn alpha(parts: &[&str]) -> EngineResult<f64> {
    parts
        .get(ALPHA)
        .map_or(Ok(1.0), |part| component(part, 1.0))
        .map(|alpha| alpha.clamp(0.0, 1.0))
}
/** Clamped 0..1 channels become bytes. */
fn bytes(channels: [f64; 3], alpha: f64) -> [u8; 4] {
    let byte = |value: f64| (value.clamp(0.0, 1.0) * CHANNEL).round() as u8;
    [
        byte(channels[0]),
        byte(channels[1]),
        byte(channels[2]),
        byte(alpha),
    ]
}
/** `rgb()` channels are 0..255 numbers or percentages. */
fn rgb(parts: &[&str]) -> EngineResult<[u8; 4]> {
    let channel = |index: usize| component(parts[index], CHANNEL).map(|value| value / CHANNEL);
    Ok(bytes(
        [channel(0)?, channel(1)?, channel(2)?],
        alpha(parts)?,
    ))
}
/** Hue angles accept deg, rad, grad and turn. */
fn hue(source: &str) -> EngineResult<f64> {
    super::values::angle(source)
        .or_else(|_| component(source, TURN))
        .map(|degrees| degrees.rem_euclid(TURN))
}
/** The CSS Color 4 HSL to sRGB conversion. */
fn hsl(parts: &[&str]) -> EngineResult<[u8; 4]> {
    let h = hue(parts[0])?;
    let s = component(parts[1], 1.0)?.clamp(0.0, 1.0);
    let l = component(parts[2], 1.0)?.clamp(0.0, 1.0);
    let f = |n: f64| {
        let k = (n + h / 30.0) % 12.0;
        let a = s * l.min(1.0 - l);
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    Ok(bytes([f(0.0), f(8.0), f(4.0)], alpha(parts)?))
}
/** OKLab (or OKLCH) to gamma-encoded sRGB, clipped to the gamut. */
fn oklab(parts: &[&str], polar: bool) -> EngineResult<[u8; 4]> {
    let lightness = component(parts[0], 1.0)?;
    let (a, b) = if polar {
        let chroma = component(parts[1], 0.4)?;
        let angle = hue(parts[2])?.to_radians();
        (chroma * angle.cos(), chroma * angle.sin())
    } else {
        (component(parts[1], 0.4)?, component(parts[2], 0.4)?)
    };
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    let linear = [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ];
    Ok(bytes(linear.map(gamma), alpha(parts)?))
}
/** The sRGB transfer function. */
fn gamma(linear: f64) -> f64 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.max(0.0).powf(1.0 / 2.4) - 0.055
    }
}
