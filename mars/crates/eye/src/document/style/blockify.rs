/** CSS 2.1 section 9.7 (Blink's StyleAdjuster::AdjustStyleForDisplay): an absolutely positioned
or floated box is blockified, so a table part (row, row group, cell, column) becomes a
plain block. Without it an abspos `display: table-row-group` waited for a table that never came
and painted nothing (CSS2 `top-applies-to-*`).
ponytail: a caption keeps its kind, since Eye's table places an out-of-flow caption itself and a
block child would break the table (`top-applies-to-015`). */
use super::{ComputedStyle, Float, Position, TableBox};

/** Turn an out-of-flow or floated table part into a block box. */
pub(super) fn blockify_table_part(style: &mut ComputedStyle) {
    let out_of_flow = matches!(style.position, Position::Absolute | Position::Fixed);
    let floated = style.float != Float::None;
    let part = !matches!(
        style.table_box,
        TableBox::None | TableBox::Table | TableBox::Caption
    );
    if part && (out_of_flow || floated) {
        style.table_box = TableBox::None;
        style.inline = false;
        style.inline_block = false;
    }
}
