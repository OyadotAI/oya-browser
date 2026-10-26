/** Layout shapes Eye cannot place exactly fall back to block layout instead of refusing the page. */
use crate::document::{Document, Node};

/** Noted when a flex container falls back to block layout. */
const FLEX_AS_BLOCK: &str =
    "A flex container over text, tables or table structure was laid out as block flow.";
/** Noted when automatic margins Eye cannot distribute are treated as zero. */
const AUTO_MARGINS: &str = "Automatic margins on flex items or table structure were treated as 0.";

/** Table structure and non-container native controls never become flex containers. */
const NO_FLEX_TAGS: [&str; 11] = [
    "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td", "input", "select", "textarea",
];
/** Table structure ignores automatic margins. */
const TABLE_TAGS: [&str; 8] = [
    "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
];

/// Returns a copy with unsupported flex and auto-margin uses demoted, or None when
/// nothing changes. Render lays out the copy; styling reports `notes` so the page says why.
/// ponytail: demotion approximates (flex becomes block, auto margins become 0); anonymous
/// flex items for bare text and flex auto-margin space distribution are the real fix.
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
        (clear_auto_margins(document), AUTO_MARGINS),
    ]
    .into_iter()
    .filter_map(|(changed, note)| changed.then_some(note))
    .collect()
}

/** Flex on table internals, controls, or over bare text or tables becomes block. Flex inside a
table cell stays flex, as Blink lays it out. */
fn demote_flex(document: &mut Document) -> bool {
    let mut changed = false;
    for position in 0..document.nodes.len() {
        let node = &document.nodes[position];
        if node.style.display_flex && unsupported_flex(document, node) {
            document.nodes[position].style.display_flex = false;
            changed = true;
        }
    }
    changed
}

/** The shapes the flex layout code refuses. */
fn unsupported_flex(document: &Document, node: &Node) -> bool {
    NO_FLEX_TAGS.contains(&node.tag.as_str())
        || (node.tag == "button"
            && !document
                .nodes
                .iter()
                .any(|child| child.parent == Some(node.local_id) && !child.removed()))
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
