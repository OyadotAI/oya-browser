/** Wrapper geometry and decoration require no synthetic DOM or targeting identities. */
use crate::document::Document;
/** Non-target body paint remains distinct from the background propagated onto the canvas. */
pub(super) fn decoration(
    document: &Document,
    x: f64,
    y: f64,
    width: f64,
    bottom: f64,
) -> Option<super::DisplayDecoration> {
    let body = &document.formatting.body.style;
    if body.hidden {
        return None;
    }
    let root_background = document
        .formatting
        .html
        .style
        .background
        .as_ref()
        .is_some_and(|color| color != "transparent");
    let color = root_background
        .then(|| body.background.clone())
        .flatten()
        .filter(|color| color != "transparent");
    if color.is_none() && body.border == 0.0 {
        return None;
    }
    let sides = super::sizing::border_sides(body);
    let left = x - body.padding[3] - sides[3];
    let top = y - body.padding[0] - sides[0];
    Some(super::DisplayDecoration {
        x: left,
        y: top,
        width: width + body.padding[1] + body.padding[3] + sides[1] + sides[3],
        height: (bottom - top).max(0.0),
        color,
        border: body.border,
        border_color: body.border_color.clone(),
    })
}
/** How far the root's content box sits inside the viewport per side (top, right, bottom, left):
its margin, border and padding, narrowed by an authored width and moved by a positioned root's
offsets (`root_box`). The body sits inside them. Root margins never collapse with the body's
(CSS 2.1 8.3.1), so they simply add up. */
pub(super) fn root_edges(document: &Document, viewport: (f64, f64)) -> [f64; 4] {
    let root = &document.formatting.html.style;
    let sides = super::sizing::border_sides(root);
    let placed = super::root_box::root_box(document, viewport);
    let [left, top] = placed.content;
    let right = viewport.0 - left - placed.content_width;
    [
        top,
        right,
        root.margin[2] + sides[2] + root.padding[2],
        left,
    ]
}
/** The root element's border around its border box; its background is the canvas's already. */
pub(super) fn root_decoration(
    document: &Document,
    viewport: (f64, f64),
    height: f64,
) -> Option<super::DisplayDecoration> {
    let root = &document.formatting.html.style;
    if root.hidden || root.border <= 0.0 {
        return None;
    }
    let placed = super::root_box::root_box(document, viewport);
    let auto = height - placed.y - root.margin[2];
    Some(super::DisplayDecoration {
        x: placed.x,
        y: placed.y,
        width: placed.width.max(0.0),
        height: placed.height.unwrap_or(auto).max(0.0),
        color: None,
        border: root.border,
        border_color: root.border_color.clone(),
    })
}
/** The body's definite content height when body clips its own overflow, so content past it adds
 * nothing to the document's scrollable extent. Body keeps its overflow only while the root's is
 * not visible (otherwise it propagates to the viewport); a consent dialog's
 * `body{height:100vh;overflow:hidden}` leaves Chromium's document one viewport tall. */
pub(super) fn clipped_height(document: &Document, definite: Option<f64>) -> Option<f64> {
    use crate::document::Overflow::Visible;
    let (root, body) = (
        &document.formatting.html.style,
        &document.formatting.body.style,
    );
    let propagates = root.overflow_x == Visible && root.overflow_y == Visible;
    definite.filter(|_| !propagates && body.overflow_y != Visible)
}
/** Content-box bounds subtract edges before max width, then resolve automatic horizontal margins. */
pub(super) fn content(style: &crate::document::ComputedStyle, viewport: f64) -> (f64, f64, f64) {
    // Per-side borders (GNU's `border: .1em solid; border-top: 0`) inset the content too.
    let sides = super::sizing::border_sides(style);
    let edges = style.padding[1] + style.padding[3] + sides[1] + sides[3];
    let adjustment = if style.box_sizing == crate::document::BoxSizing::ContentBox {
        edges
    } else {
        0.0
    };
    let available = (viewport - style.margin[1] - style.margin[3]).max(0.0);
    let specified = style
        .width_expr
        .as_ref()
        .map(|expr| expr.resolve(viewport).max(0.0))
        .or_else(|| {
            style.width.map(|width| {
                if style.percent_width {
                    viewport * width / 100.0 + style.width_offset
                } else {
                    width
                }
            })
        });
    let minimum = style
        .min_width
        .map(|width| width + adjustment)
        .unwrap_or(edges)
        .max(edges);
    let maximum = style
        .max_width_expr
        .as_ref()
        .map(|expr| expr.resolve(viewport).max(0.0) + adjustment)
        .or_else(|| {
            style.max_width.map(|width| {
                if style.percent_max_width {
                    viewport * width / 100.0 + style.max_width_offset
                } else {
                    width + adjustment
                }
            })
        })
        .unwrap_or(crate::constants::LAYOUT_EXTENT)
        .max(minimum);
    let width = specified
        .map(|width| width + adjustment)
        .unwrap_or(available)
        .min(maximum)
        .max(minimum);
    let x = margin_start(style, viewport, width) + style.padding[3] + sides[3];
    (
        x,
        style.margin[0] + style.padding[0] + sides[0],
        (width - edges).max(0.0),
    )
}
/** An overflowing auto-centered box keeps its inline-start margin nonnegative. */
pub(super) fn margin_start(
    style: &crate::document::ComputedStyle,
    available: f64,
    width: f64,
) -> f64 {
    let free = (available - width - style.margin[1] - style.margin[3]).max(0.0);
    style.margin[3]
        + if style.margin_auto[3] {
            if style.margin_auto[1] {
                free / 2.0
            } else {
                free
            }
        } else {
            0.0
        }
}

/**
 * A copy of the document whose top-level nodes sit in one layout-only body node, when the body
 * is a flex or grid container. Eye keeps the body as formatting data rather than a node, so
 * without this a `display: flex` body (rustdoc's sidebar and content, sticky-footer pages)
 * stacked its children as blocks. The node carries the body's container properties but none of
 * its box or paint, which `content` and `decoration` already apply; returns its local id so the
 * caller can drop the items it paints.
 */
pub(super) fn container_root(document: &Document) -> Option<(Document, usize)> {
    let body = &document.formatting.body.style;
    if !(body.display_flex || body.display_grid || body.vertical_writing) || body.hidden {
        return None;
    }
    let id = document.nodes.iter().map(|node| node.local_id).max()? + 1;
    let mut root = crate::document::markup::node("body", id, None, Default::default()).ok()?;
    root.style = layout_only(body);
    let mut copy = document.clone();
    for node in copy.nodes.iter_mut().filter(|node| node.parent.is_none()) {
        node.parent = Some(id);
    }
    copy.nodes.insert(0, root);
    Some((copy, id))
}
/** The body's container properties without the margin, border, padding, width or paint the
 * wrapper geometry already applied, so nothing is counted twice. */
fn layout_only(body: &crate::document::ComputedStyle) -> crate::document::ComputedStyle {
    let initial = crate::document::ComputedStyle::default();
    let mut style = body.clone();
    // A vertical body keeps its margins on this box, so in its rotated layout they collapse
    // with its first and last children's as Blink's block-direction margins do; the wrapper
    // then lays the body out without them (flow.rs `layout_once`).
    if !body.vertical_writing {
        style.margin = initial.margin;
        style.margin_auto = initial.margin_auto;
    }
    style.padding = initial.padding;
    style.border = initial.border;
    style.border_widths = initial.border_widths;
    style.width = None;
    style.max_width = None;
    style.background = None;
    style.backgrounds = initial.backgrounds;
    style.shadows = initial.shadows;
    style.generated = initial.generated;
    style.opacity = initial.opacity;
    style.position = initial.position;
    style.transform = initial.transform;
    style.overflow = initial.overflow;
    style
}
