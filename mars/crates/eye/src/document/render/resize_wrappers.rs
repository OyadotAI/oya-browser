/** Wrapper ResizeObserver boxes come from resolved layout, never viewport or document-scroll guesses. */
use crate::document::{ComputedStyle, Document, Position};
use mars_core::geometry::{BoxMetrics, WrapperMetrics};

/** Capture physical sizes before paint transforms, with incomplete auto layouts left unmeasured. */
pub(super) fn measure(
    document: &Document,
    viewport: (f64, f64),
    body: &ComputedStyle,
    [width, top, end]: [f64; 3],
    in_flow_end: f64,
    truncated: bool,
) -> WrapperMetrics {
    let root = &document.formatting.html.style;
    let placed = super::root_box::root_box(document, viewport);
    let body_height = body
        .height
        .and_then(|height| height.resolve_size(Some(viewport.1)))
        .map(|height| super::sizing::content_height(body, height));
    let bottom = body.padding[2] + super::sizing::border_sides(body)[2];
    let body_size = [width, body_height.unwrap_or((end - top - bottom).max(0.0))];
    let root_height = placed
        .height
        .map(|height| (height - vertical_edges(root)).max(0.0));
    // A positioned or absent body does not contribute to the root's in-flow automatic height.
    // Root generated siblings of such a body currently use a shared positioning path; do not
    // manufacture an observation until that path can report their independent flow extent.
    let body_outside = body.hidden || matches!(body.position, Position::Absolute | Position::Fixed);
    let special_flow = body_outside
        && document
            .nodes
            .iter()
            .any(super::root_generated::is_root_box);
    let auto_height = if body_outside {
        0.0
    } else {
        (in_flow_end - placed.content[1]).max(0.0)
    };
    WrapperMetrics {
        html: measured(
            root,
            [placed.content_width, root_height.unwrap_or(auto_height)],
            root.hidden,
            root_height.is_some() || !truncated && !special_flow,
        ),
        body: measured(
            body,
            body_size,
            root.hidden || body.hidden || body.display_contents,
            body_height.is_some() || !truncated,
        ),
    }
}

/** Content and border dimensions remain distinct, including a known absence's initial zero size. */
fn measured(
    style: &ComputedStyle,
    size: [f64; 2],
    absent: bool,
    complete: bool,
) -> Option<BoxMetrics> {
    let edges = super::sizing::border_sides(style);
    let metrics = if absent {
        BoxMetrics {
            content: [0.0; 4],
            border: [0.0; 2],
            vertical: style.vertical_writing,
        }
    } else if complete {
        BoxMetrics {
            content: [style.padding[3], style.padding[0], size[0], size[1]],
            border: [
                size[0] + style.padding[1] + style.padding[3] + edges[1] + edges[3],
                size[1] + vertical_edges(style),
            ],
            vertical: style.vertical_writing,
        }
    } else {
        return None;
    };
    metrics.valid().then_some(metrics)
}

/** The same resolved padding and border edges used by wrapper layout enclose its content height. */
fn vertical_edges(style: &ComputedStyle) -> f64 {
    let edges = super::sizing::border_sides(style);
    style.padding[0] + style.padding[2] + edges[0] + edges[2]
}
