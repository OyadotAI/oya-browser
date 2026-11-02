/** Batch agent visibility resolves each ancestor once instead of rescanning a mutated DOM. */
use super::super::{Document, Node};
use std::collections::BTreeMap;

/** Hidden flags in node order, with the same aria-hidden and SVG ancestry rules as agent_hidden. */
pub fn agent_hidden_nodes(document: &Document) -> Vec<bool> {
    let mut positions = BTreeMap::new();
    for (index, node) in document.nodes.iter().enumerate() {
        positions.entry(node.local_id).or_insert(index);
    }
    let mut hidden = vec![None; document.nodes.len()];
    let mut path = Vec::new();
    for index in 0..document.nodes.len() {
        let mut current = Some(index);
        let mut inherited = false;
        while let Some(at) = current {
            let node = &document.nodes[at];
            if let Some(value) = hidden[at] {
                inherited = value || node.tag == "svg";
                break;
            }
            path.push(at);
            current = node.parent.and_then(|id| positions.get(&id).copied());
        }
        for at in path.drain(..).rev() {
            let node = &document.nodes[at];
            let value = inherited || aria_hidden(node);
            hidden[at] = Some(value);
            inherited = value || node.tag == "svg";
        }
    }
    hidden
        .into_iter()
        .map(|value| value.unwrap_or(false))
        .collect()
}

/** A child's aria-hidden=false cannot override an ancestor that excluded the subtree. */
fn aria_hidden(node: &Node) -> bool {
    node.attributes
        .get("aria-hidden")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}
