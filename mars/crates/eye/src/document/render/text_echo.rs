/**
 * A decorated block's own text in the inline paint phase. One display item carries a block's
 * background and its own text runs, so that text would paint with the background, under later
 * sibling blocks' backgrounds and borders (CSS 2.1 Appendix E paints all inline content after
 * them). Instead the block's item keeps its runs (for agents and text selection) but marks them
 * `text_deferred`, and an echo item placed in the inline phase tells painters to draw them there.
 */
use super::{
    DisplayItem,
    effects::{BLOCK, INLINE, Step, only_text, table_part},
};
use crate::document::Node;

/** The paint key of a block's text echo: its own key with the last step moved from the block
 * phase to the inline phase; None when the item needs no echo. */
pub(super) fn inline_key(node: &Node, item: &DisplayItem, key: &[Step]) -> Option<Vec<Step>> {
    if !deferred(node, item, key) {
        return None;
    }
    let mut echo = key.to_vec();
    echo.last_mut()?.0 = INLINE;
    Some(echo)
}

/** Whether a block item paints its own text runs later: it has runs, paints in the block
 * phase, is decorated (an undecorated one paints whole in the inline phase already) and is no
 * table part (whose decoration other items paint). */
pub(super) fn deferred(node: &Node, item: &DisplayItem, key: &[Step]) -> bool {
    !item.lines.is_empty()
        && !item.bare
        && key.last().is_some_and(|step| step.0 == BLOCK)
        && !only_text(item)
        && !table_part(node)
}

/** Marks `item`'s runs as painted later and returns its echo: the item with no runs and no
 * decoration of its own, at its place, clip and opacity. */
pub(super) fn defer(item: &mut DisplayItem) -> DisplayItem {
    item.text_deferred = true;
    let mut echo = item.clone();
    echo.text_deferred = false;
    echo.text_echo = true;
    echo.lines.clear();
    echo.background = None;
    echo.backgrounds.clear();
    echo.border = 0.0;
    echo.border_widths = [0.0; 4];
    echo.shadows.clear();
    echo.mask = None;
    echo.border_image = None;
    echo.vector.clear();
    echo
}
