/**
 * Resolved CSS values for `getComputedStyle`, read from Eye's cascade as Blink's
 * ComputedStyleUtils serializes them: colors as `rgb()`/`rgba()`, lengths in px, keywords in
 * their canonical spelling. Scripts read these to decide layout and visibility (GSAP, React
 * libraries, Bing's lazy loaders); before this V9 answered from inline styles and tag defaults.
 */
use super::model::ComputedStyle;
use super::{
    AlignItems, BoxSizing, FlexDirection, FlexWrap, Float, FontStyle, JustifyContent, LineHeight,
    Overflow, Position, TextTransform, WhiteSpace,
};

/** Sides in the order the per-side arrays store them. */
const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/** The properties this serializer answers, so callers can tell "unsupported" from "empty". */
pub const COMPUTED_PROPERTIES: &[&str] = &[
    "display", "visibility", "opacity", "position", "z-index", "overflow", "overflow-x",
    "overflow-y", "color", "background-color", "font-size", "font-weight", "font-style",
    "font-family", "line-height", "box-sizing", "flex-direction", "flex-wrap", "flex-grow",
    "flex-shrink", "justify-content", "align-items", "row-gap", "column-gap", "white-space",
    "text-transform", "text-align", "float", "cursor", "transform", "top", "right", "bottom",
    "left", "margin-top", "margin-right", "margin-bottom", "margin-left", "padding-top",
    "padding-right", "padding-bottom", "padding-left", "border-top-width", "border-right-width",
    "border-bottom-width", "border-left-width", "border-top-color", "border-right-color",
    "border-bottom-color", "border-left-color", "letter-spacing", "word-spacing", "order",
];

/** A property's resolved value, or None when this serializer does not cover it. */
pub fn computed_value(style: &ComputedStyle, property: &str) -> Option<String> {
    if let Some(value) = side_value(style, property) {
        return Some(value);
    }
    Some(match property {
        "display" => display(style).into(),
        "visibility" => if style.invisible { "hidden" } else { "visible" }.into(),
        "opacity" => number(style.opacity),
        "position" => position(style.position).into(),
        "z-index" => style.z_index.map_or("auto".into(), |z| z.to_string()),
        "overflow" => format_overflow(style),
        "overflow-x" => overflow(style.overflow_x).into(),
        "overflow-y" => overflow(style.overflow_y).into(),
        "color" => css_color(style.color.as_deref().unwrap_or("#000000")),
        "background-color" => css_color(style.background.as_deref().unwrap_or("transparent")),
        _ => return text_value(style, property).or_else(|| box_value(style, property)),
    })
}

/** Font and text properties. */
fn text_value(style: &ComputedStyle, property: &str) -> Option<String> {
    Some(match property {
        "font-size" => px(style.font_size),
        "font-weight" => style.font_weight.to_string(),
        "font-style" => font_style(style.font_style).into(),
        "font-family" => font_family(style),
        "line-height" => line_height(style),
        "white-space" => white_space(style.white_space).into(),
        "text-transform" => text_transform(style.text_transform).into(),
        "text-align" => if style.align.is_empty() { "start".into() } else { style.align.clone() },
        "letter-spacing" => if style.letter_spacing == 0.0 { "normal".into() } else { px(style.letter_spacing) },
        "word-spacing" => px(style.word_spacing),
        "cursor" => if style.cursor_pointer { "pointer" } else { "auto" }.into(),
        _ => return None,
    })
}

/** Box, flex and transform properties. */
fn box_value(style: &ComputedStyle, property: &str) -> Option<String> {
    Some(match property {
        "box-sizing" => match style.box_sizing { BoxSizing::BorderBox => "border-box", BoxSizing::ContentBox => "content-box" }.into(),
        "flex-direction" => flex_direction(style).into(),
        "flex-wrap" => flex_wrap(style).into(),
        "flex-grow" => number(style.flex_grow),
        "flex-shrink" => number(style.flex_shrink),
        "justify-content" => justify(style.justify_content).into(),
        "align-items" => align(style.align_items).into(),
        "row-gap" => if style.gap_set[0] { px(style.row_gap) } else { "normal".into() },
        "column-gap" => if style.gap_set[1] { px(style.column_gap) } else { "normal".into() },
        "float" => match style.float { Float::None => "none", Float::Left => "left", Float::Right => "right" }.into(),
        "order" => style.order.to_string(),
        "transform" => style.transform.map_or("none".into(), matrix),
        _ => return None,
    })
}

/** Per-side properties: inset, margin, padding, border width and color. */
fn side_value(style: &ComputedStyle, property: &str) -> Option<String> {
    let (prefix, side) = SIDES.iter().enumerate().find_map(|(index, side)| {
        let prefix = property.strip_suffix(side)?.strip_suffix('-').unwrap_or("");
        Some((prefix, index))
    }).or_else(|| border_side(property))?;
    Some(match prefix {
        "" => style.inset[side].map_or("auto".into(), px),
        "margin" => px(style.margin[side]),
        "padding" => px(style.padding[side]),
        "border-width" => px(style.border_widths[side]),
        "border-color" => css_color(style.border_colors[side].as_deref().or(style.color.as_deref()).unwrap_or("#000000")),
        _ => return None,
    })
}

/** `border-<side>-width` and `border-<side>-color` as (kind, side index). */
fn border_side(property: &str) -> Option<(&'static str, usize)> {
    let rest = property.strip_prefix("border-")?;
    SIDES.iter().enumerate().find_map(|(index, side)| {
        match rest.strip_prefix(side)? {
            "-width" => Some(("border-width", index)),
            "-color" => Some(("border-color", index)),
            _ => None,
        }
    })
}

/** The outer and inner display type, as the display property serializes. */
fn display(style: &ComputedStyle) -> &'static str {
    if style.display_none || style.hidden { return "none"; }
    if style.display_contents { return "contents"; }
    let inline = style.inline || style.inline_block;
    match (style.display_flex, style.display_grid, inline) {
        (true, _, true) => "inline-flex",
        (true, _, false) => "flex",
        (_, true, true) => "inline-grid",
        (_, true, false) => "grid",
        _ if style.list_item => "list-item",
        _ if style.flow_root => "flow-root",
        _ if style.inline_block => "inline-block",
        _ if style.inline => "inline",
        _ => "block",
    }
}

/** A number as CSS serializes it: no trailing zeros. */
fn number(value: f64) -> String {
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    format!("{rounded}")
}

/** A length in px. */
fn px(value: f64) -> String {
    format!("{}px", number(value))
}

/** `matrix(a, b, c, d, e, f)`. */
fn matrix(m: [f64; 6]) -> String {
    let parts: Vec<String> = m.iter().map(|value| number(*value)).collect();
    format!("matrix({})", parts.join(", "))
}

/** A painter color literal (`#rrggbb`, `#rrggbbaa`, `transparent`, a name) as `rgb()`/`rgba()`. */
pub fn css_color(literal: &str) -> String {
    match super::colors::painter_rgba(literal) {
        Some([r, g, b, 255]) => format!("rgb({r}, {g}, {b})"),
        Some([r, g, b, a]) => format!("rgba({r}, {g}, {b}, {})", number(f64::from(a) / 255.0)),
        None => literal.to_string(),
    }
}

/** `overflow`, collapsed to one keyword when both axes agree. */
fn format_overflow(style: &ComputedStyle) -> String {
    let (x, y) = (overflow(style.overflow_x), overflow(style.overflow_y));
    if x == y { x.into() } else { format!("{x} {y}") }
}

fn overflow(value: Overflow) -> &'static str {
    match value {
        Overflow::Visible => "visible",
        Overflow::Auto => "auto",
        Overflow::Scroll => "scroll",
        Overflow::Hidden => "hidden",
        Overflow::Clip => "clip",
    }
}

fn position(value: Position) -> &'static str {
    match value {
        Position::Static => "static",
        Position::Relative => "relative",
        Position::Absolute => "absolute",
        Position::Fixed => "fixed",
        Position::Sticky => "sticky",
    }
}

fn font_style(value: FontStyle) -> &'static str {
    match value {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
        FontStyle::Oblique => "oblique",
    }
}

/** The authored family list, names with spaces quoted as Blink serializes them; a list the
`font` shorthand left empty reads as its generic family. */
fn font_family(style: &ComputedStyle) -> String {
    if style.font_families.is_empty() {
        return match style.font_family {
            super::FontFamily::Serif => "serif",
            super::FontFamily::SansSerif => "sans-serif",
            super::FontFamily::Monospace => "monospace",
            super::FontFamily::SystemUi => "system-ui",
        }
        .into();
    }
    let quote = |name: &String| if name.contains(' ') { format!("\"{name}\"") } else { name.clone() };
    style.font_families.iter().map(quote).collect::<Vec<_>>().join(", ")
}

/** `normal`, or the used line height in px. */
fn line_height(style: &ComputedStyle) -> String {
    match style.line_height {
        LineHeight::Normal => "normal".into(),
        LineHeight::Number(factor) => px(factor * style.font_size),
        LineHeight::Length(length) => px(length),
    }
}

fn white_space(value: WhiteSpace) -> &'static str {
    match value {
        WhiteSpace::Normal => "normal",
        WhiteSpace::Pre => "pre",
        WhiteSpace::Nowrap => "nowrap",
        WhiteSpace::PreWrap => "pre-wrap",
        WhiteSpace::PreLine => "pre-line",
    }
}

fn text_transform(value: TextTransform) -> &'static str {
    match value {
        TextTransform::None => "none",
        TextTransform::Uppercase => "uppercase",
        TextTransform::Lowercase => "lowercase",
        TextTransform::Capitalize => "capitalize",
    }
}

fn flex_direction(style: &ComputedStyle) -> &'static str {
    match (style.flex_direction, style.flex_reverse) {
        (FlexDirection::Row, false) => "row",
        (FlexDirection::Row, true) => "row-reverse",
        (FlexDirection::Column, false) => "column",
        (FlexDirection::Column, true) => "column-reverse",
    }
}

fn flex_wrap(style: &ComputedStyle) -> &'static str {
    match (style.flex_wrap, style.wrap_reverse) {
        (FlexWrap::NoWrap, _) => "nowrap",
        (FlexWrap::Wrap, false) => "wrap",
        (FlexWrap::Wrap, true) => "wrap-reverse",
    }
}

fn justify(value: JustifyContent) -> &'static str {
    match value {
        JustifyContent::Stretch => "normal",
        JustifyContent::Start => "flex-start",
        JustifyContent::Center => "center",
        JustifyContent::End => "flex-end",
        JustifyContent::LogicalStart => "start",
        JustifyContent::LogicalEnd => "end",
        JustifyContent::Left => "left",
        JustifyContent::Right => "right",
        JustifyContent::SpaceBetween => "space-between",
        JustifyContent::SpaceAround => "space-around",
        JustifyContent::SpaceEvenly => "space-evenly",
    }
}

fn align(value: AlignItems) -> &'static str {
    match value {
        AlignItems::Start => "flex-start",
        AlignItems::Center => "center",
        AlignItems::End => "flex-end",
        AlignItems::Stretch => "normal",
        AlignItems::Baseline => "baseline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Painter literals serialize as getComputedStyle reports colors. */
    #[test]
    fn colors_serialize_as_rgb() {
        assert_eq!(css_color("#ff0000"), "rgb(255, 0, 0)");
        assert_eq!(css_color("#00000080"), "rgba(0, 0, 0, 0.501961)");
        assert_eq!(css_color("transparent"), "rgba(0, 0, 0, 0)");
        assert_eq!(css_color("white"), "rgb(255, 255, 255)");
    }

    /** Lengths are px and numbers drop trailing zeros. */
    #[test]
    fn lengths_and_numbers_are_canonical() {
        assert_eq!(px(12.0), "12px");
        assert_eq!(number(0.5), "0.5");
        assert_eq!(matrix([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]), "matrix(1, 0, 0, 1, 10, 0)");
    }

    /** A default style answers block, visible, static and the sides in px. */
    #[test]
    fn default_style_resolves() {
        let style = ComputedStyle::default();
        assert_eq!(computed_value(&style, "display").as_deref(), Some("block"));
        assert_eq!(computed_value(&style, "visibility").as_deref(), Some("visible"));
        assert_eq!(computed_value(&style, "margin-left").as_deref(), Some("0px"));
        assert_eq!(computed_value(&style, "border-top-width").as_deref(), Some("0px"));
        assert_eq!(computed_value(&style, "top").as_deref(), Some("auto"));
        assert_eq!(computed_value(&style, "nonexistent"), None);
    }
}
