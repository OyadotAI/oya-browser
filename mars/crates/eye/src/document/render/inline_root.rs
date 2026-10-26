/** Each item's inline formatting root: the nearest block container around its runs (itself when
 * it is one). Painters draw adjacent runs of one root as one shaped text, as Blink shapes a whole
 * paragraph, and never join runs of different roots, such as two table cells side by side. */
use super::flow::{DisplayItem, Document, Node};

/** Sets `inline_root` on every item. */
pub(super) fn mark(document: &Document, items: &mut [DisplayItem]) {
    let positions = super::sizing::positions(document);
    let node = |local: usize| {
        positions
            .get(local)
            .copied()
            .flatten()
            .and_then(|at| document.nodes.get(at))
    };
    for item in items.iter_mut() {
        let mut current = node(item.local_id);
        for _ in 0..crate::constants::DOCUMENT_DEPTH {
            let Some(found) = current else {
                break;
            };
            if container(found) {
                item.inline_root = found.local_id;
                break;
            }
            current = found.parent.and_then(node);
        }
    }
}
/** A box whose inline content is its own formatting context's: anything but a plain inline box
 * or text. An inline-block, image or control counts, so its text never joins its line's. */
fn container(node: &Node) -> bool {
    let inline = node.style.inline && !node.style.inline_block;
    !inline && node.tag != "#text"
}
