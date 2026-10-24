/** Replaced image geometry uses owned intrinsic dimensions and the common inline cursor. */
use super::flow::*;
/** Allocated flex image boxes never resolve authored widths for a second time. */
pub(super) fn image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
    forced: Option<BorderBox>,
) -> EngineResult<()> {
    if let Some(bounds) = forced {
        let mut inner = Flow::new(
            bounds.x + node.style.padding[3] + node.style.border,
            bounds.y + node.style.padding[0] + node.style.border,
            (bounds.width - horizontal_edges(node)).max(0.0),
        );
        let top = inner.y;
        let lines = if node.image_size.is_none() && !sized(node, flow.definite) {
            let lines = super::text::text(context, &node.text, node, &mut inner, usize::MAX)?;
            fitting_alt(lines, &node.text, inner.width)
        } else {
            Vec::new()
        };
        if lines.is_empty() {
            inner.bottom = top;
        }
        let natural_height = if node.image_size.is_some() || sized(node, flow.definite) {
            let mut fixed = node.clone();
            fixed.style.width = Some((bounds.width - horizontal_edges(node)).max(0.0));
            fixed.style.percent_width = false;
            fixed.style.box_sizing = crate::document::BoxSizing::ContentBox;
            image_size(&fixed, bounds.width, flow.definite).1
        } else {
            inner.bottom - bounds.y + node.style.padding[2] + node.style.border
        };
        let height = bounds.height.unwrap_or(natural_height);
        let item = box_item(node, bounds.x, bounds.y, bounds.width, height, lines);
        items.push(drawn(context, node, item)?);
        flow.y = bounds.y + height;
        flow.bottom = flow.bottom.max(flow.y);
        return Ok(());
    }
    if node.style.inline {
        inline_image(context, node, flow, items)
    } else {
        block_image(context, node, flow, items)
    }
}
/** Intrinsic image sizes preserve aspect ratio and clamp to the containing flow. */
fn block_image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    flow.newline();
    let sized = sized(node, flow.definite);
    if node.image_size.is_none() && !sized {
        let lines = super::text::text(context, &node.text, node, flow, items.len())?;
        let width = lines
            .iter()
            .map(|line| line.x + line.width)
            .fold(flow.x, f64::max)
            - flow.x;
        let height = flow.line.max(super::sizing::advance(&node.style));
        items.push(box_item(node, flow.x, flow.y, width, height, lines));
        flow.after_block(flow.y + height, node.style.margin[2]);
        flow.newline();
        return Ok(());
    }
    let (natural_width, natural_height) = image_size(node, flow.width, flow.definite);
    let y = flow.block_top(node.style.margin[0]);
    let x = flow.x + super::body::margin_start(&node.style, flow.width, natural_width);
    let item = box_item(node, x, y, natural_width, natural_height, Vec::new());
    items.push(drawn(context, node, item)?);
    flow.after_block(y + natural_height, node.style.margin[2]);
    Ok(())
}
/** Chromium clips alt text to an image box it was given, so alt that only fits by breaking words
(a narrow or zero-width positioned image still loading) shows nothing, not a column of letters. */
fn fitting_alt(lines: Vec<super::PaintLine>, alt: &str, width: f64) -> Vec<super::PaintLine> {
    let words = alt.split_whitespace().count();
    let broken = lines.len() > words || lines.iter().any(|line| line.width > width + 0.5);
    if broken { Vec::new() } else { lines }
}
/** Authored CSS or HTML dimensions give an image a box even before its bytes arrive. */
fn sized(node: &Node, definite: Option<f64>) -> bool {
    let width = node.style.width.is_some();
    let height = node
        .style
        .height
        .and_then(|height| height.resolve_size(definite))
        .is_some();
    width && height
}
/** Border-box size from CSS sizes, then HTML dimensions, then intrinsic size, keeping the aspect ratio. */
pub(super) fn image_size(node: &Node, available: f64, definite: Option<f64>) -> (f64, f64) {
    let intrinsic = node
        .image_size
        .map(|(width, height)| (f64::from(width), f64::from(height)));
    let available = available.max(0.0);
    let css_height = node
        .style
        .height
        .and_then(|height| height.resolve_size(definite))
        .map(|height| super::sizing::content_size(node, height, false));
    // HTML width and height arrive as presentational hints, so an author's `auto` overrides them.
    let height = css_height;
    let width = super::sizing::specified_content(node, available);
    let ratio = match (
        intrinsic,
        dimension(node, "width"),
        dimension(node, "height"),
    ) {
        (Some((w, h)), _, _) => h / w.max(1.0),
        (None, Some(w), Some(h)) => h / w.max(1.0),
        _ => 1.0,
    };
    let content_width = match (width, height) {
        (Some(width), _) => width,
        (None, Some(height)) => height / ratio.max(f64::EPSILON),
        (None, None) => intrinsic.map_or(0.0, |(w, _)| w),
    };
    let border = border_width(node, available, available, Some(content_width));
    let clamped = (border - horizontal_edges(node)).max(0.0);
    let content_height = match height {
        Some(height) if clamped >= content_width => height,
        _ => clamped * ratio,
    };
    (
        border,
        content_height.min(LAYOUT_LIMIT) + vertical_edges(node),
    )
}
/** Image heights stay inside layout geometry bounds even for extreme aspect ratios. */
const LAYOUT_LIMIT: f64 = crate::constants::LAYOUT_EXTENT;
/** Inline images occupy replaced-content geometry in the same line as adjacent text. */
fn inline_image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let (width, height) = image_size(node, flow.width, flow.definite);
    if node.image_size.is_none() && !sized(node, flow.definite) {
        return inline_alt(context, node, flow, items);
    }
    let style = &node.style;
    let outer = width + style.margin[1] + style.margin[3];
    if flow.cursor > 0.0 && flow.cursor + outer > flow.room() {
        flow.newline();
    }
    flow.fit(outer);
    let start = items.len();
    let item = box_item(
        node,
        flow.left() + flow.cursor + style.margin[3],
        flow.y + style.margin[0],
        width,
        height,
        Vec::new(),
    );
    items.push(drawn(context, node, item)?);
    // A replaced box's baseline is its bottom margin edge.
    let margin_box = height + style.margin[0] + style.margin[2];
    super::text::place_atomic(
        flow,
        (start, start + 1),
        outer,
        (margin_box, margin_box),
        style,
        (style.margin[3], style.margin[1]),
    );
    Ok(())
}
/** Failed inline images remain readable text without dropping adjacent content. */
fn inline_alt(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let start = flow.cursor;
    let y = flow.y;
    let lines = super::text::text(context, &node.text, node, flow, items.len())?;
    let left = lines
        .iter()
        .map(|line| line.x)
        .fold(flow.x + start, f64::min);
    let right = lines
        .iter()
        .map(|line| line.x + line.width)
        .fold(left, f64::max);
    items.push(box_item(
        node,
        left,
        y,
        right - left,
        (flow.y + flow.line - y).max(super::sizing::advance(&node.style)),
        lines,
    ));
    Ok(())
}
/** HTML dimensions accept only positive integers; malformed values fall back to intrinsic size. */
fn dimension(node: &Node, name: &str) -> Option<f64> {
    node.attributes
        .get(name)?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .map(f64::from)
}
/// An img showing an SVG drawing carries its paths fitted to the content box, painted in the
/// img's text color for `currentColor`; any other img is returned unchanged.
fn drawn(
    context: &mut LayoutContext<'_>,
    node: &Node,
    mut item: DisplayItem,
) -> EngineResult<DisplayItem> {
    let Some(source) = node
        .image_src
        .as_deref()
        .filter(|_| node.image_size.is_some())
    else {
        return Ok(item);
    };
    let edge = node.style.border;
    let content = (
        item.x + node.style.padding[3] + edge,
        item.y + node.style.padding[0] + edge,
        (item.width - horizontal_edges(node)).max(0.0),
        (item.height - vertical_edges(node)).max(0.0),
    );
    let current = item.color.clone().unwrap_or_else(|| "#000000".into());
    item.vector = super::svg::image_paths(context, source, content, &current)?;
    Ok(item)
}
