/**
 * The root element's own flow: its `::before` boxes above the body and its `::after` boxes below
 * it, in the root's content box (the root is the document's block formatting context, so their
 * margins collapse with the body's as sibling blocks' do).
 */
use super::flow::*;

/// A root flow's result: where its content ends and the margin left open below it, which the
/// next block's top margin collapses with.
pub(super) struct RootRun {
    /// The bottom of the last box laid out (its border edge), or the start when none was.
    pub end: f64,
    /// The trailing bottom margin, not yet counted in `end`.
    pub margin: f64,
}

/**
 * Lays out the root's `::before` (or with `after`, `::after`) stand-ins from `top` in the
 * root's content box `(x, width)`, pushing their items.
 */
pub(super) fn root_boxes(
    context: &mut LayoutContext<'_>,
    document: &Document,
    items: &mut Vec<DisplayItem>,
    (x, top, width): (f64, f64, f64),
    after: bool,
) -> EngineResult<RootRun> {
    let tag = if after { "::after" } else { "::before" };
    let boxes = document
        .nodes
        .iter()
        .filter(|node| super::root_generated::is_root_box(node) && node.tag == tag);
    let start = items.len();
    let mut flow = Flow::new(x, top, width);
    for node in boxes {
        node_layout(context, node, &mut flow, items, None)?;
    }
    flow.newline();
    super::line::settle(context, &mut flow, items, start)?;
    Ok(RootRun {
        end: (flow.y - flow.collapsible).max(top),
        margin: flow.collapsible,
    })
}

/** How far the body moves down below the root's `::before` boxes that ended at `run`, from
 * the root content top `top`: their height, plus whatever of their bottom margin the body's
 * own top margin `body_margin` does not already cover (the two collapse). */
pub(super) fn body_shift(run: &RootRun, top: f64, body_margin: f64) -> f64 {
    if run.end <= top && run.margin == 0.0 {
        return 0.0;
    }
    (run.end - top) + (run.margin - body_margin).max(0.0)
}
