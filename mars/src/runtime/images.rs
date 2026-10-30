/** Static image decoding validates dimensions before retaining page assets; web font files are
fetched in the same batch and unpacked to sfnt bytes the document carries. */
use crate::{
    constants::{
        IMAGE_BYTES, IMAGE_COUNT, IMAGE_DECODED_BYTES, IMAGE_DIMENSION, IMAGE_TOTAL_BYTES,
        TEXT_BYTES,
    },
    document::{
        Document, FONT_FILE_BYTES, FONT_FILES, FONT_TOTAL_BYTES, LoadedFont, Node, content_key,
        font_requests,
    },
    protocol::{EngineError, EngineResult},
};
use std::io::Read;

/// A font file travels through the same binary body path as an image, so it cannot be allowed
/// more than Relay admits for one.
const _: () = assert!(FONT_FILE_BYTES <= IMAGE_BYTES);
/// A page's unpacked fonts are base64 inside every document frame; they must stay a small share
/// of the frame bound beside the styled nodes and sheets.
const _: () = assert!(FONT_TOTAL_BYTES / 3 * 4 <= crate::constants::FRAME_BYTES / 8);
/// WOFF 1.0 file header bytes (signature through privLength).
const WOFF_HEADER: usize = 44;
/// WOFF 1.0 table directory entry bytes.
const WOFF_ENTRY: usize = 20;
/// sfnt offset table bytes (sfntVersion, numTables, searchRange, entrySelector, rangeShift).
const SFNT_HEADER: usize = 12;
/// sfnt table record bytes (tag, checksum, offset, length).
const SFNT_RECORD: usize = 16;
/// Tables one font may list; real fonts have a few dozen.
const FONT_TABLES: usize = 64;
use std::collections::BTreeMap;
use std::io::Cursor;

pub(super) mod canvas;
#[cfg(test)]
#[path = "images/lifecycle_tests.rs"]
mod lifecycle_tests;

/// Decoded metadata enforces a separate memory budget from transferred bytes.
pub(super) struct DecodedImage {
    pub width: u32,
    pub height: u32,
}

/** A committed image keeps its original encoded bytes for AppKit's native decoder. */
#[derive(Clone)]
pub struct ImageAsset {
    pub mime: String,
    pub bytes: std::sync::Arc<Vec<u8>>,
    /// Decoded bitmap bytes this image costs, so later script-inserted images are charged
    /// against what the page already holds; zero for a drawing.
    pub decoded: u64,
    /// An actual image decode's selected request URL and density; backgrounds have no image provenance.
    pub(super) image_source: Option<(String, f64)>,
    /// True only for local data or an acquisition whose every hop stayed on the document origin.
    pub(super) origin_clean: bool,
}

/** PNG and JPEG are decoded by audited Rust libraries; WebP and GIF headers give their size. */
pub(super) fn dimensions(mime: &str, bytes: &[u8]) -> EngineResult<DecodedImage> {
    let image = match mime {
        "image/png" => png(bytes)?,
        "image/jpeg" => jpeg(bytes)?,
        "image/webp" => webp(bytes)?,
        "image/gif" => gif(bytes)?,
        _ => return Err(invalid()),
    };
    if image.width == 0
        || image.height == 0
        || image.width > IMAGE_DIMENSION
        || image.height > IMAGE_DIMENSION
        || u64::from(image.width) * u64::from(image.height) * 4 > IMAGE_DECODED_BYTES as u64
    {
        return Err(invalid());
    }
    Ok(image)
}

/** The raster format the bytes' signature names, as the image sniffing rules read it. */
pub(super) fn sniffed(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/** PNG decoder expands palette data only after its dimensions fit the page budget. */
fn png(bytes: &[u8]) -> EngineResult<DecodedImage> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| invalid())?;
    let info = reader.info();
    checked(info.width, info.height)?;
    if reader.output_buffer_size() > IMAGE_DECODED_BYTES {
        return Err(invalid());
    }
    let mut output = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut output).map_err(|_| invalid())?;
    checked(frame.width, frame.height)?;
    Ok(DecodedImage {
        width: frame.width,
        height: frame.height,
    })
}

/** JPEG decoding rejects unsupported CMYK output rather than mispainting its colors. */
fn jpeg(bytes: &[u8]) -> EngineResult<DecodedImage> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.read_info().map_err(|_| invalid())?;
    let header = decoder.info().ok_or_else(invalid)?;
    checked(u32::from(header.width), u32::from(header.height))?;
    decoder.decode().map_err(|_| invalid())?;
    let info = decoder.info().ok_or_else(invalid)?;
    if info.pixel_format == jpeg_decoder::PixelFormat::CMYK32 {
        return Err(invalid());
    }
    // A quarter-turned EXIF orientation shows the picture with its width and height swapped.
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    if super::exif::swaps_axes(super::exif::orientation(bytes)) {
        checked(height, width)
    } else {
        checked(width, height)
    }
}

/// WebP's size comes from its first chunk header (lossy VP8, lossless VP8L or extended VP8X);
/// AppKit decodes the pixels, so only the dimensions are read here, within the same bounds.
fn webp(bytes: &[u8]) -> EngineResult<DecodedImage> {
    if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WEBP") {
        return Err(invalid());
    }
    let byte = |at: usize| bytes.get(at).copied().map(u32::from).ok_or_else(invalid);
    match bytes.get(12..16) {
        Some(b"VP8 ") if bytes.get(23..26) == Some(&[0x9d, 0x01, 0x2a]) => checked(
            (byte(26)? | byte(27)? << 8) & 0x3fff,
            (byte(28)? | byte(29)? << 8) & 0x3fff,
        ),
        Some(b"VP8L") if byte(20)? == 0x2f => checked(
            1 + (byte(21)? | (byte(22)? & 0x3f) << 8),
            1 + (byte(22)? >> 6 | byte(23)? << 2 | (byte(24)? & 0x0f) << 10),
        ),
        Some(b"VP8X") => checked(
            1 + (byte(24)? | byte(25)? << 8 | byte(26)? << 16),
            1 + (byte(27)? | byte(28)? << 8 | byte(29)? << 16),
        ),
        _ => Err(invalid()),
    }
}

/** GIF's logical screen size follows its signature; AppKit decodes the frames. */
fn gif(bytes: &[u8]) -> EngineResult<DecodedImage> {
    if !matches!(bytes.get(..6), Some(b"GIF87a" | b"GIF89a")) || bytes.len() < 10 {
        return Err(invalid());
    }
    checked(
        u32::from(u16::from_le_bytes([bytes[6], bytes[7]])),
        u32::from(u16::from_le_bytes([bytes[8], bytes[9]])),
    )
}

/** One shared check prevents multiplication overflow and oversized decoded surfaces. */
fn checked(width: u32, height: u32) -> EngineResult<DecodedImage> {
    if width == 0
        || height == 0
        || width > IMAGE_DIMENSION
        || height > IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) * 4 > IMAGE_DECODED_BYTES as u64
    {
        return Err(invalid());
    }
    Ok(DecodedImage { width, height })
}

/** Image decode errors never expose bytes, URLs or decoder internals. */
fn invalid() -> EngineError {
    EngineError::unsupported("The image is invalid, unsupported, or exceeds browser limits.")
}

/** The private network service base64-encodes image bytes to preserve JSON framing. */
pub(super) fn decode_base64(input: &str) -> EngineResult<Vec<u8>> {
    decode_base64_within(input, crate::constants::IMAGE_BYTES)
}

/** The same decoding under a caller's byte budget; downloads use the page budget. */
pub(super) fn decode_base64_within(input: &str, limit: usize) -> EngineResult<Vec<u8>> {
    if input.len() / 4 * 3 > limit + 2 {
        return Err(invalid());
    }
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut value = 0u32;
    let mut bits = 0;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(invalid()),
        };
        value = (value << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((value >> bits) as u8);
        }
    }
    if output.len() > limit {
        return Err(invalid());
    }
    Ok(output)
}

/** Device pixel ratio srcset selection assumes; Mars paints Retina windows at 2x. */
const PIXEL_RATIO: f64 = 2.0;
/** CSS px in one `em` inside `sizes` and media features: the initial font size. */
const EM_PX: f64 = 16.0;
/** Most srcset candidates, sizes entries or picture sources read for one image. */
const CANDIDATES: usize = 64;
/** Attributes lazy-loading scripts keep the real address in until they run. */
const LAZY_SOURCES: [&str; 3] = ["data-src", "data-lazy-src", "data-original"];
/** Attributes lazy-loading scripts keep the real srcset in until they run. */
const LAZY_SETS: [&str; 2] = ["data-srcset", "data-lazy-srcset"];
/** `<source type>` values Mars can show; AVIF and anything unknown are skipped. */
const SOURCE_TYPES: [&str; 6] = [
    "image/webp",
    "image/png",
    "image/jpeg",
    "image/jpg",
    "image/gif",
    "image/svg+xml",
];

/// The addresses an `<img>` may show, best first, each with the density its pixels are drawn
/// at: the first matching `<picture>` source by type and media, else the img's srcset and sizes;
/// then src as a fallback for when that choice cannot be loaded (Mars cannot decode every format
/// a page offers). Where a lazy-loading script would have filled them in, `data-srcset` and
/// `data-src` stand in for a missing srcset and an empty or `data:` placeholder src.
pub(super) fn sources(document: &Document, node: &Node, viewport: f64) -> Vec<(String, f64)> {
    if stand_in(document, node) {
        return Vec::new();
    }
    let chosen = picture_source(document, node, viewport)
        .or_else(|| set_source(node, &own_set(node)?, viewport));
    let plain = plain_source(node).map(|src| (src, 1.0));
    let mut list = chosen.into_iter().chain(plain).collect::<Vec<_>>();
    list.dedup_by(|a, b| a.0 == b.0);
    list
}

/** Lifecycle selection reads only real HTML source attributes; paint-only lazy heuristics cannot report a load. */
pub(super) fn selected_source(
    document: &Document,
    node: &Node,
    viewport: f64,
) -> Option<(String, f64)> {
    let picture = node.parent.and_then(|parent| {
        if !document
            .nodes
            .iter()
            .any(|item| item.local_id == parent && item.tag == "picture")
        {
            return None;
        }
        document
            .nodes
            .iter()
            .take_while(|item| item.local_id != node.local_id)
            .filter(|item| item.parent == Some(parent) && item.tag == "source")
            .take(CANDIDATES)
            .filter(|item| supported_type(item) && media_attribute(item, viewport))
            .find_map(|item| set_source(item, &first_attribute(item, &["srcset"])?, viewport))
    });
    picture
        .or_else(|| set_source(node, &first_attribute(node, &["srcset"])?, viewport))
        .or_else(|| first_attribute(node, &["src"]).map(|url| (url, 1.0)))
}

/// A placeholder img a script swaps out: its file is named as a placeholder and the very next
/// sibling is the real img. BBC stacks an opaque grey placeholder (positioned, so it paints on
/// top) before every photo and removes it from script; Mars does not run those module scripts,
/// so showing the placeholder would hide the photo. It is not fetched and shows its alt text.
// ponytail: naming heuristic; drop it once module scripts run and remove the placeholder.
fn stand_in(document: &Document, node: &Node) -> bool {
    let named = node.image_src.as_deref().is_some_and(|src| {
        let file = src.rsplit('/').next().unwrap_or("");
        file.to_ascii_lowercase().contains("placeholder")
    });
    named
        && document
            .nodes
            .iter()
            .skip_while(|item| item.local_id != node.local_id)
            .skip(1)
            .find(|item| item.parent == node.parent)
            .is_some_and(|next| next.tag == "img")
}

/** The first `<source>` sibling before the img in its picture that Mars can show and whose media matches. */
fn picture_source(document: &Document, node: &Node, viewport: f64) -> Option<(String, f64)> {
    let parent = node.parent?;
    let picture = document.nodes.iter().find(|item| item.local_id == parent)?;
    if picture.tag != "picture" {
        return None;
    }
    document
        .nodes
        .iter()
        .take_while(|item| item.local_id != node.local_id)
        .filter(|item| item.parent == Some(parent) && item.tag == "source")
        .take(CANDIDATES)
        .filter(|item| supported_type(item) && media_attribute(item, viewport))
        .find_map(|item| {
            let set =
                first_attribute(item, &["srcset"]).or_else(|| first_attribute(item, &LAZY_SETS))?;
            set_source(item, &set, viewport)
        })
}

/** A missing or recognized type; parameters such as `codecs` are ignored. */
fn supported_type(node: &Node) -> bool {
    node.attributes.get("type").is_none_or(|value| {
        let essence = value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        SOURCE_TYPES.contains(&essence.as_str())
    })
}

/** A missing media attribute matches every viewport. */
fn media_attribute(node: &Node, viewport: f64) -> bool {
    node.attributes
        .get("media")
        .is_none_or(|media| media_matches(media, viewport))
}

/** The img's own srcset, or a lazy one when it has none. */
fn own_set(node: &Node) -> Option<String> {
    first_attribute(node, &["srcset"]).or_else(|| first_attribute(node, &LAZY_SETS))
}

/** src, unless it is empty or a `data:` placeholder and a lazy attribute holds the real address. */
fn plain_source(node: &Node) -> Option<String> {
    let src = node.image_src.as_deref().map(str::trim).unwrap_or("");
    let placeholder = src.is_empty()
        || src
            .get(..5)
            .is_some_and(|s| s.eq_ignore_ascii_case("data:"));
    if placeholder && let Some(lazy) = first_attribute(node, &LAZY_SOURCES) {
        return Some(lazy);
    }
    (!src.is_empty()).then(|| src.to_owned())
}

/** The first of these attributes with a non-blank value, trimmed. */
fn first_attribute(node: &Node, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        let value = node.attributes.get(*name)?.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Chromium's choice from one srcset: `w` descriptors become densities against the slot width
/// from `sizes`, candidates sort by density, and the walk stops at the first whose geometric mean
/// with the next is at least the device pixel ratio.
fn set_source(node: &Node, set: &str, viewport: f64) -> Option<(String, f64)> {
    let slot = slot_width(node.attributes.get("sizes").map(String::as_str), viewport);
    let mut candidates = parse_srcset(set)
        .into_iter()
        .filter_map(|(url, descriptor)| Some((url, descriptor.density(slot)?)))
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut chosen = 0;
    while chosen + 1 < candidates.len()
        && PIXEL_RATIO > (candidates[chosen].1 * candidates[chosen + 1].1).sqrt()
    {
        chosen += 1;
    }
    candidates.into_iter().nth(chosen)
}

/** One srcset descriptor; a candidate without one is `1x`. */
#[derive(Clone, Copy, Debug, PartialEq)]
enum Descriptor {
    Width(f64),
    Density(f64),
}
impl Descriptor {
    /** Pixels per CSS px when drawn into a slot this wide; None for a degenerate value. */
    fn density(self, slot: f64) -> Option<f64> {
        let density = match self {
            Self::Width(width) => width / slot.max(1.0),
            Self::Density(density) => density,
        };
        (density.is_finite() && density > 0.0).then_some(density)
    }
}

/// HTML's srcset parsing: a URL is a run of non-whitespace (trailing commas end the candidate),
/// then descriptors up to the next comma outside parentheses. Malformed candidates are dropped.
fn parse_srcset(set: &str) -> Vec<(String, Descriptor)> {
    let mut candidates = Vec::new();
    let mut rest = set;
    while candidates.len() < CANDIDATES {
        rest = rest.trim_start_matches(|c: char| c == ',' || c.is_ascii_whitespace());
        if rest.is_empty() {
            break;
        }
        let end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let url = &rest[..end];
        rest = &rest[end..];
        let descriptors = if url.ends_with(',') {
            ""
        } else {
            let close = descriptor_end(rest);
            let text = &rest[..close];
            rest = &rest[close..];
            text
        };
        if let Some(descriptor) = descriptor(descriptors) {
            candidates.push((url.trim_end_matches(',').to_owned(), descriptor));
        }
    }
    candidates
}

/** The byte index of the comma ending one candidate's descriptors, skipping parenthesized text. */
fn descriptor_end(rest: &str) -> usize {
    let mut depth = 0usize;
    for (index, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return index,
            _ => {}
        }
    }
    rest.len()
}

/** `Nw` or `Nx` (an `h` hint is ignored); none means `1x`; anything else drops the candidate. */
fn descriptor(text: &str) -> Option<Descriptor> {
    let mut found = None;
    for token in text.split_ascii_whitespace() {
        let (number, unit) = token.split_at(token.len().saturating_sub(1));
        let value = number
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)?;
        let next = match unit {
            "w" => Descriptor::Width(value),
            "x" => Descriptor::Density(value),
            "h" => continue,
            _ => return None,
        };
        if found.replace(next).is_some() {
            return None;
        }
    }
    Some(found.unwrap_or(Descriptor::Density(1.0)))
}

/// The slot width `sizes` gives at this viewport: the first entry whose media condition matches
/// and whose length parses; 100vw when none does.
fn slot_width(sizes: Option<&str>, viewport: f64) -> f64 {
    sizes
        .into_iter()
        .flat_map(|sizes| sizes.split(','))
        .take(CANDIDATES)
        .find_map(|entry| {
            let entry = entry.trim();
            let split = entry.rfind(|c: char| c.is_ascii_whitespace() || c == ')');
            let (condition, length) = match split {
                Some(at) => entry.split_at(at + 1),
                None => ("", entry),
            };
            let width = length_px(length.trim(), viewport)?;
            (condition.trim().is_empty() || media_matches(condition, viewport)).then_some(width)
        })
        .unwrap_or(viewport)
}

/** A `px`, `vw`, `em` or `rem` length in CSS px; `auto`, `calc()` and others return None. */
fn length_px(text: &str, viewport: f64) -> Option<f64> {
    let text = text.to_ascii_lowercase();
    let (number, scale) = if let Some(n) = text.strip_suffix("px") {
        (n, 1.0)
    } else if let Some(n) = text.strip_suffix("vw") {
        (n, viewport / 100.0)
    } else if let Some(n) = text.strip_suffix("rem").or_else(|| text.strip_suffix("em")) {
        (n, EM_PX)
    } else if text == "0" {
        (text.as_str(), 1.0)
    } else {
        return None;
    };
    let value = number.trim().parse::<f64>().ok()? * scale;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// A media query list for a light-mode screen of this width at PIXEL_RATIO: any comma-separated
/// query matches when all its `and`ed parts do. Width, resolution, orientation and color scheme
/// features are evaluated; an unknown feature or type does not match, as in browsers.
pub(super) fn media_matches(list: &str, viewport: f64) -> bool {
    list.split(',').take(CANDIDATES).any(|query| {
        let query = query.trim().to_ascii_lowercase();
        let (negated, query) = match query.strip_prefix("not ") {
            Some(rest) => (true, rest.to_owned()),
            None => (
                false,
                query.strip_prefix("only ").unwrap_or(&query).to_owned(),
            ),
        };
        let matched = !query.is_empty()
            && query
                .split(" and ")
                .all(|part| media_part(part.trim(), viewport));
        matched != negated
    })
}

/** One media type or parenthesized feature. */
fn media_part(part: &str, viewport: f64) -> bool {
    let Some(feature) = part.strip_prefix('(').and_then(|p| p.strip_suffix(')')) else {
        return matches!(part, "all" | "screen");
    };
    if let Some((name, value)) = feature.split_once(':') {
        return media_feature(name.trim(), value.trim(), viewport);
    }
    range_feature(feature, viewport).unwrap_or(false)
}

/** `name: value` features; prefixed and unknown ones never match. */
fn media_feature(name: &str, value: &str, viewport: f64) -> bool {
    let resolution = || {
        value
            .strip_suffix("dppx")
            .or_else(|| value.strip_suffix('x'))
            .map(|v| v.trim().parse::<f64>().ok())
            .unwrap_or_else(|| value.parse::<f64>().ok())
    };
    match name {
        "min-width" => length_px(value, viewport).is_some_and(|w| viewport >= w),
        "max-width" => length_px(value, viewport).is_some_and(|w| viewport <= w),
        "min-resolution" | "-webkit-min-device-pixel-ratio" => {
            resolution().is_some_and(|r| PIXEL_RATIO >= r)
        }
        "max-resolution" | "-webkit-max-device-pixel-ratio" => {
            resolution().is_some_and(|r| PIXEL_RATIO <= r)
        }
        "orientation" => value == "landscape",
        "prefers-color-scheme" => value == "light",
        "prefers-reduced-motion" => value == "no-preference",
        _ => false,
    }
}

/** Range syntax on width, such as `width >= 600px` or `width < 40em`. */
fn range_feature(feature: &str, viewport: f64) -> Option<bool> {
    let rest = feature.strip_prefix("width")?.trim_start();
    let (operator, value) = [">=", "<=", ">", "<", "="]
        .iter()
        .find_map(|op| rest.strip_prefix(op).map(|value| (*op, value)))?;
    let width = length_px(value.trim(), viewport)?;
    Some(match operator {
        ">=" => viewport >= width,
        "<=" => viewport <= width,
        ">" => viewport > width,
        "<" => viewport < width,
        _ => viewport == width,
    })
}

/** What one image request produced. */
#[derive(Clone)]
pub(super) enum Fetched {
    /// Encoded bitmap bytes for AppKit's decoder, with their pixel size.
    Raster(ImageAsset, (u32, u32)),
    /// SVG markup reduced to the drawing subset Eye paints as vector paths, with its intrinsic
    /// size in CSS px.
    Vector(String, (u32, u32)),
}
impl Fetched {
    /// The encoded bytes painters receive for a CSS background; a drawing goes as its markup.
    pub(super) fn into_asset(self) -> ImageAsset {
        match self {
            Self::Raster(asset, _) => asset,
            Self::Vector(markup, _) => ImageAsset {
                mime: SVG_MIME.into(),
                bytes: std::sync::Arc::new(markup.into_bytes()),
                decoded: 0,
                image_source: None,
                origin_clean: false,
            },
        }
    }
}

impl super::supervisor::Runtime {
    /** Publish a real decode to the owned layout node; hidden and detached images still load without paint assets. */
    pub(super) fn commit_image_decode(
        &self,
        page: &mut super::page::Page,
        id: usize,
        fetched: Fetched,
        density: f64,
    ) -> (u32, u32) {
        let size = match &fetched {
            Fetched::Raster(_, size) => css_size(*size, density),
            Fetched::Vector(_, size) => *size,
        };
        let Some(node) = page
            .document
            .nodes
            .iter_mut()
            .find(|node| node.local_id == id)
        else {
            page.scripting
                .image_loading
                .hidden
                .insert(id, fetched.into_asset());
            return size;
        };
        node.image_size = Some(size);
        node.text.clear();
        // An SVG without a natural ratio stretches on both axes; a raster always has one.
        node.image_ratio_free =
            matches!(&fetched, Fetched::Vector(markup, _) if !vector_ratio(markup));
        if let Fetched::Vector(markup, _) = &fetched {
            node.image_src = Some(svg_url(markup));
        }
        page.images.remove(&id);
        let hidden = node.hidden();
        if hidden {
            page.scripting
                .image_loading
                .hidden
                .insert(id, fetched.into_asset());
        } else {
            page.scripting.image_loading.hidden.remove(&id);
            page.images.insert(id, fetched.into_asset());
        }
        size
    }

    /** A style change moves already decoded pixels between native storage and the painted snapshot. */
    pub(super) fn refresh_image_visibility(&self, page: &mut super::page::Page) {
        for node in page.document.nodes.iter().filter(|node| node.tag == "img") {
            let id = node.local_id;
            if !page.scripting.image_loading.managed.contains(&id) {
                continue;
            }
            if node.hidden() {
                if let Some(asset) = page.images.remove(&id) {
                    page.scripting.image_loading.hidden.insert(id, asset);
                }
            } else if let Some(asset) = page.scripting.image_loading.hidden.remove(&id) {
                page.images.insert(id, asset);
            }
        }
    }
}

/// Page-wide image bounds: IMAGE_COUNT requests, IMAGE_TOTAL_BYTES encoded bytes (Relay already
/// holds each one to IMAGE_BYTES) and IMAGE_DECODED_BYTES of decoded bitmap pixels.
/// Drawings decode to no bitmap, so only their markup counts.
#[derive(Clone, Default)]
pub(super) struct Budget {
    requests: usize,
    bytes: usize,
    decoded: u64,
    /// Images already fetched on this page by address: a repeated address reuses its result
    /// instead of spending another request (CSS2 tables' references draw 130 swatches from four
    /// files, and their sixty-fifth showed as alt text).
    fetched: std::collections::HashMap<String, Fetched>,
}
impl Budget {
    /// Background or legacy image work may add assets between lifecycle batches; never undercount them.
    pub(super) fn include_assets(
        &mut self,
        images: &BTreeMap<usize, ImageAsset>,
        hidden: &BTreeMap<usize, ImageAsset>,
    ) {
        self.requests = self.requests.max(images.len() + hidden.len());
        self.bytes = self.bytes.max(
            images
                .values()
                .chain(hidden.values())
                .map(|asset| asset.bytes.len())
                .sum(),
        );
        self.decoded = self.decoded.max(
            images
                .values()
                .chain(hidden.values())
                .map(|asset| asset.decoded)
                .sum(),
        );
    }
    /// The budget a committed page has left: its images count as made requests and their
    /// bytes as charged, so images scripts insert later share one page-wide bound.
    pub(super) fn spent(images: &BTreeMap<usize, ImageAsset>) -> Self {
        Self {
            requests: images.len(),
            bytes: images.values().map(|asset| asset.bytes.len()).sum(),
            decoded: images.values().map(|asset| asset.decoded).sum(),
            fetched: std::collections::HashMap::new(),
        }
    }
    /// An image this page already fetched from `source`, reused without another request: Some
    /// with the image (or None when its bytes no longer fit the byte budgets, which still count
    /// every copy), or None when the address is new.
    pub(super) fn reused(&mut self, source: &str) -> Option<Option<Fetched>> {
        let seen = self.fetched.get(source.trim())?.clone();
        Some(self.admit(&seen).then_some(seen))
    }
    /// Remembers what `source` fetched, for `reused`.
    pub(super) fn remember(&mut self, source: &str, fetched: &Fetched) {
        self.fetched
            .insert(source.trim().to_owned(), fetched.clone());
    }
    /// Whether no request is left to make.
    pub(super) fn exhausted(&self) -> bool {
        self.requests >= IMAGE_COUNT
    }
    /// Claims one request; false once IMAGE_COUNT have been made.
    pub(super) fn request(&mut self) -> bool {
        let open = self.requests < IMAGE_COUNT;
        self.requests += usize::from(open);
        open
    }
    /// Charges a fetched image; false, charging nothing, when it would pass a byte budget.
    pub(super) fn admit(&mut self, fetched: &Fetched) -> bool {
        let (bytes, decoded) = match fetched {
            Fetched::Raster(asset, (w, h)) => {
                (asset.bytes.len(), u64::from(*w) * u64::from(*h) * 4)
            }
            Fetched::Vector(markup, _) => (markup.len(), 0),
        };
        let bytes = self.bytes.saturating_add(bytes);
        let decoded = self.decoded.saturating_add(decoded);
        let fits = bytes <= IMAGE_TOTAL_BYTES && decoded <= IMAGE_DECODED_BYTES as u64;
        if fits {
            (self.bytes, self.decoded) = (bytes, decoded);
        }
        fits
    }
}

/// The CSS px size of a bitmap drawn at `density` pixels per px, as a srcset choice is: a 1600w
/// candidate picked for an 800px slot lays out 800px wide, not 1600.
pub(super) fn css_size((width, height): (u32, u32), density: f64) -> (u32, u32) {
    let scale = |pixels: u32| {
        (f64::from(pixels) / density)
            .round()
            .clamp(1.0, f64::from(IMAGE_DIMENSION)) as u32
    };
    (scale(width), scale(height))
}

/** Drawing markup as the data URL Eye recognizes; `%` is the only character it must escape. */
pub(super) fn svg_url(markup: &str) -> String {
    format!("{SVG_DATA}{}", markup.replace('%', "%25"))
}

impl super::supervisor::Runtime {
    /**
     * Fetch the images of the nodes `wanted` selects under `budget`: each visible img gets the
     * source HTML would pick at this viewport (picture sources, srcset and sizes, src, lazy-load
     * attributes), then each selected element's first CSS background image, which needs the
     * document already styled. A failed img shows its alt text. Skipped work is described in
     * `notes`; authority and engine-service failures, and the deadline, return an error.
     */
    pub(super) fn load_images(
        &mut self,
        document: &mut Document,
        base: &url::Url,
        deadline: std::time::Instant,
        budget: &mut Budget,
        wanted: &dyn Fn(&Node) -> bool,
        notes: &mut Vec<String>,
    ) -> EngineResult<BTreeMap<usize, ImageAsset>> {
        self.load_images_skipping(
            document,
            base,
            deadline,
            budget,
            wanted,
            notes,
            &std::collections::BTreeSet::new(),
        )
    }

    /** Lifecycle-managed foreground images are excluded without suppressing their CSS backgrounds. */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn load_images_skipping(
        &mut self,
        document: &mut Document,
        base: &url::Url,
        deadline: std::time::Instant,
        budget: &mut Budget,
        wanted: &dyn Fn(&Node) -> bool,
        notes: &mut Vec<String>,
        skip: &std::collections::BTreeSet<usize>,
    ) -> EngineResult<BTreeMap<usize, ImageAsset>> {
        let viewport = self.render_context.width;
        let sources = document
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.tag == "img"
                    && !node.hidden()
                    && wanted(node)
                    && !skip.contains(&node.local_id)
            })
            .map(|(position, node)| (position, sources(document, node, viewport)))
            .collect::<Vec<_>>();
        let backgrounds = crate::document::background_urls(document, base.as_str())
            .into_iter()
            // A later layer's key names its element; the canvas's is always wanted.
            .filter(|(id, _)| {
                crate::document::background_owner(*id).is_none_or(|owner| {
                    document
                        .nodes
                        .iter()
                        .find(|node| node.local_id == owner)
                        .is_some_and(wanted)
                })
            })
            .collect::<Vec<_>>();
        let mut images = BTreeMap::new();
        for (position, candidates) in sources {
            let mut fetched = None;
            for (src, density) in candidates {
                fetched = self
                    .read_image(base, &src, deadline, budget, notes)?
                    .map(|fetched| {
                        (
                            fetched,
                            density,
                            base.join(src.trim()).ok().map(|url| url.to_string()),
                        )
                    });
                if fetched.is_some() {
                    break;
                }
            }
            let node = &mut document.nodes[position];
            match fetched {
                Some((Fetched::Raster(mut asset, size), density, selected)) => {
                    asset.image_source = selected.map(|url| (url, density));
                    node.image_size = Some(css_size(size, density));
                    images.insert(node.local_id, asset);
                }
                Some((Fetched::Vector(markup, size), _, _)) => {
                    node.image_ratio_free = !vector_ratio(&markup);
                    node.image_src = Some(svg_url(&markup));
                    node.image_size = Some(size);
                }
                None => node.text = node.image_alt.clone(),
            }
        }
        for (id, url) in backgrounds {
            if let Some(fetched) = self.read_image(base, &url, deadline, budget, notes)? {
                images.insert(id, fetched.into_asset());
            }
        }
        self.load_fonts(document, base, deadline, notes)?;
        Ok(images)
    }
    /**
     * Fetches the font files the styled document's text needs (`font_requests`: the first listed
     * family with `@font-face` rules, the face nearest each run's weight and style, only the
     * `unicode-range` subsets its characters use) and stores their unpacked sfnt bytes in the
     * document, within FONT_FILES files and FONT_TOTAL_BYTES. Fonts already loaded are kept. A
     * file that fails, is not a font, or is over budget leaves its family on the fallback face.
     */
    fn load_fonts(
        &mut self,
        document: &mut Document,
        base: &url::Url,
        deadline: std::time::Instant,
        notes: &mut Vec<String>,
    ) -> EngineResult<()> {
        let mut bytes: usize = document
            .formatting
            .fonts
            .iter()
            .map(|font| font.data.len() / 4 * 3)
            .sum();
        for request in font_requests(document) {
            if document.formatting.fonts.len() >= FONT_FILES {
                notes.push(format!(
                    "Fonts after the first {FONT_FILES} use fallback faces."
                ));
                break;
            }
            match self.fetch_font_retrying(base, &request.url, deadline) {
                Ok(sfnt) if bytes + sfnt.len() > FONT_TOTAL_BYTES => {
                    notes.push("Fonts past the page font budget use fallback faces.".into());
                }
                Ok(sfnt) => {
                    // One file often serves several faces (a variable font declared per
                    // weight, or per script); its bytes travel once, under the first.
                    let key = content_key(&sfnt);
                    let carried = document.formatting.fonts.iter().any(|font| font.key == key);
                    bytes += if carried { 0 } else { sfnt.len() };
                    document.formatting.fonts.push(LoadedFont {
                        face: request.face,
                        key,
                        data: if carried {
                            String::new()
                        } else {
                            base64(&sfnt)
                        },
                    });
                }
                Err(error) if stops_loading(&error) => return Err(error),
                Err(_) => {
                    notes.push("A web font failed to load; its text uses a fallback face.".into())
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(EngineError::navigation_timeout());
            }
        }
        Ok(())
    }
    /**
     * `fetch_font`, tried again up to FONT_ATTEMPTS times while the failure is transient (the
     * connection, not the file): under load a loopback fetch can drop, and a silently missing
     * font lays the page out in the fallback face. A refused or non-font file is final.
     */
    fn fetch_font_retrying(
        &mut self,
        base: &url::Url,
        source: &str,
        deadline: std::time::Instant,
    ) -> EngineResult<Vec<u8>> {
        let mut attempt = 1;
        loop {
            match self.fetch_font(base, source, deadline) {
                Err(error)
                    if attempt < FONT_ATTEMPTS
                        && retryable(&error)
                        && std::time::Instant::now() < deadline =>
                {
                    attempt += 1;
                }
                result => return result,
            }
        }
    }
    /** One font file under the page's authority, unpacked to sfnt bytes. */
    fn fetch_font(
        &mut self,
        base: &url::Url,
        source: &str,
        deadline: std::time::Instant,
    ) -> EngineResult<Vec<u8>> {
        let url = base
            .join(source.trim())
            .map_err(|_| EngineError::invalid("Invalid font URL."))?;
        let request = crate::network::WebRequest {
            destination: crate::network::Destination::Font,
            ..crate::network::WebRequest::get(url.as_str())
        };
        let acquired = self.acquire(request, deadline, Some(base.clone()), false, true)?;
        let response = acquired.response;
        if response.status != HTTP_OK || !response.image_data {
            return Err(EngineError::unsupported("The font could not be loaded."));
        }
        sfnt(&decode_base64_within(&response.source, FONT_FILE_BYTES)?)
    }
    /**
     * A single image under the page budget, using the existing exact-origin policy, deadline and
     * service authority; `data:` images decode locally. None, noted, past IMAGE_COUNT or the
     * byte budgets, and None on an ordinary failure.
     */
    pub(super) fn read_image(
        &mut self,
        base: &url::Url,
        source: &str,
        deadline: std::time::Instant,
        budget: &mut Budget,
        notes: &mut Vec<String>,
    ) -> EngineResult<Option<Fetched>> {
        if let Some(seen) = budget.reused(source) {
            return Ok(seen);
        }
        if !budget.request() {
            notes.push(format!(
                "Images after the first {IMAGE_COUNT} show their alt text."
            ));
            return Ok(None);
        }
        let fetched = if source
            .get(..5)
            .is_some_and(|s| s.eq_ignore_ascii_case("data:"))
        {
            data_url(source)
        } else {
            self.fetch_image(base, source, deadline)
        };
        if std::time::Instant::now() >= deadline {
            return Err(EngineError::navigation_timeout());
        }
        match fetched {
            Ok(fetched) if budget.admit(&fetched) => {
                budget.remember(source, &fetched);
                Ok(Some(fetched))
            }
            Ok(_) => {
                notes.push("Images past the page image budget show their alt text.".into());
                Ok(None)
            }
            Err(error) if stops_loading(&error) => Err(error),
            Err(_) => Ok(None),
        }
    }
    /// One network image under the page's authority. A `.svg` address is fetched as text and
    /// reduced to a drawing; anything else must arrive as bitmap bytes Relay admits. An address
    /// without the extension that Relay refuses as a bitmap is asked for once more as text, so
    /// an SVG served by type alone (LinkedIn's hero illustration) still draws; the second ask is
    /// answered from the HTTP cache when the response allowed caching.
    pub(super) fn fetch_image(
        &mut self,
        base: &url::Url,
        source: &str,
        deadline: std::time::Instant,
    ) -> EngineResult<Fetched> {
        let url = base
            .join(source.trim())
            .map_err(|_| EngineError::invalid("Invalid image URL."))?;
        let bitmap = !svg_address(&url);
        match self.fetch_image_as(base, &url, bitmap, deadline) {
            Err(error) if bitmap && error.code == UNSUPPORTED && error.service.is_none() => {
                self.fetch_image_as(base, &url, false, deadline)
            }
            fetched => fetched,
        }
    }
    /** One image request, as bitmap bytes or as SVG text. */
    fn fetch_image_as(
        &mut self,
        base: &url::Url,
        url: &url::Url,
        bitmap: bool,
        deadline: std::time::Instant,
    ) -> EngineResult<Fetched> {
        let (response, origin_clean) = canvas::fetch(self, base, url, bitmap, deadline)?;
        if response.status != HTTP_OK {
            return Err(EngineError::unsupported("The image could not be loaded."));
        }
        if response.image_data {
            let mut fetched = raster(response.mime, decode_base64(&response.source)?)?;
            if let Fetched::Raster(asset, _) = &mut fetched {
                asset.origin_clean = origin_clean;
            }
            Ok(fetched)
        } else {
            vector(&response.mime, &response.source)
        }
    }
}
/// The sfnt bytes of a font file: WOFF2 and WOFF 1.0 containers are unpacked, TrueType and CFF
/// OpenType pass through; anything else is refused. The unpacked size is bounded by the page
/// font budget before CoreText sees it.
pub(super) fn sfnt(bytes: &[u8]) -> EngineResult<Vec<u8>> {
    let unpacked = match bytes.get(..4) {
        Some(b"wOF2") => {
            woff2_patched::convert_woff2_to_ttf(&mut &bytes[..]).map_err(|_| font_invalid())?
        }
        Some(b"wOFF") => woff1(bytes)?,
        Some([0, 1, 0, 0] | b"OTTO" | b"true") => bytes.to_vec(),
        _ => return Err(font_invalid()),
    };
    if unpacked.len() > FONT_TOTAL_BYTES {
        return Err(EngineError::limit("font bytes", FONT_TOTAL_BYTES));
    }
    Ok(unpacked)
}
/// WOFF 1.0 (W3C REC): a header, a table directory and per-table zlib streams, rebuilt into an
/// sfnt with 4-byte aligned tables. Lengths are checked against the file before any read.
fn woff1(bytes: &[u8]) -> EngineResult<Vec<u8>> {
    let u32_at = |at: usize| -> EngineResult<usize> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or_else(font_invalid)
    };
    let u16_at = |at: usize| -> EngineResult<usize> {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
            .ok_or_else(font_invalid)
    };
    let tables = u16_at(12)?;
    if tables == 0 || tables > FONT_TABLES || bytes.len() < WOFF_HEADER + tables * WOFF_ENTRY {
        return Err(font_invalid());
    }
    let mut out = Vec::with_capacity(bytes.len() * 2);
    out.extend_from_slice(&bytes[4..8]);
    out.extend_from_slice(&(tables as u16).to_be_bytes());
    let power = (tables as u32).ilog2();
    let search = 16u16 << power;
    out.extend_from_slice(&search.to_be_bytes());
    out.extend_from_slice(&(power as u16).to_be_bytes());
    out.extend_from_slice(&(tables as u16 * 16 - search).to_be_bytes());
    let mut data = Vec::new();
    let mut offset = SFNT_HEADER + tables * SFNT_RECORD;
    for index in 0..tables {
        let entry = WOFF_HEADER + index * WOFF_ENTRY;
        let (start, compressed, original) =
            (u32_at(entry + 4)?, u32_at(entry + 8)?, u32_at(entry + 12)?);
        let table = bytes
            .get(start..start + compressed)
            .ok_or_else(font_invalid)?;
        let table = if compressed < original {
            inflate(table, original)?
        } else {
            table.to_vec()
        };
        if table.len() != original || data.len() + original > FONT_TOTAL_BYTES {
            return Err(font_invalid());
        }
        out.extend_from_slice(&bytes[entry..entry + 4]);
        out.extend_from_slice(&bytes[entry + 16..entry + 20]);
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(original as u32).to_be_bytes());
        data.extend_from_slice(&table);
        data.resize(data.len().next_multiple_of(4), 0);
        offset += table.len().next_multiple_of(4);
    }
    out.extend_from_slice(&data);
    Ok(out)
}
/** One zlib stream, read to exactly `size` bytes. */
fn inflate(table: &[u8], size: usize) -> EngineResult<Vec<u8>> {
    let mut out = Vec::with_capacity(size);
    flate2::read::ZlibDecoder::new(table)
        .take(size as u64)
        .read_to_end(&mut out)
        .map_err(|_| font_invalid())?;
    Ok(out)
}
/** Font errors never expose bytes, URLs or decoder internals. */
fn font_invalid() -> EngineError {
    EngineError::unsupported("The font file is not a supported font.")
}
/** Standard base64 with padding, the encoding the document carries font bytes in. */
pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((value >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/** Authority, ownership and engine-service failures stop image loading; others show alt text. */
pub(super) fn stops_loading(error: &EngineError) -> bool {
    error.service.is_some()
        || ["permission_denied", "control_denied"].contains(&error.code.as_str())
}
/** Whether a font fetch may succeed if tried again: not a refusal, a bad file or a bad URL. */
fn retryable(error: &EngineError) -> bool {
    !stops_loading(error) && ![UNSUPPORTED, INVALID, LIMIT].contains(&error.code.as_str())
}
/// Tries one font file gets before its family falls back.
const FONT_ATTEMPTS: usize = 3;
/** The code an unparseable font URL fails with. */
const INVALID: &str = "invalid_request";
/** The code a font past a byte budget fails with. */
const LIMIT: &str = "limit_exceeded";
/** The code Relay refuses a non-bitmap image body with. */
const UNSUPPORTED: &str = "unsupported_feature";
/** The only status whose body is the image. */
const HTTP_OK: u16 = 200;

/** The data URL prefix Eye recognizes as a drawing it renders itself. */
pub(super) const SVG_DATA: &str = "data:image/svg+xml,";
/** The SVG media type. */
const SVG_MIME: &str = "image/svg+xml";
/** The size HTML gives an svg with neither dimensions nor a viewBox. */
const SVG_DEFAULT: (f64, f64) = (300.0, 150.0);

/** Bitmap bytes held to the decoder bounds. */
pub(super) fn raster(mime: String, bytes: Vec<u8>) -> EngineResult<Fetched> {
    // Browsers pick an image's format from its leading bytes, not its declared type: YouTube's
    // placeholder is PNG data labeled image/gif, and every one of them failed to decode.
    let mime = sniffed(&bytes).map_or(mime, str::to_owned);
    let size = dimensions(&mime, &bytes)?;
    Ok(Fetched::Raster(
        ImageAsset {
            mime,
            bytes: std::sync::Arc::new(bytes),
            decoded: u64::from(size.width) * u64::from(size.height) * 4,
            image_source: None,
            origin_clean: false,
        },
        (size.width, size.height),
    ))
}

/// An SVG document parsed by Eye, reduced to the drawing subset Eye paints (svg, g, path, use)
/// and carried as a data URL. Text past IMAGE_BYTES, or a reduced drawing whose URL would pass
/// TEXT_BYTES (the bound on any node value), is refused rather than truncated.
pub(super) fn vector(mime: &str, text: &str) -> EngineResult<Fetched> {
    if mime != SVG_MIME || text.len() > IMAGE_BYTES {
        return Err(invalid());
    }
    let document = crate::document::parse(text).map_err(|_| invalid())?;
    let svg = document
        .nodes
        .iter()
        .find(|node| node.tag == "svg")
        .ok_or_else(invalid)?;
    let mut markup = String::new();
    serialize(&document, svg, &mut markup);
    if svg_url(&markup).len() > TEXT_BYTES {
        return Err(invalid());
    }
    Ok(Fetched::Vector(markup, svg_size(svg)))
}

/** One element and its drawing descendants as markup, attributes escaped. */
fn serialize(document: &Document, node: &Node, out: &mut String) {
    out.push('<');
    out.push_str(&node.tag);
    for (name, value) in &presentation(node) {
        out.push_str(&format!(" {name}=\""));
        for c in value.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '"' => out.push_str("&quot;"),
                '<' => out.push_str("&lt;"),
                _ => out.push(c),
            }
        }
        out.push('"');
    }
    out.push('>');
    for child in document
        .nodes
        .iter()
        .filter(|child| child.parent == Some(node.local_id))
    {
        serialize(document, child, out);
    }
    out.push_str(&format!("</{}>", node.tag));
}

/** Paint properties Eye's drawing reads from attributes; illustrators often set them in `style`. */
const PRESENTATION: [&str; 4] = ["fill", "stroke", "stroke-width", "stroke-linejoin"];

/// A drawing element's attributes with its `style` paint declarations lifted into presentation
/// attributes, which they override as in CSS. Without this an illustration that writes
/// `style="fill: #fbf1e2"` (LinkedIn's hero) paints every shape in the default black.
fn presentation(node: &Node) -> BTreeMap<String, String> {
    let mut attributes = node.attributes.clone();
    let style = attributes.get("style").cloned().unwrap_or_default();
    for declaration in style.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
        if PRESENTATION.contains(&name.as_str()) && !value.is_empty() {
            attributes.insert(name, value.to_owned());
        }
    }
    attributes
}

/// Whether reduced SVG markup has a natural aspect ratio: its root carries a viewBox, or both a
/// width and a height (the serializer writes every root attribute into its start tag).
fn vector_ratio(markup: &str) -> bool {
    let root = markup.split('>').next().unwrap_or_default();
    // A positive length; a zero or a percentage gives no ratio.
    let has = |name: &str| {
        let key = format!(" {name}=\"");
        root.split_once(&key)
            .and_then(|(_, rest)| rest.split('"').next())
            .and_then(|value| value.trim().trim_end_matches("px").parse::<f64>().ok())
            .is_some_and(|value| value > 0.0)
    };
    root.contains(" viewbox=\"") || (has("width") && has("height"))
}

/// The intrinsic size of an svg: its width and height, one of them with the viewBox ratio, the
/// viewBox size, or 300x150; percentages count as absent. Clamped to 1..=IMAGE_DIMENSION.
fn svg_size(svg: &Node) -> (u32, u32) {
    let number = |name: &str| {
        let value = svg.attributes.get(name)?.trim();
        let value = value.strip_suffix("px").unwrap_or(value);
        value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let view = svg.attributes.get("viewbox").and_then(|value| {
        let parts = value
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect::<Option<Vec<_>>>()?;
        (parts.len() == 4 && parts[2] > 0.0 && parts[3] > 0.0).then(|| (parts[2], parts[3]))
    });
    let (width, height) = match (number("width"), number("height"), view) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        (_, _, Some(view)) => view,
        _ => SVG_DEFAULT,
    };
    let clamp = |v: f64| v.round().clamp(1.0, f64::from(IMAGE_DIMENSION)) as u32;
    (clamp(width), clamp(height))
}

/// An image carried in a `data:` URL: base64 or percent-encoded bytes, a bitmap or a drawing,
/// under the same bounds as fetched images. Nothing is fetched.
pub(super) fn data_url(src: &str) -> EngineResult<Fetched> {
    let (mime, bytes) = data_body(src, crate::constants::IMAGE_BYTES)?;
    if mime == SVG_MIME {
        vector(&mime, std::str::from_utf8(&bytes).map_err(|_| invalid())?)
    } else {
        let mut fetched = raster(mime, bytes)?;
        if let Fetched::Raster(asset, _) = &mut fetched {
            asset.origin_clean = true;
        }
        Ok(fetched)
    }
}

/**
 * A `data:` URL's media type essence (lowercase, parameters such as `charset` dropped) and its
 * decoded bytes, base64 or percent-encoded, refused past `limit` bytes. Images, stylesheets and
 * scripts all decode here; each caller applies its own type checks, as to a fetched body.
 */
pub(super) fn data_body(src: &str, limit: usize) -> EngineResult<(String, Vec<u8>)> {
    let rest = src
        .get(..5)
        .filter(|scheme| scheme.eq_ignore_ascii_case("data:"))
        .map(|_| &src[5..])
        .ok_or_else(invalid)?;
    let (meta, payload) = rest.split_once(',').ok_or_else(invalid)?;
    let meta = meta.to_ascii_lowercase();
    let mime = meta.split(';').next().unwrap_or("").trim().to_owned();
    let base64 = meta.contains(';') && meta.rsplit(';').next().map(str::trim) == Some("base64");
    let bytes = if base64 {
        let compact = payload
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect::<String>();
        decode_base64_within(
            &percent_decode(&compact).map(char::from).collect::<String>(),
            limit,
        )?
    } else {
        percent_decode(payload).collect()
    };
    if bytes.len() > limit {
        return Err(invalid());
    }
    Ok((mime, bytes))
}

/** URL percent-decoding; a `%` not followed by two hex digits stays literal. */
fn percent_decode(text: &str) -> impl Iterator<Item = u8> + '_ {
    let bytes = text.as_bytes();
    let mut at = 0;
    std::iter::from_fn(move || {
        let byte = *bytes.get(at)?;
        let hex = bytes
            .get(at + 1..at + 3)
            .filter(|pair| pair.iter().all(u8::is_ascii_hexdigit))
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match hex {
            Some(value) if byte == b'%' => {
                at += 3;
                Some(value)
            }
            _ => {
                at += 1;
                Some(byte)
            }
        }
    })
}

/** An address whose path ends in `.svg` is fetched as text; Relay passes only bitmaps as image bytes. */
pub(super) fn svg_address(url: &url::Url) -> bool {
    url.path().to_ascii_lowercase().ends_with(".svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    /** The img in a parsed fragment, and its document. */
    fn img(html: &str) -> (Document, usize) {
        let document = crate::document::parse(html).unwrap();
        let position = document.nodes.iter().position(|n| n.tag == "img").unwrap();
        (document, position)
    }

    /** Paint set in a drawing element's style reaches the drawing as an attribute, over the attribute. */
    #[test]
    fn style_paint_becomes_presentation_attributes() {
        let svg = "<svg width=\"4\" height=\"4\"><path fill=\"red\" style=\"fill: #fbf1e2; stroke:blue\" d=\"M0,0H4V4Z\"/></svg>";
        let Ok(Fetched::Vector(markup, _)) = vector(SVG_MIME, svg) else {
            panic!("the drawing parses");
        };
        assert!(markup.contains("fill=\"#fbf1e2\""), "{markup}");
        assert!(markup.contains("stroke=\"blue\""), "{markup}");
    }

    /** The source the img picks at this viewport. */
    fn pick(html: &str, viewport: f64) -> Option<(String, f64)> {
        let (document, position) = img(html);
        sources(&document, &document.nodes[position], viewport)
            .into_iter()
            .next()
    }

    /** Width descriptors resolve against sizes, then Chromium's geometric-mean walk at 2x. */
    #[test]
    fn width_descriptors_pick_the_candidate_for_the_slot_at_2x() {
        let html = "<img sizes='(min-width: 768px) 50vw, 100vw' srcset='a 240w, b 480w, c 800w, d 1024w, e 1536w'>";
        let (url, density) = pick(html, 1100.0).unwrap();
        assert_eq!(url, "d");
        assert!((density - 1024.0 / 550.0).abs() < 1e-9);
        assert_eq!(pick(html, 300.0).unwrap().0, "b");
    }

    /** Density descriptors pick 2x; a bare candidate is 1x. */
    #[test]
    fn density_descriptors_pick_2x() {
        assert_eq!(
            pick("<img src=a srcset='b, c 2x, d 3x'>", 800.0).unwrap(),
            ("c".into(), 2.0)
        );
    }

    /** URLs with commas survive, and malformed candidates are dropped. */
    #[test]
    fn srcset_parsing_keeps_commas_in_urls() {
        let set = parse_srcset("https://x/a,w_100.jpg 1x, bad 1q, https://x/b.jpg 2x");
        assert_eq!(set.len(), 2);
        assert_eq!(set[0].0, "https://x/a,w_100.jpg");
    }

    /** Picture sources match by type and media, skipping AVIF, before the img's own src. */
    #[test]
    fn picture_takes_the_first_supported_matching_source() {
        let html = "<picture><source type='image/avif' srcset='a.avif'><source media='(max-width: 600px)' srcset='small.webp'><source type='image/webp' srcset='big.webp'><img src='fallback.jpg'></picture>";
        assert_eq!(pick(html, 1100.0).unwrap().0, "big.webp");
        let (document, position) = img(html);
        let all = sources(&document, &document.nodes[position], 1100.0);
        assert_eq!(
            all.last().unwrap().0,
            "fallback.jpg",
            "src stays as the fallback"
        );
        assert_eq!(pick(html, 500.0).unwrap().0, "small.webp");
    }

    /** Lazy-load attributes replace an empty or data: placeholder, never a real src. */
    #[test]
    fn lazy_attributes_replace_placeholders_only() {
        assert_eq!(
            pick(
                "<img src='data:image/gif;base64,R0lGOD' data-src='real.jpg'>",
                800.0
            )
            .unwrap()
            .0,
            "real.jpg"
        );
        assert_eq!(
            pick("<img data-srcset='a 1x, b 2x'>", 800.0).unwrap().0,
            "b"
        );
        assert_eq!(
            pick("<img src='real.jpg' data-src='other.jpg'>", 800.0)
                .unwrap()
                .0,
            "real.jpg"
        );
        assert_eq!(pick("<img alt=x>", 800.0), None);
    }

    /** A named placeholder directly before the real img is not shown; a lone one is. */
    #[test]
    fn script_swapped_placeholders_are_skipped() {
        let html = "<div><img id=p src='/grey-placeholder.png'><img id=r src='photo.jpg'></div>";
        let document = crate::document::parse(html).unwrap();
        let placeholder = document.nodes.iter().find(|n| n.html_id == "p").unwrap();
        assert!(sources(&document, placeholder, 800.0).is_empty());
        assert_eq!(
            pick("<img src='/avatar-placeholder.png'>", 800.0)
                .unwrap()
                .0,
            "/avatar-placeholder.png"
        );
    }

    /** Media lists evaluate width, range, resolution and color scheme; unknown features fail. */
    #[test]
    fn media_queries_evaluate_the_supported_features() {
        assert!(media_matches("screen and (min-width: 768px)", 1100.0));
        assert!(!media_matches("(max-width: 40em)", 1100.0));
        assert!(media_matches("(width >= 600px)", 1100.0));
        assert!(media_matches("(min-resolution: 2dppx)", 1100.0));
        assert!(!media_matches("(prefers-color-scheme: dark)", 1100.0));
        assert!(!media_matches("(hover: hover)", 1100.0));
        assert!(media_matches("print, (min-width: 1px)", 1100.0));
    }

    /** WebP (all three chunk kinds) and GIF report their size from the header. */
    #[test]
    fn webp_and_gif_headers_give_their_size() {
        let mut lossy = b"RIFF\0\0\0\0WEBPVP8 \0\0\0\0\0\0\0\x9d\x01\x2a".to_vec();
        lossy.extend([0x40, 0x01, 0xf0, 0x00]);
        assert_eq!(size("image/webp", &lossy), Some((320, 240)));
        let mut extended = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        extended.extend([99, 0, 0, 49, 0, 0]);
        assert_eq!(size("image/webp", &extended), Some((100, 50)));
        let lossless = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f\x09\x40\x06\x00".to_vec();
        assert_eq!(size("image/webp", &lossless), Some((10, 26)));
        assert_eq!(size("image/gif", b"GIF89a\x10\x00\x08\x00"), Some((16, 8)));
    }

    /** Truncated, mislabeled and oversized headers are refused without panicking. */
    #[test]
    fn hostile_headers_are_refused() {
        for bytes in [
            &b"RIFF\0\0\0\0WEBPVP8 "[..],
            b"RIFF",
            b"GIF89a\x01",
            b"",
            b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0\xff\xff\xff\xff\xff\xff",
        ] {
            assert!(dimensions("image/webp", bytes).is_err());
            assert!(dimensions("image/gif", bytes).is_err());
        }
        assert!(dimensions("image/avif", b"anything").is_err());
    }

    /** One header's decoded size. */
    fn size(mime: &str, bytes: &[u8]) -> Option<(u32, u32)> {
        dimensions(mime, bytes).ok().map(|d| (d.width, d.height))
    }

    /** An SVG has a natural aspect ratio from a viewBox or from both a positive width and a
     * positive height; a lone height, or a zero width, gives none. */
    #[test]
    fn svg_ratio_needs_a_view_box_or_both_positive_sizes() {
        let ratio = |text: &str| {
            let Ok(Fetched::Vector(markup, _)) = vector("image/svg+xml", text) else {
                panic!("expected a drawing");
            };
            vector_ratio(&markup)
        };
        let svg = |attributes: &str| {
            format!("<svg xmlns='http://www.w3.org/2000/svg' {attributes}></svg>")
        };
        assert!(ratio(&svg("viewBox='0 0 5 10' height='20'")));
        assert!(ratio(&svg("width='10' height='20'")));
        assert!(!ratio(&svg("height='20px'")));
        assert!(!ratio(&svg("width='0px' height='20px'")));
    }

    /** SVG reduces to the drawing subset and takes its size from width, height and viewBox. */
    #[test]
    fn svg_reduces_to_its_drawing_and_sizes_from_the_view_box() {
        let text = "<?xml version='1.0'?><svg xmlns='http://www.w3.org/2000/svg' width='40' viewBox='0 0 20 10'><defs><linearGradient id='g'/></defs><title>t</title><path d='M0 0h20v10z' fill='#f00'/></svg>";
        let Ok(Fetched::Vector(markup, size)) = vector("image/svg+xml", text) else {
            panic!("expected a drawing");
        };
        assert_eq!(size, (40, 20));
        assert!(markup.contains("<path d=\"M0 0h20v10z\" fill=\"#f00\"></path>"));
        assert!(!markup.contains("linearGradient"));
        assert!(svg_url(&markup).starts_with(SVG_DATA));
    }

    /** data: images decode base64 bitmaps and percent-encoded drawings locally. */
    #[test]
    fn mislabeled_bitmaps_decode_as_what_their_bytes_are() {
        // YouTube's 1x1 placeholder: PNG bytes under an image/gif label.
        let png = data_url(
            "data:image/gif;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mN8XA8AAksBZG7LpHYAAAAASUVORK5CYII=",
        );
        assert!(matches!(png, Ok(Fetched::Raster(ref asset, (1, 1))) if asset.mime == "image/png"));
        assert_eq!(sniffed(b"GIF89a...."), Some("image/gif"));
        assert_eq!(sniffed(b"plain text"), None);
    }

    #[test]
    fn data_urls_decode_bitmaps_and_drawings() {
        let gif = data_url("data:image/gif;base64,R0lGODlhAQABAAAAACw=").ok();
        assert!(matches!(gif, Some(Fetched::Raster(_, (1, 1)))));
        let svg = data_url("data:image/svg+xml,%3Csvg%20viewBox='0 0 8 4'%3E%3C/svg%3E").ok();
        assert!(matches!(svg, Some(Fetched::Vector(_, (8, 4)))));
        assert!(data_url("data:text/html,<b>").is_err());
        assert!(data_url("data:image/png;base64,!!!").is_err());
    }

    /** A repeated address reuses its image without a request, so a page drawing many copies of
     * a few files is not cut off at IMAGE_COUNT elements (border-conflict-element-001d-ref). */
    #[test]
    fn repeated_addresses_reuse_their_image() {
        let mut budget = Budget::default();
        let gif = data_url("data:image/gif;base64,R0lGODlhAQABAAAAACw=").unwrap();
        assert!(budget.reused("swatch.png").is_none());
        budget.remember("swatch.png", &gif);
        let copies =
            (0..IMAGE_COUNT * 2).filter(|_| matches!(budget.reused(" swatch.png"), Some(Some(_))));
        assert_eq!(copies.count(), IMAGE_COUNT * 2);
        assert!(!budget.exhausted());
    }
    /** A srcset choice lays out at CSS size, and the page budget refuses past its bounds. */
    #[test]
    fn density_scales_size_and_budget_bounds_the_page() {
        assert_eq!(css_size((1024, 576), 1024.0 / 512.0), (512, 288));
        let mut budget = Budget::default();
        assert!((0..IMAGE_COUNT).all(|_| budget.request()));
        assert!(!budget.request());
        let huge = || {
            Fetched::Raster(
                ImageAsset {
                    mime: "image/png".into(),
                    bytes: Default::default(),
                    decoded: 0,
                    image_source: None,
                    origin_clean: false,
                },
                (SIDE, SIDE),
            )
        };
        const SIDE: u32 = 2048;
        let pixels = u64::from(SIDE).pow(2) * 4;
        assert!((0..IMAGE_DECODED_BYTES as u64 / pixels).all(|_| budget.admit(&huge())));
        assert!(!budget.admit(&huge()));
        let mut images = BTreeMap::new();
        images.insert(
            1,
            ImageAsset {
                mime: "image/png".into(),
                bytes: std::sync::Arc::new(vec![0; IMAGE_TOTAL_BYTES]),
                decoded: 4,
                image_source: None,
                origin_clean: false,
            },
        );
        let mut spent = Budget::spent(&images);
        let one = Fetched::Vector("<svg></svg>".into(), (1, 1));
        assert!(
            !spent.admit(&one),
            "the page total is shared with committed images"
        );
    }

    /** The one table of the test fonts: compressible, so its WOFF stream is shorter than it. */
    const TABLE: [u8; 64] = [b'a'; 64];
    /** A minimal sfnt: one `test` table, as WOFF 1.0 unpacking must rebuild it. */
    fn tiny_sfnt() -> Vec<u8> {
        let mut out = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
        out.extend_from_slice(b"test");
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&28u32.to_be_bytes());
        out.extend_from_slice(&(TABLE.len() as u32).to_be_bytes());
        out.extend_from_slice(&TABLE);
        out
    }
    /** The same font wrapped as WOFF 1.0 with its table zlib-compressed. */
    fn tiny_woff() -> Vec<u8> {
        use std::io::Write;
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&TABLE).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(b"wOFF");
        out.extend_from_slice(&[0, 1, 0, 0]);
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&[0; 2]);
        out.extend_from_slice(&[0; 4 + 2 + 2 + 4 + 4 + 4 + 4 + 4]);
        let start = WOFF_HEADER + WOFF_ENTRY;
        out.extend_from_slice(b"test");
        out.extend_from_slice(&(start as u32).to_be_bytes());
        out.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
        out.extend_from_slice(&(TABLE.len() as u32).to_be_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&compressed);
        out
    }

    /** WOFF 1.0 unpacks to exactly the sfnt it wrapped. */
    #[test]
    fn woff1_unpacks_to_the_wrapped_sfnt() {
        assert_eq!(sfnt(&tiny_woff()).unwrap(), tiny_sfnt());
    }

    /** A dropped or timed-out font fetch is tried again; a refused, bad or oversized file is not. */
    #[test]
    fn only_transient_font_failures_retry() {
        assert!(retryable(&EngineError::navigation_timeout()));
        assert!(!retryable(&font_invalid()));
        assert!(!retryable(&EngineError::invalid("Invalid font URL.")));
        assert!(!retryable(&EngineError::limit("font bytes", 1)));
    }

    /** TrueType and CFF OpenType bytes pass through; anything else is refused without detail. */
    #[test]
    fn sfnt_passes_fonts_and_refuses_the_rest() {
        assert_eq!(sfnt(&tiny_sfnt()).unwrap(), tiny_sfnt());
        assert!(sfnt(b"OTTO rest").is_ok());
        let error = sfnt(b"<html>not a font</html>").unwrap_err();
        assert!(!error.message.contains("html"));
        assert!(sfnt(b"wOFF").is_err(), "a truncated header is refused");
    }

    /** The font encoding matches RFC 4648's vectors, so Eye's decoder reads it back. */
    #[test]
    fn font_base64_matches_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
