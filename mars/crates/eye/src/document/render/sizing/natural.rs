/** The natural dimensions and ratio a replaced box brings before CSS sizes it (Blink's
 * GetNaturalDimensions): a decoded image's or canvas bitmap's size, an svg drawing's absolute
 * `width` and `height` attributes with its viewBox ratio (LayoutSVGRoot's
 * UnscaledNaturalSizingInfo, for an inline `<svg>` and for an img showing one), or a
 * size-contained box's `contain-intrinsic-size`. Either dimension, and the ratio, may be
 * missing. */
use crate::{constants::CSS_MAX_SCALAR, document::Node};

/** The data URL prefix of a drawing an img shows, as the image loader writes it. */
const SVG_DATA: &str = "data:image/svg+xml,";

/** What a replaced box's content says about its size. */
#[derive(Clone, Copy, Default)]
pub(super) struct Natural {
    /** The natural content width, if it has one. */
    pub width: Option<f64>,
    /** The natural content height, if it has one. */
    pub height: Option<f64>,
    /** The natural ratio, width over height, if it has one. */
    pub ratio: Option<f64>,
}

/** The natural dimensions of `node`; a size-contained box keeps only its stand-in size. */
pub(super) fn natural(node: &Node) -> Natural {
    if let Some((width, height)) = super::containment::contained_natural(node) {
        return Natural {
            width: Some(width),
            height: Some(height),
            ratio: None,
        };
    }
    if node.tag == "svg" {
        return svg(&|name| node.attributes.get(name).map(String::as_str));
    }
    if let Some(header) = svg_image_header(node) {
        return image_drawing(svg(&|name| attribute(&header, name)));
    }
    node.image_size
        .filter(|(w, h)| *w > 0 && *h > 0)
        .map(|(w, h)| (f64::from(w), f64::from(h)))
        .map_or_else(Natural::default, |(width, height)| Natural {
            width: Some(width),
            height: Some(height),
            ratio: Some(width / height),
        })
}

/** A drawing an img shows with a ratio but no width or height has no natural size; CSS Images 3
 * sizes that box as the default object size, 300 by 150, contained by the ratio. Other natural
 * sizes pass through unchanged. */
fn image_drawing(natural: Natural) -> Natural {
    let (default_width, default_height) = super::replaced::DEFAULT_NATURAL;
    match (natural.width, natural.height, natural.ratio) {
        (None, None, Some(ratio)) if ratio >= default_width / default_height => Natural {
            width: Some(default_width),
            height: Some(default_width / ratio),
            ratio: Some(ratio),
        },
        (None, None, Some(ratio)) => Natural {
            width: Some(default_height * ratio),
            height: Some(default_height),
            ratio: Some(ratio),
        },
        _ => natural,
    }
}

/** An svg drawing: absolute `width` and `height` attributes (a percentage, or none, gives no
 * natural size), and their ratio when neither is zero, else the viewBox's. */
fn svg<'a>(get: &dyn Fn(&str) -> Option<&'a str>) -> Natural {
    let width = get("width").and_then(length);
    let height = get("height").and_then(length);
    let ratio = match (width, height) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(w / h),
        _ => get("viewbox").and_then(view_box_ratio),
    };
    Natural {
        width,
        height,
        ratio,
    }
}

/** An absolute svg length attribute in px: a number, optionally suffixed `px`. */
fn length(value: &str) -> Option<f64> {
    let value = value.trim();
    let value = value.strip_suffix("px").unwrap_or(value);
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, CSS_MAX_SCALAR))
}

/** The viewBox's width over its height, when both are positive. */
fn view_box_ratio(source: &str) -> Option<f64> {
    let numbers: Vec<f64> = source
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<f64>().ok().filter(|value| value.is_finite()))
        .collect::<Option<_>>()?;
    match numbers[..] {
        [_, _, width, height] if width > 0.0 && height > 0.0 => Some(width / height),
        _ => None,
    }
}

/** The root `<svg ...>` start tag of the drawing an img shows, lowercased, if it shows one. The
 * loader serializes the drawing with only `%` escaped, so the tag is read without decoding the
 * rest of the markup. */
fn svg_image_header(node: &Node) -> Option<String> {
    let source = node.image_src.as_deref()?;
    let prefix = source.get(..SVG_DATA.len())?;
    if node.tag != "img" || !prefix.eq_ignore_ascii_case(SVG_DATA) {
        return None;
    }
    let markup = &source[SVG_DATA.len()..];
    let start = markup.find("<svg")?;
    let end = start + markup[start..].find('>')?;
    Some(markup[start..end].replace("%25", "%").to_ascii_lowercase())
}

/** One `name="value"` attribute of a serialized, lowercased start tag. */
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let length = tag[start..].find('"')?;
    Some(&tag[start..start + length])
}
