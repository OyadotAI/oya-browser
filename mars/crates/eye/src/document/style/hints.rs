/** HTML presentational attributes (bgcolor, width, align, cellpadding, font) as author-level hints below every rule. */
use crate::document::{Document, Node};
use std::collections::BTreeMap;

/** Legacy `<font size>` 1 to 7 as the keywords Chromium maps them to, so monospace text takes
the fixed-font sizes. */
const FONT_SIZES: [&str; 7] = [
    "x-small",
    "small",
    "medium",
    "large",
    "x-large",
    "xx-large",
    "xxx-large",
];
/** The default `<font size>` that relative sizes count from. */
const FONT_SIZE_BASE: i64 = 3;

/** Hints for one element; `document` locates its table for cell padding and borders. */
pub(super) fn hints(node: &Node, document: &Document) -> Vec<(&'static str, String)> {
    let tag = node.tag.as_str();
    let attrs = &node.attributes;
    let mut out = Vec::new();
    common(tag, attrs, &mut out);
    match tag {
        "table" => table(attrs, &mut out),
        "td" | "th" => cell(node, document, &mut out),
        "font" => font(attrs, &mut out),
        "svg" if outermost_svg(node, document) => svg_size(attrs, &mut out),
        "img" | "object" | "embed" | "iframe" => replaced(attrs, &mut out),
        "video" => {
            replaced(attrs, &mut out);
            video(attrs, &mut out);
        }
        _ => {}
    }
    out
}
/**
 * Where the parser keeps the first `<meta name=color-scheme>` content: a key no HTML attribute
 * can have (it holds a space), on the root wrapper's attributes.
 */
pub(crate) const META_COLOR_SCHEME: &str = "meta color-scheme";
/**
 * The root's hints: `<meta name=color-scheme>` sets the root's `color-scheme` as an author-level
 * hint, as Chrome does, so any author `color-scheme` on `:root` or `html` still wins.
 */
pub(super) fn html(attrs: &BTreeMap<String, String>) -> Vec<(&'static str, String)> {
    attrs
        .get(META_COLOR_SCHEME)
        .map(|content| content.trim().to_ascii_lowercase())
        .filter(|content| super::declaration::validate("color-scheme", content).is_ok())
        .map(|content| ("color-scheme", content))
        .into_iter()
        .collect()
}
/** Body attributes of the legacy page wrapper. */
pub(super) fn body(attrs: &BTreeMap<String, String>) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(color) = attrs.get("bgcolor") {
        out.push(("background-color", color.trim().to_string()));
    }
    if let Some(color) = attrs.get("text") {
        out.push(("color", color.trim().to_string()));
    }
    out
}
/** Colors, sizes and alignment shared by tables, cells and blocks. */
fn common(tag: &str, attrs: &BTreeMap<String, String>, out: &mut Vec<(&'static str, String)>) {
    if let Some(color) = attrs.get("bgcolor") {
        out.push(("background-color", color.trim().to_string()));
    }
    // HTML maps the dimension attributes of embedded content (iframe, embed, object) too.
    let embedded = ["iframe", "embed", "object"].contains(&tag);
    let sized = embedded || ["table", "td", "th", "col", "hr", "pre", "img"].contains(&tag);
    if sized && let Some(width) = attrs.get("width").and_then(|v| length(v)) {
        out.push(("width", width));
    }
    if (embedded || ["table", "td", "th", "tr", "img"].contains(&tag))
        && let Some(height) = attrs.get("height").and_then(|v| length(v))
    {
        out.push(("height", height));
    }
    let aligned = [
        "td", "th", "tr", "thead", "tbody", "tfoot", "div", "p", "h1", "h2", "h3", "h4", "h5",
        "h6", "caption",
    ];
    if aligned.contains(&tag)
        && let Some(align) = attrs.get("align").map(|v| v.trim().to_ascii_lowercase())
        && ["left", "center", "right", "justify"].contains(&align.as_str())
    {
        out.push(("text-align", align));
    }
    if ["td", "th"].contains(&tag)
        && let Some(valign) = attrs.get("valign").map(|v| v.trim().to_ascii_lowercase())
        && ["top", "middle", "bottom", "baseline"].contains(&valign.as_str())
    {
        out.push(("vertical-align", valign));
    }
    if ["td", "th"].contains(&tag) && attrs.contains_key("nowrap") {
        out.push(("white-space", "nowrap".into()));
    }
}
/** `border`, `align` and `cellspacing` on a table. */
fn table(attrs: &BTreeMap<String, String>, out: &mut Vec<(&'static str, String)>) {
    if let Some(border) = attrs.get("border").map(|v| number(v).unwrap_or(1)) {
        sides(
            out,
            &format!("{border}px"),
            if border > 0 { "outset" } else { "none" },
        );
    }
    if let Some(spacing) = attrs.get("cellspacing").and_then(|v| number(v)) {
        out.push(("border-spacing", format!("{spacing}px")));
    }
    match attrs
        .get("align")
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("center") => out.extend([
            ("margin-left", "auto".into()),
            ("margin-right", "auto".into()),
        ]),
        Some("left") => out.push(("float", "left".into())),
        Some("right") => out.push(("float", "right".into())),
        _ => {}
    }
}
/** Cells take `cellpadding` and a one pixel inset border from a bordered table. */
fn cell(node: &Node, document: &Document, out: &mut Vec<(&'static str, String)>) {
    let Some(table) = owning_table(node, document) else {
        return;
    };
    if let Some(padding) = table.attributes.get("cellpadding").and_then(|v| number(v)) {
        for side in [
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ] {
            out.push((side, format!("{padding}px")));
        }
    }
    if table
        .attributes
        .get("border")
        .is_some_and(|v| number(v).unwrap_or(1) > 0)
    {
        sides(out, "1px", "inset");
    }
}
/** The nearest table ancestor, found from parsed cell relationships or the parent chain. */
fn owning_table<'a>(node: &Node, document: &'a Document) -> Option<&'a Node> {
    let id = node.table_cell.as_ref().map(|cell| cell.table)?;
    mars_core::dom::local_position(&document.nodes, id, |node| node.local_id)
        .map(|position| &document.nodes[position])
}
/** An outermost `<svg>`'s `width` and `height` are presentation attributes for the CSS
 * properties (SVG 2 geometry properties; Blink's SVGSVGElement maps them only on the outermost
 * one), so CSS min and max sizes, `aspect-ratio` and keywords size the box around them. */
fn svg_size(attrs: &BTreeMap<String, String>, out: &mut Vec<(&'static str, String)>) {
    for name in ["width", "height"] {
        let Some(value) = attrs.get(name).map(|v| v.trim()) else {
            continue;
        };
        let number = value.strip_suffix("px").unwrap_or(value);
        if let Ok(px) = number.parse::<f64>()
            && px.is_finite()
            && px >= 0.0
        {
            out.push((name, format!("{px}px")));
        } else if let Some(percent) = value.strip_suffix('%').and_then(|v| v.parse::<f64>().ok())
            && percent.is_finite()
            && percent >= 0.0
        {
            out.push((name, format!("{percent}%")));
        }
    }
}
/** Whether no ancestor of this `<svg>` is an `<svg>`: an inner one is a viewport inside the
 * drawing, not a CSS box. */
fn outermost_svg(node: &Node, document: &Document) -> bool {
    let mut parent = node.parent;
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some(id) = parent else {
            return true;
        };
        let Some(position) = mars_core::dom::local_position(&document.nodes, id, |n| n.local_id)
        else {
            return true;
        };
        let ancestor = &document.nodes[position];
        if ancestor.tag == "svg" {
            return false;
        }
        parent = ancestor.parent;
    }
    true
}
/** `<font color size face>`. */
fn font(attrs: &BTreeMap<String, String>, out: &mut Vec<(&'static str, String)>) {
    if let Some(color) = attrs.get("color") {
        out.push(("color", color.trim().to_string()));
    }
    if let Some(face) = attrs.get("face") {
        out.push(("font-family", face.trim().to_string()));
    }
    if let Some(size) = attrs.get("size").map(|v| v.trim()) {
        let parsed = size.trim_start_matches(['+', '-']).parse::<i64>().ok();
        let level = match (size.chars().next(), parsed) {
            (Some('+'), Some(n)) => Some(FONT_SIZE_BASE + n),
            (Some('-'), Some(n)) => Some(FONT_SIZE_BASE - n),
            (_, Some(n)) => Some(n),
            _ => None,
        };
        if let Some(level) = level {
            let index = level.clamp(1, FONT_SIZES.len() as i64) as usize - 1;
            out.push(("font-size", FONT_SIZES[index].into()));
        }
    }
}
/** Replaced elements float with `align=left|right`. */
fn replaced(attrs: &BTreeMap<String, String>, out: &mut Vec<(&'static str, String)>) {
    match attrs
        .get("align")
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("left") => out.push(("float", "left".into())),
        Some("right") => out.push(("float", "right".into())),
        Some("middle" | "absmiddle" | "center") => out.push(("vertical-align", "middle".into())),
        _ => {}
    }
}
/** A video's `width` and `height`, and its `poster` frame as a contained, centered background,
 * which is what Chromium shows before playback (Wikipedia's and gnu.org's videos were empty). */
fn video(attrs: &BTreeMap<String, String>, out: &mut Vec<(&'static str, String)>) {
    for name in ["width", "height"] {
        if let Some(value) = attrs.get(name).and_then(|v| length(v)) {
            out.push((name, value));
        }
    }
    // Both attributes also give the box their ratio, so `width: 100%; height: auto` keeps it
    // (gnu.org's video collapsed to nothing).
    let pixels = |name: &str| attrs.get(name).and_then(|v| v.trim().parse::<f64>().ok());
    if let (Some(width), Some(height)) = (pixels("width"), pixels("height"))
        && width > 0.0
        && height > 0.0
    {
        out.push(("aspect-ratio", format!("auto {width} / {height}")));
    }
    let Some(poster) = attrs
        .get("poster")
        .map(|url| url.trim())
        .filter(|url| !url.is_empty())
    else {
        return;
    };
    let escaped = poster
        .replace(['\n', '\r'], "")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    out.extend([
        ("background-image", format!("url(\"{escaped}\")")),
        ("background-size", "contain".into()),
        ("background-position", "center".into()),
        ("background-repeat", "no-repeat".into()),
    ]);
}
/** All four border sides with one width and style, in the current color. */
fn sides(out: &mut Vec<(&'static str, String)>, width: &str, style: &str) {
    for (w, s, c) in [
        ("border-top-width", "border-top-style", "border-top-color"),
        (
            "border-right-width",
            "border-right-style",
            "border-right-color",
        ),
        (
            "border-bottom-width",
            "border-bottom-style",
            "border-bottom-color",
        ),
        (
            "border-left-width",
            "border-left-style",
            "border-left-color",
        ),
    ] {
        out.extend([(w, width.into()), (s, style.into()), (c, "gray".into())]);
    }
}
/** A non-negative integer attribute's leading digits. */
fn number(value: &str) -> Option<u32> {
    let digits: String = value
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse::<u32>().ok().map(|n| n.min(MAX_ATTRIBUTE))
}
/** Attribute pixel values clamp here so hints never exceed CSS bounds. */
const MAX_ATTRIBUTE: u32 = 100_000;
/** `N` as px or `N%` as a percentage. */
fn length(value: &str) -> Option<String> {
    let value = value.trim();
    let n = number(value)?;
    let rest = value.trim_start_matches(|c: char| c.is_ascii_digit());
    Some(if rest.starts_with('%') {
        format!("{n}%")
    } else {
        format!("{n}px")
    })
}
