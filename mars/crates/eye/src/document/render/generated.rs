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
        if style.display_none || matches!(style.position, Position::Absolute | Position::Fixed) {
            continue;
        }
        let mut synthetic = synthetic(node, generated);
        if synthetic.style.hidden {
            invisible(&mut synthetic.style);
        }
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
 * the element's own box is final. A static element's wait, like any out-of-flow box, until their
 * containing block (an ancestor, or the viewport for fixed ones) is laid out. */
pub(super) fn absolute(
    context: &mut LayoutContext<'_>,
    node: &Node,
    items: &mut Vec<DisplayItem>,
    index: usize,
) -> EngineResult<()> {
    if node.style.generated.is_empty() {
        return Ok(());
    }
    let block = super::positioned::padding_box(node, &items[index]);
    if node.style.position == Position::Static && node.style.transform.is_none() {
        if out_of_flow(node).next().is_some() {
            context.defer_generated(node.local_id, (block.x, block.y));
        }
        return Ok(());
    }
    for generated in &node.style.generated {
        let style = &generated.style;
        if style.hidden || style.position != Position::Absolute {
            continue;
        }
        // The same out-of-flow sizing as elements, at the padding box's origin as the static
        // position; Static keeps the stand-in from being deferred again.
        let mut synthetic = synthetic(node, generated);
        synthetic.style.position = Position::Static;
        super::positioned::place_in(context, &synthetic, block, (block.x, block.y), items)?;
    }
    Ok(())
}
/** A static element's absolute and fixed generated boxes. */
fn out_of_flow(node: &Node) -> impl Iterator<Item = &crate::document::style::Generated> {
    node.style.generated.iter().filter(|generated| {
        !generated.style.hidden
            && matches!(
                generated.style.position,
                Position::Absolute | Position::Fixed
            )
    })
}
/** Stand-ins for a static element's absolute and fixed generated boxes, placed by the
 * out-of-flow pass in the nearest positioned ancestor's padding box (Blink makes them children of
 * the element, whose containing block is that ancestor's). LinkedIn's full-bleed section bands
 * are `::after { position: absolute; width: 100vw }` on a static flex row. */
pub(super) fn deferred(node: &Node) -> Vec<Node> {
    out_of_flow(node)
        .map(|generated| synthetic(node, generated))
        .collect()
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
    synthetic.style = (*generated.style).clone().into();
    synthetic.style.generated.clear();
    synthetic
}
/** A `visibility: hidden` generated box still lays out (the clearfix `::after { content: "";
 * display: block; visibility: hidden; clear: both }` clears floats), but paints nothing: its
 * text, background, border and shadows are transparent. */
fn invisible(style: &mut crate::document::ComputedStyle) {
    const CLEAR: &str = "transparent";
    style.hidden = false;
    style.interactive = false;
    style.color = Some(CLEAR.into());
    style.background = None;
    style.backgrounds.clear();
    style.shadows.clear();
    style.border_colors = std::array::from_fn(|_| Some(CLEAR.into()));
    style.border_color = Some(CLEAR.into());
    style.text_decoration_none = true;
    style.underline = false;
    style.strikethrough = false;
}
/** Generated boxes have no children of their own; their identity is their element's. */
pub(super) fn is_generated(node: &Node) -> bool {
    node.tag.starts_with("::")
}
