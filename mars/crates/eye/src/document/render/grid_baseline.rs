/** Baseline self-alignment of grid items in the row axis (css-align-3 9.3, Blink's
 * `GridLayoutAlgorithm::ComputeGridItemBaselines`): the `align-self: baseline` items starting in
 * one row share a baseline, and each is pushed down by its shim, the gap between its own
 * ascent and the row's largest, which also counts toward the row's size. */
use super::super::flow::*;
use super::placement::Placed;
use super::tracks;
use crate::document::{AlignItems, ComputedStyle};

/** Whether an item takes part in its row's baseline sharing group: `align-self` (or the
 * grid's `align-items`) is `baseline`, neither block margin is `auto`, and it is no row
 * subgrid, which fills its rows. A vertical item in a horizontal grid joins with the baseline
 * its content box would give its font (`orthogonal_ascent`); a horizontal item that a vertical
 * grid's rotated layout turned back (`counter_rotate`) has none in this axis.
 * ponytail: the turned-back case keeps start alignment; Chromium groups it by the opposite
 * edge (grid-self-baseline-vertical-*). */
pub(super) fn aligned(container: &ComputedStyle, node: &Node) -> bool {
    node.style.align_self.unwrap_or(container.align_items) == AlignItems::Baseline
        && (node.style.vertical_writing == container.vertical_writing
            || node.style.counter_rotate == 0)
        && !super::subgrid::inherits(node, 0)
        && !node.style.margin_auto[0]
        && !node.style.margin_auto[2]
}

/** Whether a height, min-height or max-height is a percentage, so the item's height follows
 * the size of the rows it spans. */
fn percent_height(node: &Node) -> bool {
    let style = &node.style;
    [style.height, style.min_height, style.max_height]
        .iter()
        .any(|extent| extent.is_some_and(|extent| extent.percent != 0.0))
}

/** Whether every row an item spans has a fixed size, so its height cannot depend on them. */
fn fixed_rows(item: &Placed<'_>, rows: &[tracks::TrackSizing]) -> bool {
    let end = (item.row + item.rows).min(rows.len());
    rows[item.row.min(end)..end].iter().all(|row| {
        matches!(row.min, tracks::MinSizing::Fixed(_))
            && matches!(row.max, tracks::MaxSizing::Fixed(_))
    })
}

/** An item's ascent from its top margin edge: its first line's baseline, or for an item with no
 * line, or one orthogonal to the row axis, the synthesized border-box bottom. None when that
 * synthesized baseline would follow a percentage height through intrinsic rows: the item then
 * falls back to start alignment (css-grid-2 10.6, Blink's `SetAlignmentFallback`). */
fn ascent(
    context: &mut LayoutContext<'_>,
    item: &Placed<'_>,
    width: f64,
    rows: &[tracks::TrackSizing],
) -> EngineResult<Option<f64>> {
    let node = item.node;
    let top = node.style.margin[0];
    let cyclic = percent_height(node) && !fixed_rows(item, rows);
    if node.style.vertical_writing {
        return Ok((!cyclic).then(|| top + orthogonal_ascent(&node.style)));
    }
    let baseline = { super::super::flex_layout::first_baseline(context, node, width)? };
    if let Some(baseline) = baseline {
        return Ok(Some(top + baseline));
    }
    if cyclic {
        return Ok(None);
    }
    let height = super::super::flex_layout::item_height(context, node, width)?;
    Ok(Some(top + height))
}

/** A vertical item's baseline in a horizontal row from its border-box top: its top border and
 * padding plus its font's ascent, as Chromium aligns orthogonal grid items
 * (grid-self-baseline-horiz-*). */
fn orthogonal_ascent(style: &ComputedStyle) -> f64 {
    super::super::sizing::border_sides(style)[0]
        + style.padding[0]
        + super::super::sizing::text_ascent(style)
}

/** Every placed item's baseline shim, zero for items that do not align by baseline; `natural`
 * holds each item's (border-box width, outer height) and `rows` the row sizing functions. */
pub(super) fn shims(
    context: &mut LayoutContext<'_>,
    container: &ComputedStyle,
    placed: &[Placed<'_>],
    natural: &[(f64, f64)],
    rows: &[tracks::TrackSizing],
) -> EngineResult<Vec<f64>> {
    let mut ascents = vec![None; placed.len()];
    let mut largest = std::collections::BTreeMap::<usize, f64>::new();
    for (index, item) in placed.iter().enumerate() {
        if !aligned(container, item.node) {
            continue;
        }
        let Some(value) = ascent(context, item, natural[index].0, rows)? else {
            continue;
        };
        let row = largest.entry(item.row).or_insert(value);
        *row = row.max(value);
        ascents[index] = Some(value);
    }
    Ok(placed
        .iter()
        .zip(ascents)
        .map(|(item, ascent)| ascent.map_or(0.0, |a| largest[&item.row] - a))
        .collect())
}
