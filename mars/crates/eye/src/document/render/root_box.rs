/** The root element's border box. Eye keeps `<html>` as formatting data rather than a node, so
its authored `width`, `height` and `position` offsets are applied here: a sized root narrows the
body inside it, and an absolutely, fixed or relatively positioned root moves by its `left` and
`top` against the initial containing block (the viewport), as in CSS 2.1 10.1. A positioned body
(also formatting data) is placed the same way, in the root's padding box when the root is
positioned. */
use crate::document::{ComputedStyle, Document, Position};

/** Index of the top side in CSS order (top, right, bottom, left). */
const TOP: usize = 0;
/** Index of the bottom side in CSS order. */
const BOTTOM: usize = 2;
/** Index of the left side in CSS order. */
const LEFT: usize = 3;

/** The root's border box in document coordinates and its content box's left and top edges. */
pub(super) struct RootBox {
    /// Border box left edge.
    pub x: f64,
    /// Border box top edge.
    pub y: f64,
    /// Border box width.
    pub width: f64,
    /// Border box height when the root has a definite `height`; auto roots end with content.
    pub height: Option<f64>,
    /// Content box left and top edges, where the body's margin box starts.
    pub content: [f64; 2],
    /// Content box width, the body's available width.
    pub content_width: f64,
}

/** How far a positioned root moves from where it would sit: its `left` and `top` resolved
against the viewport, zero for a static root or an `auto` inset. */
fn offset(style: &ComputedStyle, viewport: (f64, f64)) -> [f64; 2] {
    if style.position == Position::Static {
        return [0.0; 2];
    }
    let left = super::positioned::inset(style, LEFT, viewport.0).unwrap_or(0.0);
    let top = super::positioned::inset(style, TOP, viewport.1).unwrap_or(0.0);
    [left, top]
}

/** The root element's box in a `viewport` of (width, height). */
pub(super) fn root_box(document: &Document, viewport: (f64, f64)) -> RootBox {
    let style = &document.formatting.html.style;
    let (x, y, content_width) = super::body::content(style, viewport.0);
    let [dx, dy] = offset(style, viewport);
    let sides = super::sizing::border_sides(style);
    let (left, top) = (style.padding[3] + sides[3], style.padding[0] + sides[0]);
    let horizontal = left + style.padding[1] + sides[1];
    let height = style
        .height
        .and_then(|height| height.resolve_size(Some(viewport.1)))
        .map(|height| {
            super::sizing::content_height(style, height) + top + style.padding[2] + sides[2]
        });
    RootBox {
        x: x - left + dx,
        y: y - top + dy,
        width: content_width + horizontal,
        height,
        content: [x + dx, y + dy],
        content_width,
    }
}

/** The rectangle (x, y, width, height) an absolutely positioned body resolves its insets in: a
positioned root's padding box (CSS 2.1 10.1: the nearest positioned ancestor), else the initial
containing block, the viewport. */
fn body_containing_block(document: &Document, viewport: (f64, f64)) -> [f64; 4] {
    let root = &document.formatting.html.style;
    if root.position == Position::Static {
        return [0.0, 0.0, viewport.0, viewport.1];
    }
    let placed = root_box(document, viewport);
    let sides = super::sizing::border_sides(root);
    let height = placed.height.unwrap_or(viewport.1) - sides[0] - sides[2];
    [
        placed.x + sides[3],
        placed.y + sides[0],
        placed.width - sides[1] - sides[3],
        height,
    ]
}

/** Where the body's content box starts, given where normal flow put it (`flowed`): a relatively
positioned body shifts by its insets, and an absolutely or fixed positioned one is placed by them
in its containing block (`top`, else `bottom` with a definite height), keeping the flowed
position on an axis whose insets are `auto`. */
pub(super) fn body_origin(document: &Document, viewport: (f64, f64), flowed: [f64; 2]) -> [f64; 2] {
    let body = &document.formatting.body.style;
    match body.position {
        Position::Static | Position::Sticky => flowed,
        Position::Relative => {
            let [dx, dy] = offset(body, viewport);
            [flowed[0] + dx, flowed[1] + dy]
        }
        Position::Absolute | Position::Fixed => {
            let block = body_containing_block(document, viewport);
            [
                placed_x(body, block, flowed[0]),
                placed_y(body, block, flowed[1]),
            ]
        }
    }
}

/** An out-of-flow body's content-box left edge from its `left` inset. */
fn placed_x(body: &ComputedStyle, block: [f64; 4], flowed: f64) -> f64 {
    let sides = super::sizing::border_sides(body);
    let lead = body.margin[LEFT] + sides[LEFT] + body.padding[LEFT];
    super::positioned::inset(body, LEFT, block[2]).map_or(flowed, |left| block[0] + left + lead)
}

/** An out-of-flow body's content-box top edge from its `top` inset, or from `bottom` and a
definite height. */
fn placed_y(body: &ComputedStyle, block: [f64; 4], flowed: f64) -> f64 {
    let sides = super::sizing::border_sides(body);
    if let Some(top) = super::positioned::inset(body, TOP, block[3]) {
        return block[1] + top + body.margin[TOP] + sides[TOP] + body.padding[TOP];
    }
    let height = body
        .height
        .and_then(|height| height.resolve_size(Some(block[3])))
        .map(|height| super::sizing::content_height(body, height));
    match (super::positioned::inset(body, BOTTOM, block[3]), height) {
        (Some(bottom), Some(height)) => {
            let trail = body.margin[BOTTOM] + sides[BOTTOM] + body.padding[BOTTOM];
            block[1] + block[3] - bottom - trail - height
        }
        _ => flowed,
    }
}
