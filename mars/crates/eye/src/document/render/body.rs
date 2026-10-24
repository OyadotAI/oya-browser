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
    let left = x - body.padding[3] - body.border;
    let top = y - body.padding[0] - body.border;
    Some(super::DisplayDecoration {
        x: left,
        y: top,
        width: width + body.padding[1] + body.padding[3] + body.border * 2.0,
        height: (bottom - top).max(0.0),
        color,
        border: body.border,
        border_color: body.border_color.clone(),
    })
}
/** Content-box bounds subtract edges before max width, then resolve automatic horizontal margins. */
pub(super) fn content(style: &crate::document::ComputedStyle, viewport: f64) -> (f64, f64, f64) {
    let edges = style.padding[1] + style.padding[3] + style.border * 2.0;
    let adjustment = if style.box_sizing == crate::document::BoxSizing::ContentBox {
        edges
    } else {
        0.0
    };
    let available = (viewport - style.margin[1] - style.margin[3]).max(0.0);
    let specified = style.width.map(|width| {
        if style.percent_width {
            viewport * width / 100.0
        } else {
            width
        }
    });
    let minimum = style
        .min_width
        .map(|width| width + adjustment)
        .unwrap_or(edges)
        .max(edges);
    let maximum = style
        .max_width
        .map(|width| {
            if style.percent_max_width {
                viewport * width / 100.0
            } else {
                width + adjustment
            }
        })
        .unwrap_or(crate::constants::LAYOUT_EXTENT)
        .max(minimum);
    let width = specified
        .map(|width| width + adjustment)
        .unwrap_or(available)
        .min(maximum)
        .max(minimum);
    let x = margin_start(style, viewport, width) + style.padding[3] + style.border;
    (
        x,
        style.margin[0] + style.padding[0] + style.border,
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
