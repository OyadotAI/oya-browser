/** List item markers: a bullet or number outside the content box, on the first line. */
use super::flow::*;
use crate::document::style::ListStyle;

/** The marker run for a list item, or None for `list-style: none`. */
pub(super) fn marker(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (content_x, top): (f64, f64),
    items: &[DisplayItem],
) -> EngineResult<Option<PaintLine>> {
    let text = match node.style.list_style {
        ListStyle::None => return Ok(None),
        ListStyle::Disc => "\u{2022} ".to_string(),
        ListStyle::Circle => "\u{25e6} ".to_string(),
        ListStyle::Square => "\u{25aa} ".to_string(),
        ListStyle::Decimal => format!("{}. ", context.ordinal(node)?),
    };
    let face = super::sizing::face(&node.style);
    let width = context.measure(&text, face)?;
    // The first line box's top, so the marker shares its baseline when fonts match.
    let y = items
        .iter()
        .flat_map(|item| item.lines.iter())
        .map(|line| line.y)
        .reduce(f64::min)
        .unwrap_or(top);
    Ok(Some(PaintLine {
        text,
        x: content_x - width,
        y,
        width,
    }))
}
