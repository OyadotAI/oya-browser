/** Column layout distributes only actual definite space and otherwise derives height from content. */
use super::{
    DisplayItem,
    flex::{self, FlexItem},
    flex_layout::{
        item_height, item_intrinsic, justification, own_text, place_item, self_alignment,
    },
    flow::Flow,
    sizing::{BorderBox, LayoutContext, border_width, content_size, vertical_edges},
};
use crate::{
    constants::LAYOUT_EXTENT,
    document::{AlignItems, FlexBasis, FlexWrap, JustifyContent, Node, style::Extent},
    protocol::EngineResult,
};

/** The main extent a column lays out into: a definite height, or content height that a
`min-height` can raise so `justify-content` and auto margins have space to distribute. */
#[derive(Clone, Copy, Debug)]
pub(super) enum ColumnExtent {
    /// An assigned height; percentages inside resolve against it.
    Definite(f64),
    /// Content-sized, but never below this minimum.
    AtLeast(f64),
}
impl ColumnExtent {
    /** The height percentages resolve against, when there is one. */
    fn definite(self) -> Option<f64> {
        match self {
            Self::Definite(height) => Some(height),
            Self::AtLeast(_) => None,
        }
    }
    /** The extent the items are placed into, given their summed intrinsic outer sizes. */
    fn resolve(self, intrinsic: f64) -> f64 {
        match self {
            Self::Definite(height) => height,
            Self::AtLeast(minimum) => intrinsic.max(minimum),
        }
    }
}

/** Cross widths use intrinsic size unless automatic stretch has a definite containing width. */
fn cross_width(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
    inner: f64,
) -> EngineResult<f64> {
    if own_text(parent, child) {
        let intrinsic = super::sizing::text_intrinsic(context, parent)?;
        return Ok(if parent.style.align_items == AlignItems::Stretch {
            inner
        } else {
            intrinsic.max_content.min(inner)
        });
    }
    let intrinsic = item_intrinsic(context, child)?;
    let available = (inner - child.style.margin[1] - child.style.margin[3]).max(0.0);
    let auto_cross = child.style.margin_auto[1] || child.style.margin_auto[3];
    let natural = if self_alignment(parent, child) == AlignItems::Stretch
        && child.style.width.is_none()
        && !child.style.width_fit
        && !auto_cross
    {
        None
    } else {
        Some(intrinsic.max_content)
    };
    Ok(border_width(child, inner, available, natural))
}

/**
 * A percentage basis or height resolves only when an actually assigned main extent is definite.
 * An automatic basis is the item's `height` (Flexbox 9.2 step 3), and `min-height`/`max-height`
 * bound it; without an authored minimum the item keeps at least its content or its specified
 * height, whichever is smaller (Flexbox 4.5).
 */
fn item(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
    width: f64,
    definite_height: Option<f64>,
) -> EngineResult<FlexItem> {
    if own_text(parent, child) {
        let natural = super::sizing::measure_own_text_height(context, parent, width)?;
        return Ok(FlexItem {
            base_content: natural,
            hypothetical_content: natural,
            min_content: natural,
            max_content: LAYOUT_EXTENT,
            margin_start: 0.0,
            margin_end: 0.0,
            auto_margins: [false, false],
            main_edges: 0.0,
            grow: 0.0,
            shrink: 1.0,
        });
    }
    let natural = item_height(context, child, width)?;
    let bound = |extent: Option<Extent>| {
        extent
            .and_then(|extent| extent.resolve_size(definite_height))
            .map(|value| content_size(child, value, false))
    };
    let specified = bound(child.style.height);
    let natural_content = if specified.is_some() {
        content_extent(context, child, width)?
    } else {
        (natural - vertical_edges(child)).max(0.0)
    };
    let base = match child.style.flex_basis {
        FlexBasis::Pixels(value) => content_size(child, value, false),
        FlexBasis::Percent(value) if definite_height.is_some() => content_size(
            child,
            definite_height.unwrap_or_default() * value / 100.0,
            false,
        ),
        _ => specified.unwrap_or(natural_content),
    };
    let max = bound(child.style.max_height).unwrap_or(LAYOUT_EXTENT);
    let automatic = if flex::scroll_container(child) {
        0.0
    } else {
        natural_content
            .min(specified.unwrap_or(LAYOUT_EXTENT))
            .min(max)
    };
    let minimum = bound(child.style.min_height).unwrap_or(automatic);
    let maximum = max.max(minimum);
    Ok(FlexItem {
        base_content: base,
        hypothetical_content: base.max(minimum).min(maximum),
        min_content: minimum,
        max_content: maximum,
        margin_start: child.style.margin[0],
        margin_end: child.style.margin[2],
        auto_margins: [child.style.margin_auto[0], child.style.margin_auto[2]],
        main_edges: vertical_edges(child),
        grow: child.style.flex_grow,
        shrink: child.style.flex_shrink,
    })
}

/** The height an item's content takes at `width`, ignoring its own `height`: the content size
suggestion its automatic minimum uses (css-flexbox-1 4.5), which for a `height: 20px` box with
no content is zero. Measured from where its in-flow children end below its content-box top.
ponytail: an item with its own text keeps its laid-out height; add line extents if that bites. */
fn content_extent(context: &mut LayoutContext<'_>, child: &Node, width: f64) -> EngineResult<f64> {
    let mut items = Vec::new();
    let height = place_item(
        context,
        child,
        BorderBox {
            x: 0.0,
            y: 0.0,
            width,
            height: None,
        },
        &mut items,
    )?;
    if !child.text.trim().is_empty() {
        return Ok((height - vertical_edges(child)).max(0.0));
    }
    let top = child.style.padding[0] + super::sizing::border_sides(&child.style)[0];
    let kids = context.children(child.local_id);
    let mut bottom = top;
    for kid in kids {
        if super::flow::out_of_flow(kid) {
            continue;
        }
        if let Some(item) = items.iter().find(|item| item.local_id == kid.local_id) {
            bottom = bottom.max(item.y + item.height + kid.style.margin[2]);
        }
    }
    Ok((bottom - top).max(0.0))
}

/** Auto main extent adds only child outer sizes and between-item row gaps. A `flex-wrap`
column with a definite height breaks into lines placed side by side (Blink's column
`FlexLineBreaker`); `align-content` shares the width between them. */
pub(super) fn column(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    children: &[&Node],
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
    extent: ColumnExtent,
) -> EngineResult<()> {
    let container_index = output.len().saturating_sub(1);
    let wrap = parent.style.flex_wrap == FlexWrap::Wrap && extent.definite().is_some();
    let mut records = Vec::new();
    let mut widths = Vec::new();
    for child in children {
        let width = if wrap {
            fit_width(context, parent, child, flow.width)?
        } else {
            cross_width(context, parent, child, flow.width)?
        };
        widths.push(width);
        records.push(item(context, parent, child, width, extent.definite())?);
    }
    let intrinsic = records
        .iter()
        .map(|record| record.outer(record.hypothetical_content))
        .sum::<f64>()
        + parent.style.row_gap * records.len().saturating_sub(1) as f64;
    let extent = extent.resolve(intrinsic);
    let mut bottom = flow.y;
    if !wrap {
        let line = ColumnLine {
            children,
            records: &records,
            widths: &widths,
            x: flow.x,
            width: flow.width,
        };
        bottom = place_line(
            context,
            parent,
            line,
            (flow.y, extent),
            container_index,
            output,
        )?;
    } else {
        let ranges = flex::break_lines(&records, extent, parent.style.row_gap, true);
        let mut crosses = ranges
            .iter()
            .map(|range| {
                range
                    .clone()
                    .map(|index| {
                        let margins = children[index].style.margin;
                        widths[index] + margins[1] + margins[3]
                    })
                    .fold(0.0_f64, f64::max)
            })
            .collect::<Vec<_>>();
        let gap = parent.style.column_gap;
        let mut free =
            flow.width - crosses.iter().sum::<f64>() - gap * crosses.len().saturating_sub(1) as f64;
        if free >= 0.0 && parent.style.align_content == JustifyContent::Stretch {
            let share = free / crosses.len().max(1) as f64;
            crosses.iter_mut().for_each(|cross| *cross += share);
            free = 0.0;
        }
        let (offset, between) = justification(
            parent.style.align_content,
            free,
            crosses.len(),
            parent.style.wrap_reverse,
        );
        let order = (0..ranges.len()).collect::<Vec<_>>();
        let order = if parent.style.wrap_reverse {
            order.into_iter().rev().collect()
        } else {
            order
        };
        let mut x = flow.x + offset;
        for index in order {
            let range = ranges[index].clone();
            let cross = crosses[index];
            let mut line_widths = Vec::with_capacity(range.len());
            for child in &children[range.clone()] {
                line_widths.push(cross_width(context, parent, child, cross)?);
            }
            let line = ColumnLine {
                children: &children[range.clone()],
                records: &records[range],
                widths: &line_widths,
                x,
                width: cross,
            };
            let end = place_line(
                context,
                parent,
                line,
                (flow.y, extent),
                container_index,
                output,
            )?;
            bottom = bottom.max(end);
            x += cross + gap + between;
        }
    }
    flow.y += extent;
    flow.bottom = flow.bottom.max(flow.y.max(bottom));
    Ok(())
}

/** An item's border-box width before its line's cross size is known: its own width, or its
max-content fitted into the container, which is what a column line's cross size grows to. */
fn fit_width(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
    inner: f64,
) -> EngineResult<f64> {
    if own_text(parent, child) {
        let intrinsic = super::sizing::text_intrinsic(context, parent)?;
        return Ok(intrinsic.max_content.min(inner));
    }
    let intrinsic = item_intrinsic(context, child)?;
    let available = (inner - child.style.margin[1] - child.style.margin[3]).max(0.0);
    Ok(border_width(
        child,
        inner,
        available,
        Some(intrinsic.max_content),
    ))
}

/** One column line to place: its items, their records and border-box widths, and the line's
left edge and cross size. */
struct ColumnLine<'a> {
    /** The line's items in order. */
    children: &'a [&'a Node],
    /** Their flex records. */
    records: &'a [FlexItem],
    /** Their border-box widths. */
    widths: &'a [f64],
    /** The line's left edge. */
    x: f64,
    /** The line's cross size. */
    width: f64,
}

/** Resolves one column line's flexible heights into `extent` from `top`, justifies it and lays
its items out across the line's width; returns the bottom of the last item. */
fn place_line(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    line: ColumnLine<'_>,
    (top, extent): (f64, f64),
    container_index: usize,
    output: &mut Vec<DisplayItem>,
) -> EngineResult<f64> {
    let ColumnLine {
        children,
        records,
        widths,
        x: line_x,
        width: line_width,
    } = line;
    context.charge(
        records
            .len()
            .saturating_mul(records.len().saturating_add(1)),
    )?;
    let sizes = flex::resolve_line(records, extent, parent.style.row_gap)?;
    let used = records
        .iter()
        .zip(&sizes)
        .map(|(record, size)| record.outer(*size))
        .sum::<f64>()
        + parent.style.row_gap * records.len().saturating_sub(1) as f64;
    let share = flex::auto_margin_share(records, extent - used);
    let (offset, between) = if share > 0.0 {
        (0.0, 0.0)
    } else {
        // A horizontal column's main axis runs top down, so `left` and `right` fall back to
        // `start` (Blink's `ResolvedJustifyContent`).
        let kind = match parent.style.justify_content {
            JustifyContent::Left | JustifyContent::Right => JustifyContent::LogicalStart,
            kind => kind,
        };
        justification(
            kind,
            extent - used,
            records.len(),
            parent.style.flex_reverse,
        )
    };
    let mut y = top + offset;
    // `column-reverse` places the last item at the top, with start and end swapped.
    let order = if parent.style.flex_reverse {
        (0..children.len()).rev().collect::<Vec<_>>()
    } else {
        (0..children.len()).collect()
    };
    for (position, index) in order.into_iter().enumerate() {
        let child = children[index];
        let last = position + 1 == children.len();
        if own_text(parent, child) {
            let free = (line_width - widths[index]).max(0.0);
            let offset = match parent.style.align_items {
                AlignItems::Center => free / 2.0,
                AlignItems::End => free,
                _ => 0.0,
            };
            super::sizing::place_own_text(
                context,
                parent,
                (line_x + offset, y, widths[index]),
                container_index,
                output,
            )?;
            y += sizes[index];
            if !last {
                y += parent.style.row_gap + between;
            }
            continue;
        }
        // Unsafe self-alignment, as in rows: an item wider than the line centers past both edges.
        let overflowing =
            line_width - widths[index] - child.style.margin[1] - child.style.margin[3];
        let free = overflowing.max(0.0);
        let auto_cross =
            flex::auto_cross_offset(child.style.margin_auto[3], child.style.margin_auto[1], free);
        let offset = auto_cross.unwrap_or(match self_alignment(parent, child) {
            AlignItems::Center => overflowing / 2.0,
            AlignItems::End => overflowing,
            _ => 0.0,
        });
        let [auto_start, auto_end] = records[index].auto_margins;
        y += child.style.margin[0] + if auto_start { share } else { 0.0 };
        place_item(
            context,
            child,
            BorderBox {
                x: line_x + child.style.margin[3] + offset,
                y,
                width: widths[index],
                height: Some(sizes[index] + records[index].main_edges),
            },
            output,
        )?;
        y += sizes[index]
            + records[index].main_edges
            + child.style.margin[2]
            + if auto_end { share } else { 0.0 };
        if !last {
            y += parent.style.row_gap + between;
        }
    }
    Ok(y)
}
