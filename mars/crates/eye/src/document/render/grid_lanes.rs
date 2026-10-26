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
            let mut measure = |context: &mut LayoutContext<'_>, probe: &_| {
                let (defs, _, contributions) = lane_tracks(context, kids, probe, None)?;
                Ok((defs, contributions))
            };
            intrinsic_tracks(context, kids, style, (1, width), &mut measure)?
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
    let spans = lanes
        .iter()
        .map(|lane| (lane.1, lane.2))
        .collect::<Vec<_>>();
    let lines = collapse_fit(&style.grid_columns, &mut columns, explicit, &spans);
    for lane in &mut lanes {
        lane.1 = lane.1.map(|start| super::remap(&lines, start));
    }
    let explicit = super::remap(&lines, explicit);
    let count = columns.len();
    let defs = super::track_sizing(
        [&style.grid_columns, &style.grid_auto_columns],
        explicit,
        &columns,
        width,
    );
    let mut contributions = Vec::new();
    let mut measure = |context: &mut LayoutContext<'_>, kid: &Placed<'_>, _: f64| {
        super::column_contribution(context, kid, &defs)
    };
    let sizing = super::subgrid::Sizing {
        axis: 1,
        gap: style.column_gap,
        content: 0.0,
    };
    for &(node, start, span) in &lanes {
        let starts = start.map_or(0..=count - span, |start| start..=start);
        for column in starts {
            let item = lane_item(node, column, span, 1);
            // A column subgrid's items size the lanes it spans (css-grid-3 subgrids).
            match super::subgrid::contributions(
                context,
                style,
                start.is_some(),
                &item,
                sizing,
                0,
                &mut measure,
            )? {
                Some(list) => contributions.extend(list),
                None => contributions.push(super::column_contribution(context, &item, &defs)?),
            }
        }
    }
    Ok((defs, lanes, contributions))
}

/** Collapses the empty repetitions of an `auto-fit` group of `explicit` lanes, after Blink's
 * grid-lanes heuristic (css-grid-3 repeat-auto-fit): a lane of the group stays when a definitely
 * placed item `(start, span)` covers it or it lies within the summed spans of the auto-placed
 * items, which fill the first lanes. Returns, for each old line index, the new one. */
fn collapse_fit(
    template: &GridTemplate,
    lanes: &mut Vec<Track>,
    explicit: usize,
    spans: &[(Option<usize>, usize)],
) -> Vec<usize> {
    let fit = super::fit_range(template, explicit);
    let unplaced = spans.iter().filter(|s| s.0.is_none()).map(|s| s.1).sum();
    let definite = spans
        .iter()
        .filter_map(|&(start, span)| start.map(|s| (s, span)));
    super::collapse(lanes, fit, definite.chain(std::iter::once((0, unplaced))))
}

/** An item at lane `start` spanning `span` lanes in the grid `axis` (0 rows, 1 columns), one
 * track in the stacking axis, which has none. */
fn lane_item(node: &Node, start: usize, span: usize, axis: usize) -> Placed<'_> {
    let (row, rows, column, columns) = if axis == 0 {
        (start, span, 0, 1)
    } else {
        (0, 1, start, span)
    };
    Placed {
        node,
        row,
        column,
        rows,
        columns,
    }
}

/** Whether the auto-repeat group has a track with no fixed size, which grid-lanes repeats as
 * often as the size its items give it fits (css-grid-3 intrinsic repeat). */
fn intrinsic_repeat(template: &GridTemplate, width: f64) -> bool {
    template.repeat.iter().any(|track| {
        template.length(track.min, Some(width)).is_none()
            && template.length(track.max, Some(width)).is_none()
    })
}

/** Measures the lane track definitions and contributions of a probe style, for
 * `intrinsic_tracks`. */
type Measure<'m> = dyn FnMut(
        &mut LayoutContext<'_>,
        &crate::document::ComputedStyle,
    ) -> EngineResult<(Vec<tracks::TrackSizing>, Vec<tracks::Contribution>)>
    + 'm;

/**
 * The lanes of an intrinsically sized auto-repeat in grid `axis` (0 rows, 1 columns) of `size`,
 * after Blink's `ComputeAutomaticRepetitions` and `CalculateIntrinsicTrackSizes`: repeat the
 * group `2 + (largest span - 2) / group size` times, size those tracks with every item placed
 * everywhere it could go, take each repeated definition's largest size as if it were fixed to
 * count the repetitions that fit, then repeat the original definitions that many times.
 */
fn intrinsic_tracks(
    context: &mut LayoutContext<'_>,
    kids: &[&Node],
    style: &crate::document::ComputedStyle,
    (axis, size): (usize, f64),
    measure: &mut Measure<'_>,
) -> EngineResult<Vec<Track>> {
    let (template, gap) = if axis == 1 {
        (&style.grid_columns, style.column_gap)
    } else {
        (&style.grid_rows, style.row_gap)
    };
    let group = template.repeat.len();
    let largest = kids
        .iter()
        .map(|node| lane_lines(node, style, axis, 0).1)
        .max()
        .unwrap_or(1);
    let times = 2 + largest.saturating_sub(2) / group;
    let mut probe = template.clone();
    probe.repeat.clear();
    let at = template.repeat_at.min(template.tracks.len());
    let repeated = (0..times).flat_map(|_| template.repeat.iter().copied());
    probe.tracks.splice(at..at, repeated);
    let mut probe_style = style.clone();
    *if axis == 1 {
        &mut probe_style.grid_columns
    } else {
        &mut probe_style.grid_rows
    } = probe;
    let (defs, contributions) = measure(context, &probe_style)?;
    let space = tracks::Space::MaxContent(0.0);
    let sizes = tracks::size_tracks(&defs, &contributions, space, gap, false);
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
    let mut lanes = super::expand(&sized, size, gap);
    let end = (at + lanes.len().saturating_sub(template.tracks.len())).min(lanes.len());
    for (offset, track) in lanes[at..end].iter_mut().enumerate() {
        *track = template.repeat[offset % group];
    }
    Ok(lanes)
}

/** Two used sizes the same within layout rounding. */
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/**
 * The stacking state of every lane, after Blink's `GridLanesRunningPositions`: each lane is a
 * list of openings `(start, end)` whose last one is the open end at the lane's running
 * position. Dense packing keeps the gaps a spanner leaves above itself as openings later
 * items may fill; track-reverse breaks ties towards the last lane and walks the cursor back.
 */
struct Running {
    /** Each lane's openings `(start, end, item above)` in stacking order; the last one ends
     * at infinity. The item above is the one stacking-axis alignment gives the opening to. */
    openings: Vec<Vec<(f64, f64, Option<usize>)>>,
    /** Each lane's used size in the grid axis, which a dense item's span must keep. */
    sizes: Vec<f64>,
    /** The auto-placement cursor: a line, the start of the next search. */
    cursor: usize,
    /** How far above the shortest lane an eligible lane may be (`flow-tolerance`). */
    tolerance: f64,
    /** `grid-lanes-direction: track-reverse`. */
    reverse: bool,
    /** `grid-lanes-pack: dense`. */
    dense: bool,
    /** Whether some item aligns in the stacking axis, which keeps the openings below items. */
    align: bool,
}

impl Running {
    /** Fresh lanes for a container; the cursor starts at the last line under track-reverse. */
    fn new(sizes: &[f64], style: &crate::document::ComputedStyle, tolerance: f64) -> Self {
        let reverse = style.grid_lanes_track_reverse;
        Self {
            openings: vec![vec![(0.0, f64::INFINITY, None)]; sizes.len()],
            sizes: sizes.to_vec(),
            cursor: if reverse { sizes.len() } else { 0 },
            tolerance,
            reverse,
            dense: style.grid_lanes_dense,
            align: false,
        }
    }

    /** A lane's running position: the start of its open end. */
    fn position(&self, lane: usize) -> f64 {
        self.openings[lane].last().map_or(0.0, |opening| opening.0)
    }

    /** The highest running position among the lanes `start..start + span`. */
    fn max_for(&self, start: usize, span: usize) -> f64 {
        (start..start + span)
            .map(|lane| self.position(lane))
            .fold(0.0, f64::max)
    }

    /** Every start a span may take, from `from` on in the track direction, wrapping once
     * (Blink's `RunningPositionsIterator`). */
    fn starts(&self, span: usize, from: usize) -> Vec<usize> {
        let last = self.openings.len() - span;
        let count = last + 1;
        if self.reverse {
            let first = if from < span { last } else { from - span };
            (0..count)
                .map(|step| (first + count - step) % count)
                .collect()
        } else {
            let first = if from > last { 0 } else { from };
            (0..count).map(|step| (first + step) % count).collect()
        }
    }

    /** The start lane of an auto-placed item spanning `span` lanes and its top: the first,
     * from the cursor, whose highest spanned running position is within the tolerance of the
     * lowest such position (Blink's `GetFirstEligibleLine`). */
    fn first_eligible(&self, span: usize) -> (usize, f64) {
        let last = self.openings.len() - span;
        let lowest = (0..=last)
            .map(|start| self.max_for(start, span))
            .fold(f64::INFINITY, f64::min);
        let allowed = lowest + self.tolerance;
        let starts = self.starts(span, self.cursor);
        let start = starts
            .iter()
            .copied()
            .find(|&start| self.max_for(start, span) <= allowed)
            .unwrap_or(starts[0]);
        (start, self.max_for(start, span))
    }

    /** The openings, one per lane from `lane` on, that together hold `size` within the range
     * `(low, high)`, and the highest top they share (Blink's
     * `AccumulateTrackOpeningsToAccommodateItem`). Indices come back last lane first. */
    fn fit(
        &self,
        lane: usize,
        left: usize,
        size: f64,
        low: f64,
        high: f64,
    ) -> Option<(f64, Vec<usize>)> {
        for (index, &(start, end, _)) in self.openings[lane].iter().enumerate() {
            let (top, bottom) = (low.max(start), high.min(end));
            if top > bottom || bottom - top < size {
                continue;
            }
            let found = if left == 0 {
                Some((top, Vec::new()))
            } else {
                self.fit(lane + 1, left - 1, size, top, bottom)
            };
            if let Some((top, mut indices)) = found {
                indices.push(index);
                return Some((top, indices));
            }
        }
        None
    }

    /**
     * Dense packing: the highest opening, from the first lane in the track direction, that
     * holds an item of stacking size `size` across lanes of the same used size as its
     * placement `(start, span)` at `top`, and only when it is above that placement (or level
     * and earlier). The opening is then split around the item (Blink's
     * `GetEligibleTrackOpeningAndUpdateGridLanesItemSpan`).
     */
    fn dense_opening(
        &mut self,
        item: usize,
        start: usize,
        span: usize,
        auto: bool,
        size: f64,
        top: f64,
    ) -> Option<(usize, f64)> {
        let used = self.sizes[start..start + span].iter().sum::<f64>();
        let from = if self.reverse { self.openings.len() } else { 0 };
        let mut best: Option<(usize, f64, Vec<usize>)> = None;
        for lane in self.starts(span, from) {
            let lane = if auto { lane } else { start };
            if !same(self.sizes[lane..lane + span].iter().sum(), used) {
                continue;
            }
            let (best_lane, best_top) = best.as_ref().map_or((start, top), |b| (b.0, b.1));
            if self.openings[lane].first().is_some_and(|o| o.0 >= best_top) {
                continue;
            }
            let Some((found, indices)) = self.fit(lane, span - 1, size, 0.0, f64::INFINITY) else {
                continue;
            };
            let earlier = if self.reverse {
                lane > best_lane
            } else {
                lane < best_lane
            };
            if found < best_top || (found == best_top && earlier) {
                best = Some((lane, found, indices));
            }
        }
        let (lane, found, indices) = best?;
        for (offset, index) in indices.into_iter().rev().enumerate() {
            let openings = &mut self.openings[lane + offset];
            let (start, end, above) = openings[index];
            let mut at = index;
            if start < found {
                openings.insert(at, (start, found, above));
                at += 1;
            }
            if end.is_finite() && found + size >= end - 0.01 {
                openings.remove(at);
            } else {
                openings[at].0 = found + size;
                openings[at].2 = Some(item);
            }
        }
        Some((lane, found))
    }

    /** Stacks an item of stacking contribution `size` at `top` across `start..start + span`;
     * under dense packing the gap each lane had below `top` stays an opening. */
    fn place(&mut self, item: usize, start: usize, span: usize, top: f64, size: f64) {
        for lane in start..start + span {
            let current = self.position(lane);
            let openings = &mut self.openings[lane];
            let last = openings.len() - 1;
            if (self.dense || self.align) && current < top {
                openings[last].1 = top;
                openings.push((top + size, f64::INFINITY, Some(item)));
            } else {
                openings[last].0 = top + size;
                openings[last].2 = Some(item);
            }
        }
        self.cursor = if self.reverse { start } else { start + span };
    }

    /** Places one item of stacking contribution `size` and returns its start lane and top. */
    fn next(&mut self, item: usize, start: Option<usize>, span: usize, size: f64) -> (usize, f64) {
        let (lane, top) = match start {
            Some(start) => (start, self.max_for(start, span)),
            None => self.first_eligible(span),
        };
        if self.dense
            && let Some(found) = self.dense_opening(item, lane, span, start.is_none(), size, top)
        {
            return found;
        }
        self.place(item, lane, span, top, size);
        (lane, top)
    }

    /**
     * The free space below `item` in the stacking axis once the lanes end at `extent`: the
     * smallest opening it is above across `start..start + span`, nothing when a spanned lane
     * has another item right below it (Blink's `GetAvailableAlignmentSpaceForItem`, after
     * `FinalizeTrackOpeningsForStackingAxisAlignment` takes the trailing gap off).
     */
    fn alignment_space(
        &self,
        item: usize,
        start: usize,
        span: usize,
        extent: f64,
        gap: f64,
    ) -> f64 {
        let mut space = f64::INFINITY;
        for lane in start..start + span {
            let openings = &self.openings[lane];
            let Some(index) = openings.iter().position(|o| o.2 == Some(item)) else {
                return 0.0;
            };
            let (top, end, _) = openings[index];
            let (top, end) = if index + 1 == openings.len() {
                (top - gap, extent)
            } else {
                (top, end)
            };
            space = space.min(end - top);
        }
        space.max(0.0)
    }

    /** The stacking size the lanes reach. */
    fn extent(&self) -> f64 {
        (0..self.openings.len())
            .map(|lane| self.position(lane))
            .fold(0.0, f64::max)
    }
}

/** Whether an item stretches in the stacking axis: its `align-self` (or the container's
 * `align-items`) is literally `stretch`, as `normal` keeps it at the start (Blink's
 * grid-lanes stacking alignment). */
pub(super) fn explicit_stretch(container: &crate::document::ComputedStyle, child: &Node) -> bool {
    match child.style.align_self {
        Some(align) => align == AlignItems::Stretch && child.style.align_stretch[1],
        None => container.align_items == AlignItems::Stretch && container.align_stretch[0],
    }
}

/** `explicit_stretch` for row lanes, whose stacking axis `justify-self` aligns in. */
pub(super) fn explicit_justify_stretch(
    container: &crate::document::ComputedStyle,
    child: &Node,
) -> bool {
    match child.style.justify_self {
        Some(align) => align == AlignItems::Stretch && child.style.align_stretch[3],
        None => container.justify_items == AlignItems::Stretch && container.align_stretch[2],
    }
}

/** Where content distribution (`align-content` for column lanes, `justify-content` for row
 * lanes, with its `safe` flag) puts a stack `total` long in `extent`: its start and end
 * offsets. `safe` packs an overflowing stack at its start, the end edge under fill-reverse;
 * under `normal` a fill-reverse stack still ends at the container's end. */
fn stack_block(
    style: &crate::document::ComputedStyle,
    (content, safe): (JustifyContent, bool),
    (extent, total): (f64, f64),
) -> (f64, f64) {
    if content == JustifyContent::Stretch {
        return (0.0, extent);
    }
    let (lead, _) = super::super::flex_layout::justification(content, extent - total, 1, false);
    let lead = match (safe && extent < total, style.grid_lanes_fill_reverse) {
        (true, true) => extent - total,
        (true, false) => 0.0,
        _ => lead,
    };
    (lead, lead + total)
}

/** Whether an item's size in an axis may stretch: it is `auto` there, with no aspect ratio, no
 * `auto` margin and no replaced content (css-align-3 `stretch`). */
fn stretchable(child: &Node) -> bool {
    child.style.height.is_none()
        && !child.style.height_fit
        && child.style.aspect_ratio.is_none()
        && !child.style.margin_auto[0]
        && !child.style.margin_auto[2]
        && !["img", "canvas", "svg", "video"].contains(&child.tag.as_str())
}

/** `stretchable` in the inline axis. */
fn stretchable_width(child: &Node) -> bool {
    child.style.width.is_none()
        && !child.style.width_fit
        && child.style.aspect_ratio.is_none()
        && !child.style.margin_auto[1]
        && !child.style.margin_auto[3]
        && !["img", "canvas", "svg", "video"].contains(&child.tag.as_str())
}

/** Whether an item's stacking-axis alignment moves it within the space beside it: centered
 * or end items, and under fill-reverse (which stacks from the end) start and baseline ones too;
 * `normal` and `stretch` share a value here and stay put. Such items keep the openings their
 * lanes leave around them. */
fn stacking_aligned(align: AlignItems, reverse: bool) -> bool {
    matches!(align, AlignItems::Center | AlignItems::End)
        || (reverse && matches!(align, AlignItems::Start | AlignItems::Baseline))
}

/** How far an item moves in the stacking axis within the free `space` below it (above it
 * under fill-reverse, where items already sit at the end), after Blink's
 * `ApplyStackingAxisAlignment`. */
fn stacking_shift(align: AlignItems, space: f64, reverse: bool) -> f64 {
    let space = if space.is_finite() { space } else { 0.0 };
    match (align, reverse) {
        (AlignItems::Center, false) => space / 2.0,
        (AlignItems::End, false) => space,
        (AlignItems::Center, true) => -space / 2.0,
        (AlignItems::Start | AlignItems::Baseline, true) => -space,
        _ => 0.0,
    }
}

/** The used `flow-tolerance`, its percentage against the stacking axis's available size
 * (zero when indefinite), else 1em (Blink's `ResolveFlowToleranceForGridLanes`). */
fn tolerance(style: &crate::document::ComputedStyle, available: Option<f64>) -> f64 {
    match style.flow_tolerance {
        None => style.font_size,
        Some(px) if px.is_infinite() => px,
        Some(px) => {
            let percent = style.flow_tolerance_percent.unwrap_or(0.0);
            (px + percent * available.unwrap_or(0.0) / 100.0).max(0.0)
        }
    }
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
    let starts = super::offsets(inner.x + lead, &widths, gap + between);
    let rtl = style.rtl;
    let lefts = super::physical_lefts(&starts, &widths, (inner.x, inner.width), rtl);
    let mut running = Running::new(&widths, style, tolerance(style, inner.definite));
    let stacking = |child: &Node| child.style.align_self.unwrap_or(style.align_items);
    // Stretched items, like aligned ones, need the gaps left below them.
    running.align = kids.iter().any(|child| {
        stacking_aligned(stacking(child), style.grid_lanes_fill_reverse)
            || explicit_stretch(style, child)
    });
    let mut plan = Vec::with_capacity(lanes.len());
    for (index, (child, start, span)) in lanes.into_iter().enumerate() {
        context.charge(1)?;
        let margins = child.style.margin;
        // The item's width does not depend on the lane it lands in once the span's used size is
        // known, so measure against the span at its first candidate start.
        let probe = start.unwrap_or_else(|| running.first_eligible(span).0);
        let probe_area = super::span(&widths, probe, span, gap + between);
        // A column subgrid takes the lanes it spans as its columns (css-grid-3 subgrids).
        // An auto-placed one inherits no line names: its lanes are not known up front.
        let names = start.is_some();
        super::subgrid::offer(
            style,
            names,
            &lane_item(child, probe, span, 1),
            1,
            &widths,
            gap + between,
        );
        let width = if super::replaced_stretch(style, child, 1) {
            (probe_area - child.style.margin[1] - child.style.margin[3]).max(0.0)
        } else {
            super::item_width(
                context,
                child,
                probe_area,
                super::sizing_justify(style, child),
            )?
        };
        let height = super::super::flex_layout::item_height(context, child, width)?;
        // Margins never move the running position backwards (Blink clamps the contribution).
        let size = (height + margins[0] + margins[2] + stack_gap).max(0.0);
        let (column, top) = running.next(index, start, span, size);
        if column != probe {
            // ponytail: the height stays the one measured in the probe lanes, exact while lanes
            // of one span share a used size; measure again here if a page mixes lane sizes.
            let item = lane_item(child, column, span, 1);
            super::subgrid::offer(style, names, &item, 1, &widths, gap + between);
        }
        plan.push((child, column, span, top, width, height));
    }
    let placed = plan
        .iter()
        .map(|&(child, column, span, ..)| lane_item(child, column, span, 1))
        .collect::<Vec<_>>();
    let total = if kids.is_empty() {
        0.0
    } else {
        (running.extent() - stack_gap).max(0.0)
    };
    let extent = inner.definite.unwrap_or(total);
    // `align-content` places the whole stack in a taller container; under `normal` a
    // fill-reverse stack still starts from the container's end.
    let (lead, end) = stack_block(
        style,
        (style.align_content, style.content_safe[0]),
        (extent, total),
    );
    // Items of one lane line up their synthesized column baselines, as in a grid column.
    let natural = plan
        .iter()
        .map(|&(.., width, height)| (width, height))
        .collect::<Vec<_>>();
    let column_shims = super::column_baseline::shims(style, &placed, &natural);
    for (index, (child, column, span, top, width, height)) in plan.into_iter().enumerate() {
        let space = running.alignment_space(index, column, span, extent, stack_gap);
        let shift = stacking_shift(stacking(child), space, style.grid_lanes_fill_reverse);
        // A stretched item with an auto height fills the space below it in its lanes.
        let stretched = explicit_stretch(style, child) && stretchable(child);
        let height = height
            + if stretched && space.is_finite() {
                space
            } else {
                0.0
            };
        let area = super::span(&widths, column, span, gap + between);
        let justify = super::justify_of(style, child);
        let margins = child.style.margin;
        let auto = child.style.margin_auto;
        let free = (area - width - margins[1] - margins[3]).max(0.0);
        let first = if rtl { column + span - 1 } else { column };
        let x = lefts[first]
            + margins[3]
            + super::super::flex::auto_cross_offset(auto[3], auto[1], free)
                .unwrap_or_else(|| super::inline_offset(super::offset(justify, free), free, rtl))
            + column_shims[index];
        // fill-reverse stacks from the end edge, the item's margin box mirrored.
        let y = shift
            + if style.grid_lanes_fill_reverse {
                inner.y + end - top - height - margins[2]
            } else {
                inner.y + lead + top + margins[0]
            };
        let mut target = Flow::new(x, y, width);
        let border_box = BorderBox {
            x,
            y,
            width,
            height: stretched.then_some(height),
        };
        node_layout(context, child, &mut target, items, Some(border_box))?;
    }
    for child in out_of_flow_kids {
        context.defer(child.local_id, (inner.x, inner.y));
    }
    super::subgrid::withdraw(&placed);
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

/** A row grid-lanes container's min- and max-content widths: the items stacked along the rows
 * by the running positions, at their min- or max-content outer widths, as layout stacks them. */
pub(super) fn row_intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
    kids: &[&Node],
) -> EngineResult<super::super::sizing::IntrinsicWidths> {
    let style = &node.style;
    let explicit = super::expand(&style.grid_rows, 0.0, style.row_gap).len();
    let mut lanes = Vec::with_capacity(kids.len());
    for child in kids {
        let (start, span) = lane_lines(child, style, 0, explicit);
        let widths = super::column_contribution(context, &lane_item(child, 0, 1, 1), &[])?;
        lanes.push((start, span.clamp(1, GRID_COLUMNS), widths));
    }
    let reach = lanes.iter().filter_map(|lane| lane.0.map(|s| s + lane.1));
    let count = reach
        .max()
        .unwrap_or(0)
        .max(explicit)
        .clamp(1, GRID_COLUMNS);
    let stack_gap = style.column_gap;
    let extent = |pick: fn(&tracks::Contribution) -> f64| {
        let mut running = Running::new(&vec![0.0; count], style, tolerance(style, None));
        for (index, (start, span, widths)) in lanes.iter().enumerate() {
            let start = start.map(|start| start.min(count - span.min(&count)));
            running.next(index, start, (*span).min(count), pick(widths) + stack_gap);
        }
        if lanes.is_empty() {
            0.0
        } else {
            (running.extent() - stack_gap).max(0.0)
        }
    };
    Ok(super::super::sizing::IntrinsicWidths {
        min_content: extent(|widths| widths.min_content),
        max_content: extent(|widths| widths.max_content),
    })
}

/** Whether the lanes are rows: `grid-lanes-direction: row`, or under `normal` when
 * `grid-template-rows` is set and `grid-template-columns` is not (Blink's
 * `GridLanesTrackSizingDirection`). */
pub(super) fn row_lanes(style: &crate::document::ComputedStyle) -> bool {
    let set = |template: &GridTemplate| !template.tracks.is_empty() || !template.repeat.is_empty();
    style.grid_lanes
        && style
            .grid_lanes_rows
            .unwrap_or_else(|| !set(&style.grid_columns) && set(&style.grid_rows))
}

/** A row lane per item: node, definite start (None when auto-placed), span, width and height. */
type RowLane<'a> = (&'a Node, Option<usize>, usize, f64, f64);
/** Row lane track definitions, lanes and contributions, as `row_lane_tracks` returns them. */
type RowLanes<'a> = (
    Vec<tracks::TrackSizing>,
    Vec<RowLane<'a>>,
    Vec<tracks::Contribution>,
);

/** The row lane track definitions, every item's lane with its `measured` (width, height),
 * and the row contributions, auto-placed items once per possible start, for rows of
 * `height` (None when indefinite) separated by `gap`. */
fn row_lane_tracks<'a>(
    context: &mut LayoutContext<'_>,
    kids: &[&'a Node],
    measured: &[(f64, f64)],
    style: &crate::document::ComputedStyle,
    (height, gap): (Option<f64>, f64),
) -> EngineResult<RowLanes<'a>> {
    let mut tracks_list = match height {
        Some(height) if intrinsic_repeat(&style.grid_rows, height) => {
            let mut measure = |context: &mut LayoutContext<'_>, probe: &_| {
                let (defs, _, contributions) =
                    row_lane_tracks(context, kids, measured, probe, (None, gap))?;
                Ok((defs, contributions))
            };
            intrinsic_tracks(context, kids, style, (0, height), &mut measure)?
        }
        _ => super::expand(&style.grid_rows, height.unwrap_or(0.0), gap),
    };
    let explicit = tracks_list.len();
    let mut lanes = Vec::with_capacity(kids.len());
    for (child, &(width, size)) in kids.iter().zip(measured) {
        let (start, span) = lane_lines(child, style, 0, explicit);
        lanes.push((*child, start, span.clamp(1, GRID_COLUMNS), width, size));
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
    for lane in &mut lanes {
        lane.2 = lane.2.min(count);
        lane.1 = lane.1.map(|start| start.min(count - lane.2));
    }
    let spans = lanes
        .iter()
        .map(|lane| (lane.1, lane.2))
        .collect::<Vec<_>>();
    let lines = collapse_fit(&style.grid_rows, &mut tracks_list, explicit, &spans);
    for lane in &mut lanes {
        lane.1 = lane.1.map(|start| super::remap(&lines, start));
    }
    let explicit = super::remap(&lines, explicit);
    let count = tracks_list.len();
    let defs = super::track_sizing(
        [&style.grid_rows, &style.grid_auto_rows],
        explicit,
        &tracks_list,
        height,
    );
    // Items placed in a row share its baseline; an auto-placed one joins a row only when it
    // can land nowhere else, since its row is not known while the rows are sized.
    let only = |lane: &RowLane<'_>| lane.1.or((lane.2 == count).then_some(0));
    let shims = lane_shims(context, (style, &defs), &lanes, only)?;
    let mut contributions = Vec::new();
    for (lane, shim) in lanes.iter().zip(shims) {
        let margins = lane.0.style.margin[0] + lane.0.style.margin[2] + shim;
        // A row subgrid's items are measured in its own columns, at its shrink-to-fit width.
        let sizing = super::subgrid::Sizing {
            axis: 0,
            gap,
            content: lane.3 - horizontal_edges(lane.0),
        };
        for row in lane.1.map_or(0..=count - lane.2, |start| start..=start) {
            let item = lane_item(lane.0, row, lane.2, 0);
            match super::subgrid::contributions(
                context,
                style,
                lane.1.is_some(),
                &item,
                sizing,
                0,
                &mut super::row_measure,
            )? {
                Some(list) => contributions.extend(list),
                None => contributions.push(super::row_contribution(&item, lane.4 + margins)),
            }
        }
    }
    Ok((defs, lanes, contributions))
}

/** The baseline shim of each row lane item in the row `row` gives it, None joining no row. */
fn lane_shims(
    context: &mut LayoutContext<'_>,
    (style, rows): (&crate::document::ComputedStyle, &[tracks::TrackSizing]),
    lanes: &[RowLane<'_>],
    row: impl Fn(&RowLane<'_>) -> Option<usize>,
) -> EngineResult<Vec<f64>> {
    let mut joined = Vec::new();
    let mut natural = Vec::new();
    let mut at = Vec::with_capacity(lanes.len());
    for lane in lanes {
        at.push(row(lane).map(|start| {
            joined.push(lane_item(lane.0, start, lane.2, 0));
            natural.push((lane.3, lane.4));
            joined.len() - 1
        }));
    }
    let shims = super::baseline::shims(context, style, &joined, &natural, rows)?;
    Ok(at
        .iter()
        .map(|index| index.map_or(0.0, |i| shims[i]))
        .collect())
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
    // Percentage gaps as in grid layout: the stacking gap against the width, the row gap
    // against a definite height, else against the sized rows once they are sized.
    let gap = style.row_gap + style.gap_percent[1] * inner.definite.unwrap_or(0.0) / 100.0;
    let stack_gap = style.column_gap + style.gap_percent[0] * inner.width / 100.0;
    let mut measured = Vec::with_capacity(kids.len());
    for child in kids {
        let width = super::item_width(context, child, inner.width, AlignItems::Start)?;
        let height = super::super::flex_layout::item_height(context, child, width)?;
        measured.push((width, height));
    }
    let (defs, lanes, contributions) =
        row_lane_tracks(context, kids, &measured, style, (inner.definite, gap))?;
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
    let late = match inner.definite {
        None => style.gap_percent[1] * used / 100.0,
        Some(_) => 0.0,
    };
    let row_gap = gap + between + late;
    let tops = super::offsets(inner.y + lead, &heights, row_gap);
    let mut running = Running::new(&heights, style, tolerance(style, Some(inner.width)));
    let stacking = |child: &Node| super::justify_of(style, child);
    running.align = kids.iter().any(|child| {
        stacking_aligned(stacking(child), style.grid_lanes_fill_reverse)
            || explicit_justify_stretch(style, child)
    });
    let mut plan = Vec::with_capacity(lanes.len());
    let mut auto = Vec::with_capacity(lanes.len());
    for (index, (child, start, span, width, height)) in lanes.into_iter().enumerate() {
        context.charge(1)?;
        let size = (width + child.style.margin[1] + child.style.margin[3] + stack_gap).max(0.0);
        let (row, left) = running.next(index, start, span, size);
        plan.push((child, row, span, left, width, height));
        auto.push(start.is_none());
    }
    let placed = plan
        .iter()
        .map(|&(child, row, span, ..)| lane_item(child, row, span, 0))
        .collect::<Vec<_>>();
    let landed = plan
        .iter()
        .map(|&(child, row, span, _, width, height)| (child, Some(row), span, width, height))
        .collect::<Vec<_>>();
    let shims = lane_shims(context, (style, &defs), &landed, |lane| lane.1)?;
    let total = (running.extent() - stack_gap).max(0.0);
    let content = (style.justify_content, style.content_safe[1]);
    let (lead, end) = stack_block(style, content, (inner.width, total));
    for (index, (child, row, span, left, width, height)) in plan.into_iter().enumerate() {
        // A row subgrid takes the rows it spans as its own (css-grid-3 subgrids), and their
        // line names unless it was auto-placed.
        super::subgrid::offer(style, !auto[index], &placed[index], 0, &heights, row_gap);
        let space = running.alignment_space(index, row, span, inner.width, stack_gap);
        // Under rtl the rows stack from the right edge, as fill-reverse stacks from the end.
        let from_right = style.grid_lanes_fill_reverse != style.rtl;
        // The shift is logical: rtl mirrors it with the stacking direction.
        let shift = stacking_shift(stacking(child), space, style.grid_lanes_fill_reverse);
        let along = if style.rtl { -shift } else { shift };
        // A stretched item with an auto width fills the space after it in its lanes.
        let stretched = explicit_justify_stretch(style, child) && stretchable_width(child);
        let width = width
            + if stretched && space.is_finite() {
                space
            } else {
                0.0
            };
        let area = super::span(&heights, row, span, row_gap);
        let margins = child.style.margin;
        let align = child.style.align_self.unwrap_or(style.align_items);
        // A subgrid fills its area in a subgridded axis whatever its alignment.
        let stretch = super::subgrid::inherits(child, 0)
            || (align == AlignItems::Stretch
                && child.style.height.is_none()
                && !child.style.height_fit
                && child.style.aspect_ratio.is_none()
                && !child.style.margin_auto[0]
                && !child.style.margin_auto[2]);
        let free = (area - height - margins[0] - margins[2]).max(0.0);
        let auto = child.style.margin_auto;
        let shift = super::super::flex::auto_cross_offset(auto[0], auto[2], free);
        let y = tops[row]
            + margins[0]
            + if stretch {
                0.0
            } else {
                shift.unwrap_or_else(|| super::offset(align, free)) + shims[index]
            };
        // fill-reverse stacks from the end edge, the item's margin box mirrored.
        let x = along
            + match (from_right, style.rtl) {
                (true, false) => inner.x + end - left - width - margins[1],
                (true, true) => inner.x + inner.width - left - width - margins[1],
                (false, _) => inner.x + lead + left + margins[3],
            };
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
    }
    for child in out_of_flow_kids {
        context.defer(child.local_id, (inner.x, inner.y));
    }
    super::subgrid::withdraw(&placed);
    inner.y += used;
    inner.bottom = inner.bottom.max(inner.y);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Running, stacking_shift};
    use crate::document::AlignItems;
    use crate::document::ComputedStyle;

    fn lanes(count: usize, configure: impl FnOnce(&mut ComputedStyle), tolerance: f64) -> Running {
        let mut style = ComputedStyle::default();
        configure(&mut style);
        Running::new(&vec![100.0; count], &style, tolerance)
    }

    #[test]
    fn auto_placement_takes_the_first_lane_within_tolerance_from_the_cursor() {
        let mut running = lanes(3, |_| {}, 5.0);
        running.openings = vec![
            vec![(20.0, f64::INFINITY, None)],
            vec![(10.0, f64::INFINITY, None)],
            vec![(12.0, f64::INFINITY, None)],
        ];
        running.cursor = 2;
        // Shortest is lane 1 (10); lane 2 (12) is within a 5px tolerance and after the cursor.
        assert_eq!(running.first_eligible(1).0, 2);
        // With no tolerance only the shortest lane qualifies.
        running.tolerance = 0.0;
        assert_eq!(running.first_eligible(1).0, 1);
        // A cursor past the last start wraps to the first lane.
        let mut wrap = lanes(3, |_| {}, 0.0);
        wrap.cursor = 3;
        assert_eq!(wrap.first_eligible(2).0, 0);
    }

    #[test]
    fn track_reverse_breaks_ties_towards_the_last_lane_and_walks_back() {
        let mut running = lanes(3, |style| style.grid_lanes_track_reverse = true, 0.0);
        assert_eq!(running.next(0, None, 1, 10.0), (2, 0.0));
        assert_eq!(running.next(0, None, 1, 10.0), (1, 0.0));
        assert_eq!(running.next(0, None, 1, 10.0), (0, 0.0));
        assert_eq!(running.next(0, None, 2, 10.0), (1, 10.0));
    }

    #[test]
    fn dense_packing_fills_the_opening_a_spanner_left_above_itself() {
        let mut running = lanes(2, |style| style.grid_lanes_dense = true, 0.0);
        assert_eq!(running.next(0, None, 1, 50.0), (0, 0.0));
        // The spanner sits below lane 0's item, leaving 50px open in lane 1.
        assert_eq!(running.next(0, None, 2, 10.0), (0, 50.0));
        assert_eq!(running.next(0, None, 1, 30.0), (1, 0.0));
        assert_eq!(running.next(0, None, 1, 30.0), (0, 60.0));
        // Without dense packing the hole stays empty.
        let mut sparse = lanes(2, |_| {}, 0.0);
        sparse.next(0, None, 1, 50.0);
        sparse.next(0, None, 2, 10.0);
        assert_eq!(sparse.next(0, None, 1, 30.0), (0, 60.0));
    }

    #[test]
    fn stacking_alignment_takes_the_space_below_the_last_item_in_each_lane() {
        let mut running = lanes(2, |_| {}, 0.0);
        running.align = true;
        running.next(0, None, 1, 50.0);
        running.next(1, None, 1, 20.0);
        // Lanes end at 100: item 1 has 100 - (20 - 0 gap) = 80 below it, item 0 has 50.
        assert_eq!(running.alignment_space(1, 1, 1, 100.0, 0.0), 80.0);
        assert_eq!(running.alignment_space(0, 0, 1, 100.0, 0.0), 50.0);
        // A spanner under item 0 leaves it only the opening it sits above.
        running.next(2, None, 2, 10.0);
        assert_eq!(running.alignment_space(0, 0, 1, 100.0, 0.0), 0.0);
        assert_eq!(running.alignment_space(1, 1, 1, 100.0, 0.0), 30.0);
        assert_eq!(stacking_shift(AlignItems::Center, 30.0, false), 15.0);
        assert_eq!(stacking_shift(AlignItems::Start, 30.0, true), -30.0);
        assert_eq!(stacking_shift(AlignItems::End, 30.0, true), 0.0);
    }
}
