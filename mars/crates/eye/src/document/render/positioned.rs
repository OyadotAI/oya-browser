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
                && !clipped(context, node)?
                && let Some(item) = items.get(start)
            {
                bottom = bottom.max(item.y + item.height + node.style.margin[2]);
            }
        }
    }
}
/** Whether an ancestor at or above this absolute box's containing block clips its overflow, so
 * the box adds nothing to the document's scrollable extent (Blink propagates scrollable overflow
 * up the containing-block chain and stops at overflow clips). An opacity-0 ripple inside an
 * `overflow: hidden` search box made Google's home page scroll. Mirrors effects' clip chain. */
fn clipped(context: &LayoutContext<'_>, node: &Node) -> EngineResult<bool> {
    let mut escaping = true;
    let mut parent = node.parent;
    while let Some(local) = parent {
        let style = &context.node(local)?.style;
        escaping &= style.position == Position::Static && style.transform.is_none();
        if !escaping && style.overflow != crate::document::Overflow::Visible {
            return Ok(true);
        }
        escaping |= style.position == Position::Absolute;
        if style.position == Position::Fixed {
            return Ok(false);
        }
        parent = context.node(local)?.parent;
    }
    Ok(false)
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
/** The content-box width `aspect-ratio` transfers from an authored height, which Blink uses for
an auto width before shrink-to-fit (Wikipedia's video play button is `width:auto;
height:min(50px,70%);aspect-ratio:1/1`, and fit-content of its empty box would be 0). */
fn aspect_width(node: &Node, block: Block) -> Option<f64> {
    let ratio = node.style.aspect_ratio?;
    let height = node.style.height?.resolve_size(Some(block.height))?;
    Some(
        if node.style.box_sizing == crate::document::BoxSizing::BorderBox {
            (height.max(vertical_edges(node)) * ratio - horizontal_edges(node)).max(0.0)
        } else {
            height * ratio
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
    if let Some(width) = aspect_width(node, block) {
        return Ok(border_width(node, block.width, available, Some(width)));
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
/** The nearest positioned or transformed ancestor's padding box; for a fixed box the nearest
 * one that contains fixed boxes (`contains_fixed`); the viewport when there is none. */
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
    // ponytail: fixed boxes use the unscrolled viewport; follow scroll_y when scrolling is layered.
    let fixed = node.style.position == Position::Fixed;
    let mut current = node.parent;
    while let Some(local) = current {
        let ancestor = context.node(local)?;
        if contains_fixed(&ancestor.style)
            || (!fixed && ancestor.style.position != Position::Static)
        {
            let Some(item) = items.iter().find(|item| item.local_id == local) else {
                return Ok(viewport);
            };
            let block = padding_box(ancestor, item);
            return Ok(if splittable_inline(ancestor) {
                inline_container(context, ancestor, items, block)
            } else {
                block
            });
        }
        current = ancestor.parent;
    }
    Ok(viewport)
}
/** Blink's CanContainFixedPositionObjects: a transform, a filter or paint containment makes a
 * box the containing block of its fixed descendants as well as its absolute ones. */
fn contains_fixed(style: &crate::document::ComputedStyle) -> bool {
    style.transform.is_some() || style.filter_blur > 0.0 || style.contain_paint
}
/** Runs whose tops differ by less than this share a line. */
const LINE_TOLERANCE: f64 = 0.5;
/** A non-atomic inline box, which may break across lines into several fragments. */
fn splittable_inline(node: &Node) -> bool {
    node.style.inline
        && !node.style.inline_block
        && !["img", "svg", "input", "button", "select", "textarea", "br"]
            .contains(&node.tag.as_str())
}
/** Whether `node` sits inside the inline `container` through inline boxes only, so its runs are
 * part of the container's fragments. */
fn inside_inline(context: &LayoutContext<'_>, node: &Node, container: usize) -> bool {
    let mut current = Some(node);
    while let Some(node) = current {
        if node.local_id == container {
            return true;
        }
        if !node.style.inline || matches!(node.style.position, Position::Absolute | Position::Fixed)
        {
            return false;
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    false
}
/**
 * Blink's AddInlineContainingBlockInfo: the containing block an inline box forms when it breaks
 * across lines runs from the logical start of its first fragment to the logical end of its last
 * one (padding edges), never negative, rather than the union of every line. `block` is the
 * union's padding box, whose block-axis edges already are the first line's top and the last
 * line's bottom.
 */
fn inline_container(
    context: &LayoutContext<'_>,
    node: &Node,
    items: &[DisplayItem],
    block: Block,
) -> Block {
    let mut runs: Vec<(f64, f64, f64)> = Vec::new();
    for item in items {
        let Ok(owner) = context.node(item.local_id) else {
            continue;
        };
        if !inside_inline(context, owner, node.local_id) {
            continue;
        }
        if splittable_inline(owner) || owner.tag == "#text" {
            runs.extend(item.lines.iter().map(|l| (l.y, l.x, l.x + l.width)));
        } else {
            runs.push((item.y, item.x, item.x + item.width));
        }
    }
    let (Some(first), Some(last)) = (
        runs.iter().map(|r| r.0).reduce(f64::min),
        runs.iter().map(|r| r.0).reduce(f64::max),
    ) else {
        return block;
    };
    if last - first < LINE_TOLERANCE {
        return block;
    }
    let line = |y: f64| {
        runs.iter()
            .filter(move |r| (r.0 - y).abs() < LINE_TOLERANCE)
    };
    let pad = node.style.padding;
    let (left, right) = if node.style.rtl {
        let start = line(first).map(|r| r.2).fold(f64::MIN, f64::max) + pad[1];
        let end = line(last).map(|r| r.1).fold(f64::MAX, f64::min) - pad[3];
        (end.min(start), start)
    } else {
        let start = line(first).map(|r| r.1).fold(f64::MAX, f64::min) - pad[3];
        let end = line(last).map(|r| r.2).fold(f64::MIN, f64::max) + pad[1];
        (start, end.max(start))
    };
    Block {
        x: left,
        width: right - left,
        ..block
    }
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
