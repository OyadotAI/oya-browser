use super::{
    DisplayItem,
    flex::{self, FlexItem},
    flex_column::ColumnExtent,
    flow::Flow,
    sizing::{
        BorderBox, IntrinsicWidths, LayoutContext, content_size, horizontal_edges,
        specified_content,
    },
};
/** Flexible child placement shares intrinsic measurements, exact border boxes and one work budget. */
use crate::document::style::{HEIGHT, MIN_WIDTH};
use crate::{
    constants::LAYOUT_EXTENT,
    document::{AlignItems, FlexBasis, FlexDirection, FlexWrap, JustifyContent, Node},
    protocol::EngineResult,
};

/** Direct text and table formatting remain explicit unsupported boundaries for this slice.
`min_height` is the container's minimum content height: a single-line row's line is at least it. */
pub(super) fn children(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
    definite_height: Option<f64>,
    min_height: f64,
) -> EngineResult<()> {
    let resolved = resolved_gaps(node, flow.width, definite_height);
    let node = resolved.as_ref().unwrap_or(node);
    // `::before` is the first item and `::after` the last (Blink's pseudo-element children).
    let [before, after] = super::generated::items(node);
    let mut children: Vec<&Node> = before.iter().collect();
    let mut positioned = Vec::new();
    if !node.text.trim().is_empty() {
        children.push(node);
    }
    for child in context.children(node.local_id) {
        if super::flow::blank_text(child) {
            continue;
        }
        if super::flow::out_of_flow(child) {
            positioned.push(child);
        } else {
            children.push(child);
        }
    }
    children.extend(&after);
    // `order` rearranges placement only; the stable sort keeps source order among equals.
    children.sort_by_key(|child| {
        if own_text(node, child) {
            0
        } else {
            child.style.order
        }
    });
    let origin = (flow.x, flow.y);
    if node.style.flex_direction == FlexDirection::Column {
        super::flex_column::column(
            context,
            node,
            &children,
            flow,
            output,
            match (definite_height, node.style.calc_size[HEIGHT]) {
                (Some(height), _) => ColumnExtent::Definite(height),
                (None, Some(calc)) => ColumnExtent::Mapped(min_height, calc),
                (None, None) => ColumnExtent::AtLeast(min_height),
            },
        )?;
    } else {
        row(
            context,
            node,
            &children,
            flow,
            output,
            definite_height.unwrap_or(min_height),
            definite_height.is_some(),
        )?;
    }
    // Out-of-flow children wait for the container's size: they align as its sole item would.
    let inner = (flow.width, flow.y - origin.1);
    for child in positioned {
        let at = super::flex_abspos::static_position(context, node, child, origin, inner)?;
        context.defer(child.local_id, at);
    }
    Ok(())
}

/** The container with its percentage gaps resolved into lengths: a column gap against its
inner width, a row gap against its definite inner height or zero without one (css-align-3 8.3).
None when it has no percentage gap, the common case, which needs no copy. */
fn resolved_gaps(node: &Node, width: f64, height: Option<f64>) -> Option<Node> {
    let [column, row] = node.style.gap_percent;
    if column == 0.0 && row == 0.0 {
        return None;
    }
    let mut copy = node.clone();
    copy.style.column_gap += column * width / 100.0;
    copy.style.row_gap += row * height.unwrap_or(0.0) / 100.0;
    copy.style.gap_percent = [0.0; 2];
    Some(copy)
}

/** Whether an item is the container's own leading text, the one item that is not a box. Generated
items share the container's identity, so only the reference itself tells them apart. */
pub(super) fn own_text(parent: &Node, child: &Node) -> bool {
    std::ptr::eq(parent, child)
}

/** An item's intrinsic widths; a generated item has only its text, and its identity is its
element's, so the per-identity cache would answer for the element instead. */
pub(super) fn item_intrinsic(
    context: &mut LayoutContext<'_>,
    child: &Node,
) -> EngineResult<IntrinsicWidths> {
    if super::generated::is_generated(child) {
        return super::sizing::text_intrinsic(context, child);
    }
    context.intrinsic_widths(child.local_id)
}

/** An item's border-box height at `width`, measured from the node itself for a generated item. */
pub(super) fn item_height(
    context: &mut LayoutContext<'_>,
    child: &Node,
    width: f64,
) -> EngineResult<f64> {
    if !super::generated::is_generated(child) {
        return context.measure_height(child.local_id, width);
    }
    // ponytail: generated items are measured uncached; they hold one text run.
    let bounds = BorderBox {
        x: 0.0,
        y: 0.0,
        width,
        height: None,
    };
    place_item(context, child, bounds, &mut Vec::new())
}

/** Lays an item out in its assigned border box and returns its height, from the node itself for a
generated item rather than its element's identity. */
pub(super) fn place_item(
    context: &mut LayoutContext<'_>,
    child: &Node,
    bounds: BorderBox,
    output: &mut Vec<DisplayItem>,
) -> EngineResult<f64> {
    if !super::generated::is_generated(child) {
        return context.layout_at(child.local_id, bounds, output);
    }
    let start = output.len();
    let mut target = Flow::new(bounds.x, bounds.y, bounds.width);
    super::flow::node_layout(context, child, &mut target, output, Some(bounds))?;
    Ok(output.get(start).map_or(0.0, |item| item.height))
}

/** Source-defined row bases and constraints use content units, including automatic minimums. */
fn row_item(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    node: &Node,
    (inner, cross): (f64, Option<f64>),
) -> EngineResult<FlexItem> {
    if own_text(parent, node) {
        let intrinsic = super::sizing::text_intrinsic(context, parent)?;
        return Ok(FlexItem {
            base_content: intrinsic.max_content,
            hypothetical_content: intrinsic.max_content,
            min_content: intrinsic.min_content.min(intrinsic.max_content),
            max_content: LAYOUT_EXTENT,
            margin_start: 0.0,
            margin_end: 0.0,
            auto_margins: [false, false],
            main_edges: 0.0,
            grow: 0.0,
            shrink: 1.0,
        });
    }
    let mut intrinsic = item_intrinsic(context, node)?;
    let specified = specified_content(node, inner);
    // An item with an aspect ratio and a definite cross size has that size through the ratio as
    // its content size in the main axis (Blink's `ComputeFlexBasis` and its min-content size).
    let transferred = transferred_width(parent, node, cross);
    if specified.is_none()
        && let Some(width) = transferred
    {
        intrinsic = IntrinsicWidths {
            min_content: width,
            max_content: width,
        };
    }
    let base = match node.style.flex_basis {
        FlexBasis::Auto => specified.unwrap_or(intrinsic.max_content),
        FlexBasis::Content => transferred.unwrap_or(intrinsic.max_content),
        FlexBasis::Pixels(value) => content_size(node, value, true),
        // css-values-4 range checking: a basis resolving below zero is zero, as is a `calc()` one.
        FlexBasis::Percent(value) => content_size(node, (inner * value / 100.0).max(0.0), true),
        FlexBasis::Mixed { px, percent } => {
            content_size(node, (px + inner * percent / 100.0).max(0.0), true)
        }
    };
    let max = super::sizing::max_width(node, Some(inner));
    let automatic =
        |size: f64| crate::document::style::calc_size_through(&node.style, MIN_WIDTH, size);
    let minimum = match super::sizing::min_width(node, Some(inner)) {
        Some(value) => value,
        // A scroll container's automatic minimum is zero (Flexbox 4.5; Blink's
        // `ShouldApplyMinSizeAutoForChild`), so `overflow: hidden` items shrink below content.
        None if flex::scroll_container(node) => 0.0,
        // A replaced item's content size ignores its authored width (its height through the ratio).
        None if super::flow::replaced_image(node) => automatic(
            transferred
                .unwrap_or_else(|| super::sizing::replaced_suggestion(node, cross))
                .min(specified.unwrap_or(LAYOUT_EXTENT))
                .min(max),
        ),
        // A `calc-size()` min-width maps the automatic minimum (CSS Values 5).
        None => automatic(
            intrinsic
                .min_content
                .min(specified.unwrap_or(LAYOUT_EXTENT))
                .min(max),
        ),
    };
    // A table is never narrower than its min-content width, whatever its `width`, `max-width`
    // or flexing say (Blink sizes table boxes no smaller than their minimum inline size).
    let minimum = if super::table::is_table(context, node) {
        minimum.max(intrinsic.min_content)
    } else {
        minimum
    };
    let maximum = max.max(minimum);
    Ok(FlexItem {
        base_content: base,
        hypothetical_content: base.max(minimum).min(maximum),
        min_content: minimum,
        max_content: maximum,
        margin_start: node.style.margin[3],
        margin_end: node.style.margin[1],
        auto_margins: [node.style.margin_auto[3], node.style.margin_auto[1]],
        main_edges: horizontal_edges(node),
        grow: node.style.flex_grow,
        shrink: node.style.flex_shrink,
    })
}

/** The content width an item's `aspect-ratio` gives it from a definite cross size: its own
`height`, or, in a single-line container with a definite height, the stretched line. The ratio
sizes the border box under `box-sizing: border-box`. None without a ratio or a definite height. */
fn transferred_width(parent: &Node, node: &Node, cross: Option<f64>) -> Option<f64> {
    let (ratio, border_box) = match node.style.aspect_ratio {
        Some(ratio) => (
            ratio,
            node.style.box_sizing == crate::document::BoxSizing::BorderBox,
        ),
        // A stretched replaced item transfers through its natural ratio; with its own height,
        // its replaced sizing already does, min- and max-height included.
        None if super::flow::replaced_image(node) && node.style.height.is_none() => {
            super::sizing::replaced_ratio(node)?
        }
        None => return None,
    };
    let height = match node.style.height {
        Some(extent) => content_size(node, extent.resolve_size(cross)?, false),
        None => {
            let stretched = self_alignment(parent, node) == AlignItems::Stretch
                && parent.style.flex_wrap == FlexWrap::NoWrap
                && !node.style.margin_auto[0]
                && !node.style.margin_auto[2];
            let margins = node.style.margin[0] + node.style.margin[2];
            let outer = cross.filter(|_| stretched)?;
            (outer - margins - super::sizing::vertical_edges(node)).max(0.0)
        }
    };
    Some(if border_box {
        let edges = super::sizing::vertical_edges(node);
        ((height + edges) * ratio - horizontal_edges(node)).max(0.0)
    } else {
        height * ratio
    })
}

/** An item's intrinsic widths as a row container measures its own: an automatic-width item with
an aspect ratio, stretched to the container's definite height, is as wide as the ratio makes that
height (Blink's min/max-content contribution with the stretched block size), so an `inline-flex`
of height 100px around a square item is 100px wide rather than empty. */
pub(super) fn row_item_widths(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
) -> EngineResult<IntrinsicWidths> {
    let raw = context.intrinsic_widths(child.local_id)?;
    let row = parent.style.display_flex && parent.style.flex_direction == FlexDirection::Row;
    let ratio = child.style.aspect_ratio.is_some() || super::flow::replaced_image(child);
    if !row || child.style.width.is_some() || !ratio {
        return Ok(raw);
    }
    let depth = crate::constants::DOCUMENT_DEPTH;
    let cross = super::image::definite_content_height(context, parent, depth);
    Ok(
        transferred_width(parent, child, cross).map_or(raw, |width| IntrinsicWidths {
            min_content: width,
            max_content: width,
        }),
    )
}

/** One measured flex line: its items, their resolved main sizes, measured border heights,
first baselines for baseline-aligned items, and the line's cross size. */
struct MeasuredLine {
    range: std::ops::Range<usize>,
    sizes: Vec<f64>,
    heights: Vec<f64>,
    baselines: Vec<Option<f64>>,
    baseline: f64,
    cross: f64,
}

/** Wrapping precedes flexible-length resolution and each line measures actual assigned widths.
A single-line container's line takes the container's inner height when it is definite, and is at
least its min-height otherwise, so `align-items` centers and stretches within the container (CSS
Flexbox 9.4 steps 8 and 15) instead of within the tallest item. Lines are all measured before
any is placed, so `wrap-reverse` can stack them from the cross-end edge (Blink's
`FlexLayoutAlgorithm` places lines after `ComputeLineItemsPosition`). */
fn row(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    children: &[&Node],
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
    container_cross: f64,
    definite: bool,
) -> EngineResult<()> {
    let container_index = output.len().saturating_sub(1);
    let mut records = Vec::new();
    for child in children {
        records.push(row_item(
            context,
            parent,
            child,
            (flow.width, definite.then_some(container_cross)),
        )?);
    }
    let ranges = flex::break_lines_in(
        &records,
        flow.width,
        parent.style.column_gap,
        flex::Breaking::of(parent),
    );
    let mut lines = Vec::with_capacity(ranges.len());
    for range in ranges {
        let space = (flow.width, definite.then_some(container_cross));
        let line = measure_row_line(context, parent, children, &records, range, space)?;
        lines.push(line);
    }
    let content = distribute_lines(parent, &mut lines, container_cross, definite);
    let (offset, between) = justification(
        parent.style.align_content,
        safe_content(content.free, parent.style.content_safe[0]),
        lines.len(),
        parent.style.wrap_reverse,
    );
    // `wrap-reverse` stacks the lines from the cross end: Blink reverses them and places top down.
    let order: Vec<&MeasuredLine> = if parent.style.wrap_reverse {
        lines.iter().rev().collect()
    } else {
        lines.iter().collect()
    };
    let mut y = flow.y + offset;
    for line in order {
        place_row_line(
            context,
            parent,
            RowLine {
                records: &records[line.range.clone()],
                children: &children[line.range.clone()],
                line,
                y,
                container_index,
                basis: definite.then_some(container_cross),
            },
            flow,
            output,
        )?;
        y += line.cross + parent.style.row_gap + between;
    }
    flow.y += content.size;
    flow.bottom = flow.bottom.max(flow.y);
    Ok(())
}

/** The cross extent of a row container's lines and the free space `align-content` places. */
struct CrossContent {
    /** The container's inner cross size. */
    size: f64,
    /** What the lines and gaps leave of it; negative when they overflow. */
    free: f64,
}

/** Sizes the lines in the cross axis as Blink's `GiveItemsFinalPositionAndSize` does: a
single-line (`nowrap`) container's one line is the container's inner cross size, which is
definite, or its content at least its min-height; a multi-line container's lines share
non-negative free space equally when `align-content` is `normal` or `stretch`. */
fn distribute_lines(
    parent: &Node,
    lines: &mut [MeasuredLine],
    container_cross: f64,
    definite: bool,
) -> CrossContent {
    let sum = lines.iter().map(|line| line.cross).sum::<f64>()
        + parent.style.row_gap * lines.len().saturating_sub(1) as f64;
    let size = if definite {
        container_cross
    } else {
        sum.max(container_cross)
    };
    let free = size - sum;
    let multi_line = parent.style.flex_wrap == FlexWrap::Wrap;
    if let [line] = lines
        && !multi_line
    {
        line.cross = size;
        return CrossContent { size, free: 0.0 };
    }
    if multi_line && free >= 0.0 && parent.style.align_content == JustifyContent::Stretch {
        // Blink divides in 1/64 px LayoutUnits, truncating each share.
        let share = super::sizing::layout_unit_floor(free / lines.len().max(1) as f64);
        for line in lines.iter_mut() {
            line.cross += share;
        }
        return CrossContent { size, free: 0.0 };
    }
    CrossContent { size, free }
}

/** Resolves one line's flexible lengths, measures each item at its size, and sizes the line:
the largest outer height, or for baseline-aligned items the largest ascent plus the largest
descent below their shared baseline (Flexbox 9.4 step 8). */
fn measure_row_line(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    children: &[&Node],
    all: &[FlexItem],
    range: std::ops::Range<usize>,
    (inner, basis): (f64, Option<f64>),
) -> EngineResult<MeasuredLine> {
    let records = &all[range.clone()];
    context.charge(
        records
            .len()
            .saturating_mul(records.len().saturating_add(1)),
    )?;
    let sizes = flex::resolve_line(records, inner, parent.style.column_gap)?;
    let line = (&children[range.clone()], records, sizes.as_slice());
    let heights = measure_line(context, parent, line, basis)?;
    let (mut cross, mut ascent, mut descent) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut baselines = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let child = children[range.start + index];
        if own_text(parent, child) {
            cross = cross.max(heights[index]);
            baselines.push(None);
            continue;
        }
        let margins = child.style.margin[0] + child.style.margin[2];
        let outer = heights[index] + margins;
        if baseline_aligned(parent, child) {
            let width = sizes[index] + record.main_edges;
            let own = first_baseline(context, child, width)?.unwrap_or(heights[index]);
            let above = child.style.margin[0] + own;
            ascent = ascent.max(above);
            descent = descent.max(outer - above);
            baselines.push(Some(own));
        } else {
            cross = cross.max(outer);
            baselines.push(None);
        }
    }
    Ok(MeasuredLine {
        range,
        sizes,
        heights,
        baselines,
        baseline: ascent,
        cross: cross.max(ascent + descent),
    })
}

/** Whether an item takes part in baseline alignment: `align-self` (or the container's
`align-items`) is `baseline` and neither cross margin is `auto`. */
fn baseline_aligned(parent: &Node, child: &Node) -> bool {
    self_alignment(parent, child) == AlignItems::Baseline
        && !child.style.margin_auto[0]
        && !child.style.margin_auto[2]
}

/** An item's first baseline from its border-box top: the top line box's text baseline, found by
laying it out at its used width; None when it has no line, so the caller synthesizes the
border-box bottom as Blink does for boxes without a baseline. */
pub(super) fn first_baseline(
    context: &mut LayoutContext<'_>,
    node: &Node,
    width: f64,
) -> EngineResult<Option<f64>> {
    let mut items = Vec::new();
    place_item(
        context,
        node,
        BorderBox {
            x: 0.0,
            y: 0.0,
            width,
            height: None,
        },
        &mut items,
    )?;
    let mut best: Option<(f64, usize)> = None;
    for item in &items {
        context.charge(1)?;
        if let Some(line) = item.lines.iter().min_by(|a, b| a.y.total_cmp(&b.y))
            && best.is_none_or(|(y, _)| line.y < y)
        {
            best = Some((line.y, item.local_id));
        }
    }
    let Some((y, local)) = best else {
        return Ok(None);
    };
    // A generated item's lines carry its element's identity but its own style.
    let style = if local == node.local_id {
        &node.style
    } else {
        &context.node(local)?.style
    };
    Ok(Some(y + super::sizing::text_ascent(style)))
}

/** Dry-run and final passes share memoization; measurements include validation rows. */
fn measure_line(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    (children, records, sizes): (&[&Node], &[FlexItem], &[f64]),
    basis: Option<f64>,
) -> EngineResult<Vec<f64>> {
    records
        .iter()
        .zip(children)
        .zip(sizes)
        .map(|((record, child), size)| {
            if own_text(parent, child) {
                super::sizing::measure_own_text_height(context, parent, *size)
            } else if let Some(height) = percent_height(child, basis) {
                Ok(height)
            } else {
                let height = item_height(context, child, size + record.main_edges)?;
                Ok(super::flow::clamp_height(child, height, basis))
            }
        })
        .collect()
}

/** The leading offset and the extra space between neighbours that content distribution gives
`count` items or lines sharing `free` space, as Blink's `InitialContentPositionOffset` and
`ContentDistributionSpace`: a position may go negative when the content overflows (unsafe
alignment), a distribution falls back to `flex-start` or safe `center`, and on a `reverse`d axis
`flex-start` packs at the far end because the items are placed in reverse order. */
pub(super) fn justification(
    kind: JustifyContent,
    free: f64,
    count: usize,
    reverse: bool,
) -> (f64, f64) {
    let start = if reverse { free } else { 0.0 };
    let distributed = free > 0.0 && count > 0;
    let n = count.max(1) as f64;
    match kind {
        JustifyContent::SpaceBetween if distributed && count > 1 => (0.0, free / (n - 1.0)),
        JustifyContent::SpaceAround if distributed => (free / (2.0 * n), free / n),
        JustifyContent::SpaceEvenly if distributed => (free / (n + 1.0), free / (n + 1.0)),
        JustifyContent::SpaceAround | JustifyContent::SpaceEvenly => ((free / 2.0).max(0.0), 0.0),
        JustifyContent::SpaceBetween | JustifyContent::Stretch | JustifyContent::Start => {
            (start, 0.0)
        }
        JustifyContent::Center => (free / 2.0, 0.0),
        JustifyContent::End => (if reverse { 0.0 } else { free }, 0.0),
        JustifyContent::LogicalStart | JustifyContent::Left => (0.0, 0.0),
        JustifyContent::LogicalEnd | JustifyContent::Right => (free, 0.0),
    }
}

/** The free space content distribution aligns in: `safe` alignment never lets content that
overflows its container spill past the start edge, so it packs at the start instead. */
pub(super) fn safe_content(free: f64, safe: bool) -> f64 {
    if safe { free.max(0.0) } else { free }
}

/** A row's `justify-content` with `left` and `right` coerced to their logical sides, as Blink's
`ResolvedJustifyContent`: `left` is the start of a left-to-right row and the end of a
right-to-left one. */
pub(super) fn row_justify(parent: &Node) -> JustifyContent {
    match (parent.style.justify_content, parent.style.rtl) {
        (JustifyContent::Left, false) | (JustifyContent::Right, true) => {
            JustifyContent::LogicalStart
        }
        (JustifyContent::Left, true) | (JustifyContent::Right, false) => JustifyContent::LogicalEnd,
        (kind, _) => kind,
    }
}

/** Whether a row's items run right to left physically: `row-reverse` in a left-to-right
container, or `row` in a right-to-left one, whose main-start is its right edge. */
fn physically_reversed(parent: &Node) -> bool {
    parent.style.flex_reverse != parent.style.rtl
}

/** One measured line and where it goes, as one placement input. */
struct RowLine<'a> {
    /** Source-order flex records for this line. */
    records: &'a [FlexItem],
    /** The line's items, matching `records`. */
    children: &'a [&'a Node],
    /** The line's resolved sizes, heights, baselines and cross size. */
    line: &'a MeasuredLine,
    /** The line's top edge. */
    y: f64,
    /** Display-item index of the flex container, used when placing its own leading text. */
    container_index: usize,
    /** The container's definite inner height, which items' percentage heights resolve against. */
    basis: Option<f64>,
}

/** Physical left-to-right order of a line's items: source order, or reversed when the main axis
runs right to left (`row-reverse`, or `row` under `direction: rtl`). */
fn line_order(parent: &Node, count: usize) -> Vec<usize> {
    if physically_reversed(parent) {
        (0..count).rev().collect()
    } else {
        (0..count).collect()
    }
}

/** Cross-axis alignment as placed top down: `wrap-reverse` swaps start and end. */
pub(super) fn physical_align(parent: &Node, align: AlignItems) -> AlignItems {
    match (parent.style.wrap_reverse, align) {
        (true, AlignItems::Start | AlignItems::Baseline) => AlignItems::End,
        (true, AlignItems::End) => AlignItems::Start,
        (_, align) => align,
    }
}

/** Allocated cross-axis stretch does not rerun the main-axis width algorithm. */
fn place_row_line(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    line: RowLine<'_>,
    flow: &Flow,
    output: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let RowLine {
        records,
        children,
        line,
        y,
        container_index,
        basis,
    } = line;
    let (sizes, heights, cross) = (&line.sizes, &line.heights, line.cross);
    let used = records
        .iter()
        .zip(sizes)
        .map(|(record, size)| record.outer(*size))
        .sum::<f64>()
        + parent.style.column_gap * records.len().saturating_sub(1) as f64;
    let share = flex::auto_margin_share(records, flow.width - used);
    let free = flow.width - used;
    let (offset, between) = if share > 0.0 {
        (0.0, 0.0)
    } else {
        // Blink computes the offset along the inline direction; a right-to-left row measures it
        // from the right edge, so the physical lead is what the logical trail leaves.
        let (lead, between) = justification(
            row_justify(parent),
            safe_content(free, parent.style.content_safe[1]),
            records.len(),
            parent.style.flex_reverse,
        );
        if parent.style.rtl {
            let gaps = records.len().saturating_sub(1) as f64;
            (free - lead - between * gaps, between)
        } else {
            (lead, between)
        }
    };
    let mut x = flow.x + offset;
    for index in line_order(parent, records.len()) {
        let (record, child) = (&records[index], children[index]);
        if own_text(parent, child) {
            let free_cross = (cross - heights[index]).max(0.0);
            let offset = match physical_align(parent, parent.style.align_items) {
                AlignItems::Center => free_cross / 2.0,
                AlignItems::End => free_cross,
                _ => 0.0,
            };
            super::sizing::place_own_text(
                context,
                parent,
                (x, y + offset, sizes[index]),
                container_index,
                output,
            )?;
            x += sizes[index] + parent.style.column_gap + between;
            continue;
        }
        let [auto_start, auto_end] = record.auto_margins;
        x += record.margin_start + if auto_start { share } else { 0.0 };
        // Self-alignment is unsafe by default, as in Blink's `AlignmentOffset`: an item taller
        // than its line centers or ends past the line's edges (LinkedIn's 840px image in a 720px
        // row sits 60px above it). Auto margins never go negative.
        let overflowing = safe_free(
            child,
            cross - heights[index] - child.style.margin[0] - child.style.margin[2],
        );
        let free_cross = overflowing.max(0.0);
        let align = self_alignment(parent, child);
        let auto_cross = flex::auto_cross_offset(
            child.style.margin_auto[0],
            child.style.margin_auto[2],
            free_cross,
        );
        let baseline = line.baselines[index]
            .filter(|_| !parent.style.wrap_reverse)
            .map(|own| line.baseline - child.style.margin[0] - own);
        let offset = auto_cross
            .or(baseline)
            .unwrap_or(match physical_align(parent, align) {
                AlignItems::Center => overflowing / 2.0,
                AlignItems::End => overflowing,
                _ => 0.0,
            });
        // Only an automatic height stretches; an authored height or a cross auto margin keeps
        // its size at the line start. A stretched size wins over `aspect-ratio`, which then only
        // gave the main size (Blink treats the stretched cross size as definite).
        let height = (align == AlignItems::Stretch
            && auto_cross.is_none()
            && child.style.height.is_none()
            && !child.style.height_fit)
            .then(|| {
                let stretched = (cross - child.style.margin[0] - child.style.margin[2]).max(0.0);
                super::flow::clamp_height(child, stretched, basis)
            })
            .or_else(|| percent_bounded(child, basis).then_some(heights[index]));
        place_item(
            context,
            child,
            BorderBox {
                x,
                y: y + child.style.margin[0] + offset,
                width: sizes[index] + record.main_edges,
                height,
            },
            output,
        )?;
        x += sizes[index]
            + record.main_edges
            + record.margin_end
            + if auto_end { share } else { 0.0 }
            + parent.style.column_gap
            + between;
    }
    Ok(())
}

/** An item's own cross-axis alignment: `align-self`, or the container's `align-items` for `auto`. */
pub(super) fn self_alignment(parent: &Node, child: &Node) -> AlignItems {
    child
        .style
        .align_self_safe
        .or(child.style.align_self)
        .unwrap_or(parent.style.align_items)
}

/** The cross space an item aligns in: `safe` alignment (css-align-3 4.4) keeps an item that
overflows its line at the start rather than past it. */
pub(super) fn safe_free(child: &Node, free: f64) -> f64 {
    if child.style.align_self_safe.is_some() {
        free.max(0.0)
    } else {
        free
    }
}

/** An item's border-box height from a percentage `height` resolved against the container's
definite inner height, min- and max-height applied. Items lay out in a box of their own, which
has no containing height to resolve it against, so a `height: 50%` item in a 200px row was only
as tall as its content. None for any other height or an indefinite container. */
fn percent_height(child: &Node, basis: Option<f64>) -> Option<f64> {
    let extent = child.style.height.filter(|extent| extent.percent != 0.0)?;
    let content = extent.resolve_size(basis)?;
    let height = super::sizing::border_height(child, content);
    Some(super::flow::clamp_height(child, height, basis))
}

/** Whether an item's height, min-height or max-height is a percentage the container's definite
inner height resolves: its measured height then already holds them, and is placed as given. */
fn percent_bounded(child: &Node, basis: Option<f64>) -> bool {
    let style = &child.style;
    basis.is_some()
        && [style.height, style.min_height, style.max_height]
            .iter()
            .any(|extent| extent.is_some_and(|extent| extent.percent != 0.0))
}
