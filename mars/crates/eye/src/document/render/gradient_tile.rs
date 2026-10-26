/**
 * Where a gradient background layer paints, after CSS Backgrounds: a gradient has no size of its
 * own, so `background-size` (each `auto` axis taking the positioning area's, the
 * `background-origin` box), then `background-position` against the room left over, place one
 * tile, which `background-repeat` tiles (`round` rescaling it to fit whole, `space` spreading
 * whole tiles apart) or not. Tests across css-text paint a red `linear-gradient(red, red) 0 0/2ch
 * 2ch no-repeat` square under text that must cover it; filling the whole box showed red past it.
 */
use super::display::GradientTile;
use crate::document::style::RepeatStyle::{self, NoRepeat, Repeat, Round, Space};
use crate::document::style::{BackgroundBox, BackgroundRepeat, BackgroundSize, Extent};

/// A percentage's whole.
const PERCENT: f64 = 100.0;

/** The tile a gradient layer of an item `width` by `height` (its border box) paints in; None
 * when it is the whole border box repeated, which painters draw as one gradient over the box. */
pub(super) fn tile(
    style: &crate::document::ComputedStyle,
    layer: usize,
    (width, height): (f64, f64),
) -> Option<GradientTile> {
    let (size_rule, position, repeat_rule) =
        crate::document::style::background_layers::geometry(style, layer);
    let [top, right, bottom, left] = origin(style);
    let area = [
        (width - left - right).max(0.0),
        (height - top - bottom).max(0.0),
    ];
    let repeat = axes(repeat_rule);
    let size = fitted(size(size_rule, area), area, repeat);
    let [x, y] = position;
    let gap = [
        spacing(area[0], size[0], repeat[0]),
        spacing(area[1], size[1], repeat[1]),
    ];
    let tile = GradientTile {
        x: left + placed(x, area[0], size[0], gap[0]),
        y: top + placed(y, area[1], size[1], gap[1]),
        width: size[0],
        height: size[1],
        repeat: std::array::from_fn(|i| {
            repeat[i] != NoRepeat && (repeat[i] != Space || gap[i].is_some())
        }),
        gap: gap.map(|g| g.unwrap_or(0.0)),
    };
    let whole = tile.x == 0.0 && tile.y == 0.0 && size == [width, height];
    (!whole || tile.repeat != [true, true]).then_some(tile)
}

/** The positioning area as insets from the border box: `background-origin`'s box. */
pub(super) fn origin(style: &crate::document::ComputedStyle) -> [f64; 4] {
    let sides = super::sizing::border_sides(style);
    match style.background_origin {
        BackgroundBox::BorderBox | BackgroundBox::BorderArea => [0.0; 4],
        BackgroundBox::PaddingBox => sides,
        BackgroundBox::ContentBox => std::array::from_fn(|i| sides[i] + style.padding[i]),
    }
}

/** The tile's size: `auto` axes, `cover` and `contain` take the positioning area's, since a
 * gradient has no intrinsic size or ratio to keep. */
fn size(size: BackgroundSize, [w, h]: [f64; 2]) -> [f64; 2] {
    match size {
        BackgroundSize::Explicit(x, y) => [
            x.and_then(|x| x.resolve(Some(w))).unwrap_or(w).max(0.0),
            y.and_then(|y| y.resolve(Some(h))).unwrap_or(h).max(0.0),
        ],
        _ => [w, h],
    }
}

/** Under `round`, a tile rescaled so a whole number of tiles spans the area. */
fn fitted(size: [f64; 2], area: [f64; 2], repeat: [RepeatStyle; 2]) -> [f64; 2] {
    std::array::from_fn(|i| match repeat[i] {
        Round if size[i] > 0.0 => area[i] / (area[i] / size[i]).round().max(1.0),
        _ => size[i],
    })
}

/** Under `space`, the gap between whole tiles when at least two fit; None otherwise, where a
 * single tile is placed by `background-position` and not repeated. */
fn spacing(area: f64, size: f64, repeat: RepeatStyle) -> Option<f64> {
    if repeat != Space || size <= 0.0 {
        return None;
    }
    let count = (area / size).floor();
    (count >= 2.0).then(|| (area - count * size) / (count - 1.0))
}

/** A tile's offset in its area: `background-position` against the room left over, or, for
 * spaced tiles, the area's start. */
fn placed(extent: Extent, area: f64, size: f64, gap: Option<f64>) -> f64 {
    match gap {
        Some(_) => 0.0,
        None => extent.px + (area - size) * extent.percent / PERCENT,
    }
}

/** Each axis's repeat style. */
fn axes(repeat: BackgroundRepeat) -> [RepeatStyle; 2] {
    match repeat {
        BackgroundRepeat::Repeat => [Repeat, Repeat],
        BackgroundRepeat::NoRepeat => [NoRepeat, NoRepeat],
        BackgroundRepeat::RepeatX => [Repeat, NoRepeat],
        BackgroundRepeat::RepeatY => [NoRepeat, Repeat],
        BackgroundRepeat::Axes(x, y) => [x, y],
    }
}
