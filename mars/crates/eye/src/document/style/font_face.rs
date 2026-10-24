/**
 * `@font-face` descriptors and the fonts a page loaded for them. A descriptor names a family
 * and the files (or installed fonts) that supply one style and weight range of it; the app
 * fetches the files the page's text needs and hands the sfnt bytes back inside the document,
 * so the document service that measures text and the window that paints it load the very same
 * bytes, for this process only, never into the system font registry.
 */
use super::syntax::split_top;
use serde::{Deserialize, Serialize};

/** `@font-face` rules one page keeps; Google Fonts emits one per family, weight and script subset. */
pub const FONT_FACES: usize = 256;
/** Font files one page may fetch; the rest render with their fallback families. */
pub const FONT_FILES: usize = 24;
/** Encoded bytes of one font file; a transfer past this is refused by the network layer. */
pub const FONT_FILE_BYTES: usize = 2 * 1024 * 1024;
/** Unpacked sfnt bytes all of a page's fonts may hold together; later files are skipped. */
pub const FONT_TOTAL_BYTES: usize = 8 * 1024 * 1024;
/** Families one `font-family` list keeps. */
pub const FONT_FAMILY_NAMES: usize = 8;
/** Bytes of one family or local font name. */
pub const FONT_NAME_BYTES: usize = 64;
/** `src` entries one descriptor keeps. */
pub const FONT_SOURCES: usize = 8;
/** `unicode-range` entries one descriptor keeps. */
pub const UNICODE_RANGES: usize = 64;
/** The last Unicode scalar value. */
const UNICODE_MAX: u32 = 0x10FFFF;
/** Unpacked bytes of one font file; a variable family with every script inside fits well under this. */
const FONT_SFNT_BYTES: usize = 4 * 1024 * 1024;
/** `font-weight` keyword `normal`. */
const WEIGHT_NORMAL: u16 = 400;
/** `font-weight` keyword `bold`. */
const WEIGHT_BOLD: u16 = 700;
/** The CSS weight range. */
const WEIGHTS: std::ops::RangeInclusive<u16> = 1..=1000;
/** File formats the app can unpack; a `format()` hint outside these skips the source, as Chrome does. */
const FORMATS: [&str; 6] = [
    "woff2",
    "woff",
    "truetype",
    "opentype",
    "woff2-variations",
    "truetype-variations",
];

/** `font-style` of a computed style or a descriptor. */
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum FontStyle {
    /// Upright.
    #[default]
    Normal,
    /// A designed italic face.
    Italic,
    /// A slanted face; an angle is accepted and ignored.
    Oblique,
}

/** `font-display`: recorded per descriptor. Fonts load in the navigation's resource batch before
the settled page is shown, so every value behaves as `swap`: fallback text until the file is in,
then the web font. */
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FontDisplay {
    #[default]
    Auto,
    Block,
    Swap,
    Fallback,
    Optional,
}

/** One `src` entry. */
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FontSource {
    /// A file to fetch, with its `format()` hint when given; the URL is already absolute.
    Url { url: String, format: Option<String> },
    /// An installed font by full or PostScript name.
    Local(String),
}

/** One `@font-face` rule, validated at parse time. */
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FontFace {
    /// Lowercased, unquoted family name, how `font-family` lists are matched.
    pub family: String,
    /// Sources in order; the first the app can load wins.
    pub sources: Vec<FontSource>,
    /// Weight range (a single weight repeats); a range marks a variable font.
    pub weight: (u16, u16),
    /// Style this face supplies.
    pub style: FontStyle,
    /// Loading policy, recorded only.
    pub display: FontDisplay,
    /// Inclusive scalar ranges this face covers; empty covers everything.
    pub ranges: Vec<(u32, u32)>,
}
impl Default for FontFace {
    /** The descriptor defaults: weight 400, normal style, every character. */
    fn default() -> Self {
        Self {
            family: String::new(),
            sources: Vec::new(),
            weight: (WEIGHT_NORMAL, WEIGHT_NORMAL),
            style: FontStyle::Normal,
            display: FontDisplay::Auto,
            ranges: Vec::new(),
        }
    }
}
impl FontFace {
    /** Whether this face supplies glyphs for `c` (no ranges means all of Unicode). */
    pub fn covers(&self, c: char) -> bool {
        let code = c as u32;
        self.ranges.is_empty()
            || self
                .ranges
                .iter()
                .any(|(low, high)| (*low..=*high).contains(&code))
    }
    /** The first source the app can fetch: a URL whose format hint, if any, is one it unpacks. */
    pub fn fetchable(&self) -> Option<&str> {
        self.sources.iter().find_map(|source| match source {
            FontSource::Url { url, format } => format
                .as_deref()
                .is_none_or(|hint| FORMATS.contains(&hint))
                .then_some(url.as_str()),
            FontSource::Local(_) => None,
        })
    }
    /** Local names listed before the first fetchable URL, which win when installed. */
    pub fn locals(&self) -> impl Iterator<Item = &str> {
        self.sources.iter().filter_map(|source| match source {
            FontSource::Local(name) => Some(name.as_str()),
            FontSource::Url { .. } => None,
        })
    }
}

/** One font file the app unpacked for a descriptor, carried inside the document. */
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct LoadedFont {
    /// Index into the document's `font_faces`.
    pub face: usize,
    /// Content hash of `data`, the identity measurement and painting share.
    pub key: u64,
    /// Base64 of the sfnt (TrueType or CFF OpenType) bytes; empty when an earlier entry with the
    /// same key carries them (one file declared for several weights or scripts).
    pub data: String,
}
impl LoadedFont {
    /** The unpacked bytes, None when the text is not base64 or past the per-file bound. */
    pub fn bytes(&self) -> Option<Vec<u8>> {
        if self.data.len() / 4 * 3 > FONT_SFNT_BYTES {
            return None;
        }
        base64(&self.data)
    }
}

/** Every `@font-face` rule of a page's sheets, in source order, within FONT_FACES. */
#[derive(Clone, Debug, Default)]
pub(super) struct FontFaces {
    /// Kept rules.
    pub faces: Vec<FontFace>,
}
impl FontFaces {
    /** Records one `@font-face` block; malformed or incomplete rules are dropped with a note. */
    pub(super) fn parse(&mut self, body: &str, diagnostics: &mut Vec<String>) {
        if self.faces.len() >= FONT_FACES {
            return super::note(
                diagnostics,
                "Font faces past the page bound were ignored.".into(),
            );
        }
        match face(body) {
            Some(face) => self.faces.push(face),
            None => super::note(
                diagnostics,
                "A malformed or unsupported @font-face rule was ignored.".into(),
            ),
        }
    }
}

/** A face from a descriptor block; None without a family or a usable source. */
fn face(body: &str) -> Option<FontFace> {
    let mut face = FontFace::default();
    for item in split_top(body, b';') {
        let Some((name, value)) = item.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().trim_end_matches("!important").trim();
        match name.as_str() {
            "font-family" => face.family = unquote(value).to_ascii_lowercase(),
            "src" => face.sources = sources(value)?,
            "font-weight" => face.weight = weight_range(value)?,
            "font-style" => face.style = style(value)?,
            "font-display" => face.display = display(value),
            "unicode-range" => face.ranges = ranges(value)?,
            _ => {}
        }
    }
    (!face.family.is_empty() && face.family.len() <= FONT_NAME_BYTES && !face.sources.is_empty())
        .then_some(face)
}
/** Strips one pair of matching quotes. */
fn unquote(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}
/** `url(x) format(y)` and `local(name)` entries; unknown functions skip their entry. */
fn sources(value: &str) -> Option<Vec<FontSource>> {
    let mut out = Vec::new();
    for entry in split_top(value, b',') {
        let entry = entry.trim();
        if let Some(name) = argument(entry, "local(") {
            let name = unquote(name).to_owned();
            if !name.is_empty() && name.len() <= FONT_NAME_BYTES {
                out.push(FontSource::Local(name));
            }
        } else if let Some(url) = argument(entry, "url(") {
            let url = unquote(url).to_owned();
            let rest = &entry[entry.find(')').map_or(entry.len(), |i| i + 1)..];
            let format = argument(rest.trim(), "format(").map(|f| unquote(f).to_ascii_lowercase());
            if !url.is_empty() && url.len() <= crate::constants::URL_BYTES {
                out.push(FontSource::Url { url, format });
            }
        }
        if out.len() >= FONT_SOURCES {
            break;
        }
    }
    (!out.is_empty()).then_some(out)
}
/** The argument of a leading `name(` call, case-insensitively. */
fn argument<'a>(entry: &'a str, function: &str) -> Option<&'a str> {
    let head = entry.get(..function.len())?;
    if !head.eq_ignore_ascii_case(function) {
        return None;
    }
    let rest = &entry[function.len()..];
    rest.find(')').map(|end| rest[..end].trim())
}
/** `normal`, `bold`, one weight or a `low high` range, each within 1..=1000. */
fn weight_range(value: &str) -> Option<(u16, u16)> {
    let one = |token: &str| match token {
        "normal" => Some(WEIGHT_NORMAL),
        "bold" => Some(WEIGHT_BOLD),
        _ => token.parse::<u16>().ok().filter(|w| WEIGHTS.contains(w)),
    };
    let mut parts = value.split_whitespace();
    let low = one(parts.next()?)?;
    let high = parts.next().map_or(Some(low), one)?;
    parts
        .next()
        .is_none()
        .then_some((low.min(high), low.max(high)))
}
/** `normal`, `italic` or `oblique` with an optional angle. */
fn style(value: &str) -> Option<FontStyle> {
    match value.split_whitespace().next()? {
        "normal" => Some(FontStyle::Normal),
        "italic" => Some(FontStyle::Italic),
        "oblique" => Some(FontStyle::Oblique),
        _ => None,
    }
}
/** `font-display` keywords; an unknown one is `auto`. */
fn display(value: &str) -> FontDisplay {
    match value {
        "block" => FontDisplay::Block,
        "swap" => FontDisplay::Swap,
        "fallback" => FontDisplay::Fallback,
        "optional" => FontDisplay::Optional,
        _ => FontDisplay::Auto,
    }
}
/** `U+XXXX`, `U+XXXX-YYYY` and `U+XX??` entries; a malformed entry drops the descriptor. */
fn ranges(value: &str) -> Option<Vec<(u32, u32)>> {
    let mut out = Vec::new();
    for entry in value.split(',') {
        let entry = entry.trim();
        let hex = entry
            .strip_prefix("U+")
            .or_else(|| entry.strip_prefix("u+"))?;
        let (low, high) = match hex.split_once('-') {
            Some((low, high)) => (
                u32::from_str_radix(low, 16).ok()?,
                u32::from_str_radix(high, 16).ok()?,
            ),
            None if hex.contains('?') => (
                u32::from_str_radix(&hex.replace('?', "0"), 16).ok()?,
                u32::from_str_radix(&hex.replace('?', "F"), 16).ok()?,
            ),
            None => {
                let code = u32::from_str_radix(hex, 16).ok()?;
                (code, code)
            }
        };
        if low > high || high > UNICODE_MAX {
            return None;
        }
        out.push((low, high));
        if out.len() >= UNICODE_RANGES {
            break;
        }
    }
    (!out.is_empty()).then_some(out)
}

/** FNV-1a over bytes: the stable identity of a font's data across the processes that load it. */
pub fn content_key(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}
/** Standard base64 with padding, as the app encodes font bytes; None on any other character. */
pub(super) fn base64(text: &str) -> Option<Vec<u8>> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 2);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in text.bytes() {
        buffer = (buffer << 6) | u32::from(value(byte)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}
