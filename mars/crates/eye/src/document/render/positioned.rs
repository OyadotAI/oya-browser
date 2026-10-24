/** CSS 2.1 absolute and fixed placement runs after normal flow, so out-of-flow boxes never move it. */
use super::flow::*;
use crate::document::Position;

/** A containing block's padding box in document points. */
#[derive(Clone, Copy)]
struct Block {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Places every deferred out-of-flow box, including ones discovered inside earlier placements.
/// Returns the lowest bottom edge of absolute boxes, which extends the scrollable document.
pub(super) fn place(
    context: &mut LayoutContext<'_>,
    items: &mut Vec<DisplayItem>,
    viewport: (f64, f64),
) -> EngineResult<f64> {
    let mut placed = std::collections::BTreeSet::new();
    let mut bottom: f64 = 0.0;
    loop {
        let pending = context.take_deferred();
        let pending = pending
            .into_iter()
            .filter(|(local, _)| placed.insert(*local))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(bottom);
        }
        for (local, position) in pending {
            context.charge(1)?;
            let node = context.node(local)?;
            let start = items.len();
            one(context, node, position, items, viewport)?;
            if node.style.position == Position::Absolute
                && let Some(item) = items.get(start)
            {
                bottom = bottom.max(item.y + item.height + node.style.margin[2]);
            }
        }
    }
}
/** One box: width from insets or shrink-to-fit, then each axis from its insets or its static place. */
fn one(
    context: &mut LayoutContext<'_>,
    node: &Node,
    (static_x, static_y): (f64, f64),
    items: &mut Vec<DisplayItem>,
    viewport: (f64, f64),
) -> EngineResult<()> {
    context.charge(items.len())?;
    let block = containing(context, node, items, viewport)?;
    let style = &node.style;
    let resolve = |side: usize, basis: f64| -> Option<f64> {
        if style.inset[side].is_some() || style.inset_percent[side] != 0.0 {
            Some(style.inset[side].unwrap_or(0.0) + basis * style.inset_percent[side] / 100.0)
        } else {
            None
        }
    };
    let [top, right, bottom, left] = [
        resolve(0, block.height),
        resolve(1, block.width),
        resolve(2, block.height),
        resolve(3, block.width),
    ];
    let margins = (
        style.margin[1] + style.margin[3],
        style.margin[0] + style.margin[2],
    );
    let width = width(context, node, block, (left, right), margins.0)?;
    let x = match (left, right) {
        (Some(left), _) => block.x + left + style.margin[3],
        (None, Some(right)) => block.x + block.width - right - style.margin[1] - width,
        (None, None) => static_x + style.margin[3],
    };
    let height = match (top, bottom) {
        (Some(top), Some(bottom)) => Some((block.height - top - bottom - margins.1).max(0.0)),
        _ => None,
    };
    let y = match (top, bottom) {
        (Some(top), _) => block.y + top + style.margin[0],
        (None, Some(bottom)) => {
            let height = context.measure_height(node.local_id, width)?;
            block.y + block.height - bottom - style.margin[2] - height
        }
        (None, None) => static_y + style.margin[0],
    };
    context.layout_at(
        node.local_id,
        BorderBox {
            x,
            y,
            width,
            height,
        },
        items,
    )?;
    Ok(())
}
/** Both insets fix the width; otherwise an auto width shrinks to fit its content. */
fn width(
    context: &mut LayoutContext<'_>,
    node: &Node,
    block: Block,
    (left, right): (Option<f64>, Option<f64>),
    margins: f64,
) -> EngineResult<f64> {
    let available = (block.width - left.unwrap_or(0.0) - right.unwrap_or(0.0) - margins).max(0.0);
    if node.style.width.is_some() || (left.is_some() && right.is_some()) {
        return Ok(border_width(node, block.width, available, None));
    }
    let intrinsic = context.intrinsic_widths(node.local_id)?;
    let edges = horizontal_edges(node);
    let fit = intrinsic
        .max_content
        .min((available - edges).max(intrinsic.min_content));
    Ok(border_width(
        node,
        block.width,
        available.max(fit + edges),
        Some(fit),
    ))
}
/** The nearest positioned or transformed ancestor's padding box; the viewport for fixed and for none. */
fn containing(
    context: &LayoutContext<'_>,
    node: &Node,
    items: &[DisplayItem],
    (width, height): (f64, f64),
) -> EngineResult<Block> {
    let viewport = Block {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    if node.style.position == Position::Fixed {
        // ponytail: fixed boxes use the unscrolled viewport; follow scroll_y when scrolling is layered.
        return Ok(viewport);
    }
    let mut current = node.parent;
    while let Some(local) = current {
        let ancestor = context.node(local)?;
        if ancestor.style.position != Position::Static || ancestor.style.transform.is_some() {
            return Ok(items
                .iter()
                .find(|item| item.local_id == local)
                .map(|item| padding_box(ancestor, item))
                .unwrap_or(viewport));
        }
        current = ancestor.parent;
    }
    Ok(viewport)
}
/** Borders are inside the border box, so the padding box starts past them. */
fn padding_box(node: &Node, item: &DisplayItem) -> Block {
    let [top, right, bottom, left] = super::sizing::border_sides(&node.style);
    Block {
        x: item.x + left,
        y: item.y + top,
        width: (item.width - left - right).max(0.0),
        height: (item.height - top - bottom).max(0.0),
    }
}
