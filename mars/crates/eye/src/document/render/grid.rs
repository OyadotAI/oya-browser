/** CSS grid: explicit and auto-repeated tracks, line and span placement, row auto-flow and item alignment. */
use super::flow::*;
use crate::document::style::SizeKeyword;
use crate::document::style::{ContentKeyword, GridTemplate, Track, TrackSize};
use crate::document::{AlignItems, JustifyContent};

#[path = "grid_abspos.rs"]
pub(super) mod abspos;
#[path = "grid_baseline.rs"]
pub(super) mod baseline;
#[path = "grid_lanes.rs"]
mod lanes;
#[path = "grid_placement.rs"]
mod placement;
#[path = "grid_subgrid.rs"]
mod subgrid;
#[path = "grid_tracks.rs"]
mod tracks;
use placement::{Placed, implicit_track, place};

/** Columns a grid lays out, explicit, repeated or implicit; items past it are clamped in. */
const GRID_COLUMNS: usize = 64;

/** Lays out a grid container's children into its tracks and advances `inner` past the grid. */
pub(super) fn children(
    context: &mut LayoutContext<'_>,
    node: &Node,
    inner: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    // A subgrid lays out in the tracks its parent offered (css-grid-2 9).
    let adopted = subgrid::adopt(node);
    let style = adopted.as_ref().unwrap_or(&node.style);
    // `::before` is the first item and `::after` the last (Blink's pseudo-element children).
    let [before, after] = super::generated::items(node);
    let mut kids: Vec<&Node> = before.iter().collect();
    kids.extend(context.children(node.local_id));
    kids.extend(&after);
    let mut out_of_flow_kids = Vec::new();
    kids.retain(|child| {
        if super::flow::blank_text(child) {
            false
        } else if out_of_flow(child) {
            out_of_flow_kids.push(*child);
            false
        } else {
            true
        }
    });
    // `order` changes auto-placement order only; the stable sort keeps source order among equals.
    kids.sort_by_key(|child| child.style.order);
    if style.grid_lanes {
        return lanes::children(context, node, inner, items, &kids, &out_of_flow_kids);
    }
    // Percentage gaps resolve against the content box, a row gap only against a definite
    // height (Blink's GridLayoutAlgorithm::GutterSize); intrinsic sizing counted them as zero.
    let gap = (
        style.column_gap + style.gap_percent[0] * inner.width / 100.0,
        style.row_gap + style.gap_percent[1] * inner.definite.unwrap_or(0.0) / 100.0,
    );
    let mut columns = expand(&style.grid_columns, inner.width, gap.0);
    let explicit_columns = columns.len();
    // Rows repeat into a definite height, else once (css-grid-2 7.2.3.2).
    let mut row_explicit = expand(&style.grid_rows, inner.definite.unwrap_or(0.0), gap.1);
    let mut placed = place(context, &kids, style, &mut columns, row_explicit.len())?;
    subgrid::clamp(node, &mut placed, &mut columns, row_explicit.len());
    let fit = fit_range(&style.grid_columns, explicit_columns);
    let column_lines = collapse(
        &mut columns,
        fit,
        placed.iter().map(|p| (p.column, p.columns)),
    );
    let fit = fit_range(&style.grid_rows, row_explicit.len());
    let row_lines = collapse(
        &mut row_explicit,
        fit,
        placed.iter().map(|p| (p.row, p.rows)),
    );
    for item in &mut placed {
        item.column = remap(&column_lines, item.column);
        item.row = remap(&row_lines, item.row);
    }
    let explicit_columns = remap(&column_lines, explicit_columns);
    let column_defs = track_sizing(
        [&style.grid_columns, &style.grid_auto_columns],
        explicit_columns,
        &columns,
        Some(inner.width),
    );
    let column_items = column_items(context, style, &placed, &column_defs, gap.0)?;
    let widths = tracks::size_tracks(
        &column_defs,
        &column_items,
        tracks::Space::Definite(inner.width),
        gap.0,
        style.justify_content == JustifyContent::Stretch,
    );
    let used = widths.iter().sum::<f64>() + gap.0 * widths.len().saturating_sub(1) as f64;
    let (lead, between) = super::flex_layout::justification(
        style.justify_content,
        inner.width - used,
        widths.len(),
        false,
    );
    let starts = offsets(inner.x + lead, &widths, gap.0 + between);
    let rtl = style.rtl;
    let lefts = physical_lefts(&starts, &widths, (inner.x, inner.width), rtl);
    for item in &placed {
        subgrid::offer(style, true, item, 1, &widths, gap.0 + between);
    }
    let span_width = |p: &Placed<'_>| span(&widths, p.column, p.columns, gap.0 + between);
    // The explicit rows exist with or without items in them (css-grid-2 7.1).
    let row_count = placed
        .iter()
        .map(|p| p.row + p.rows)
        .max()
        .unwrap_or(0)
        .max(row_explicit.len());
    let row_tracks = (0..row_count)
        .map(|index| {
            row_explicit
                .get(index)
                .copied()
                .unwrap_or_else(|| implicit_track(&style.grid_auto_rows, row_explicit.len(), index))
        })
        .collect::<Vec<_>>();
    let row_defs = track_sizing(
        [&style.grid_rows, &style.grid_auto_rows],
        row_explicit.len(),
        &row_tracks,
        inner.definite,
    );
    // One implicit row of a definite-height grid is that height, which percentage heights of its
    // items resolve against (BBC's `height: 100%` NEWS logo was drawn at 300px wide).
    let row_definite = inner
        .definite
        .filter(|_| row_explicit.is_empty() && row_count == 1);
    let mut natural = Vec::with_capacity(placed.len());
    let mut row_items = Vec::with_capacity(placed.len());
    // Which row contributions are an item's own, by item index, to take its baseline shim.
    let mut own = Vec::with_capacity(placed.len());
    for item in &placed {
        let width = if replaced_stretch(style, item.node, 1) {
            let margins = item.node.style.margin;
            (span_width(item) - margins[1] - margins[3]).max(0.0)
        } else if item.node.tag == "svg"
            && item.node.style.width.is_none()
            && row_definite.is_some()
        {
            super::svg::width_for_height(item.node, row_definite) + horizontal_edges(item.node)
        } else {
            item_width(
                context,
                item.node,
                span_width(item),
                justify_of(style, item.node),
            )?
        };
        let margins = item.node.style.margin;
        let height =
            super::flex_layout::item_height(context, item.node, width)? + margins[0] + margins[2];
        natural.push((width, height));
        let sizing = subgrid::Sizing {
            axis: 0,
            gap: gap.1,
            content: width - horizontal_edges(item.node),
        };
        match subgrid::contributions(context, style, true, item, sizing, 0, &mut row_measure)? {
            Some(list) => row_items.extend(list),
            None => {
                own.push((row_items.len(), natural.len() - 1));
                row_items.push(row_contribution(item, height));
            }
        }
    }
    let shims = baseline::shims(context, style, &placed, &natural, &row_defs)?;
    for (at, index) in own {
        let contribution = &mut row_items[at];
        contribution.minimum += shims[index];
        contribution.min_content += shims[index];
        contribution.max_content += shims[index];
    }
    let heights = tracks::size_tracks(
        &row_defs,
        &row_items,
        inner.definite.map_or(
            tracks::Space::MaxContent(min_rows(node)),
            tracks::Space::Definite,
        ),
        gap.1,
        style.align_content == JustifyContent::Stretch,
    );
    // `align-content` places the rows in a definite-height grid; an auto height has no free space.
    let rows_used = heights.iter().sum::<f64>() + gap.1 * heights.len().saturating_sub(1) as f64;
    let (row_lead, row_between) = super::flex_layout::justification(
        style.align_content,
        inner.definite.map_or(0.0, |height| height - rows_used),
        heights.len(),
        false,
    );
    // An auto height sized the rows with a percentage row gap as zero; the gap then resolves
    // against that height and the rows overflow it (Blink lays out again at the used size).
    let late_gap = match inner.definite {
        None => style.gap_percent[1] * rows_used / 100.0,
        Some(_) => 0.0,
    };
    let row_gap = gap.1 + row_between + late_gap;
    let tops = offsets(inner.y + row_lead, &heights, row_gap);
    for item in &placed {
        subgrid::offer(style, true, item, 0, &heights, row_gap);
    }
    for ((item, (width, height)), shim) in placed.iter().zip(natural).zip(shims) {
        context.charge(1)?;
        let area_width = span_width(item);
        let area_height = span(&heights, item.row, item.rows, row_gap);
        let node_style = &item.node.style;
        let width = ratio_width(style, item.node, area_height).unwrap_or(width);
        let margins = node_style.margin;
        let auto = node_style.margin_auto;
        let free_x = (area_width - width - margins[1] - margins[3]).max(0.0);
        let x = lefts[start_column(item, rtl)]
            + margins[3]
            + super::flex::auto_cross_offset(auto[3], auto[1], free_x).unwrap_or_else(|| {
                inline_offset(offset(justify_of(style, item.node), free_x), free_x, rtl)
            });
        let align = node_style.align_self.unwrap_or(style.align_items);
        let auto_y =
            super::flex::auto_cross_offset(auto[0], auto[2], (area_height - height).max(0.0));
        // A subgrid fills its area in a subgridded axis whatever its alignment.
        let stretch = subgrid::inherits(item.node, 0)
            || (align == AlignItems::Stretch
                && auto_y.is_none()
                && node_style.height.is_none()
                && !node_style.height_fit
                && node_style.aspect_ratio.is_none()
                && (!["img", "canvas", "svg"].contains(&item.node.tag.as_str())
                    || replaced_stretch(style, item.node, 0)));
        let free_y = (area_height - height).max(0.0);
        let y = tops[item.row]
            + margins[0]
            + if stretch {
                0.0
            } else {
                auto_y.unwrap_or_else(|| offset(align, free_y)) + shim
            };
        let forced_height = stretch.then_some((area_height - margins[0] - margins[2]).max(0.0));
        let mut target = Flow::new(x, y, width);
        // The grid area is the containing block, definite once the tracks are sized, so a
        // `height: 100%` item fills its row (LinkedIn's hero figure).
        target.definite = Some((area_height - margins[0] - margins[2]).max(0.0));
        node_layout(
            context,
            item.node,
            &mut target,
            items,
            Some(BorderBox {
                x,
                y,
                width,
                height: forced_height,
            }),
        )?;
    }
    let total = heights.iter().sum::<f64>() + gap.1 * heights.len().saturating_sub(1) as f64;
    let padding = style.padding;
    let content_height = inner.definite.unwrap_or(total);
    let origin = (inner.x - padding[3], inner.y - padding[0]);
    // Columns are kept from the inline-start padding edge, the right one under rtl.
    let start_padding = if rtl { padding[1] } else { padding[3] };
    let relative = |starts: &[f64], from: f64| starts.iter().map(|s| s - from).collect();
    let frame = abspos::Frame {
        axes: [
            abspos::Axis {
                starts: relative(&tops, origin.1),
                sizes: heights.clone(),
            },
            abspos::Axis {
                starts: relative(&starts, inner.x - start_padding),
                sizes: widths.clone(),
            },
        ],
        explicit: [row_explicit.len(), explicit_columns],
        lines: [row_lines, column_lines],
        content: [
            (padding[0], padding[0] + content_height),
            (start_padding, start_padding + inner.width),
        ],
        adopted: adopted.clone(),
        rtl,
    };
    let extent = [
        content_height + padding[0] + padding[2],
        inner.width + padding[1] + padding[3],
    ];
    for child in out_of_flow_kids {
        let at = abspos::static_position(context, node, child, &frame, (origin, extent))?;
        context.defer(child.local_id, at);
    }
    context.record_grid_frame(node.local_id, frame);
    subgrid::withdraw(&placed);
    inner.y += total;
    inner.bottom = inner.bottom.max(inner.y);
    Ok(())
}
/** Each column's physical left edge from `starts`, its left edge laid out from the content
 * box's start, mirrored inside the content box `(x, width)` under `direction: rtl`, where
 * columns run right to left (css-grid-2 7.1 follows the inline direction). */
fn physical_lefts(starts: &[f64], widths: &[f64], (x, width): (f64, f64), rtl: bool) -> Vec<f64> {
    if !rtl {
        return starts.to_vec();
    }
    starts
        .iter()
        .zip(widths)
        .map(|(start, size)| x + x + width - start - size)
        .collect()
}
/** The column whose physical left edge an item's area starts at: its last one under rtl. */
fn start_column(item: &Placed<'_>, rtl: bool) -> usize {
    if rtl {
        item.column + item.columns.max(1) - 1
    } else {
        item.column
    }
}
/** An inline-axis alignment offset from the left: under rtl, `start` is the right edge. */
fn inline_offset(offset: f64, free: f64, rtl: bool) -> f64 {
    if rtl { free - offset } else { offset }
}
/** Whether a replaced item (an `<img>`, or an `<svg>` with no `viewBox`) stretches in `axis`
 * (0 rows, 1 columns): only an authored `stretch` does, `normal` keeps its natural size
 * (css-align-3 6.1, Blink's `IsStretchedReplaced`). An image with a natural aspect ratio then
 * takes its other size from the ratio unless that axis stretches too. */
pub(super) fn replaced_stretch(
    container: &crate::document::ComputedStyle,
    node: &Node,
    axis: usize,
) -> bool {
    let ratio_free = match node.tag.as_str() {
        "img" => true,
        "svg" => !node.attributes.contains_key("viewbox"),
        _ => false,
    };
    let auto = if axis == 0 {
        node.style.height.is_none()
    } else {
        node.style.width.is_none()
    };
    ratio_free
        && auto
        && if axis == 0 {
            lanes::explicit_stretch(container, node)
        } else {
            lanes::explicit_justify_stretch(container, node)
        }
}
/** The border-box width an image stretched only in the block axis to `height` (its outer
 * height in the area) takes from its natural aspect ratio; None when it keeps its width. */
fn ratio_width(
    container: &crate::document::ComputedStyle,
    node: &Node,
    height: f64,
) -> Option<f64> {
    let (natural_width, natural_height) = node.image_size.filter(|_| !node.image_ratio_free)?;
    if node.tag != "img"
        || natural_height == 0
        || !replaced_stretch(container, node, 0)
        || replaced_stretch(container, node, 1)
    {
        return None;
    }
    let margins = node.style.margin[0] + node.style.margin[2];
    let content = (height - margins - vertical_edges(node)).max(0.0);
    let ratio = f64::from(natural_width) / f64::from(natural_height);
    Some(content * ratio + horizontal_edges(node))
}
/** In-flow items of a grid in layout order: `::before` first and `::after` last (Blink's
 * pseudo-element children), then sorted by `order`. */
fn items_of<'a>(
    context: &LayoutContext<'a>,
    node: &'a Node,
    [before, after]: &'a [Vec<Node>; 2],
) -> Vec<&'a Node> {
    let mut kids: Vec<&Node> = before.iter().collect();
    kids.extend(context.children(node.local_id));
    kids.extend(after);
    kids.retain(|child| !super::flow::blank_text(child) && !out_of_flow(child));
    kids.sort_by_key(|child| child.style.order);
    kids
}
/** Every item's column contributions; a column subgrid contributes its own items instead. */
fn column_items(
    context: &mut LayoutContext<'_>,
    style: &crate::document::ComputedStyle,
    placed: &[Placed<'_>],
    defs: &[tracks::TrackSizing],
    gap: f64,
) -> EngineResult<Vec<tracks::Contribution>> {
    let mut out = Vec::with_capacity(placed.len());
    let mut measure = |context: &mut LayoutContext<'_>, kid: &Placed<'_>, _: f64| {
        // ponytail: the kid's fixed-maximum clamp reads the parent's defs at the kid's own
        // index; shift them by the subgrid's start if a page needs it.
        column_contribution(context, kid, defs)
    };
    let sizing = subgrid::Sizing {
        axis: 1,
        gap,
        content: 0.0,
    };
    for item in placed {
        match subgrid::contributions(context, style, true, item, sizing, 0, &mut measure)? {
            Some(list) => out.extend(list),
            None => out.push(column_contribution(context, item, defs)?),
        }
    }
    Ok(out)
}
/** A row subgrid's item's row contribution, at the width of its area in the subgrid's
 * inherited columns, or `own_area` wide when the subgrid sized its own columns. */
fn row_measure(
    context: &mut LayoutContext<'_>,
    kid: &Placed<'_>,
    own_area: f64,
) -> EngineResult<tracks::Contribution> {
    let parent = kid
        .node
        .parent
        .map(|parent| context.node(parent))
        .transpose()?;
    let columns = parent.filter(|parent| subgrid::inherits(parent, 1));
    let adopted = parent.and_then(subgrid::adopt);
    let area = adopted
        .as_ref()
        .filter(|_| columns.is_some())
        .map_or(own_area.max(0.0), |style| {
            let widths = style
                .grid_columns
                .tracks
                .iter()
                .map(|track| track.min.length(None).unwrap_or(0.0));
            let widths = widths.collect::<Vec<_>>();
            span(&widths, kid.column, kid.columns, style.column_gap)
        });
    let justify = adopted
        .as_ref()
        .map_or(AlignItems::Stretch, |style| justify_of(style, kid.node));
    let width = item_width(context, kid.node, area, justify)?;
    let margins = kid.node.style.margin;
    let height =
        super::flex_layout::item_height(context, kid.node, width)? + margins[0] + margins[2];
    Ok(row_contribution(kid, height))
}
/** A grid's column widths and used column gap at a `content` width for its `placed` items, as
 * `children` sizes them; a row subgrid's items are measured in them before it lays out. */
fn column_widths(
    context: &mut LayoutContext<'_>,
    style: &crate::document::ComputedStyle,
    placed: &[Placed<'_>],
    columns: &[Track],
    explicit: usize,
    content: f64,
) -> EngineResult<(Vec<f64>, f64)> {
    let content = content.max(0.0);
    let gap = style.column_gap + style.gap_percent[0] * content / 100.0;
    let sources = [&style.grid_columns, &style.grid_auto_columns];
    let defs = track_sizing(sources, explicit, columns, Some(content));
    let items = column_items(context, style, placed, &defs, gap)?;
    let stretch = style.justify_content == JustifyContent::Stretch;
    let space = tracks::Space::Definite(content);
    Ok((tracks::size_tracks(&defs, &items, space, gap, stretch), gap))
}
/** The content-box height a grid's `min-height` gives, which `auto` rows stretch to when its
 * height is indefinite (Blink's `StretchAutoTracks` uses `min_available_size_`). A percentage
 * of an unknown containing block is zero. */
fn min_rows(node: &Node) -> f64 {
    node.style
        .min_height
        .and_then(|extent| extent.resolve_size(None))
        .map_or(0.0, |height| {
            super::sizing::content_size(node, height, false)
        })
}
/** An item's inline-axis self alignment: `justify-self`, or the container's `justify-items`. */
fn justify_of(container: &crate::document::ComputedStyle, node: &Node) -> AlignItems {
    if subgrid::inherits(node, 1) {
        // A subgrid fills its area in a subgridded axis whatever its alignment.
        return AlignItems::Stretch;
    }
    let (align, side) = match node.style.justify_self {
        Some(align) => (align, node.style.align_side[3]),
        None => (container.justify_items, container.align_side[2]),
    };
    sided(align, side, container.rtl, node.style.rtl)
}
/** A start or end position whose keyword was `left`, `right`, `self-start` or `self-end`,
 * turned into the container's start or end: `left` is the end under rtl, and an item's own
 * start is the container's end when their directions differ. */
fn sided(align: AlignItems, side: u8, rtl: bool, own_rtl: bool) -> AlignItems {
    use crate::document::style::{SIDE_LEFT, SIDE_RIGHT, SIDE_SELF_END, SIDE_SELF_START};
    let start = match side {
        SIDE_LEFT => !rtl,
        SIDE_RIGHT => rtl,
        SIDE_SELF_START => rtl == own_rtl,
        SIDE_SELF_END => rtl != own_rtl,
        _ => return align,
    };
    if start {
        AlignItems::Start
    } else {
        AlignItems::End
    }
}
/** Sizing functions against the grid's size in this axis; a percentage of an indefinite size
 * behaves as `auto`, as css-grid-2 7.2 says. Tracks past `explicit` are implicit and resolve
 * their math expressions against the `grid-auto-*` list they came from. */
fn track_sizing(
    [source, auto]: [&GridTemplate; 2],
    explicit: usize,
    template: &[Track],
    basis: Option<f64>,
) -> Vec<tracks::TrackSizing> {
    template
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let source = if index < explicit { source } else { auto };
            let content = track.content;
            let fit = content.fit_limit.and_then(|limit| limit.resolve(basis));
            tracks::TrackSizing {
                min: match (source.length(track.min, basis), content.min) {
                    (Some(length), _) => tracks::MinSizing::Fixed(length),
                    (None, ContentKeyword::MinContent) => tracks::MinSizing::MinContent,
                    (None, ContentKeyword::MaxContent) => tracks::MinSizing::MaxContent,
                    (None, ContentKeyword::Auto) => tracks::MinSizing::Auto,
                },
                max: match (track.max, source.length(track.max, basis), content.max, fit) {
                    (TrackSize::Fr(factor), ..) => tracks::MaxSizing::Fr(factor.max(0.0)),
                    (_, Some(length), ..) => tracks::MaxSizing::Fixed(length),
                    (.., Some(limit)) => tracks::MaxSizing::FitContent(limit.max(0.0)),
                    (_, _, ContentKeyword::MinContent, _) => tracks::MaxSizing::MinContent,
                    (_, _, ContentKeyword::MaxContent, _) => tracks::MaxSizing::MaxContent,
                    _ => tracks::MaxSizing::Auto,
                },
            }
        })
        .collect()
}
/** An item's column contributions as outer widths: min-content, max-content and the minimum
 * contribution, which is its `min-width`, or its automatic minimum (the content-based minimum,
 * clamped by the fixed tracks it spans, or zero for a scroll container). */
fn column_contribution(
    context: &mut LayoutContext<'_>,
    item: &Placed<'_>,
    defs: &[tracks::TrackSizing],
) -> EngineResult<tracks::Contribution> {
    let node = item.node;
    let outer = horizontal_edges(node) + node.style.margin[1] + node.style.margin[3];
    let lower = node
        .style
        .min_width
        .map(|width| super::sizing::content_size(node, width, true))
        .unwrap_or(0.0);
    let upper = super::sizing::max_width(node, None).max(lower);
    let clamp = |content: f64| content.max(lower).min(upper) + outer;
    let intrinsic = super::flex_layout::item_intrinsic(context, node)?;
    // A `min-content` or `max-content` width is as definite a size as a length.
    let fixed = match node.style.size_keywords[0] {
        SizeKeyword::MinContent => Some(intrinsic.min_content),
        SizeKeyword::MaxContent => Some(intrinsic.max_content),
        _ => node
            .style
            .width
            .filter(|_| !node.style.percent_width)
            .map(|width| super::sizing::content_size(node, width, true)),
    };
    let (min_content, max_content) = match fixed {
        Some(width) => (width, width),
        None => (intrinsic.min_content, intrinsic.max_content),
    };
    let spanned = &defs[item.column.min(defs.len())..(item.column + item.columns).min(defs.len())];
    let minimum = if node.style.min_width.is_some() || super::flex::scroll_container(node) {
        clamp(0.0)
    } else {
        let mut content = fixed.unwrap_or(intrinsic.min_content);
        if let Some(limit) = fixed_maxima(spanned) {
            content = content.min((limit - outer).max(0.0));
        }
        clamp(content)
    };
    Ok(tracks::Contribution {
        start: item.column,
        span: item.columns,
        minimum,
        min_content: clamp(min_content),
        max_content: clamp(max_content),
    })
}
/** The summed fixed maxima of spanned tracks when every one is fixed, which caps a
 * content-based minimum (css-grid-2 6.6). */
fn fixed_maxima(spanned: &[tracks::TrackSizing]) -> Option<f64> {
    spanned
        .iter()
        .map(|track| match track.max {
            tracks::MaxSizing::Fixed(length) => Some(length),
            _ => None,
        })
        .sum()
}
/** An item's row contributions from its outer height at its final width; a scroll container or
 * an authored `min-height` gives a smaller minimum contribution. */
fn row_contribution(item: &Placed<'_>, height: f64) -> tracks::Contribution {
    let node = item.node;
    let margins = node.style.margin[0] + node.style.margin[2];
    let minimum = match node
        .style
        .min_height
        .and_then(|extent| extent.resolve_size(None))
    {
        Some(min) => super::sizing::border_height(node, min) + margins,
        None if super::flex::scroll_container(node) => {
            super::sizing::vertical_edges(node) + margins
        }
        None => height,
    };
    tracks::Contribution {
        start: item.row,
        span: item.rows,
        minimum: minimum.min(height),
        min_content: height,
        max_content: height,
    }
}
/** Explicit tracks with any auto-fill group repeated as often as it fits `width`, the grid's
 * size in that axis (columns or rows). */
fn expand(template: &GridTemplate, width: f64, gap: f64) -> Vec<Track> {
    let mut tracks = template.tracks.clone();
    if !template.repeat.is_empty() {
        let fixed = |track: &Track| {
            (template.length(track.min, Some(width)))
                .or(template.length(track.max, Some(width)))
                .unwrap_or(0.0)
        };
        let group =
            template.repeat.iter().map(fixed).sum::<f64>() + gap * template.repeat.len() as f64;
        let others = tracks.iter().map(fixed).sum::<f64>() + gap * tracks.len() as f64;
        let room = width - others + gap;
        let times = if group > 0.0 {
            ((room / group).floor() as usize).max(1)
        } else {
            1
        };
        let times = times
            .min(GRID_COLUMNS / template.repeat.len().max(1))
            .max(1);
        let at = template.repeat_at.min(tracks.len());
        let repeated = (0..times).flat_map(|_| template.repeat.iter().copied());
        tracks.splice(at..at, repeated);
    }
    tracks.truncate(GRID_COLUMNS);
    tracks
}
/** The expanded tracks an `auto-fit` group repeated into, of `expanded` explicit tracks. */
fn fit_range(template: &GridTemplate, expanded: usize) -> std::ops::Range<usize> {
    if !template.repeat_fit || template.repeat.is_empty() {
        return 0..0;
    }
    let at = template.repeat_at.min(template.tracks.len());
    at..(at + expanded.saturating_sub(template.tracks.len())).min(expanded)
}
/** Removes the `fit` tracks no item spans: css-grid-2 7.2.3.2 collapses empty `auto-fit`
 * repetitions to zero with their gutters, and content distribution then ignores them (Blink's
 * `GridTrackCollection` collapsed ranges). Returns, for each old line index, the new one. */
fn collapse(
    tracks: &mut Vec<Track>,
    fit: std::ops::Range<usize>,
    spans: impl Iterator<Item = (usize, usize)>,
) -> Vec<usize> {
    let mut keep = (0..tracks.len())
        .map(|i| !fit.contains(&i))
        .collect::<Vec<_>>();
    for (start, count) in spans {
        let end = (start + count).min(keep.len());
        keep[start.min(end)..end].fill(true);
    }
    let mut lines = Vec::with_capacity(keep.len() + 1);
    let mut kept = 0;
    for &k in &keep {
        lines.push(kept);
        kept += usize::from(k);
    }
    lines.push(kept);
    let mut index = 0;
    tracks.retain(|_| {
        index += 1;
        keep[index - 1]
    });
    lines
}
/** A line or track index once collapsed tracks are gone; past the mapped lines, implicit
 * tracks shift down by every collapsed one. */
fn remap(lines: &[usize], index: usize) -> usize {
    let last = lines.len().saturating_sub(1);
    lines
        .get(index)
        .copied()
        .unwrap_or_else(|| index - (last - lines.last().copied().unwrap_or(0)))
}
/** Start offsets of consecutive tracks separated by `gap`. */
fn offsets(start: f64, sizes: &[f64], gap: f64) -> Vec<f64> {
    let mut at = start;
    sizes
        .iter()
        .map(|size| {
            let here = at;
            at += size + gap;
            here
        })
        .collect()
}
/** The extent of `count` tracks from `first`, gaps between them included. */
fn span(sizes: &[f64], first: usize, count: usize, gap: f64) -> f64 {
    let end = (first + count).min(sizes.len());
    sizes[first.min(end)..end].iter().sum::<f64>() + gap * count.saturating_sub(1) as f64
}
/** Border-box width in its area: stretched unless sized or aligned, else shrink-to-fit. */
fn item_width(
    context: &mut LayoutContext<'_>,
    node: &Node,
    area: f64,
    justify: AlignItems,
) -> EngineResult<f64> {
    let margins = node.style.margin[1] + node.style.margin[3];
    let room = (area - margins).max(0.0);
    if node.style.width.is_some() {
        return Ok(border_width(node, area, room, None));
    }
    if justify == AlignItems::Stretch
        && !node.style.width_fit
        && !["img", "canvas", "svg"].contains(&node.tag.as_str())
    {
        return Ok(border_width(node, area, room, None));
    }
    if !super::generated::is_generated(node) {
        return super::float::shrink_width(context, node, area, margins);
    }
    // Shrink-to-fit from the generated box's own text, not its element's cached widths.
    let intrinsic = super::flex_layout::item_intrinsic(context, node)?;
    let edges = horizontal_edges(node);
    let fit = intrinsic
        .max_content
        .min((room - edges).max(intrinsic.min_content));
    Ok(border_width(node, area, room.max(fit + edges), Some(fit)))
}
/** Offset of an aligned item inside its free space. */
fn offset(align: AlignItems, free: f64) -> f64 {
    match align {
        AlignItems::Center => free / 2.0,
        AlignItems::End => free,
        _ => 0.0,
    }
}
/**
 * A grid container's min- and max-content widths, as Blink's `GridLayoutAlgorithm::
 * ComputeMinMaxSizes` finds them: place the items, then run the column track sizing algorithm
 * under a min-content and under a max-content constraint and sum the tracks and gutters.
 * Under either constraint an `auto` minimum takes the items' limited min-content contributions,
 * and under min-content a flexible track keeps its base size, so `minmax(0, 1fr)` adds nothing
 * (nodejs.org's code sample, a `display: grid` `<code>`, no longer widens its column).
 */
pub(super) fn intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
) -> EngineResult<super::sizing::IntrinsicWidths> {
    let style = &node.style;
    // The same items `children` lays out, so a shrink-to-fit grid is never narrower than its
    // generated items.
    let generated = super::generated::items(node);
    let kids = items_of(context, node, &generated);
    if style.grid_lanes {
        return if lanes::row_lanes(style) {
            lanes::row_intrinsic(context, node, &kids)
        } else {
            lanes::intrinsic(context, node, &kids)
        };
    }
    let gap = style.column_gap;
    // With no definite width an auto-repeat group repeats once (css-grid-2 7.2.3.2).
    let mut columns = expand(&style.grid_columns, 0.0, gap);
    let explicit_columns = columns.len();
    let rows = expand(&style.grid_rows, 0.0, style.row_gap).len();
    let mut placed = place(context, &kids, style, &mut columns, rows)?;
    let fit = fit_range(&style.grid_columns, explicit_columns);
    let column_lines = collapse(
        &mut columns,
        fit,
        placed.iter().map(|p| (p.column, p.columns)),
    );
    for item in &mut placed {
        item.column = remap(&column_lines, item.column);
    }
    let explicit_columns = remap(&column_lines, explicit_columns);
    let defs = track_sizing(
        [&style.grid_columns, &style.grid_auto_columns],
        explicit_columns,
        &columns,
        None,
    );
    let mut items = column_items(context, style, &placed, &defs, gap)?;
    for contribution in &mut items {
        contribution.minimum = limited_min_content(contribution, &defs);
    }
    let total = |space| {
        let widths = tracks::size_tracks(&defs, &items, space, gap, false);
        widths.iter().sum::<f64>() + gap * widths.len().saturating_sub(1) as f64
    };
    Ok(super::sizing::IntrinsicWidths {
        min_content: total(tracks::Space::MinContent),
        max_content: total(tracks::Space::MaxContent(0.0)),
    })
}
/** An item's limited min-content contribution (css-grid-2 12.5.1): its min-content contribution
 * capped by the spanned tracks' fixed maximum when they all have one, never below its minimum
 * contribution. */
fn limited_min_content(item: &tracks::Contribution, defs: &[tracks::TrackSizing]) -> f64 {
    let end = (item.start + item.span).min(defs.len());
    let spanned = &defs[item.start.min(end)..end];
    let limited =
        fixed_maxima(spanned).map_or(item.min_content, |limit| item.min_content.min(limit));
    limited.max(item.minimum)
}
