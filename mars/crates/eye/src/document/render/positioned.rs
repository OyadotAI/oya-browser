/** CSS 2.1 absolute and fixed placement runs after normal flow, so out-of-flow boxes never move it. */
use super::flow::*;
use crate::document::Position;

/** A containing block's padding box in document points. */
#[derive(Clone, Copy)]
pub(super) struct Block {
    pub(super) x: f64,
    pub(super) y: f64,
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
        for (local, position) in context.take_deferred_generated() {
            let node = context.node(local)?;
            for synthetic in super::generated::deferred(node) {
                context.charge(1)?;
                one(context, &synthetic, position, items, viewport)?;
            }
        }
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
/** One axis of an absolutely positioned box, Blink's inset-modified containing block (IMCB). */
#[derive(Clone, Copy)]
struct Axis {
    /** The containing block's size on this axis. */
    size: f64,
    /** IMCB start and end offsets from the containing block's edges. */
    start: f64,
    end: f64,
    /** Whether the end inset is the only one set, so free space goes before the box. */
    end_biased: bool,
    /** Whether either inset is `auto`, which turns auto margins into zero. */
    auto_inset: bool,
    /** Start and end margins; None is `auto`. */
    margins: (Option<f64>, Option<f64>),
}
impl Axis {
    /** Blink's ComputeUnclampedIMCBInOneAxis for a start-biased static position. */
    fn new(
        size: f64,
        insets: (Option<f64>, Option<f64>),
        static_offset: f64,
        margins: (Option<f64>, Option<f64>),
    ) -> Self {
        let (start, end, end_biased) = match insets {
            (None, None) => (static_offset, 0.0, false),
            (start, end) => (start.unwrap_or(0.0), end.unwrap_or(0.0), start.is_none()),
        };
        Self {
            size,
            start,
            end,
            end_biased,
            auto_inset: insets.0.is_none() || insets.1.is_none(),
            margins,
        }
    }
    /** The IMCB's extent, never negative. */
    fn imcb(self) -> f64 {
        (self.size - self.start - self.end).max(0.0)
    }
    /** Room for the border box once the non-auto margins are taken out. */
    fn available(self) -> f64 {
        (self.imcb() - self.margins.0.unwrap_or(0.0) - self.margins.1.unwrap_or(0.0)).max(0.0)
    }
    /** Both insets are set, so an `auto` size stretches to fill the IMCB. */
    fn stretches(self) -> bool {
        !self.auto_inset
    }
    /**
     * The border box's start offset from the containing block, after Blink's ComputeMargins and
     * ComputeInsets: auto margins share the free space only when both insets are set (in the
     * inline axis an overflowing box keeps its start margin at zero), otherwise the weaker inset
     * absorbs the free space.
     */
    fn offset(self, size: f64, block_axis: bool) -> f64 {
        let (start, end) = self.margins;
        if self.auto_inset || (start.is_some() && end.is_some()) {
            let (start, end) = (start.unwrap_or(0.0), end.unwrap_or(0.0));
            let free = self.size - self.start - self.end - (start + size + end);
            let imcb_start = if self.end_biased {
                self.start + free
            } else {
                self.start
            };
            return imcb_start + start;
        }
        let free = self.imcb() - size - start.unwrap_or(0.0) - end.unwrap_or(0.0);
        let margin = match (start, end) {
            (None, None) if free > 0.0 || block_axis => free / 2.0,
            (None, None) => 0.0,
            (None, Some(_)) => free,
            (Some(start), _) => start,
        };
        self.start + margin
    }
}
/** One margin side; None when it is `auto`. */
fn margin(style: &crate::document::ComputedStyle, side: usize) -> Option<f64> {
    (!style.margin_auto[side]).then_some(style.margin[side])
}
/** One box placed in its nearest positioned ancestor's padding box. */
fn one(
    context: &mut LayoutContext<'_>,
    node: &Node,
    static_position: (f64, f64),
    items: &mut Vec<DisplayItem>,
    viewport: (f64, f64),
) -> EngineResult<()> {
    context.charge(items.len())?;
    let block = containing(context, node, items, viewport)?;
    place_in(context, node, block, static_position, items)
}
/**
 * Lays one absolutely positioned box out in `block`, after Blink's ComputeOofInlineDimensions and
 * ComputeOofBlockDimensions. Generated `::before` and `::after` boxes come through here too, so
 * insets, auto margins and `aspect-ratio` treat them like elements (nodejs.org's glowing
 * backdrop, an `inset: 0; margin: auto` square, was 400px tall and stuck to the corner).
 */
pub(super) fn place_in(
    context: &mut LayoutContext<'_>,
    node: &Node,
    block: Block,
    (static_x, static_y): (f64, f64),
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let style = &node.style;
    let inline = Axis::new(
        block.width,
        (inset(style, 3, block.width), inset(style, 1, block.width)),
        static_x - block.x,
        (margin(style, 3), margin(style, 1)),
    );
    let vertical = Axis::new(
        block.height,
        (inset(style, 0, block.height), inset(style, 2, block.height)),
        static_y - block.y,
        (margin(style, 0), margin(style, 2)),
    );
    let replaced = (node.tag == "img" && node.image_size.is_some())
        .then(|| super::sizing::replaced_size(node, Some(block.width), Some(block.height)));
    let width = match replaced {
        Some((width, _)) => width,
        None => width(context, node, block, inline)?,
    };
    let height = match replaced {
        Some((_, height)) => Some(height),
        None => block_size(node, block, vertical, width),
    };
    let x = block.x + inline.offset(width, false);
    let needs_height = vertical.end_biased
        || (vertical.stretches() && (vertical.margins.0.is_none() || vertical.margins.1.is_none()));
    let laid_height = match height {
        Some(height) => height,
        None if needs_height => super::flex_layout::item_height(context, node, width)?,
        None => 0.0,
    };
    let y = block.y + vertical.offset(laid_height, true);
    let bounds = BorderBox {
        x,
        y,
        width,
        height,
    };
    super::flex_layout::place_item(context, node, bounds, items)?;
    Ok(())
}
/** The border-box height when it needs no layout: an authored height (a percentage of the
containing block), else the one `aspect-ratio` transfers from the used width, else a stretch
between two set insets, clamped by min and max height. Blink sizes an auto height with an
aspect ratio as fit-content rather than stretching it (CanComputeBlockSizeWithoutLayout). */
fn block_size(node: &Node, block: Block, axis: Axis, width: f64) -> Option<f64> {
    let authored = node
        .style
        .height
        .and_then(|height| height.resolve_size(Some(block.height)))
        .map(|height| super::sizing::border_height(node, height));
    // ponytail: content taller than the ratio does not grow the box (css-sizing-4's automatic
    // minimum); lay the box out at the ratio height and take the larger when a page needs it.
    let size = authored
        .or_else(|| aspect_height(node, width))
        .or_else(|| axis.stretches().then(|| axis.available()))?;
    Some(clamp_height(node, size, Some(block.height)))
}
/** The border-box height `aspect-ratio` gives at this border-box width: the ratio sizes the
content box unless `box-sizing: border-box` makes it size the border box. */
fn aspect_height(node: &Node, width: f64) -> Option<f64> {
    let ratio = node.style.aspect_ratio?;
    let edges = vertical_edges(node);
    Some(
        if node.style.box_sizing == crate::document::BoxSizing::BorderBox {
            (width / ratio).max(edges)
        } else {
            (width - horizontal_edges(node)).max(0.0) / ratio + edges
        },
    )
}
/** One inset side in px against its containing basis: px plus percentage; None is `auto`. */
pub(super) fn inset(
    style: &crate::document::ComputedStyle,
    side: usize,
    basis: f64,
) -> Option<f64> {
    if style.inset[side].is_some() || style.inset_percent[side] != 0.0 {
        Some(style.inset[side].unwrap_or(0.0) + basis * style.inset_percent[side] / 100.0)
    } else {
        None
    }
}
/** An authored width resolves against the containing block; an `auto` width stretches between
two set insets and otherwise shrinks to fit the IMCB, clamped by min and max width. */
fn width(
    context: &mut LayoutContext<'_>,
    node: &Node,
    block: Block,
    axis: Axis,
) -> EngineResult<f64> {
    let available = axis.available();
    if node.style.width.is_some() || axis.stretches() {
        return Ok(border_width(node, block.width, available, None));
    }
    let intrinsic = super::flex_layout::item_intrinsic(context, node)?;
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
pub(super) fn padding_box(node: &Node, item: &DisplayItem) -> Block {
    let [top, right, bottom, left] = super::sizing::border_sides(&node.style);
    Block {
        x: item.x + left,
        y: item.y + top,
        width: (item.width - left - right).max(0.0),
        height: (item.height - top - bottom).max(0.0),
    }
}
