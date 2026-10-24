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
        let atomic = synthetic.style.inline_block;
        if synthetic.style.inline_block && synthetic.style.width.is_none() {
            synthetic.style.inline_block = false;
        }
        if synthetic.style.display_flex || synthetic.style.display_grid {
            synthetic.style.display_flex = false;
            synthetic.style.display_grid = false;
        }
        node_layout(context, &synthetic, flow, items, None)?;
        if atomic && synthetic.style.inline {
            // An inline-block, even an empty one, opens a line box, so an element holding only
            // `content: ""; display: inline-block` is one line tall (nodejs.org's blank code
            // lines collapsed to nothing).
            flow.line = flow.line.max(super::sizing::advance(&synthetic.style));
            flow.bottom = flow.bottom.max(flow.y + flow.line);
        }
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
        let inset = |side: usize, basis: f64| super::positioned::inset(style, side, basis);
        let [top, right, bottom, left] = [
            inset(0, height),
            inset(1, width),
            inset(2, height),
            inset(3, width),
        ];
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
/** The element's `::before` and `::after` boxes as flex or grid items, in that order. CSS Display 3
 * blockifies them and Blink makes them the container's first and last children, so they size,
 * align, flex and auto-place like any item (MDN's nav chevrons were never painted). */
pub(super) fn items(node: &Node) -> [Vec<Node>; 2] {
    [false, true].map(|after| {
        node.style
            .generated
            .iter()
            .filter(|g| g.after == after && !g.style.hidden)
            .filter(|g| !matches!(g.style.position, Position::Absolute | Position::Fixed))
            .map(|generated| {
                let mut item = synthetic(node, generated);
                item.style.float = crate::document::style::Float::None;
                item.style.inline = false;
                item.style.inline_block = false;
                // ponytail: a generated flex or grid container lays out as a block of its text;
                // its own items would need identities of their own.
                item.style.display_flex = false;
                item.style.display_grid = false;
                item
            })
            .collect()
    })
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
