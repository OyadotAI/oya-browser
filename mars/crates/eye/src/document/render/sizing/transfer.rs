/** `aspect-ratio`'s transferred min and max sizes for a non-replaced block (css-sizing-4 "size
 * transfers", Blink's ComputeMinMaxInlineSizes with ComputeTransferredMinMaxInlineSizes): while
 * its width is `auto`, `min-height` and `max-height` carry through the ratio to bound the width,
 * before its own min- and max-width do, so a `max-height: 100px; aspect-ratio: 1` block is
 * 100px wide instead of filling its container. */
use super::{IntrinsicWidths, horizontal_edges, vertical_edges};
use crate::document::Node;

/**
 * The content width an auto-width block with an aspect ratio takes in `available` content width
 * once the transferred bounds clamp it; None when nothing transfers (no ratio, a non-auto width,
 * or no block bound). The caller still applies min- and max-width afterwards.
 */
pub(in crate::document::render) fn transferred_width(
    node: &Node,
    definite: Option<f64>,
    available: f64,
) -> Option<f64> {
    let style = &node.style;
    let ratio = style.aspect_ratio?;
    if style.width.is_some() || style.width_fit || style.width_stretch {
        return None;
    }
    let bound = |extent: Option<crate::document::style::Extent>| {
        let value = extent?.resolve_size(definite)?;
        Some(super::border_height(node, value))
    };
    let (min, max) = (bound(style.min_height), bound(style.max_height));
    if min.is_none() && max.is_none() {
        return None;
    }
    let inline = |block: f64| inline_from_ratio(node, ratio, block);
    let min = min.map_or(0.0, inline);
    let max = max.map_or(f64::INFINITY, inline).max(min);
    let edges = horizontal_edges(node);
    Some((available.max(edges).min(max).max(min) - edges).max(0.0))
}

/**
 * A box's own min- and max-content widths under its `aspect-ratio` (Blink's
 * BlockNode::ComputeMinMaxSizes): with a definite height (a percentage resolves against
 * `basis`, the containing block's definite content height) both are the width the ratio gives
 * it; otherwise `min-height` and `max-height` transferred through the ratio clamp the content's
 * widths, so a `max-height: 100px; aspect-ratio: 1; width: max-content` box is 100px wide.
 */
pub(in crate::document::render) fn ratio_widths(
    node: &Node,
    raw: IntrinsicWidths,
    basis: Option<f64>,
) -> IntrinsicWidths {
    let Some(ratio) = node.style.aspect_ratio.filter(|_| !excluded(node)) else {
        return raw;
    };
    let edges = horizontal_edges(node);
    let bound = |extent: Option<crate::document::style::Extent>| {
        Some(super::border_height(node, extent?.resolve_size(basis)?))
    };
    let (min, max) = (bound(node.style.min_height), bound(node.style.max_height));
    if let Some(height) = bound(node.style.height) {
        let height = height
            .min(max.unwrap_or(f64::INFINITY))
            .max(min.unwrap_or(0.0));
        let width = (inline_from_ratio(node, ratio, height) - edges).max(0.0);
        return definite_widths(node, raw, width);
    }
    let min = min.map_or(0.0, |block| inline_from_ratio(node, ratio, block) - edges);
    let max = max.map_or(f64::INFINITY, |block| {
        inline_from_ratio(node, ratio, block) - edges
    });
    let clamp = |size: f64| size.min(max.max(min)).max(min);
    IntrinsicWidths {
        min_content: clamp(raw.min_content),
        max_content: clamp(raw.max_content),
    }
}

/** `min-width: auto`, which for a box with an aspect ratio is css-sizing-4's automatic minimum
 * (the content's min-content); a length, a percentage or a content keyword is not. */
pub(in crate::document::render) fn min_width_auto(style: &crate::document::ComputedStyle) -> bool {
    style.min_width.is_none()
        && style.min_width_percent.is_none()
        && style.size_keywords[1] == crate::document::style::SizeKeyword::None
}

/** Replaced boxes transfer through their own sizing (replaced.rs), internal table boxes ignore
 * `aspect-ratio`, and a vertical box's block axis is its width. */
fn excluded(node: &Node) -> bool {
    super::super::flow::replaced_image(node)
        || node.tag == "svg"
        || node.style.table_box != crate::document::style::TableBox::None
        || super::super::table::is_cell_box(node)
        || node.style.vertical_writing
}

/** Both widths of a box whose ratio gives `width`, raised to the content's min-content by
 * css-sizing-4's automatic minimum (`min-width: auto` and visible overflow). */
fn definite_widths(node: &Node, raw: IntrinsicWidths, width: f64) -> IntrinsicWidths {
    let style = &node.style;
    if super::super::flex::scroll_container(node) {
        // ponytail: Blink's widths are the ratio width here too, but Eye's grid sizes auto
        // tracks from them where Blink's scroll-container item contributes zero
        // (grid-aspect-ratio-027); keep the content's until grid takes that minimum.
        return raw;
    }
    let automatic = min_width_auto(style);
    let width = if automatic {
        width.max(raw.min_content)
    } else {
        width
    };
    IntrinsicWidths {
        min_content: width,
        max_content: width,
    }
}

/** The border-box width the ratio gives a border-box height (Blink's InlineSizeFromAspectRatio):
 * the ratio sizes the content box unless `box-sizing: border-box` makes it size the border box. */
fn inline_from_ratio(node: &Node, ratio: f64, block: f64) -> f64 {
    if super::super::flow::ratio_sizes_border_box(node) {
        (block * ratio).max(horizontal_edges(node))
    } else {
        (block - vertical_edges(node)).max(0.0) * ratio + horizontal_edges(node)
    }
}
