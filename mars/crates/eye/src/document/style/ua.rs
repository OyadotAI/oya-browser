/** Chromium's html.css defaults that matter for layout, as lowest-priority declarations. */
use super::ComputedStyle;
use crate::document::Node;
/** Elements whose initial display is block-level in Chromium's html.css; every other element, including unknown and custom ones, is inline. */
const BLOCK: [&str; 54] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "hr",
    "html",
    "legend",
    "li",
    "listing",
    "main",
    "menu",
    "nav",
    "ol",
    "optgroup",
    "option",
    "p",
    "plaintext",
    "pre",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];
/** Block-axis margins for vertical rhythm elements. */
const VERTICAL: [(&str, &str); 13] = [
    ("p", "1em"),
    ("dl", "1em"),
    ("ul", "1em"),
    ("ol", "1em"),
    ("menu", "1em"),
    ("pre", "1em"),
    ("blockquote", "1em"),
    ("figure", "1em"),
    ("h1", "0.67em"),
    ("h2", "0.83em"),
    ("h3", "1em"),
    ("h4", "1.33em"),
    ("h5", "1.67em"),
];
/** Heading sizes relative to the parent font. */
const HEADINGS: [(&str, &str); 6] = [
    ("h1", "2em"),
    ("h2", "1.5em"),
    ("h3", "1.17em"),
    ("h4", "1em"),
    ("h5", "0.83em"),
    ("h6", "0.67em"),
];
/** html.css gives these the monospace family; its 13px default size follows from the family. */
const MONOSPACE: [&str; 5] = ["pre", "code", "kbd", "samp", "tt"];
/** Initial display and other non-declarative defaults. */
pub(super) fn defaults(node: &Node, style: &mut ComputedStyle) {
    let tag = node.tag.as_str();
    style.inline = !BLOCK.contains(&tag);
    style.list_item = tag == "li";
    // A video is a replaced box: an inline-block with Chromium's 300 by 150 default size.
    style.inline_block = tag == "video";
    match tag {
        "ol" => style.list_style = super::ListStyle::Decimal,
        "ul" | "menu" => style.list_style = super::ListStyle::Disc,
        "td" | "th" => style.vertical_align = super::VerticalAlign::Middle,
        // Blink resets the legacy -webkit-center alignment at tables, as `<center>` tables show.
        "table" if style.align == "webkit-center" => style.align = "start".into(),
        _ => {}
    }
}
/** UA declarations for one element, applied below every author declaration. */
pub(super) fn declarations(node: &Node) -> Vec<(&'static str, &'static str)> {
    let tag = node.tag.as_str();
    let mut out = Vec::new();
    if let Some((_, size)) = HEADINGS.iter().find(|(name, _)| *name == tag) {
        out.extend([("font-size", *size), ("font-weight", "bold")]);
    }
    if let Some((_, margin)) = VERTICAL.iter().find(|(name, _)| *name == tag) {
        out.extend([
            ("margin-block-start", *margin),
            ("margin-block-end", *margin),
        ]);
    }
    if tag == "h6" {
        out.extend([
            ("margin-block-start", "2.33em"),
            ("margin-block-end", "2.33em"),
        ]);
    }
    match tag {
        "ul" | "ol" | "menu" => out.push(("padding-inline-start", "40px")),
        "dd" => out.push(("margin-inline-start", "40px")),
        "blockquote" | "figure" => out.extend([
            ("margin-inline-start", "40px"),
            ("margin-inline-end", "40px"),
        ]),
        "strong" | "b" | "th" => out.push(("font-weight", "bold")),
        "em" | "i" | "cite" | "dfn" | "var" | "address" => out.push(("font-style", "italic")),
        "u" | "ins" => out.push(("text-decoration-line", "underline")),
        "s" | "del" | "strike" => out.push(("text-decoration-line", "line-through")),
        "small" | "sub" | "sup" => out.push(("font-size", "smaller")),
        "center" => out.push(("text-align", "-webkit-center")),
        // Chromium's html.css sizes a table's border box: `table { box-sizing: border-box }`.
        "table" => out.extend([("border-spacing", "2px"), ("box-sizing", "border-box")]),
        "video" => out.extend([("width", "300px"), ("height", "150px")]),
        // html.css: `iframe { border: 2px inset }`, its color the current color.
        "iframe" => out.extend([
            ("border-top-width", "2px"),
            ("border-right-width", "2px"),
            ("border-bottom-width", "2px"),
            ("border-left-width", "2px"),
            ("border-top-style", "inset"),
            ("border-right-style", "inset"),
            ("border-bottom-style", "inset"),
            ("border-left-style", "inset"),
        ]),
        // Chromium's html.css hides these; script-inserted ones stay in the tree as DOM nodes.
        "script" | "style" | "template" => out.push(("display", "none")),
        // A slot's box is its assigned nodes', as Chromium's `slot { display: contents }`.
        "slot" => out.push(("display", "contents")),

        "hr" => out.extend(rule()),
        _ => {}
    }
    match tag {
        "td" | "th" => out.extend([
            ("padding-top", "1px"),
            ("padding-right", "1px"),
            ("padding-bottom", "1px"),
            ("padding-left", "1px"),
        ]),
        "sup" => out.push(("vertical-align", "super")),
        "sub" => out.push(("vertical-align", "sub")),
        _ => {}
    }
    if MONOSPACE.contains(&tag) {
        out.push(("font-family", "monospace"));
    }
    if tag == "pre" {
        out.push(("white-space", "pre"));
    }
    if node.href.is_some() {
        out.push(("cursor", "pointer"));
    }
    // html.css sizes push buttons by their border box, so an author `min-height: 32px` with
    // padding stays 32px tall (LinkedIn's "Show more" pill was 14px taller).
    let push_input = tag == "input"
        && ["button", "submit", "reset"].contains(&node.input_type().to_ascii_lowercase().as_str());
    if tag == "button" || push_input {
        out.push(("box-sizing", "border-box"));
    }
    // html.css gives form controls `font: -webkit-small-control`, whose shorthand resets the
    // inherited line height to `normal`; a page's `line-height: 1.5` never reaches a button.
    if ["button", "input", "select", "textarea"].contains(&tag) {
        out.push(("line-height", "normal"));
    }
    out
}
/** `hr`: half-em block margins, centered, a one pixel inset gray rule. */
fn rule() -> [(&'static str, &'static str); 16] {
    [
        ("margin-block-start", "0.5em"),
        ("margin-block-end", "0.5em"),
        ("margin-inline-start", "auto"),
        ("margin-inline-end", "auto"),
        ("border-top-width", "1px"),
        ("border-right-width", "1px"),
        ("border-bottom-width", "1px"),
        ("border-left-width", "1px"),
        ("border-top-style", "inset"),
        ("border-right-style", "inset"),
        ("border-bottom-style", "inset"),
        ("border-left-style", "inset"),
        ("border-top-color", "gray"),
        ("border-right-color", "gray"),
        ("border-bottom-color", "gray"),
        ("border-left-color", "gray"),
    ]
}
/** Chromium's body margin. */
pub(super) fn body() -> Vec<(&'static str, &'static str)> {
    ["margin-top", "margin-right", "margin-bottom", "margin-left"]
        .into_iter()
        .map(|side| (side, "8px"))
        .collect()
}
/**
 * Content the UA stylesheet hides whatever the page says: a `dialog` without `open`, and every
 * child of a closed `details` but its first `summary`. Chromium keeps these out of rendering;
 * an author rule must not make them agent-actionable.
 */
pub(super) fn closed(node: &Node, parent: Option<&Node>, summary: Option<usize>) -> bool {
    let shut = |node: &Node| !node.attributes.contains_key("open");
    (node.tag == "dialog" && shut(node))
        || parent.is_some_and(|parent| {
            parent.tag == "details" && shut(parent) && summary != Some(node.local_id)
        })
}
