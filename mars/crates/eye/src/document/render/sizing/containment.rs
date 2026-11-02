/** Size containment in layout, after Blink's `LayoutBox::ShouldApplySizeContainment` and the
 * `contain-intrinsic-size` override (ComputeIntrinsicBlockSizeForSizeContainment): a contained
 * axis measures as an empty box, or one holding a single child of the `contain-intrinsic-*`
 * size, while the content is still laid out and painted, overflowing it. */
use super::IntrinsicWidths;
use crate::document::Node;

/** The content-box min- and max-content widths of a box whose inline axis is size-contained:
 * both are its `contain-intrinsic-width`, or zero for `none`. */
pub(in crate::document::render) fn contained_widths(node: &Node) -> Option<IntrinsicWidths> {
    if !node.style.contain_size[0] || node.style.vertical_writing {
        return None;
    }
    let width = node.style.contain_intrinsic[0].unwrap_or(0.0);
    Some(IntrinsicWidths {
        min_content: width,
        max_content: width,
    })
}

/** The border-box content height of a box, `natural`, unless its block axis is size-contained:
 * then its `contain-intrinsic-height` (zero for `none`) inside its border and padding. */
pub(in crate::document::render) fn contained_height(node: &Node, natural: f64) -> f64 {
    if !node.style.contain_size[1] || node.style.vertical_writing {
        return natural;
    }
    node.style.contain_intrinsic[1].unwrap_or(0.0) + super::vertical_edges(node)
}

/** The natural size a size-contained replaced box takes in place of its content's, with no
 * ratio: its `contain-intrinsic-size`, each `none` axis zero. */
pub(in crate::document::render) fn contained_natural(node: &Node) -> Option<(f64, f64)> {
    let [inline, block] = node.style.contain_size;
    (inline && block).then(|| {
        let [width, height] = node.style.contain_intrinsic;
        (width.unwrap_or(0.0), height.unwrap_or(0.0))
    })
}
