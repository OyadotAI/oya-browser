/** A `display: table` root with an `auto` width shrinks to fit its content instead of spanning the
viewport (css-tables html-display-table): the root is formatting data, not a node, so its used
width is written into a copy of the document before layout, where `root_box` reads it. */
use super::flow::{LayoutContext, inline_level};
use crate::document::{Document, RenderContext, style::TableBox};
use crate::protocol::EngineResult;

/** The copy of `document` whose table root carries its shrink-to-fit width; None when the root
is no table or has an authored width. */
pub(super) fn fit_table_root(
    document: &Document,
    context: RenderContext,
) -> EngineResult<Option<Document>> {
    let root = &document.formatting.html.style;
    if root.table_box != TableBox::Table || root.width.is_some() || root.hidden {
        return Ok(None);
    }
    let content = max_content(document, context)?;
    let available = context.width - root.margin[1] - root.margin[3] - edges(root);
    let mut copy = document.clone();
    let style = &mut copy.formatting.html.style;
    style.width = Some(content.min(available.max(0.0)));
    style.percent_width = false;
    Ok(Some(copy))
}

/** The root's horizontal border and padding. */
fn edges(style: &crate::document::ComputedStyle) -> f64 {
    let sides = super::sizing::border_sides(style);
    sides[1] + sides[3] + style.padding[1] + style.padding[3]
}

/** The body's max-content margin-box width: inline-level top-level boxes in a run share a line,
block-level ones each take their own. */
fn max_content(document: &Document, context: RenderContext) -> EngineResult<f64> {
    let mut layout = LayoutContext::new(document)?;
    layout.viewport = (context.width, context.height);
    let (mut widest, mut line) = (0.0_f64, 0.0);
    for node in document.nodes.iter().filter(|node| node.parent.is_none()) {
        let margins = node.style.margin[1] + node.style.margin[3];
        // Shrink-to-fit in the viewport: an authored width, else max-content capped by the room.
        let outer =
            super::float::shrink_width(&mut layout, node, context.width, margins)? + margins;
        if inline_level(node) {
            line += outer;
            widest = widest.max(line);
        } else {
            line = 0.0;
            widest = widest.max(outer);
        }
    }
    let body = &document.formatting.body.style;
    Ok(widest + edges(body) + body.margin[1] + body.margin[3])
}
