/** An inline box split by a block (CSS 2.1 9.2.1.1): the part before the first block and the part
 * after the last one are line boxes of their own. With no content there, the box's start (or
 * end) border and padding still make that line, as Blink keeps it (block-in-inline-whitespace-001
 * shows a 3px start border on its own line above the first block). */
use super::flow::*;

/** Whether a child is a block-level, in-flow box, which ends the inline box's current line. */
pub(super) fn is_block(child: &Node) -> bool {
    !inline_level(child)
        && !out_of_flow(child)
        && !child.removed()
        && child.style.float == crate::document::style::Float::None
        && child.tag != "#text"
}
/** The inline-start or inline-end edge the box has: its left (`end` false) or right padding and
 * border. */
fn edge(node: &Node, end: bool) -> f64 {
    let side = if end { 1 } else { 3 };
    node.style.padding[side] + super::sizing::border_sides(&node.style)[side]
}
/** Before the box's first block, when nothing of the box sits on the line yet and the box has a
 * start edge: an empty run, so that line exists and holds the box's first fragment.
 * `placed` says whether anything of the box was already laid out. */
pub(super) fn before_first(
    context: &LayoutContext<'_>,
    node: &Node,
    (flow, items, index): (&mut Flow, &mut [DisplayItem], usize),
    placed: bool,
) {
    if !placed && edge(node, false) > 0.0 {
        empty_run(context, node, flow, items, index);
    }
}
/** After the box's last block, when nothing followed it and the box has an end edge: an empty
 * run on the next line, holding the box's last fragment. */
pub(super) fn after_last(
    context: &LayoutContext<'_>,
    node: &Node,
    (flow, items, index): (&mut Flow, &mut [DisplayItem], usize),
) {
    if edge(node, true) > 0.0 {
        empty_run(context, node, flow, items, index);
    }
}
/** Whether the box's content ends in a block: its `::after` box decides when it has one (a
 * block one does, an inline one does not), else its children (`children_end` says). */
pub(super) fn ends_in_block(node: &Node, children_end: bool) -> bool {
    let after = node.style.generated.iter().find(|generated| {
        let positioned = matches!(
            generated.style.position,
            crate::document::Position::Absolute | crate::document::Position::Fixed
        );
        generated.after && !generated.style.hidden && !positioned
    });
    after.map_or(children_end, |generated| !generated.style.inline)
}
/** An empty run of the box at the current line position. */
fn empty_run(
    context: &LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut [DisplayItem],
    index: usize,
) {
    let run = PaintLine {
        color: None,
        text: String::new(),
        x: flow.left() + flow.cursor,
        y: flow.y,
        width: 0.0,
    };
    let line = items[index].lines.len();
    items[index].lines.push(run);
    flow.push(super::text::text_frag(context, node, index, line));
}
