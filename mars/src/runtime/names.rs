/**
 * Accessible names as agents read them: Eye's labels, plus the WAI-ARIA name-from-content rule
 * for links, buttons, headings and the like. Eye names an element by its own text run only, so
 * `<a><span>Tech</span></a>` and `<a><img alt=Home></a>` came out nameless and agents could not
 * find the links a person reads. Find, inspect, summary, diff and batch steps all name nodes
 * through here, so they agree on what a node is called.
 */
use crate::constants::TEXT_BYTES;
use crate::document::{Document, Node, role};
use std::collections::BTreeMap;

/** Roles whose name comes from their content when nothing labels them (WAI-ARIA 1.2 4.3.2). */
const FROM_CONTENT: &[&str] = &[
    "link", "button", "heading", "tab", "menuitem", "option", "treeitem", "switch", "checkbox",
    "radio",
];

/** Elements a `<label>` can name, whose Eye label may come from one. */
const LABELABLE: &[&str] = &["input", "button", "select", "textarea"];

/** Every node's accessible name, in node order, each cut to TEXT_BYTES. */
pub(super) fn names(document: &Document) -> Vec<String> {
    let mut names = document.labels();
    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, node) in document.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            children.entry(parent).or_default().push(index);
        }
    }
    for (index, node) in document.nodes.iter().enumerate() {
        // Eye's label stands when an author or a <label> gave it; otherwise its whole-subtree
        // text would also read aria-hidden content, so the content rule replaces it.
        let authored = node.attributes.contains_key("aria-label")
            || LABELABLE.contains(&node.tag.as_str()) && !names[index].trim().is_empty();
        if !authored && FROM_CONTENT.contains(&role(node)) {
            names[index] = from_content(document, &children, node);
        }
    }
    names
}

/** The node's rendered descendant text and image alternatives, else its `title` tooltip. */
fn from_content(
    document: &Document,
    children: &BTreeMap<usize, Vec<usize>>,
    node: &Node,
) -> String {
    let mut text = node.text.clone();
    content(document, children, node.local_id, &mut text);
    let mut name = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        name = node.attributes.get("title").cloned().unwrap_or_default();
    }
    name.truncate(name.floor_char_boundary(TEXT_BYTES));
    name
}

/** Appends what each shown child contributes, stopping once the name is past its bound. */
fn content(
    document: &Document,
    children: &BTreeMap<usize, Vec<usize>>,
    id: usize,
    text: &mut String,
) {
    for &index in children.get(&id).into_iter().flatten() {
        if text.len() > TEXT_BYTES {
            return;
        }
        let child = &document.nodes[index];
        // `aria-hidden` content is no part of a name; the walk stops at it, so checking each
        // child's own attribute covers its subtree.
        if child.hidden() || aria_hidden(child) {
            continue;
        }
        text.push(' ');
        if let Some(label) = child.attributes.get("aria-label") {
            text.push_str(label);
        } else if child.tag == "img" {
            text.push_str(&child.image_alt);
        } else if child.tag != "svg" {
            // An svg's subtree is drawing, not text; only its aria-label above names it.
            text.push_str(&child.text);
            content(document, children, child.local_id, text);
        }
    }
}

/** `aria-hidden="true"` on this element itself. */
fn aria_hidden(node: &Node) -> bool {
    node.attributes
        .get("aria-hidden")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}
