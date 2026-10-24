/** Flexible child placement shares intrinsic measurements, exact border boxes and one work budget. */
use super::{
    DisplayItem,
    flex::{self, FlexItem},
    flex_column::ColumnExtent,
    flow::Flow,
    sizing::{BorderBox, LayoutContext, content_size, horizontal_edges, specified_content},
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
    let mut children = Vec::new();
    if !node.text.trim().is_empty() {
        children.push(node);
    }
    for child in context.children(node.local_id) {
        if super::flow::out_of_flow(child) {
            context.defer(child.local_id, (flow.x, flow.y));
        } else {
            children.push(child);
        }
    }
    // `order` rearranges placement only; the stable sort keeps source order among equals.
    children.sort_by_key(|child| {
        if child.local_id == node.local_id {
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

/** Source-defined row bases and constraints use content units, including automatic minimums. */
fn row_item(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    node: &Node,
    inner: f64,
) -> EngineResult<FlexItem> {
    if node.local_id == parent.local_id {
        let intrinsic = super::sizing::text_intrinsic(context, parent)?;
        return Ok(FlexItem {
            local_id: parent.local_id,
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
    let intrinsic = context.intrinsic_widths(node.local_id)?;
    let specified = specified_content(node, inner);
    let base = match node.style.flex_basis {
        FlexBasis::Auto => specified.unwrap_or(intrinsic.max_content),
        FlexBasis::Pixels(value) => content_size(node, value, true),
        FlexBasis::Percent(value) => content_size(node, inner * value / 100.0, true),
    };
    let max = super::sizing::max_width(node, Some(inner));
    let minimum = node
        .style
        .min_width
        .map(|value| content_size(node, value, true))
        .unwrap_or(
            intrinsic
                .min_content
                .min(specified.unwrap_or(LAYOUT_EXTENT))
                .min(max),
        );
    let maximum = max.max(minimum);
    Ok(FlexItem {
        local_id: node.local_id,
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

/** Wrapping precedes flexible-length resolution and each line measures actual assigned widths.
A single-line container's line takes the container's inner height when it is definite, and is at
least its min-height otherwise, so `align-items` centers and stretches within the container (CSS
Flexbox 9.4 steps 8 and 15) instead of within the tallest item. */
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
        records.push(row_item(context, parent, child, flow.width)?);
    }
    let lines = flex::break_lines(
        &records,
        flow.width,
        parent.style.column_gap,
        parent.style.flex_wrap == FlexWrap::Wrap,
    );
    let mut y = flow.y;
    for (line_index, range) in lines.iter().enumerate() {
        let records = &records[range.clone()];
        context.charge(
            records
                .len()
                .saturating_mul(records.len().saturating_add(1)),
        )?;
        let sizes = flex::resolve_line(records, flow.width, parent.style.column_gap)?;
        let heights = measure_line(context, parent, records, &sizes)?;
        let cross = heights
            .iter()
            .enumerate()
            .map(|(index, height)| {
                let child = children[range.start + index];
                if child.local_id == parent.local_id {
                    *height
                } else {
                    height + child.style.margin[0] + child.style.margin[2]
                }
            })
            .fold(0.0, f64::max);
        let cross = match (lines.len(), definite) {
            (1, true) => container_cross,
            (1, false) => cross.max(container_cross),
            _ => cross,
        };
        place_row_line(
            context,
            parent,
            RowLine {
                records,
                sizes: &sizes,
                heights: &heights,
                y,
                cross,
                container_index,
            },
            flow,
            output,
        )?;
        y += cross;
        if line_index + 1 < lines.len() {
            y += parent.style.row_gap;
        }
    }
    flow.y = y;
    flow.bottom = flow.bottom.max(y);
    Ok(())
}

/** Dry-run and final passes share memoization; measurements include validation rows. */
fn measure_line(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    records: &[FlexItem],
    sizes: &[f64],
) -> EngineResult<Vec<f64>> {
    records
        .iter()
        .zip(sizes)
        .map(|(record, size)| {
            if record.local_id == parent.local_id {
                super::sizing::measure_own_text_height(context, parent, *size)
            } else {
                context.measure_height(record.local_id, size + record.main_edges)
            }
        })
        .collect()
}

/** Justification applies only leftover nonnegative space after flexing and authored margins. */
pub(super) fn justification(kind: JustifyContent, free: f64, count: usize) -> (f64, f64) {
    let free = free.max(0.0);
    match kind {
        JustifyContent::Start => (0.0, 0.0),
        JustifyContent::Center => (free / 2.0, 0.0),
        JustifyContent::End => (free, 0.0),
        JustifyContent::SpaceBetween if count > 1 => (0.0, free / (count - 1) as f64),
        JustifyContent::SpaceBetween => (0.0, 0.0),
        JustifyContent::SpaceAround => {
            let share = free / count.max(1) as f64;
            (share / 2.0, share)
        }
        JustifyContent::SpaceEvenly => {
            let share = free / (count + 1) as f64;
            (share, share)
        }
    }
}

/** Measured records and their shared cross extent form one placement input. */
struct RowLine<'a> {
    /** Source-order flex records for this line. */
    records: &'a [FlexItem],
    /** Resolved content widths corresponding to the records. */
    sizes: &'a [f64],
    /** Measured border heights at those exact widths. */
    heights: &'a [f64],
    /** Shared line origin after preceding rows and gaps. */
    y: f64,
    /** Largest outer height used for alignment and stretching. */
    cross: f64,
    /** Display-item index of the flex container, used when placing its own leading text. */
    container_index: usize,
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
        sizes,
        heights,
        y,
        cross,
        container_index,
    } = line;
    let used = records
        .iter()
        .zip(sizes)
        .map(|(record, size)| record.outer(*size))
        .sum::<f64>()
        + parent.style.column_gap * records.len().saturating_sub(1) as f64;
    let share = flex::auto_margin_share(records, flow.width - used);
    let (offset, between) = if share > 0.0 {
        (0.0, 0.0)
    } else {
        justification(
            parent.style.justify_content,
            flow.width - used,
            records.len(),
        )
    };
    let mut x = flow.x + offset;
    for (index, record) in records.iter().enumerate() {
        if record.local_id == parent.local_id {
            let free_cross = (cross - heights[index]).max(0.0);
            let offset = match parent.style.align_items {
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
        let child = context.node(record.local_id)?;
        let [auto_start, auto_end] = record.auto_margins;
        x += record.margin_start + if auto_start { share } else { 0.0 };
        let free_cross =
            (cross - heights[index] - child.style.margin[0] - child.style.margin[2]).max(0.0);
        let align = self_alignment(parent, child);
        let auto_cross = flex::auto_cross_offset(
            child.style.margin_auto[0],
            child.style.margin_auto[2],
            free_cross,
        );
        let offset = auto_cross.unwrap_or(match align {
            AlignItems::Center => free_cross / 2.0,
            AlignItems::End => free_cross,
            _ => 0.0,
        });
        // Only an automatic height stretches; an authored height, an aspect ratio or a cross
        // auto margin keeps its size at the line start.
        let height = (align == AlignItems::Stretch
            && auto_cross.is_none()
            && child.style.height.is_none()
            && child.style.aspect_ratio.is_none())
        .then_some((cross - child.style.margin[0] - child.style.margin[2]).max(0.0));
        context.layout_at(
            record.local_id,
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
