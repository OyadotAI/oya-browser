/** Baseline self-alignment of grid items in the column axis (`justify-self: baseline`,
 * css-align-3 9.3, Blink's grid baseline accumulator). Items starting in one column share a
 * baseline with the others of their writing mode: a vertical item's first line has a real one
 * (its font's ascent from the line's left edge in `vertical-lr`, its descent in `vertical-rl`),
 * a horizontal item synthesizes one at its border-box end edge (css-align-3 9.1). Each moves
 * right by its shim. */
use super::super::flow::*;
use super::placement::Placed;
use crate::document::{AlignItems, ComputedStyle};

/** Whether an item joins its column's baseline sharing group: its `justify-self` (or the
 * grid's `justify-items`) is `baseline` in a horizontal left-to-right grid, it is
 * horizontal too and neither inline margin is `auto`.
 * ponytail: vertical items keep start alignment. `ascent` already gives their first line's
 * baseline; enabling them needs the shims in the column sizes too (grid-self-baseline-horiz-006),
 * which moved the grid-lanes baseline references. */
pub(super) fn aligned(container: &ComputedStyle, node: &Node) -> bool {
    super::justify_of(container, node) == AlignItems::Baseline
        && !container.vertical_writing
        && !container.rtl
        && !node.style.vertical_writing
        && !node.style.margin_auto[1]
        && !node.style.margin_auto[3]
}

/** The baseline sharing group of an item's writing mode: horizontal, `vertical-lr` or
 * `vertical-rl` items line up only among themselves. */
fn mode(style: &ComputedStyle) -> u8 {
    match (style.vertical_writing, style.writing_lr) {
        (false, _) => 0,
        (true, true) => 1,
        (true, false) => 2,
    }
}

/** An item's baseline from its left margin edge at border-box width `width`. */
fn ascent(style: &ComputedStyle, width: f64) -> f64 {
    let start = style.margin[3];
    if !style.vertical_writing {
        return start + width;
    }
    let edge = super::super::sizing::border_sides(style)[3] + style.padding[3];
    let ascent = super::super::sizing::text_ascent(style);
    let over = if style.writing_lr {
        ascent
    } else {
        super::super::sizing::advance(style) - ascent
    };
    start + edge + over
}

/** Every placed item's column baseline shim, zero for items that do not align by baseline;
 * `natural` holds each item's (border-box width, outer height). */
pub(super) fn shims(
    container: &ComputedStyle,
    placed: &[Placed<'_>],
    natural: &[(f64, f64)],
) -> Vec<f64> {
    let mut ascents = vec![None; placed.len()];
    let mut largest = std::collections::BTreeMap::<(usize, u8), f64>::new();
    for (index, item) in placed.iter().enumerate() {
        if !aligned(container, item.node) {
            continue;
        }
        let style = &item.node.style;
        let value = ascent(style, natural[index].0);
        let group = largest.entry((item.column, mode(style))).or_insert(value);
        *group = group.max(value);
        ascents[index] = Some(value);
    }
    placed
        .iter()
        .zip(ascents)
        .map(|(item, ascent)| {
            let group = (item.column, mode(&item.node.style));
            ascent.map_or(0.0, |a| largest[&group] - a)
        })
        .collect()
}
