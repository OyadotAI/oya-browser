/** Grid-lanes layout (css-grid-3 masonry), after Blink's `GridLanesLayoutAlgorithm` and
 * `GridLanesRunningPositions`: the columns are sized like grid tracks, with every auto-placed
 * item contributing at each start it could take, then each item in order goes to the first
 * lane, from the auto-placement cursor on, whose running position is within the flow
 * tolerance of the shortest, and stacks there. */
use super::super::flow::*;
use super::placement::{Placed, implicit_track, lane_lines};
use super::{GRID_COLUMNS, tracks};
use crate::document::style::{Extent, GridTemplate, Track, TrackSize};
use crate::document::{AlignItems, JustifyContent};

/** One item in lane order: its node, definite start lane (None when auto-placed) and span. */
type Lane<'a> = (&'a Node, Option<usize>, usize);

/** The lane track definitions, every item's lane and the column contributions the track
 * sizing algorithm reads, auto-placed items once per possible start (Blink's virtual items). */
fn lane_tracks<'a>(
    context: &mut LayoutContext<'_>,
    kids: &[&'a Node],
    style: &crate::document::ComputedStyle,
    width: Option<f64>,
) -> EngineResult<(
    Vec<tracks::TrackSizing>,
    Vec<Lane<'a>>,
    Vec<tracks::Contribution>,
)> {
    let mut columns = match width {
        Some(width) if intrinsic_repeat(&style.grid_columns, width) => {
            intrinsic_columns(context, kids, style, width)?
        }
        _ => super::expand(&style.grid_columns, width.unwrap_or(0.0), style.column_gap),
    };
    let explicit = columns.len();
    let mut lanes = kids
        .iter()
        .map(|node| {
            let (start, span) = lane_lines(node, style, 1, explicit);
            (*node, start, span.clamp(1, GRID_COLUMNS))
        })
        .collect::<Vec<_>>();
    // A definite lane past the explicit ones adds implicit lanes; an empty template has one.
    let reach = lanes
        .iter()
        .filter_map(|(_, start, span)| start.map(|s| s + span));
    let count = reach
        .max()
        .unwrap_or(0)
        .max(explicit)
        .clamp(1, GRID_COLUMNS);
    while columns.len() < count {
        let index = columns.len();
        columns.push(implicit_track(&style.grid_auto_columns, explicit, index));
    }
    for lane in &mut lanes {
        lane.2 = lane.2.min(count);
        lane.1 = lane.1.map(|start| start.min(count - lane.2));
    }
    let defs = super::track_sizing(
        [&style.grid_columns, &style.grid_auto_columns],
        explicit,
        &columns,
        width,
    );
    let mut contributions = Vec::new();
    for &(node, start, span) in &lanes {
        let starts = start.map_or(0..=count - span, |start| start..=start);
        for column in starts {
            let item = Placed {
                node,
                row: 0,
                column,
                rows: 1,
                columns: span,
            };
            contributions.push(super::column_contribution(context, &item, &defs)?);
        }
    }
    Ok((defs, lanes, contributions))
}

/** Whether the auto-repeat group has a track with no fixed size, which grid-lanes repeats as
 * often as the size its items give it fits (css-grid-3 intrinsic repeat). */
fn intrinsic_repeat(template: &GridTemplate, width: f64) -> bool {
    template.repeat.iter().any(|track| {
        template.length(track.min, Some(width)).is_none()
            && template.length(track.max, Some(width)).is_none()
    })
}

/**
 * The lanes of an intrinsically sized auto-repeat, after Blink's `ComputeAutomaticRepetitions`
 * and `CalculateIntrinsicTrackSizes`: repeat the group `2 + (largest span - 2) / group size`
 * times, size those tracks with every item placed everywhere it could go, take each repeated
 * definition's largest size as if it were fixed to count the repetitions that fit, then repeat
 * the original definitions that many times.
 */
fn intrinsic_columns(
    context: &mut LayoutContext<'_>,
    kids: &[&Node],
    style: &crate::document::ComputedStyle,
    width: f64,
) -> EngineResult<Vec<Track>> {
    let template = &style.grid_columns;
    let group = template.repeat.len();
    let largest = kids
        .iter()
        .map(|node| lane_lines(node, style, 1, 0).1)
        .max()
        .unwrap_or(1);
    let times = 2 + largest.saturating_sub(2) / group;
    let mut probe = template.clone();
    probe.repeat.clear();
    let at = template.repeat_at.min(template.tracks.len());
    let repeated = (0..times).flat_map(|_| template.repeat.iter().copied());
    probe.tracks.splice(at..at, repeated);
    let probe_style = crate::document::ComputedStyle {
        grid_columns: probe,
        ..style.clone()
    };
    let (defs, _, contributions) = lane_tracks(context, kids, &probe_style, None)?;
    let sizes = tracks::size_tracks(
        &defs,
        &contributions,
        tracks::Space::MaxContent(0.0),
        style.column_gap,
        false,
    );
    // Blink records every expanded track but the last.
    let mut largest_size = vec![0.0_f64; group];
    for index in at..(at + times * group - 1).min(sizes.len()) {
        let slot = &mut largest_size[(index - at) % group];
        *slot = slot.max(sizes[index]);
    }
    let mut sized = template.clone();
    for (track, size) in sized.repeat.iter_mut().zip(&largest_size) {
        let fixed = TrackSize::Length(Extent {
            px: if kids.is_empty() { 0.0 } else { *size },
            percent: 0.0,
        });
        track.min = fixed;
        track.max = fixed;
    }
    let mut columns = super::expand(&sized, width, style.column_gap);
    let end = (at + columns.len().saturating_sub(template.tracks.len())).min(columns.len());
    for (offset, track) in columns[at..end].iter_mut().enumerate() {
        *track = template.repeat[offset % group];
    }
    Ok(columns)
}

/** The start lane of an auto-placed item spanning `span` lanes: the first, walking from the
 * cursor and wrapping, whose highest spanned running position is within `tolerance` of the
 * lowest such position (Blink's `GetFirstEligibleLine`). */
fn first_eligible(running: &[f64], span: usize, cursor: usize, tolerance: f64) -> usize {
    let last = running.len() - span;
    let highest = |start: usize| {
        running[start..start + span]
            .iter()
            .copied()
            .fold(0.0, f64::max)
    };
    let lowest = (0..=last).map(highest).fold(f64::INFINITY, f64::min);
    let from = if cursor > last { 0 } else { cursor };
    (0..=last)
        .map(|step| (from + step) % (last + 1))
        .find(|&start| highest(start) <= lowest + tolerance)
        .unwrap_or(from)
}

/** Lays out a column grid-lanes container's in-flow `kids` and advances `inner` past it. */
pub(super) fn children(
    context: &mut LayoutContext<'_>,
    node: &Node,
    inner: &mut Flow,
    items: &mut Vec<DisplayItem>,
    kids: &[&Node],
    out_of_flow_kids: &[&Node],
) -> EngineResult<()> {
    let style = &node.style;
    if row_lanes(style) {
        return rows(context, node, inner, items, kids, out_of_flow_kids);
    }
    let (gap, stack_gap) = (style.column_gap, style.row_gap);
    let (defs, lanes, contributions) = lane_tracks(context, kids, style, Some(inner.width))?;
    let widths = tracks::size_tracks(
        &defs,
        &contributions,
        tracks::Space::Definite(inner.width),
        gap,
        style.justify_content == JustifyContent::Stretch,
    );
    let used = widths.iter().sum::<f64>() + gap * widths.len().saturating_sub(1) as f64;
    let (lead, between) = super::super::flex_layout::justification(
        style.justify_content,
        inner.width - used,
        widths.len(),
        false,
    );
    let lefts = super::offsets(inner.x + lead, &widths, gap + between);
    let tolerance = style.flow_tolerance.unwrap_or(style.font_size);
    let mut running = vec![0.0; widths.len()];
    let mut cursor = 0;
    for (child, start, span) in lanes {
        context.charge(1)?;
        let column = start.unwrap_or_else(|| first_eligible(&running, span, cursor, tolerance));
        let top = running[column..column + span]
            .iter()
            .copied()
            .fold(0.0, f64::max);
        let area = super::span(&widths, column, span, gap + between);
        let justify = super::justify_of(style, child);
        let width = super::item_width(context, child, area, justify)?;
        let margins = child.style.margin;
        let height = super::super::flex_layout::item_height(context, child, width)?;
        let auto = child.style.margin_auto;
        let free = (area - width - margins[1] - margins[3]).max(0.0);
        let x = lefts[column]
            + margins[3]
            + super::super::flex::auto_cross_offset(auto[3], auto[1], free)
                .unwrap_or_else(|| super::offset(justify, free));
        let y = inner.y + top + margins[0];
        let mut target = Flow::new(x, y, width);
        let border_box = BorderBox {
            x,
            y,
            width,
            height: None,
        };
        node_layout(context, child, &mut target, items, Some(border_box))?;
        // Margins never move the running position backwards (Blink clamps the contribution).
        let next = top + (height + margins[0] + margins[2] + stack_gap).max(0.0);
        running[column..column + span].fill(next);
        cursor = column + span;
    }
    let total = running.iter().copied().fold(0.0, f64::max);
    let total = if kids.is_empty() {
        0.0
    } else {
        (total - stack_gap).max(0.0)
    };
    for child in out_of_flow_kids {
        context.defer(child.local_id, (inner.x, inner.y));
    }
    inner.y += total;
    inner.bottom = inner.bottom.max(inner.y);
    Ok(())
}

/** A column grid-lanes container's min- and max-content widths: the lane track sizing under
 * each constraint, auto-placed items contributing to every lane they could land in. */
pub(super) fn intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
    kids: &[&Node],
) -> EngineResult<super::super::sizing::IntrinsicWidths> {
    let style = &node.style;
    let gap = style.column_gap;
    let (defs, _, mut contributions) = lane_tracks(context, kids, style, None)?;
    for contribution in &mut contributions {
        contribution.minimum = super::limited_min_content(contribution, &defs);
    }
    let total = |space| {
        let widths = tracks::size_tracks(&defs, &contributions, space, gap, false);
        widths.iter().sum::<f64>() + gap * widths.len().saturating_sub(1) as f64
    };
    Ok(super::super::sizing::IntrinsicWidths {
        min_content: total(tracks::Space::MinContent),
        max_content: total(tracks::Space::MaxContent(0.0)),
    })
}

/** Whether the lanes are rows: `grid-template-rows` is set and `grid-template-columns` is not
 * (Blink's `GridLanesTrackSizingDirection` for `grid-lanes-direction: normal`). */
pub(super) fn row_lanes(style: &crate::document::ComputedStyle) -> bool {
    let set = |template: &GridTemplate| !template.tracks.is_empty() || !template.repeat.is_empty();
    style.grid_lanes && !set(&style.grid_columns) && set(&style.grid_rows)
}

/**
 * Row grid-lanes: the rows are sized from the items' heights at their shrink-to-fit widths,
 * auto-placed items contributing at every start, and each item then stacks rightwards in the
 * row the running positions pick, aligned in its row by `align-self` (Blink's
 * `GridLanesLayoutAlgorithm` with the grid axis `kForRows`).
 */
fn rows(
    context: &mut LayoutContext<'_>,
    node: &Node,
    inner: &mut Flow,
    items: &mut Vec<DisplayItem>,
    kids: &[&Node],
    out_of_flow_kids: &[&Node],
) -> EngineResult<()> {
    let style = &node.style;
    let (gap, stack_gap) = (style.row_gap, style.column_gap);
    let mut tracks_list = super::expand(&style.grid_rows, inner.definite.unwrap_or(0.0), gap);
    let explicit = tracks_list.len();
    let mut lanes = Vec::with_capacity(kids.len());
    for child in kids {
        let (start, span) = lane_lines(child, style, 0, explicit);
        let width = super::item_width(context, child, inner.width, AlignItems::Start)?;
        let height = super::super::flex_layout::item_height(context, child, width)?;
        lanes.push((*child, start, span.clamp(1, GRID_COLUMNS), width, height));
    }
    let reach = lanes.iter().filter_map(|lane| lane.1.map(|s| s + lane.2));
    let count = reach
        .max()
        .unwrap_or(0)
        .max(explicit)
        .clamp(1, GRID_COLUMNS);
    while tracks_list.len() < count {
        let index = tracks_list.len();
        tracks_list.push(implicit_track(&style.grid_auto_rows, explicit, index));
    }
    let defs = super::track_sizing(
        [&style.grid_rows, &style.grid_auto_rows],
        explicit,
        &tracks_list,
        inner.definite,
    );
    let mut contributions = Vec::new();
    for lane in &mut lanes {
        lane.2 = lane.2.min(count);
        lane.1 = lane.1.map(|start| start.min(count - lane.2));
        let margins = lane.0.style.margin[0] + lane.0.style.margin[2];
        for row in lane.1.map_or(0..=count - lane.2, |start| start..=start) {
            let item = Placed {
                node: lane.0,
                row,
                column: 0,
                rows: lane.2,
                columns: 1,
            };
            contributions.push(super::row_contribution(&item, lane.4 + margins));
        }
    }
    let space = inner.definite.map_or(
        tracks::Space::MaxContent(super::min_rows(node)),
        tracks::Space::Definite,
    );
    let stretch = style.align_content == JustifyContent::Stretch;
    let heights = tracks::size_tracks(&defs, &contributions, space, gap, stretch);
    let used = heights.iter().sum::<f64>() + gap * heights.len().saturating_sub(1) as f64;
    let free = inner.definite.map_or(0.0, |height| height - used);
    let (lead, between) =
        super::super::flex_layout::justification(style.align_content, free, heights.len(), false);
    let tops = super::offsets(inner.y + lead, &heights, gap + between);
    let tolerance = style.flow_tolerance.unwrap_or(style.font_size);
    let mut running = vec![0.0; heights.len()];
    let mut cursor = 0;
    for (child, start, span, width, height) in lanes {
        context.charge(1)?;
        let row = start.unwrap_or_else(|| first_eligible(&running, span, cursor, tolerance));
        let left = running[row..row + span].iter().copied().fold(0.0, f64::max);
        let area = super::span(&heights, row, span, gap + between);
        let margins = child.style.margin;
        let align = child.style.align_self.unwrap_or(style.align_items);
        let stretch = align == AlignItems::Stretch
            && child.style.height.is_none()
            && child.style.aspect_ratio.is_none()
            && !child.style.margin_auto[0]
            && !child.style.margin_auto[2];
        let free = (area - height - margins[0] - margins[2]).max(0.0);
        let auto = child.style.margin_auto;
        let shift = super::super::flex::auto_cross_offset(auto[0], auto[2], free);
        let y = tops[row]
            + margins[0]
            + if stretch {
                0.0
            } else {
                shift.unwrap_or_else(|| super::offset(align, free))
            };
        let x = inner.x + left + margins[3];
        let mut target = Flow::new(x, y, width);
        target.definite = Some((area - margins[0] - margins[2]).max(0.0));
        let forced = stretch.then_some((area - margins[0] - margins[2]).max(0.0));
        let border_box = BorderBox {
            x,
            y,
            width,
            height: forced,
        };
        node_layout(context, child, &mut target, items, Some(border_box))?;
        let next = left + (width + margins[1] + margins[3] + stack_gap).max(0.0);
        running[row..row + span].fill(next);
        cursor = row + span;
    }
    for child in out_of_flow_kids {
        context.defer(child.local_id, (inner.x, inner.y));
    }
    inner.y += used;
    inner.bottom = inner.bottom.max(inner.y);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::first_eligible;

    #[test]
    fn auto_placement_takes_the_first_lane_within_tolerance_from_the_cursor() {
        // Shortest is lane 1 (10); lane 2 (12) is within a 5px tolerance and after the cursor.
        assert_eq!(first_eligible(&[20.0, 10.0, 12.0], 1, 2, 5.0), 2);
        // With no tolerance only the shortest lane qualifies.
        assert_eq!(first_eligible(&[20.0, 10.0, 12.0], 1, 2, 0.0), 1);
        // A cursor past the last start wraps to the first lane.
        assert_eq!(first_eligible(&[0.0, 0.0, 0.0], 2, 3, 0.0), 0);
    }
}
