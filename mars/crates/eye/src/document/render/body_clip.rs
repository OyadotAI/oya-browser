/** The body's own overflow clip. CSS Overflow 3.3 propagates the body's `overflow` to the
viewport only while the root's is `visible` and the body is not paint-contained; otherwise the
body keeps it and clips its content at its padding box (overflow-body-propagation-010, -012).
Eye keeps the body as formatting data, not a node, so its clip is applied to the painted items
here, after the node clips. */
use super::DisplayItem;
use super::flow::LayoutContext;
use crate::document::{Document, Overflow, Position, RenderContext};
use crate::protocol::EngineResult;

/** The body padding box's (x, y, width, height); its height is open when the body's is `auto`. */
fn padding_box(document: &Document, context: RenderContext) -> super::ClipRect {
    let body = &document.formatting.body.style;
    let root = super::body::root_edges(document, (context.width, context.height));
    let inside = (context.width - root[1] - root[3]).max(0.0);
    let (x, y, width) = super::body::content(body, inside);
    let [pt, pr, pb, pl] = body.padding;
    let height = body
        .height
        .and_then(|height| height.resolve_size(Some(context.height)))
        .map(|height| super::sizing::content_height(body, height) + pt + pb)
        .unwrap_or(crate::constants::LAYOUT_EXTENT);
    super::ClipRect {
        x: x + root[3] - pl,
        y: y + root[0] - pt,
        width: width + pl + pr,
        height,
        radius: [0.0; 4],
    }
}

/** Whether the body clips its own overflow instead of handing it to the viewport. */
fn clips(document: &Document) -> bool {
    let (root, body) = (
        &document.formatting.html.style,
        &document.formatting.body.style,
    );
    let body_overflows =
        body.overflow_x != Overflow::Visible || body.overflow_y != Overflow::Visible;
    let root_visible = root.overflow_x == Overflow::Visible && root.overflow_y == Overflow::Visible;
    !body.hidden && (body.contain_paint || (body_overflows && !root_visible))
}

/** Whether an item escapes the body's clip: a fixed box, or an absolute one whose containing
block is the initial containing block (no positioned ancestor), unless the body contains them. */
fn escapes(context: &LayoutContext<'_>, document: &Document, local: usize) -> EngineResult<bool> {
    if document.formatting.body.style.contain_paint {
        return Ok(false);
    }
    let mut escaping = false;
    let mut current = Some(local);
    while let Some(id) = current {
        let node = context.node(id)?;
        let style = &node.style;
        if escaping && (style.position != Position::Static || style.transform.is_some()) {
            return Ok(false);
        }
        match style.position {
            Position::Fixed => return Ok(true),
            Position::Absolute => escaping = true,
            _ => {}
        }
        current = node.parent;
    }
    Ok(escaping)
}

/** Clips every item inside the body to the body's padding box when the body keeps its overflow. */
pub(super) fn apply(
    document: &Document,
    context: RenderContext,
    items: &mut [DisplayItem],
) -> EngineResult<()> {
    if !clips(document) {
        return Ok(());
    }
    let rect = padding_box(document, context);
    let layout = LayoutContext::new(document)?;
    for item in items.iter_mut() {
        if layout.node(item.local_id).is_err() || escapes(&layout, document, item.local_id)? {
            continue;
        }
        item.clip = Some(match &item.clip {
            Some(inner) => inner.intersect(&rect),
            None => rect,
        });
    }
    Ok(())
}
