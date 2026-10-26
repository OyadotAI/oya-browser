/**
 * The text that continues a run across an element boundary with no space between, for line
 * breaking: when a run's last word joins the next element's first letters (`high&shy;wa<b>y</b>`)
 * the line must also hold those letters, so the word breaks at its own last opportunity (here the
 * soft hyphen) instead of leaving the joined letters to overflow. Eye places runs one at a time
 * and never takes back a placed piece, so it looks ahead instead (Blink's LineBreaker sees the
 * whole inline formatting context at once).
 */
use super::flow::*;

/// Inline elements looked through, first child by first child, for the following text.
const DEPTH: usize = 8;

/** The text that follows the end of `node`'s own text in its inline formatting context, when
 * an in-flow inline element or text node comes next; None after a block, atomic or `<br>`. */
pub(super) fn following(context: &LayoutContext<'_>, node: &Node) -> Option<String> {
    let mut current = node;
    if node.tag != "#text"
        && let Some(child) = first_in_flow(context, node)
    {
        return first_text(context, child, DEPTH);
    }
    for _ in 0..DEPTH {
        let parent = context.node(current.parent?).ok()?;
        let siblings = context.children(parent.local_id);
        let at = siblings
            .iter()
            .position(|s| s.local_id == current.local_id)?;
        if current.space_after {
            return None;
        }
        if let Some(next) = siblings[at + 1..].iter().find(|s| !out_of_flow(s)) {
            return first_text(context, next, DEPTH);
        }
        if !parent.style.inline || parent.style.inline_block {
            return None;
        }
        current = parent;
    }
    None
}

/** The first in-flow child of `node`. */
fn first_in_flow<'a>(context: &LayoutContext<'a>, node: &Node) -> Option<&'a Node> {
    context
        .children(node.local_id)
        .into_iter()
        .find(|c| !out_of_flow(c))
}

/** The first text `node` lays out: its own text, else its first child's, through inline
 * elements only. */
fn first_text(context: &LayoutContext<'_>, node: &Node, depth: usize) -> Option<String> {
    let inline = node.tag == "#text" || (node.style.inline && !node.style.inline_block);
    // Source white space the parser folded into the flag separates the runs.
    if !inline || node.tag == "br" || depth == 0 || node.space_leading {
        return None;
    }
    let own = super::text::own_text(node);
    if !own.is_empty() {
        return Some(super::text::transformed(&node.style, &own).into_owned());
    }
    first_text(context, first_in_flow(context, node)?, depth - 1)
}

/** The leading part of `next` that a line cannot break away from `word`'s end: nothing when a
 * space starts it or UAX #14 allows a break at the boundary, else its letters up to its first
 * space or break opportunity. */
pub(super) fn joined_prefix(word: &str, next: &str, breaks: super::wrap::Breaks) -> String {
    let (Some(last), Some(first)) = (word.chars().last(), next.chars().next()) else {
        return String::new();
    };
    if first.is_whitespace() || super::wrap::between(last, first, breaks) {
        return String::new();
    }
    let head = next
        .split([' ', '\t', '\n', '\r', '\x0c'])
        .next()
        .unwrap_or_default();
    match super::wrap::first_break(head, breaks) {
        Some(at) => head[..at].to_string(),
        None => head.to_string(),
    }
}
