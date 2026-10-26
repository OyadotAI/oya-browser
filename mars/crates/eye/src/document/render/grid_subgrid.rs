/** Subgrids (css-grid-2 section 9), after Blink's subgridded track sizing: a grid item whose
 * `grid-template-rows` or `-columns` is `subgrid` takes its parent's tracks and line names in
 * that axis, its items take part in the parent's track sizing, and the difference between the
 * two gaps becomes half-gap margins on the subgrid's inner edges. */
use super::super::flow::*;
use super::placement::{AUTO_TRACK, Placed, place};
use super::tracks::Contribution;
use crate::document::ComputedStyle;
use crate::document::style::{Extent, GridTemplate, Track, TrackContent, TrackSize};
use std::cell::RefCell;
use std::collections::BTreeMap;

/** Nested subgrids a parent's track sizing looks into; deeper ones size as plain items. */
const SUBGRID_DEPTH: usize = 8;

/** What a subgrid inherits in one axis, in its own coordinates. */
#[derive(Clone, Debug)]
pub(super) struct Inherited {
    /** Track sizes, already less its edges and plus the half-gap differences. */
    pub sizes: Vec<f64>,
    /** The gap between them: the subgrid's own, or the parent's when `normal`. */
    pub gap: f64,
    /** The parent's line names in the span, renumbered from the subgrid's first line. */
    pub names: Vec<(String, u16)>,
}

thread_local! {
    /** Inherited `[rows, columns]` per subgrid identity, offered by the parent grid's layout
     * before it lays the subgrid out and withdrawn when that layout ends. */
    static INHERITED: RefCell<BTreeMap<usize, [Option<Inherited>; 2]>> =
        const { RefCell::new(BTreeMap::new()) };
}

/** Whether `node` is a grid whose `axis` (0 rows, 1 columns) is subgridded; a grid-lanes box or
 * a non-grid never is. */
pub(super) fn subgridded(node: &Node, axis: usize) -> bool {
    let style = &node.style;
    style.display_grid && !style.grid_lanes && template(style, own_axis(node, axis)).subgrid
}
/**
 * The subgrid's own axis that a parent's `axis` is. A node still marked vertical is seen from a
 * horizontal parent (render/vertical.rs lays a vertical subtree out with every style turned
 * horizontal), so it is orthogonal and its rows run along the parent's columns (css-grid-2 9).
 */
fn own_axis(node: &Node, axis: usize) -> usize {
    if node.style.vertical_writing {
        1 - axis
    } else {
        axis
    }
}
/** Whether an orthogonal subgrid's block axis runs against the parent's columns
 * (`vertical-rl`), so the tracks it inherits come in reverse order. */
fn reversed(node: &Node) -> bool {
    node.style.vertical_writing && !node.style.writing_lr
}
/** The template of one axis. */
fn template(style: &ComputedStyle, axis: usize) -> &GridTemplate {
    if axis == 0 {
        &style.grid_rows
    } else {
        &style.grid_columns
    }
}
/** The span an auto-spanned subgrid takes in a subgridded axis: one less than its line-name
 * lists, or 1 without any (css-grid-2 9.1). */
pub(super) fn auto_span(node: &Node, axis: usize) -> Option<u16> {
    subgridded(node, axis).then(|| {
        template(&node.style, own_axis(node, axis))
            .subgrid_lines
            .saturating_sub(1)
            .max(1)
    })
}
/** A subgrid's margin, border and padding at the start and end of `axis`, which its first and
 * last tracks give up (Blink's subgrid edge margins). */
fn edges(node: &Node, axis: usize) -> (f64, f64) {
    let style = &node.style;
    let (m, b, p) = (
        style.margin,
        super::super::sizing::border_sides(style),
        style.padding,
    );
    let (left, right) = (m[3] + b[3] + p[3], m[1] + b[1] + p[1]);
    match (axis, style.rtl) {
        (0, _) => (m[0] + b[0] + p[0], m[2] + b[2] + p[2]),
        // Under rtl the columns start at the right edge.
        (_, true) => (right, left),
        _ => (left, right),
    }
}
/** The subgrid's gap in `axis`: its own when set, else the parent's (css-grid-2 9.2). */
fn own_gap(node: &Node, axis: usize, parent_gap: f64) -> f64 {
    let style = &node.style;
    let axis = own_axis(node, axis);
    // `gap_set` and `gap_percent` are `[column, row]`.
    let index = usize::from(axis == 0);
    if !style.gap_set[index] {
        return parent_gap;
    }
    if axis == 0 {
        style.row_gap
    } else {
        style.column_gap
    }
}
/** Records what `item` inherits in `axis` from the parent's track `sizes` separated by `gap`,
 * and the line names of `parent` when `names` passes them on (an auto-placed subgrid of grid
 * lanes inherits none, css-grid-3). */
pub(super) fn offer(
    parent: &ComputedStyle,
    names: bool,
    item: &Placed<'_>,
    axis: usize,
    sizes: &[f64],
    gap: f64,
) {
    if !subgridded(item.node, axis) || parent.vertical_writing != item.node.style.vertical_writing {
        return;
    }
    let (start, count) = if axis == 0 {
        (item.row, item.rows)
    } else {
        (item.column, item.columns)
    };
    let end = (start + count).min(sizes.len());
    let spanned = &sizes[start.min(end)..end];
    let own = own_gap(item.node, axis, gap);
    let half = (gap - own) / 2.0;
    let (lead, trail) = edges(item.node, axis);
    let last = spanned.len().saturating_sub(1);
    let mut sizes: Vec<f64> = spanned
        .iter()
        .enumerate()
        .map(|(index, size)| {
            let mut size = *size;
            if index > 0 {
                size += half;
            }
            if index < last {
                size += half;
            }
            if index == 0 {
                size -= lead;
            }
            if index == last {
                size -= trail;
            }
            // A gap wider than the parent's stays centered on the parent's line, so a narrow
            // track may go negative: its neighbours keep the parent's line positions.
            size
        })
        .collect();
    let mut names = span_names(parent, names, axis, start, spanned.len());
    if reversed(item.node) {
        // ponytail: line names of a reversed span are dropped; renumber them from the end
        // when a page needs them.
        sizes.reverse();
        names.clear();
    }
    let inherited = Inherited {
        sizes,
        gap: own,
        names,
    };
    INHERITED.with(|map| {
        let slot = own_axis(item.node, axis);
        map.borrow_mut().entry(item.node.local_id).or_default()[slot] = Some(inherited)
    });
}
/** The parent's line names on the `count` tracks from `start` in `axis`, renumbered from the
 * subgrid's first line (css-grid-2 9.2); none when the parent passes none on. */
fn span_names(
    parent: &ComputedStyle,
    names: bool,
    axis: usize,
    start: usize,
    count: usize,
) -> Vec<(String, u16)> {
    if !names {
        return Vec::new();
    }
    template(parent, axis)
        .names
        .iter()
        .filter(|(_, line)| (start + 1..=start + count + 1).contains(&usize::from(*line)))
        .map(|(name, line)| (name.clone(), *line - start as u16))
        .collect()
}
/** Whether `node`'s writing mode is orthogonal to its parent's, so its rows subgrid the
 * parent's columns. Its items are laid out rotated, so they do not size the parent's tracks:
 * the subgrid contributes as a plain item. */
fn orthogonal(parent: &ComputedStyle, node: &Node) -> bool {
    parent.vertical_writing != node.style.vertical_writing
}
/** Drops what the parent offered its items once its layout is done. */
pub(super) fn withdraw(placed: &[Placed<'_>]) {
    INHERITED.with(|map| {
        let mut map = map.borrow_mut();
        for item in placed {
            map.remove(&item.node.local_id);
        }
    });
}
/** A fixed track of `px`. */
fn fixed(px: f64) -> Track {
    let length = TrackSize::Length(Extent { px, percent: 0.0 });
    Track {
        min: length,
        max: length,
        content: TrackContent::default(),
    }
}
/** The style a subgrid lays out with: each inherited axis becomes an explicit grid of fixed
 * tracks with the parent's names before its own and no auto-repeat, at the inherited gap. None
 * for a grid that inherits nothing. */
pub(super) fn adopt(node: &Node) -> Option<ComputedStyle> {
    let inherited = INHERITED.with(|map| map.borrow().get(&node.local_id).cloned())?;
    let mut style = node.style.clone();
    for (axis, axis_inherited) in inherited.into_iter().enumerate() {
        let Some(axis_inherited) = axis_inherited.filter(|_| subgridded(node, axis)) else {
            continue;
        };
        let tracks = axis_inherited.sizes.iter().copied().map(fixed).collect();
        set_axis(
            &mut style,
            axis,
            tracks,
            axis_inherited.names,
            axis_inherited.gap,
        );
    }
    Some(style)
}
/** Replaces one axis's template with `tracks`, the `inherited` names first, at `gap`. */
fn set_axis(
    style: &mut ComputedStyle,
    axis: usize,
    tracks: Vec<Track>,
    inherited: Vec<(String, u16)>,
    gap: f64,
) {
    let count = tracks.len() as u16 + 1;
    let own = template(style, axis).subgrid_names(count);
    let mut names = inherited;
    // The subgrid's own names past its last line are ignored.
    names.extend(own.into_iter().filter(|(_, line)| *line <= count));
    let template = GridTemplate {
        tracks,
        names,
        ..GridTemplate::default()
    };
    if axis == 0 {
        style.grid_rows = template;
        style.row_gap = gap;
        style.gap_percent[1] = 0.0;
    } else {
        style.grid_columns = template;
        style.column_gap = gap;
        style.gap_percent[0] = 0.0;
    }
}
/** Keeps items inside the explicit tracks of each subgridded axis: a subgrid has no implicit
 * tracks there, so an item past them is clamped into the last ones (css-grid-2 9). */
pub(super) fn clamp(node: &Node, placed: &mut [Placed<'_>], columns: &mut Vec<Track>, rows: usize) {
    if let Some(explicit) = inherited_tracks(node, 1).filter(|_| subgridded(node, 1)) {
        columns.truncate(explicit.max(1));
        clamp_axis(placed, 1, columns.len());
    }
    if inherits(node, 0) && rows > 0 {
        clamp_axis(placed, 0, rows);
    }
}
/** The inherited track count of an axis, when the parent offered one. */
fn inherited_tracks(node: &Node, axis: usize) -> Option<usize> {
    INHERITED.with(|map| {
        map.borrow().get(&node.local_id).and_then(|entry| {
            let slot = own_axis(node, axis);
            entry[slot].as_ref().map(|inherited| inherited.sizes.len())
        })
    })
}
/** Whether a grid parent offered `node` its tracks in `axis`; a subgrid of anything else, a
 * grid-lanes container among them, lays out as `none` there. */
pub(super) fn inherits(node: &Node, axis: usize) -> bool {
    subgridded(node, axis) && inherited_tracks(node, axis).is_some()
}
/**
 * The contributions a subgrid's items make to its parent's tracks in `axis`, in the parent's
 * track indexes: each item sized as it would be in the subgrid, plus the subgrid's edges on the
 * first and last tracks and minus half the gap difference on inner sides (Blink's
 * `SubgriddedItemData` with its extra margins). Nested subgrids in the same axis are looked into
 * as well. `measure` gives one item's contribution in its own grid's coordinates, given the
 * width of its area when the subgrid sizes its own columns; `parent` is the style the parent
 * lays out with, whose line names the subgrid's items place against unless `names` is false
 * (as for `offer`).
 */
pub(super) fn contributions(
    context: &mut LayoutContext<'_>,
    parent: &ComputedStyle,
    names: bool,
    item: &Placed<'_>,
    sizing: Sizing,
    depth: usize,
    measure: &mut Measure<'_>,
) -> EngineResult<Option<Vec<Contribution>>> {
    let Sizing {
        axis,
        gap: parent_gap,
        content,
    } = sizing;
    if depth >= SUBGRID_DEPTH || !subgridded(item.node, axis) || orthogonal(parent, item.node) {
        return Ok(None);
    }
    let node = item.node;
    let (start, count) = if axis == 0 {
        (item.row, item.rows)
    } else {
        (item.column, item.columns)
    };
    let own = own_gap(node, axis, parent_gap);
    let half = (parent_gap - own) / 2.0;
    let (lead, trail) = edges(node, axis);
    let generated = super::super::generated::items(node);
    let kids = super::items_of(context, node, &generated);
    // The other axis keeps what the parent already offered (its columns, while sizing rows).
    let mut style = adopt(node).unwrap_or_else(|| node.style.clone());
    // Grid lanes pass no line names on to an auto-placed subgrid.
    let auto_placed = !names;
    let names = span_names(parent, names, axis, start, count);
    set_axis(&mut style, axis, vec![AUTO_TRACK; count], names, own);
    let mut columns = super::expand(&style.grid_columns, content, style.column_gap);
    let explicit = columns.len();
    let rows = super::expand(&style.grid_rows, 0.0, style.row_gap).len();
    let mut placed = place(context, &kids, &style, &mut columns, rows)?;
    clamp_axis(&mut placed, axis, count);
    // A row subgrid with its own columns sizes them at its width to measure its items in.
    let own_columns = if axis == 0 && !inherits(node, 1) {
        Some(super::column_widths(
            context, &style, &placed, &columns, explicit, content,
        )?)
    } else {
        None
    };
    let area = |kid: &Placed<'_>| {
        own_columns.as_ref().map_or(0.0, |(widths, gap)| {
            super::span(widths, kid.column, kid.columns, *gap)
        })
    };
    // The subgrid's own edges reach the first and last tracks even with no item there, as an
    // empty item would carry them (subgrid-no-items-on-edges).
    let edge = |start, extra| Contribution {
        start,
        span: 1,
        minimum: extra,
        min_content: extra,
        max_content: extra,
    };
    // An auto-placed subgrid of grid lanes may land with any of its tracks on any lane, so
    // each lane it spans takes the larger edge (csswg-drafts#10926).
    let (first_edge, last_edge) = if auto_placed {
        let larger = lead.max(trail);
        (larger, larger)
    } else {
        (lead, trail)
    };
    let mut out = if count > 1 {
        vec![edge(start, first_edge), edge(start + count - 1, last_edge)]
    } else {
        vec![edge(start, lead + trail)]
    };
    for kid in &placed {
        let area = area(kid);
        let nested = Sizing {
            axis,
            gap: own,
            content: area - super::super::flow::horizontal_edges(kid.node),
        };
        let list = match contributions(context, &style, true, kid, nested, depth + 1, measure)? {
            Some(list) => list,
            None => vec![measure(context, kid, area)?],
        };
        for mut contribution in list {
            let first = contribution.start == 0;
            let last = contribution.start + contribution.span >= count;
            let extra = if first { lead } else { -half } + if last { trail } else { -half };
            for value in [
                &mut contribution.minimum,
                &mut contribution.min_content,
                &mut contribution.max_content,
            ] {
                *value = (*value + extra).max(0.0);
            }
            contribution.start += start;
            out.push(contribution);
        }
    }
    Ok(Some(out))
}
/** The axis a subgrid's items size its parent's tracks in: which one (0 rows, 1 columns), the
 * parent's gap there, and the subgrid's content width, which a row subgrid sizes its own
 * columns at to measure its items. */
#[derive(Clone, Copy)]
pub(super) struct Sizing {
    /** 0 rows, 1 columns. */
    pub axis: usize,
    /** The parent's gap in that axis. */
    pub gap: f64,
    /** The subgrid's content-box width; unused for columns. */
    pub content: f64,
}
/** One subgrid item's contribution in its own grid's coordinates, given its area's width when
 * the subgrid sized its own columns (else 0). */
pub(super) type Measure<'a> =
    dyn FnMut(&mut LayoutContext<'_>, &Placed<'_>, f64) -> EngineResult<Contribution> + 'a;
/** Clamps placed items into `count` tracks of `axis` as an overly large grid is clamped
 * (css-grid-2 8.4, which section 9 applies to subgrids): an area reaching past the last line
 * ends there, and one wholly past it becomes the last track. */
fn clamp_axis(placed: &mut [Placed<'_>], axis: usize, count: usize) {
    let count = count.max(1);
    for item in placed.iter_mut() {
        let (start, span) = if axis == 0 {
            (&mut item.row, &mut item.rows)
        } else {
            (&mut item.column, &mut item.columns)
        };
        if *start >= count {
            (*start, *span) = (count - 1, 1);
        } else {
            *span = (*span).clamp(1, count - *start);
        }
    }
}
