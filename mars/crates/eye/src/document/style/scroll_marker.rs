/** Blink's StyleAdjuster for `::scroll-marker-group`: the group is block-level whatever its
`display` says, always has layout containment (it is the containing block of its absolutely and
fixed positioned markers), and in flow also size containment, so its markers never size it. */
use super::{ComputedStyle, Position};

/** Adjusts a computed `::scroll-marker-group` style as Blink does. */
pub(super) fn adjust_group(style: &mut ComputedStyle) {
    // EquivalentBlockDisplay: inline becomes block, inline-flex flex, inline-grid grid.
    style.inline = false;
    style.inline_block = false;
    style.multicol.spanner_barrier = true;
    if !matches!(style.position, Position::Absolute | Position::Fixed) {
        style.contain_size = [true; 2];
        style.multicol.contain_size = true;
    }
}
