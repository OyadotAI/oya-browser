/** Layout shapes Eye cannot place exactly fall back to block layout instead of refusing the page. */
use crate::document::Document;
use std::collections::HashMap;

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

/** Positions needing a fallback, computed without cloning or repeatedly scanning all children. */
#[derive(Default)]
struct Fallbacks {
    /** Flex containers whose supported native structure requires block flow. */
    flex: Vec<usize>,
    /** Table structure whose automatic margins are ignored. */
    margins: Vec<usize>,
}

/** Child facts used by flex fallback; each child contributes once. */
#[derive(Default)]
struct Children {
    /** Buttons with visible content can use the normal flex implementation. */
    visible: bool,
    /** Table children require the existing block-flow fallback. */
    table: bool,
}

/// Returns a copy only when unsupported layout needs a fallback; unchanged pages stay borrowed.
pub(super) fn degrade(document: &Document) -> Option<Document> {
    let plan = fallbacks(document);
    if plan.flex.is_empty() && plan.margins.is_empty() {
        return None;
    }
    let mut copy = document.clone();
    for position in plan.flex {
        copy.nodes[position].style.display_flex = false;
    }
    for position in plan.margins {
        copy.nodes[position].style.margin_auto = Default::default();
    }
    Some(copy)
}

/// The value-free diagnostics for fallbacks, without copying a page just to inspect its styles.
pub(crate) fn notes(document: &Document) -> Vec<&'static str> {
    let plan = fallbacks(document);
    let mut notes = Vec::new();
    if !plan.flex.is_empty() {
        notes.push(FLEX_AS_BLOCK);
    }
    if !plan.margins.is_empty() {
        notes.push(AUTO_MARGINS);
    }
    notes.extend(super::svg::notes(document));
    notes
}

/** Index direct children by identity, including reordered nodes and hidden table children. */
fn children(document: &Document) -> HashMap<usize, Children> {
    let mut children: HashMap<usize, Children> = HashMap::new();
    for node in &document.nodes {
        if let Some(parent) = node.parent {
            let facts = children.entry(parent).or_default();
            facts.visible |= !node.removed();
            facts.table |= node.tag == "table";
        }
    }
    children
}

/** Inspect each node once; nested flex inside a table cell retains its own layout. */
fn fallbacks(document: &Document) -> Fallbacks {
    let children = children(document);
    let mut plan = Fallbacks::default();
    for (position, node) in document.nodes.iter().enumerate() {
        let kids = children.get(&node.local_id);
        let unsupported = NO_FLEX_TAGS.contains(&node.tag.as_str())
            || node.tag == "button" && !kids.is_some_and(|kids| kids.visible)
            || kids.is_some_and(|kids| kids.table);
        if node.style.display_flex && unsupported {
            plan.flex.push(position);
        }
        if node.style.margin_auto.iter().any(|auto| *auto)
            && TABLE_TAGS.contains(&node.tag.as_str())
        {
            plan.margins.push(position);
        }
    }
    plan
}
