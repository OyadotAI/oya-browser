/** `::before` and `::after` boxes: generated text or decorative blocks laid out inside their element. */
use super::flow::*;
use crate::document::Position;

/** Lays out the element's generated box on one side, if it has one. */
pub(super) fn generated(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    after: bool,
) -> EngineResult<()> {
    for generated in node.style.generated.iter().filter(|g| g.after == after) {
        let style = &generated.style;
        // ponytail: positioned and floated generated boxes are skipped or laid out in flow;
        // they need their own identities to be placed later.
        if style.hidden || matches!(style.position, Position::Absolute | Position::Fixed) {
            continue;
        }
        let mut synthetic = synthetic(node, generated);
        synthetic.style.float = crate::document::style::Float::None;
        if synthetic.style.inline_block && synthetic.style.width.is_none() {
            synthetic.style.inline_block = false;
        }
        if synthetic.style.display_flex || synthetic.style.display_grid {
            synthetic.style.display_flex = false;
            synthetic.style.display_grid = false;
        }
        node_layout(context, &synthetic, flow, items, None)?;
    }
    Ok(())
}
/** Absolutely positioned generated boxes of a positioned element, placed in its padding box once
 * the element's own box is final. Others need an ancestor's box and are skipped. */
pub(super) fn absolute(
    context: &mut LayoutContext<'_>,
    node: &Node,
    items: &mut Vec<DisplayItem>,
    index: usize,
) -> EngineResult<()> {
    if node.style.position == Position::Static || node.style.generated.is_empty() {
        return Ok(());
    }
    let (x, y, width, height) = {
        let item = &items[index];
        let sides = super::sizing::border_sides(&node.style);
        (
            item.x + sides[3],
            item.y + sides[0],
            (item.width - sides[1] - sides[3]).max(0.0),
            (item.height - sides[0] - sides[2]).max(0.0),
        )
    };
    for generated in &node.style.generated {
        let style = &generated.style;
        if style.hidden || style.position != Position::Absolute {
            continue;
        }
        let mut synthetic = synthetic(node, generated);
        synthetic.style.position = Position::Static;
        let [top, right, bottom, left] = style.inset;
        let box_width = match (style.width, left, right) {
            (Some(_), _, _) => border_width(&synthetic, width, width, None),
            (None, Some(l), Some(r)) => (width - l - r).max(0.0),
            _ => super::float::shrink_width(context, &synthetic, width, 0.0)?,
        };
        let box_height = match (style.height, top, bottom) {
            (Some(h), _, _) => h
                .resolve_size(Some(height))
                .map(|h| super::sizing::border_height(&synthetic, h)),
            (None, Some(t), Some(b)) => Some((height - t - b).max(0.0)),
            _ => None,
        };
        let bx = match (left, right) {
            (Some(l), _) => x + l,
            (None, Some(r)) => x + width - r - box_width,
            _ => x,
        };
        let by = match (top, bottom, box_height) {
            (Some(t), _, _) => y + t,
            (None, Some(b), Some(h)) => y + height - b - h,
            _ => y,
        };
        let mut flow = Flow::new(bx, by, box_width);
        let bounds = BorderBox {
            x: bx,
            y: by,
            width: box_width,
            height: box_height,
        };
        node_layout(context, &synthetic, &mut flow, items, Some(bounds))?;
    }
    Ok(())
}
/** A stand-in node carrying a generated box's text and style under its element's identity. */
fn synthetic(node: &Node, generated: &crate::document::style::Generated) -> Node {
    let mut synthetic = node.clone();
    synthetic.tag = if generated.after {
        "::after"
    } else {
        "::before"
    }
    .into();
    synthetic.text = generated.content.clone();
    synthetic.href = None;
    synthetic.image_src = None;
    synthetic.table_cell = None;
    synthetic.style = (*generated.style).clone();
    synthetic.style.generated.clear();
    synthetic
}
/** Generated boxes have no children of their own; their identity is their element's. */
pub(super) fn is_generated(node: &Node) -> bool {
    node.tag.starts_with("::")
}
