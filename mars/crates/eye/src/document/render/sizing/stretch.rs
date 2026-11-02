/** `height: stretch` (and `min-height`, `max-height`, `-webkit-fill-available`) in block flow,
 * after Blink's ResolveBlockLengthInternal for Length::kStretch: the border box fills the
 * containing block's definite content height less its margins, never below its own border and
 * padding. A margin that adjoins a parent's edge with no border or padding, inside a parent that
 * is not a new formatting context, would collapse through it, so it is not taken out
 * (BlockLayoutAlgorithm's SetIgnoreMarginsForStretch). */
use super::{LayoutContext, vertical_edges};
use crate::document::Node;

/** The border-box height `stretch` resolves to for a block-flow `node` in a containing block
 * of `definite` content height; None when that height is indefinite (it then behaves as the
 * property's initial value) or no property of the box names `stretch`. */
pub(in crate::document::render) fn stretch_height(
    context: &LayoutContext<'_>,
    node: &Node,
    definite: Option<f64>,
) -> Option<f64> {
    if !node.style.height_stretch.contains(&true) {
        return None;
    }
    let definite = definite?;
    let parent = node.parent.and_then(|parent| context.node(parent).ok());
    let collapsing = parent.filter(|parent| !super::super::flow::new_context(parent));
    let margin = |side: usize| {
        let adjoins = collapsing.is_some_and(|parent| edge(parent, side) == 0.0);
        if adjoins || node.style.margin_auto[side] {
            0.0
        } else {
            node.style.margin[side]
        }
    };
    Some((definite - margin(0) - margin(2)).max(vertical_edges(node)))
}

/** A parent's border plus padding on one block side, which keeps a child's margin inside it. */
fn edge(parent: &Node, side: usize) -> f64 {
    parent.style.padding[side] + super::border_sides(&parent.style)[side]
}

/** The authored height when it is `stretch` and resolves, as a border-box height. */
pub(in crate::document::render) fn stretched(node: &Node, stretch: Option<f64>) -> Option<f64> {
    stretch.filter(|_| node.style.height_stretch[0])
}

/** Clamps a border-box `height` by `stretch` min and max heights: the max first, then the min,
 * which wins as CSS 2.1 10.7 has it. */
pub(in crate::document::render) fn clamp_stretch(
    node: &Node,
    height: f64,
    stretch: Option<f64>,
) -> f64 {
    let Some(stretch) = stretch else {
        return height;
    };
    let [_, min, max] = node.style.height_stretch;
    let height = if max { height.min(stretch) } else { height };
    if min { height.max(stretch) } else { height }
}
