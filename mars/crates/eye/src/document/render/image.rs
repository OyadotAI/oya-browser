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
        let lines = if !shown(node) && !sized(node, flow.definite) {
            let lines = super::text::text(context, &node.text, node, &mut inner, usize::MAX)?;
            fitting_alt(lines, &node.text, inner.width)
        } else {
            Vec::new()
        };
        if lines.is_empty() {
            inner.bottom = top;
        }
        let natural_height = if shown(node) || sized(node, flow.definite) {
            let mut fixed = node.clone();
            fixed.style.width = Some((bounds.width - horizontal_edges(node)).max(0.0));
            fixed.style.percent_width = false;
            fixed.style.box_sizing = crate::document::BoxSizing::ContentBox;
            image_size(&fixed, bounds.width, flow.definite).1
        } else {
            authored_height(node, flow.definite)
                .unwrap_or(inner.bottom - bounds.y + node.style.padding[2] + node.style.border)
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
    if !shown(node) && !sized {
        return alt_block(context, node, flow, items);
    }
    let (natural_width, natural_height) = image_size(node, flow.width, flow.definite);
    let y = flow.block_top(node.style.margin[0]);
    let x = flow.x + super::body::margin_start(&node.style, flow.width, natural_width);
    let item = box_item(node, x, y, natural_width, natural_height, Vec::new());
    items.push(drawn(context, node, item)?);
    flow.after_block(y + natural_height, node.style.margin[2]);
    Ok(())
}
/** A broken or unloaded img in block flow is a box holding its alt text. An authored CSS width
 * gives the box its width (Chrome sizes it by the width and lays the alt text inside), and without
 * one the box is as wide as its text. */
fn alt_block(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let authored = alt_authored_width(node, flow);
    let x = flow.x
        + authored.map_or(0.0, |width| {
            super::body::margin_start(&node.style, flow.width, width)
        });
    let (lines, height) = alt_contents(context, node, flow, items.len(), x, flow.y, authored)?;
    let width = authored.unwrap_or_else(|| {
        lines
            .iter()
            .map(|line| line.x + line.width)
            .fold(x, f64::max)
            - x
    });
    items.push(box_item(node, x, flow.y, width, height, lines));
    flow.after_block(flow.y + height, node.style.margin[2]);
    flow.newline();
    Ok(())
}
/** The alt text of a broken or unloaded img laid inside a box at (`x`, `y`) of `width`, when one is
 * given (else the text is laid on one line), with the box height it takes. */
fn alt_contents(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &Flow,
    item: usize,
    x: f64,
    y: f64,
    width: Option<f64>,
) -> EngineResult<(Vec<super::PaintLine>, f64)> {
    let mut inner = Flow::new(x, y, width.unwrap_or(flow.width));
    let lines = super::text::text(context, &node.text, node, &mut inner, item)?;
    Ok((lines, alt_height(node, flow, inner.line)))
}
/** The border-box width an authored CSS width gives an img in `flow`, or None without one. */
fn alt_authored_width(node: &Node, flow: &Flow) -> Option<f64> {
    node.style
        .width
        .is_some()
        .then(|| image_size(node, flow.width, flow.definite).0)
}
/** The height of a broken or unloaded img in block flow: its authored height, else the aspect
 * ratio's height, else its alt text's line (at least the font's advance). */
fn alt_height(node: &Node, flow: &Flow, text_line: f64) -> f64 {
    let (_, ratio_height) = image_size(node, flow.width, flow.definite);
    authored_height(node, flow.definite).unwrap_or_else(|| {
        if ratio_height > 0.0 {
            ratio_height
        } else {
            text_line.max(super::sizing::advance(&node.style))
        }
    })
}
/** Min- and max-content widths of an img. One still showing its alt text is laid out by Blink as
 * a block of that fallback text (HTMLImageFallbackHelper), so only an authored width replaces the
 * text's width: an authored height alone (LinkedIn's lazy `h-[840px]` flex item) kept the alt's
 * 154px in Chromium but collapsed to 0 here through the replaced path's missing ratio. */
pub(super) fn intrinsic(
    context: &mut LayoutContext<'_>,
    node: &Node,
) -> EngineResult<super::sizing::IntrinsicWidths> {
    let authored_width = node.style.width.is_some() || node.attributes.contains_key("width");
    let empty_with_height = node.text.is_empty() && node.style.height.is_some();
    let (min_content, max_content) = if shown(node) || authored_width || empty_with_height {
        super::sizing::replaced_contribution(node, percent_basis(context, node))
    } else {
        let face = super::sizing::face_for(&node.style, Some(&node.text));
        let width = context.measure(&node.text, face)?;
        (width, width)
    };
    Ok(super::sizing::IntrinsicWidths {
        min_content,
        max_content,
    })
}
/** The definite content height a replaced box's percentage height resolves against while its
containing blocks are sized from their content (Blink resolves it for min- and max-content
contributions too, so a float around a `height: 100%` canvas is as wide as the scaled canvas). */
pub(super) fn percent_basis(context: &LayoutContext<'_>, node: &Node) -> Option<f64> {
    let parent = context.node(node.parent?).ok()?;
    // A grid or flex item's percentage resolves against its area or line, sized later.
    if parent.style.display_grid || parent.style.display_flex {
        return None;
    }
    definite_content_height(context, parent, crate::constants::DOCUMENT_DEPTH)
}
/** A box's definite content height: an authored length, a percentage of its own definite
basis, the span between both insets of an absolutely positioned box, or in quirks mode an auto
block's parent's (HTML's percentage height quirk); None when its content decides it. */
pub(super) fn definite_content_height(
    context: &LayoutContext<'_>,
    node: &Node,
    depth: usize,
) -> Option<f64> {
    let style = &node.style;
    let parent = || {
        let parent = context.node(node.parent?).ok()?;
        definite_content_height(context, parent, depth.checked_sub(1)?)
    };
    let content = match style.height {
        Some(height) => super::sizing::content_height(style, height.resolve_size(parent())?),
        None if super::flow::out_of_flow(node) => {
            let basis = parent()?;
            let top = super::positioned::inset(style, 0, basis)?;
            let bottom = super::positioned::inset(style, 2, basis)?;
            let margins = style.margin[0] + style.margin[2];
            (basis - top - bottom - margins - vertical_edges(node)).max(0.0)
        }
        None => match ratio_height(node) {
            Some(height) => height,
            None if context.document.formatting.quirks && !style.inline && !style.display_flex => {
                return parent();
            }
            None => return None,
        },
    };
    Some(content.max(0.0))
}
/** The definite content height `aspect-ratio` gives a box with a fixed px width, which its
children's percentage heights resolve against (an `aspect-ratio: 2/1; width: 200px` box is
100px tall before layout). */
fn ratio_height(node: &Node) -> Option<f64> {
    let style = &node.style;
    let ratio = style.aspect_ratio.filter(|_| !style.percent_width)?;
    let width = super::sizing::content_size(node, style.width?, true);
    Some(if super::flow::ratio_sizes_border_box(node) {
        ((width + horizontal_edges(node)) / ratio - vertical_edges(node)).max(0.0)
    } else {
        width / ratio
    })
}
/** Chromium clips alt text to an image box it was given, so alt that only fits by breaking words
(a narrow or zero-width positioned image still loading) shows nothing, not a column of letters. */
fn fitting_alt(lines: Vec<super::PaintLine>, alt: &str, width: f64) -> Vec<super::PaintLine> {
    let words = alt.split_whitespace().count();
    let broken = lines.len() > words || lines.iter().any(|line| line.width > width + 0.5);
    if broken { Vec::new() } else { lines }
}
/** Replaced content to lay out rather than alt text: a decoded image, or an embedded frame,
which is replaced whether or not its document loaded. */
fn shown(node: &Node) -> bool {
    node.image_size.is_some() || super::flow::frame(node)
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
/** An image with nothing to show still takes its authored height, as Blink sizes each specified
 * axis of a broken or not-yet-loaded image (LinkedIn's lazy `h-[840px]` image is 840px tall
 * before its source arrives, so it reaches the viewport where Chromium's does). */
fn authored_height(node: &Node, definite: Option<f64>) -> Option<f64> {
    let height = node.style.height?.resolve_size(definite)?;
    Some(super::sizing::border_height(node, height))
}
/** Border-box size from CSS sizes, then HTML dimensions, then intrinsic size, keeping the aspect ratio. */
pub(super) fn image_size(node: &Node, available: f64, definite: Option<f64>) -> (f64, f64) {
    let (width, height) = super::sizing::replaced_size(node, Some(available.max(0.0)), definite);
    (width, height.min(LAYOUT_LIMIT + vertical_edges(node)))
}
/** Image heights stay inside layout geometry bounds even for extreme aspect ratios. */
const LAYOUT_LIMIT: f64 = crate::constants::LAYOUT_EXTENT;
/** Inline images occupy replaced-content geometry in the same line as adjacent text. A broken or
 * unloaded img with an authored width is a box of that width holding its alt text. */
fn inline_image(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let alt = !shown(node) && !sized(node, flow.definite);
    let authored = alt_authored_width(node, flow);
    if alt && authored.is_none() {
        return inline_alt(context, node, flow, items);
    }
    let (width, height) = image_size(node, flow.width, flow.definite);
    let width = if alt {
        authored.unwrap_or(width)
    } else {
        width
    };
    let style = &node.style;
    let outer = width + style.margin[1] + style.margin[3];
    if flow.cursor > 0.0 && !super::glue::held(flow) && !flow.fits(outer) {
        flow.wrap();
    }
    flow.fit(outer);
    let start = items.len();
    let left = flow.left() + flow.cursor + style.margin[3];
    let top = flow.y + style.margin[0];
    let (lines, height) = if alt {
        alt_contents(context, node, flow, start, left, top, Some(width))?
    } else {
        (Vec::new(), height)
    };
    let item = box_item(node, left, top, width, height, lines);
    items.push(drawn(context, node, item)?);
    // A replaced box's baseline is its bottom margin edge.
    let margin_box = height + style.margin[0] + style.margin[2];
    super::text::place_atomic(
        flow,
        (start, start + 1),
        outer,
        (margin_box, margin_box),
        (style, Some(super::text::central_offset(context, node))),
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
        authored_height(node, flow.definite)
            .unwrap_or_else(|| (flow.y + flow.line - y).max(super::sizing::advance(&node.style))),
        lines,
    ));
    Ok(())
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
    let natural = node
        .image_size
        .map(|(width, height)| (f64::from(width), f64::from(height)))
        .unwrap_or((content.2, content.3));
    let object = object_rect(&node.style, natural, content);
    item.vector = super::svg::image_paths(context, source, object, &current)?;
    Ok(item)
}
/// Where replaced content paints inside its content box under `object-fit` and
/// `object-position`, a port of Blink's `LayoutReplaced::ComputeReplacedContentRect`: `fill`
/// stretches to the box, `contain` and `cover` scale the natural size uniformly to fit inside or
/// to cover it, `none` keeps the natural size, `scale-down` is the smaller of `none` and
/// `contain`; the result is then placed by `object-position` within the leftover room.
pub(super) fn object_rect(
    style: &crate::document::ComputedStyle,
    (natural_width, natural_height): (f64, f64),
    (x, y, width, height): (f64, f64, f64, f64),
) -> (f64, f64, f64, f64) {
    use crate::document::style::ObjectFit;
    if natural_width <= 0.0 || natural_height <= 0.0 || style.object_fit == ObjectFit::Fill {
        return (x, y, width, height);
    }
    let fit = |cover: bool| {
        let scale = if cover {
            (width / natural_width).max(height / natural_height)
        } else {
            (width / natural_width).min(height / natural_height)
        };
        (natural_width * scale, natural_height * scale)
    };
    let (used_width, used_height) = match style.object_fit {
        ObjectFit::Contain => fit(false),
        ObjectFit::Cover => fit(true),
        ObjectFit::None => (natural_width, natural_height),
        _ => {
            let contained = fit(false);
            if contained.0 < natural_width {
                contained
            } else {
                (natural_width, natural_height)
            }
        }
    };
    let [px, py] = style.object_position;
    let place = |extent: crate::document::style::Extent, room: f64| {
        extent.px + room * extent.percent / 100.0
    };
    (
        x + place(px, width - used_width),
        y + place(py, height - used_height),
        used_width,
        used_height,
    )
}
