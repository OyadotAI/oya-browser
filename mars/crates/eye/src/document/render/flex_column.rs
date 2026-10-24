/** Column layout distributes only actual definite space and otherwise derives height from content. */
use super::{
    DisplayItem,
    flex::{self, FlexItem},
    flex_layout::{justification, self_alignment},
    flow::Flow,
    sizing::{BorderBox, LayoutContext, border_width, content_size, vertical_edges},
};
use crate::{
    constants::LAYOUT_EXTENT,
    document::{AlignItems, FlexBasis, Node, style::Extent},
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
    if child.local_id == parent.local_id {
        let intrinsic = super::sizing::text_intrinsic(context, parent)?;
        return Ok(if parent.style.align_items == AlignItems::Stretch {
            inner
        } else {
            intrinsic.max_content.min(inner)
        });
    }
    let intrinsic = context.intrinsic_widths(child.local_id)?;
    let available = (inner - child.style.margin[1] - child.style.margin[3]).max(0.0);
    let auto_cross = child.style.margin_auto[1] || child.style.margin_auto[3];
    let natural = if self_alignment(parent, child) == AlignItems::Stretch
        && child.style.width.is_none()
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
    if child.local_id == parent.local_id {
        let natural = super::sizing::measure_own_text_height(context, parent, width)?;
        return Ok(FlexItem {
            local_id: parent.local_id,
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
    let natural = context.measure_height(child.local_id, width)?;
    let natural_content = (natural - vertical_edges(child)).max(0.0);
    let bound = |extent: Option<Extent>| {
        extent
            .and_then(|extent| extent.resolve_size(definite_height))
            .map(|value| content_size(child, value, false))
    };
    let specified = bound(child.style.height);
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
    let minimum = bound(child.style.min_height).unwrap_or(
        natural_content
            .min(specified.unwrap_or(LAYOUT_EXTENT))
            .min(max),
    );
    let maximum = max.max(minimum);
    Ok(FlexItem {
        local_id: child.local_id,
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

/** Auto main extent adds only child outer sizes and between-item row gaps. */
pub(super) fn column(
    context: &mut LayoutContext<'_>,
    parent: &Node,
    children: &[&Node],
    flow: &mut Flow,
    output: &mut Vec<DisplayItem>,
    extent: ColumnExtent,
) -> EngineResult<()> {
    let container_index = output.len().saturating_sub(1);
    let mut records = Vec::new();
    let mut widths = Vec::new();
    for child in children {
        let width = cross_width(context, parent, child, flow.width)?;
        widths.push(width);
        records.push(item(context, parent, child, width, extent.definite())?);
    }
    let intrinsic = records
        .iter()
        .map(|record| record.outer(record.hypothetical_content))
        .sum::<f64>()
        + parent.style.row_gap * records.len().saturating_sub(1) as f64;
    let extent = extent.resolve(intrinsic);
    context.charge(
        records
            .len()
            .saturating_mul(records.len().saturating_add(1)),
    )?;
    let sizes = flex::resolve_line(&records, extent, parent.style.row_gap)?;
    let used = records
        .iter()
        .zip(&sizes)
        .map(|(record, size)| record.outer(*size))
        .sum::<f64>()
        + parent.style.row_gap * records.len().saturating_sub(1) as f64;
    let share = flex::auto_margin_share(&records, extent - used);
    let (offset, between) = if share > 0.0 {
        (0.0, 0.0)
    } else {
        justification(parent.style.justify_content, extent - used, records.len())
    };
    let mut y = flow.y + offset;
    for (index, child) in children.iter().enumerate() {
        if child.local_id == parent.local_id {
            let free = (flow.width - widths[index]).max(0.0);
            let offset = match parent.style.align_items {
                AlignItems::Center => free / 2.0,
                AlignItems::End => free,
                _ => 0.0,
            };
            super::sizing::place_own_text(
                context,
                parent,
                (flow.x + offset, y, widths[index]),
                container_index,
                output,
            )?;
            y += sizes[index];
            if index + 1 < children.len() {
                y += parent.style.row_gap + between;
            }
            continue;
        }
        let free =
            (flow.width - widths[index] - child.style.margin[1] - child.style.margin[3]).max(0.0);
        let auto_cross =
            flex::auto_cross_offset(child.style.margin_auto[3], child.style.margin_auto[1], free);
        let offset = auto_cross.unwrap_or(match self_alignment(parent, child) {
            AlignItems::Center => free / 2.0,
            AlignItems::End => free,
            _ => 0.0,
        });
        let [auto_start, auto_end] = records[index].auto_margins;
        y += child.style.margin[0] + if auto_start { share } else { 0.0 };
        context.layout_at(
            child.local_id,
            BorderBox {
                x: flow.x + child.style.margin[3] + offset,
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
        if index + 1 < children.len() {
            y += parent.style.row_gap + between;
        }
    }
    flow.y += extent;
    flow.bottom = flow.bottom.max(flow.y.max(y));
    Ok(())
}
