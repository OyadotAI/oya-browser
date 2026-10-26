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
        // A replaced item is laid out against the definite cross space, as Blink does, so one
        // with only a ratio stretches into it rather than collapsing to its zero contribution.
        super::sizing::replaced_fit(child, available).or(Some(intrinsic.max_content))
    };
    Ok(border_width(child, inner, available, natural))
}

/**
 * A percentage basis or height resolves only when an actually assigned main extent is definite.
 * An automatic basis is the item's `height` (Flexbox 9.2 step 3), and `min-height`/`max-height`
 * bound it; without an authored minimum the item keeps at least its content or its specified
 * height, whichever is smaller (Flexbox 4.5).
 */
pub(super) fn item(
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
    let natural_content = if let Some(height) = replaced_content_height(child, width) {
        height
    } else if specified.is_some() {
        // Content sizes are measured at an indefinite height (Blink's min-content block size):
        // percentages and a nested column's flexing ignore the item's own height.
        context.indefinite_item = Some(child.local_id);
        let extent = content_extent(context, child, width);
        context.indefinite_item = None;
        extent?
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
        FlexBasis::Content => natural_content,
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
        // Text alone is as tall as its lines, whatever the item's own height says.
        if context.children(child.local_id).is_empty() {
            let inner = width - super::sizing::horizontal_edges(child);
            return super::sizing::measure_own_text_height(context, child, inner);
        }
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
    // A balanced column wraps even with an indefinite height: `flex-line-count` alone can make
    // lines, which then size the column by the longest of them (Blink's
    // `max_sum_hypothetical_main_size`).
    let breaking = flex::Breaking::of(parent);
    let balanced = matches!(breaking, flex::Breaking::Balance(_));
    // Without a definite height the lines break at a fixed `max-height`, as in Blink.
    let limit = extent
        .definite()
        .or_else(|| super::flex_column_intrinsic::break_size(parent));
    let wrap = parent.style.flex_wrap == FlexWrap::Wrap && (limit.is_some() || balanced);
    let mut records = Vec::new();
    let mut widths = Vec::new();
    // Balancing into `flex-line-count` lines gives each item that share of the cross space to
    // fit into (Blink's `BuildSpaceForLayout` divides the available inline size).
    let available = match breaking {
        flex::Breaking::Balance(count) if wrap => {
            let count = f64::from(count);
            ((flow.width - (count - 1.0) * parent.style.column_gap) / count).max(0.0)
        }
        _ => flow.width,
    };
    for child in children {
        let width = if wrap {
            fit_width(context, parent, child, (flow.width, available))?
        } else {
            cross_width(context, parent, child, flow.width)?
        };
        widths.push(width);
        records.push(item(context, parent, child, width, extent.definite())?);
    }
    let gap = parent.style.row_gap;
    let ranges = if wrap {
        let size = limit.unwrap_or(crate::constants::LAYOUT_EXTENT);
        flex::break_lines_in(&records, size, gap, breaking)
    } else {
        std::iter::once(0..records.len()).collect()
    };
    let intrinsic = ranges
        .iter()
        .map(|range| {
            records[range.clone()]
                .iter()
                .map(|record| record.outer(record.hypothetical_content))
                .sum::<f64>()
                + gap * range.len().saturating_sub(1) as f64
        })
        .fold(0.0_f64, f64::max);
    let main_definite = extent.definite().is_some();
    let extent = extent.resolve(intrinsic);
    let mut bottom = flow.y;
    if !wrap {
        let line = ColumnLine {
            children,
            records: &records,
            widths: &widths,
            x: flow.x,
            width: flow.width,
            main_definite,
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
            // Blink divides in 1/64 px LayoutUnits, truncating each share.
            let share = super::sizing::layout_unit_floor(free / crosses.len().max(1) as f64);
            crosses.iter_mut().for_each(|cross| *cross += share);
            free = 0.0;
        }
        let (offset, between) = justification(
            parent.style.align_content,
            super::flex_layout::safe_content(free, parent.style.content_safe[0]),
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
                main_definite,
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
max-content fitted into the space it may take, which is what a column line's cross size grows
to. Percentages resolve against the container's inner width `basis`, while `fit` is the space
to fit into (a balanced line's share of it). */
pub(super) fn fit_width(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    child: &Node,
    (inner, fit): (f64, f64),
) -> EngineResult<f64> {
    if own_text(parent, child) {
        let intrinsic = super::sizing::text_intrinsic(context, parent)?;
        return Ok(intrinsic.max_content.min(fit));
    }
    let intrinsic = item_intrinsic(context, child)?;
    let available = (fit - child.style.margin[1] - child.style.margin[3]).max(0.0);
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
    /** Whether the container's main size is definite, which makes flexed heights definite. */
    main_definite: bool,
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
        main_definite,
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
            super::flex_layout::safe_content(extent - used, parent.style.content_safe[1]),
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
        let overflowing = super::flex_layout::safe_free(
            child,
            line_width - widths[index] - child.style.margin[1] - child.style.margin[3],
        );
        let free = overflowing.max(0.0);
        let auto_cross =
            flex::auto_cross_offset(child.style.margin_auto[3], child.style.margin_auto[1], free);
        let offset = auto_cross.unwrap_or(match self_alignment(parent, child) {
            AlignItems::Center => overflowing / 2.0,
            AlignItems::End => overflowing,
            _ => 0.0,
        });
        // Under rtl the cross start is the right edge, so start-aligned items overflow left.
        let offset = match (parent.style.rtl, auto_cross) {
            (true, None) => overflowing - offset,
            _ => offset,
        };
        let [auto_start, auto_end] = records[index].auto_margins;
        y += child.style.margin[0] + if auto_start { share } else { 0.0 };
        if !main_definite && indefinite_basis(child) {
            context.indefinite_item = Some(child.local_id);
        }
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
        context.indefinite_item = None;
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

/** Whether an item's used flex basis is indefinite in a column of indefinite height (Blink's
`is_used_flex_basis_indefinite`): `content`, a percentage, or `auto` over an automatic or
percentage height, unless an `aspect-ratio` gives the height. Such an item's flexed height does
not make it definite, so its `height: 100%` children stay content-sized. */
fn indefinite_basis(child: &Node) -> bool {
    let indefinite = match child.style.flex_basis {
        FlexBasis::Pixels(_) => false,
        FlexBasis::Auto => child
            .style
            .height
            .and_then(|height| height.resolve_size(None))
            .is_none(),
        FlexBasis::Content | FlexBasis::Percent(_) => true,
    };
    indefinite && child.style.aspect_ratio.is_none()
}

/** A replaced item's content height at border-box `width` through its aspect ratio, ignoring its
own height: its content size suggestion (css-flexbox-1 4.5). It has no children to measure, so
a `height: 100px` image in a 10px column otherwise had a zero automatic minimum and collapsed. */
fn replaced_content_height(child: &Node, width: f64) -> Option<f64> {
    if !super::flow::replaced_image(child) {
        return None;
    }
    let (ratio, border_box) = super::sizing::replaced_ratio(child)?;
    let edges = super::sizing::horizontal_edges(child);
    Some(if border_box {
        (width / ratio - vertical_edges(child)).max(0.0)
    } else {
        (width - edges).max(0.0) / ratio
    })
}
