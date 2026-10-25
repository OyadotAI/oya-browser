/** Flexible child placement shares intrinsic measurements, exact border boxes and one work budget. */
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
    // `::before` is the first item and `::after` the last (Blink's pseudo-element children).
    let [before, after] = super::generated::items(node);
    let mut children: Vec<&Node> = before.iter().collect();
    if !node.text.trim().is_empty() {
        children.push(node);
    }
    for child in context.children(node.local_id) {
        if super::flow::blank_text(child) {
            continue;
        }
        if super::flow::out_of_flow(child) {
            context.defer(child.local_id, (flow.x, flow.y));
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
    if node.style.flex_direction == FlexDirection::Column {
        return super::flex_column::column(
            context,
            node,
            &children,
            flow,
            output,
            definite_height.map_or(ColumnExtent::AtLeast(min_height), ColumnExtent::Definite),
        );
    }
    row(
        context,
        node,
        &children,
        flow,
        output,
        definite_height.unwrap_or(min_height),
        definite_height.is_some(),
    )
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
    if specified.is_none()
        && let Some(width) = transferred_width(parent, node, cross)
    {
        intrinsic = IntrinsicWidths {
            min_content: width,
            max_content: width,
        };
    }
    let base = match node.style.flex_basis {
        FlexBasis::Auto => specified.unwrap_or(intrinsic.max_content),
        FlexBasis::Pixels(value) => content_size(node, value, true),
        FlexBasis::Percent(value) => content_size(node, inner * value / 100.0, true),
    };
    let max = super::sizing::max_width(node, Some(inner));
    let minimum = match super::sizing::min_width(node, Some(inner)) {
        Some(value) => value,
        // A scroll container's automatic minimum is zero (Flexbox 4.5; Blink's
        // `ShouldApplyMinSizeAutoForChild`), so `overflow: hidden` items shrink below content.
        None if flex::scroll_container(node) => 0.0,
        None => intrinsic
            .min_content
            .min(specified.unwrap_or(LAYOUT_EXTENT))
            .min(max),
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
    let ratio = node.style.aspect_ratio?;
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
    Some(
        if node.style.box_sizing == crate::document::BoxSizing::BorderBox {
            let edges = super::sizing::vertical_edges(node);
            ((height + edges) * ratio - horizontal_edges(node)).max(0.0)
        } else {
            height * ratio
        },
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
        let line = measure_row_line(context, parent, children, &records, range, flow.width)?;
        lines.push(line);
    }
    let content = distribute_lines(parent, &mut lines, container_cross, definite);
    let (offset, between) = justification(
        parent.style.align_content,
        content.free,
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
        let share = free / lines.len().max(1) as f64;
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
    inner: f64,
) -> EngineResult<MeasuredLine> {
    let records = &all[range.clone()];
    context.charge(
        records
            .len()
            .saturating_mul(records.len().saturating_add(1)),
    )?;
    let sizes = flex::resolve_line(records, inner, parent.style.column_gap)?;
    let heights = measure_line(context, parent, &children[range.clone()], records, &sizes)?;
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
    children: &[&Node],
    records: &[FlexItem],
    sizes: &[f64],
) -> EngineResult<Vec<f64>> {
    records
        .iter()
        .zip(children)
        .zip(sizes)
        .map(|((record, child), size)| {
            if own_text(parent, child) {
                super::sizing::measure_own_text_height(context, parent, *size)
            } else {
                item_height(context, child, size + record.main_edges)
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

/** A row's `justify-content` with `left` and `right` coerced to their logical sides, as Blink's
`ResolvedJustifyContent`: `left` is the start of a left-to-right row and the end of a
right-to-left one. */
fn row_justify(parent: &Node) -> JustifyContent {
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
fn physical_align(parent: &Node, align: AlignItems) -> AlignItems {
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
            free,
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
        let overflowing = cross - heights[index] - child.style.margin[0] - child.style.margin[2];
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
        let height =
            (align == AlignItems::Stretch && auto_cross.is_none() && child.style.height.is_none())
                .then_some((cross - child.style.margin[0] - child.style.margin[2]).max(0.0));
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
    child.style.align_self.unwrap_or(parent.style.align_items)
}
