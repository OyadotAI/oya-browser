/** `::scroll-button(<direction>)` selectors (CSS Overflow 5). A button is one of four logical
directions, generated in the order block-start, inline-start, inline-end, block-end; `*` styles
all four. Physical directions map to logical ones in horizontal left-to-right text.
ponytail: vertical and right-to-left mappings of `up`/`down`/`left`/`right` are not applied. */
use super::sheet::PseudoBox;

/** The four logical buttons in generation order. */
pub(super) const BUTTONS: [u8; 4] = [BLOCK_START, INLINE_START, INLINE_END, BLOCK_END];
/** `::scroll-button(block-start)`, also `up`. */
const BLOCK_START: u8 = 0;
/** `::scroll-button(inline-start)`, also `left`. */
const INLINE_START: u8 = 1;
/** `::scroll-button(inline-end)`, also `right`. */
const INLINE_END: u8 = 2;
/** `::scroll-button(block-end)`, also `down`. */
const BLOCK_END: u8 = 3;
/** `::scroll-button(*)`: every direction. */
const ANY: u8 = 4;
/** The selector's opening, lowercased. */
const OPEN: &str = "::scroll-button(";

/** The direction a `::scroll-button()` argument names. */
fn direction(argument: &str) -> Option<u8> {
    Some(match argument.trim() {
        "block-start" | "up" => BLOCK_START,
        "inline-start" | "left" => INLINE_START,
        "inline-end" | "right" => INLINE_END,
        "block-end" | "down" => BLOCK_END,
        "*" => ANY,
        _ => return None,
    })
}

/** A trailing `::scroll-button(...)` on a lowercased selector: the suffix's byte length and the
button it styles. */
pub(super) fn suffix(lower: &str) -> Option<(usize, PseudoBox)> {
    let start = lower.rfind(OPEN)?;
    let argument = lower[start + OPEN.len()..].strip_suffix(')')?;
    let button = direction(argument)?;
    Some((lower.len() - start, PseudoBox::ScrollButton(button)))
}

/** Whether a rule styling `rule` applies to the box `wanted`: the same box, or any button for
`::scroll-button(*)`. */
pub(super) fn matches(rule: Option<PseudoBox>, wanted: Option<PseudoBox>) -> bool {
    rule == wanted
        || matches!(
            (rule, wanted),
            (
                Some(PseudoBox::ScrollButton(ANY)),
                Some(PseudoBox::ScrollButton(_))
            )
        )
}
