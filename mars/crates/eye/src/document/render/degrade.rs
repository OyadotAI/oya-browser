/** Layout shapes Eye cannot place exactly fall back to block layout instead of refusing the page. */
use crate::document::{Document, FlexDirection, FlexWrap, Node};
use std::collections::BTreeMap;

/** Noted when a flex container falls back to block layout. */
const FLEX_AS_BLOCK: &str =
    "A flex container over text, tables or table structure was laid out as block flow.";
/** Noted when automatic margins Eye cannot distribute are treated as zero. */
const AUTO_MARGINS: &str = "Automatic margins on flex items or table structure were treated as 0.";
/** Noted when a wrapped column, which needs a definite height Eye lacks, stays on one line. */
const COLUMN_WRAP: &str = "A wrapped flex column was laid out without wrapping.";

/** Table structure and non-container native controls never become flex containers. */
const NO_FLEX_TAGS: [&str; 11] = [
    "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td", "input", "select", "textarea",
];
/** Table structure ignores automatic margins. */
const TABLE_TAGS: [&str; 8] = [
    "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
];

/// Returns a copy with unsupported flex, column-wrap and auto-margin uses demoted, or None when
/// nothing changes. Render lays out the copy; styling reports `notes` so the page says why.
/// ponytail: demotion approximates (flex becomes block, auto margins become 0, column wrap
/// becomes nowrap); anonymous flex items for bare text, flex auto-margin space distribution and
/// definite-height column wrapping are the real fix.
pub(super) fn degrade(document: &Document) -> Option<Document> {
    let mut copy = document.clone();
    (!demote(&mut copy).is_empty()).then_some(copy)
}

/// The value-free diagnostic for each fallback render will apply to this styled document.
pub(crate) fn notes(document: &Document) -> Vec<&'static str> {
    let mut notes = demote(&mut document.clone());
    notes.extend(super::svg::notes(document));
    notes
}

/** Applies every fallback in order and names the ones that changed something. */
fn demote(document: &mut Document) -> Vec<&'static str> {
    [
        (demote_flex(document), FLEX_AS_BLOCK),
        (unwrap_columns(document), COLUMN_WRAP),
        (clear_auto_margins(document), AUTO_MARGINS),
    ]
    .into_iter()
    .filter_map(|(changed, note)| changed.then_some(note))
    .collect()
}

/** Wrapped flex columns lay out as one nowrap column. */
fn unwrap_columns(document: &mut Document) -> bool {
    let mut changed = false;
    for node in &mut document.nodes {
        let style = &mut node.style;
        if style.display_flex
            && style.flex_direction == FlexDirection::Column
            && style.flex_wrap == FlexWrap::Wrap
        {
            style.flex_wrap = FlexWrap::NoWrap;
            changed = true;
        }
    }
    changed
}

/** Flex on table internals, controls, inside tables, or over bare text or tables becomes block. */
fn demote_flex(document: &mut Document) -> bool {
    let index = positions(document);
    let mut changed = false;
    for position in 0..document.nodes.len() {
        let node = &document.nodes[position];
        if node.style.display_flex && unsupported_flex(document, &index, node) {
            document.nodes[position].style.display_flex = false;
            changed = true;
        }
    }
    changed
}

/** The shapes the flex layout code refuses. */
fn unsupported_flex(document: &Document, index: &BTreeMap<usize, usize>, node: &Node) -> bool {
    NO_FLEX_TAGS.contains(&node.tag.as_str())
        || (node.tag == "button"
            && !document
                .nodes
                .iter()
                .any(|child| child.parent == Some(node.local_id) && !child.hidden()))
        || ancestors(document, index, node).any(|ancestor| ancestor.tag == "table")
        || document
            .nodes
            .iter()
            .any(|child| child.parent == Some(node.local_id) && child.tag == "table")
}

/** Automatic margins on table structure become zero margins; flex layout absorbs its own. */
fn clear_auto_margins(document: &mut Document) -> bool {
    let mut changed = false;
    for position in 0..document.nodes.len() {
        let node = &document.nodes[position];
        if node.style.margin_auto.iter().any(|auto| *auto)
            && TABLE_TAGS.contains(&node.tag.as_str())
        {
            document.nodes[position].style.margin_auto = Default::default();
            changed = true;
        }
    }
    changed
}

/** Vec position of each node identity. */
fn positions(document: &Document) -> BTreeMap<usize, usize> {
    document
        .nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.local_id, position))
        .collect()
}

/** Ancestors nearest first, bounded by the tree validation that runs later. */
fn ancestors<'a>(
    document: &'a Document,
    index: &'a BTreeMap<usize, usize>,
    node: &Node,
) -> impl Iterator<Item = &'a Node> + 'a {
    let mut parent = node.parent;
    std::iter::from_fn(move || {
        let node = &document.nodes[*index.get(&parent?)?];
        parent = node.parent;
        Some(node)
    })
    .take(crate::constants::DOCUMENT_DEPTH)
}
