/**
 * `unicode-bidi: plaintext` on a block (CSS Writing Modes 2.4.2, UAX #9 rules P2 and P3): its
 * paragraph's direction is not `direction` but its text's first strong character's, which also
 * decides where `text-align: start` puts the lines.
 * ponytail: the whole block is one paragraph; split at forced breaks if a page needs each
 * paragraph's own direction.
 */
use super::flow::*;

/** `unicode-bidi: plaintext`'s computed code (render/bidi.rs `MODES`). */
const PLAINTEXT: u8 = 5;

/** The direction a `plaintext` block's lines take, right to left when its first strong
character is; None for any other block, or one with no strong character. */
pub(super) fn paragraph_rtl(context: &LayoutContext<'_>, node: &Node) -> Option<bool> {
    let block = !node.style.inline || node.style.inline_block;
    if node.style.unicode_bidi != PLAINTEXT || !block {
        return None;
    }
    first_strong(context, node, crate::constants::DOCUMENT_DEPTH)
}

/** The direction of the first strong character in `node`'s text, in document order. */
fn first_strong(context: &LayoutContext<'_>, node: &Node, depth: usize) -> Option<bool> {
    if let Some(rtl) = node.text.chars().find_map(strong) {
        return Some(rtl);
    }
    let children = context.children(node.local_id);
    children
        .into_iter()
        .filter(|_| depth > 0)
        .find_map(|child| first_strong(context, child, depth - 1))
}

/** A character's strong direction: right to left for Hebrew, Arabic and the other RTL scripts,
left to right for any other letter; None for digits, spaces and punctuation. */
fn strong(c: char) -> Option<bool> {
    let rtl = matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF);
    match () {
        _ if rtl => Some(true),
        _ if c.is_alphabetic() => Some(false),
        _ => None,
    }
}
