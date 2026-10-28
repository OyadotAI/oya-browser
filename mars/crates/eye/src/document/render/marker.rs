/** List item markers: a bullet or number outside the content box, on the first line. */
use super::flow::*;
use crate::document::style::ListStyle;

/** Chromium's padding between a symbol marker and the content, plus its one-pixel inset. */
const SYMBOL_PAD: f64 = 8.0;
/** The marker run for a list item, or None for `list-style: none`. */
pub(super) fn marker(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (content_x, top): (f64, f64),
    items: &[DisplayItem],
) -> EngineResult<Option<PaintLine>> {
    let text = match node.style.list_style {
        ListStyle::None => return Ok(None),
        ListStyle::Disc => "\u{2022}".to_string(),
        ListStyle::Circle => "\u{25e6}".to_string(),
        ListStyle::Square => "\u{25aa}".to_string(),
        ListStyle::Decimal => format!("{}. ", context.ordinal(node)?),
    };
    let face = super::sizing::face(&node.style);
    let width = context.measure(&text, face)?;
    // A symbol sits two thirds of the ascent plus a 7px pad and a pixel before the content, as
    // Chromium places its outside disc (lite.cnn.com's bullets hugged their links); a counter
    // ends at the content edge.
    let start = if node.style.list_style == ListStyle::Decimal {
        content_x - width
    } else {
        let ascent = super::sizing::face_metrics(&node.style).0;
        content_x - (ascent * 2.0 / 3.0).floor() - SYMBOL_PAD
    };
    // The first line box's top, so the marker shares its baseline when fonts match.
    let y = items
        .iter()
        .flat_map(|item| item.lines.iter())
        .map(|line| line.y)
        .reduce(f64::min)
        .unwrap_or(top);
    Ok(Some(PaintLine {
        text,
        x: start,
        y,
        width,
        color: node.style.marker_color.clone(),
    }))
}
