/** CSS grid: explicit and auto-repeated tracks, line and span placement, row auto-flow and item alignment. */
use super::flow::*;
use crate::document::style::{ContentKeyword, GridTemplate, Track, TrackSize};
use crate::document::{AlignItems, JustifyContent};

#[path = "grid_placement.rs"]
mod placement;
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
    let style = &node.style;
    // `::before` is the first item and `::after` the last (Blink's pseudo-element children).
    let [before, after] = super::generated::items(node);
    let mut kids: Vec<&Node> = before.iter().collect();
    kids.extend(context.children(node.local_id));
    kids.extend(&after);
    kids.retain(|child| {
        if super::flow::blank_text(child) {
            false
        } else if out_of_flow(child) {
            context.defer(child.local_id, (inner.x, inner.y));
            false
        } else {
            true
        }
    });
    // `order` changes auto-placement order only; the stable sort keeps source order among equals.
    kids.sort_by_key(|child| child.style.order);
    let gap = (style.column_gap, style.row_gap);
    let mut columns = expand(&style.grid_columns, inner.width, gap.0);
    let explicit_columns = columns.len();
    let placed = place(context, &kids, style, &mut columns)?;
    let column_defs = track_sizing(
        [&style.grid_columns, &style.grid_auto_columns],
        explicit_columns,
        &columns,
        Some(inner.width),
    );
    let column_items = placed
        .iter()
        .map(|item| column_contribution(context, item, &column_defs))
        .collect::<EngineResult<Vec<_>>>()?;
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
    let lefts = offsets(inner.x + lead, &widths, gap.0 + between);
    let span_width = |p: &Placed<'_>| span(&widths, p.column, p.columns, gap.0 + between);
    let row_count = placed.iter().map(|p| p.row + p.rows).max().unwrap_or(0);
    let row_explicit = &style.grid_rows.tracks;
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
        .filter(|_| style.grid_rows.tracks.is_empty() && row_count == 1);
    let mut natural = Vec::with_capacity(placed.len());
    let mut row_items = Vec::with_capacity(placed.len());
    for item in &placed {
        let width = if item.node.tag == "svg"
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
        row_items.push(row_contribution(item, height));
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
    let row_gap = gap.1 + row_between;
    let tops = offsets(inner.y + row_lead, &heights, row_gap);
    for (item, (width, height)) in placed.iter().zip(natural) {
        context.charge(1)?;
        let area_width = span_width(item);
        let area_height = span(&heights, item.row, item.rows, row_gap);
        let node_style = &item.node.style;
        let margins = node_style.margin;
        let auto = node_style.margin_auto;
        let free_x = (area_width - width - margins[1] - margins[3]).max(0.0);
        let x = lefts[item.column]
            + margins[3]
            + super::flex::auto_cross_offset(auto[3], auto[1], free_x)
                .unwrap_or_else(|| offset(justify_of(style, item.node), free_x));
        let align = node_style.align_self.unwrap_or(style.align_items);
        let auto_y =
            super::flex::auto_cross_offset(auto[0], auto[2], (area_height - height).max(0.0));
        let stretch = align == AlignItems::Stretch
            && auto_y.is_none()
            && node_style.height.is_none()
            && node_style.aspect_ratio.is_none()
            && !["img", "svg"].contains(&item.node.tag.as_str());
        let free_y = (area_height - height).max(0.0);
        let y = tops[item.row]
            + margins[0]
            + if stretch {
                0.0
            } else {
                auto_y.unwrap_or_else(|| offset(align, free_y))
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
    inner.y += total;
    inner.bottom = inner.bottom.max(inner.y);
    Ok(())
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
    node.style.justify_self.unwrap_or(container.justify_items)
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
    let fixed = node
        .style
        .width
        .filter(|_| !node.style.percent_width)
        .map(|width| super::sizing::content_size(node, width, true));
    let intrinsic = super::flex_layout::item_intrinsic(context, node)?;
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
/** Explicit tracks with any auto-fill group repeated as often as it fits the width. */
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
        && !["img", "svg"].contains(&node.tag.as_str())
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
    // The same items `children` lays out, `::before` first and `::after` last, so a
    // shrink-to-fit grid is never narrower than its generated items.
    let [before, after] = super::generated::items(node);
    let mut kids: Vec<&Node> = before.iter().collect();
    kids.extend(context.children(node.local_id));
    kids.extend(&after);
    kids.retain(|child| !super::flow::blank_text(child) && !out_of_flow(child));
    kids.sort_by_key(|child| child.style.order);
    let gap = style.column_gap;
    // With no definite width an auto-repeat group repeats once (css-grid-2 7.2.3.2).
    let mut columns = expand(&style.grid_columns, 0.0, gap);
    let explicit_columns = columns.len();
    let placed = place(context, &kids, style, &mut columns)?;
    let defs = track_sizing(
        [&style.grid_columns, &style.grid_auto_columns],
        explicit_columns,
        &columns,
        None,
    );
    let mut items = Vec::with_capacity(placed.len());
    for item in &placed {
        let mut contribution = column_contribution(context, item, &defs)?;
        contribution.minimum = limited_min_content(&contribution, &defs);
        items.push(contribution);
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
