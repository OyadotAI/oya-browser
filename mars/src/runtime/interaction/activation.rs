/** Native defaults belong to the nearest supported ancestor while event dispatch retains the actual hit node. */
use crate::{
    document::{Document, Node},
    protocol::EngineResult,
};

/** Only controls already supported by runtime activation can supply a default to their painted descendants. */
const DEFAULT_TAGS: &[&str] = &["a", "button", "label", "input"];

/** Resolve within this document only; an inert supported ancestor cannot authorize a child activation. */
pub(super) fn target(document: &Document, hit: &Node) -> EngineResult<Node> {
    let Some(node) = nearest_default(document, hit) else {
        return Ok(hit.clone());
    };
    if node.hidden() || node.disabled || !node.style.interactive || node.secret() {
        return Err(super::invalid_target());
    }
    Ok(node.clone())
}

/** Whether a click on this node has a default control to act for it, whatever that control's state. */
pub(super) fn has_default(document: &Document, hit: &Node) -> bool {
    nearest_default(document, hit).is_some()
}

/** The nearest supported default control at or above the hit, walking parents in this document. */
fn nearest_default<'a>(document: &'a Document, hit: &'a Node) -> Option<&'a Node> {
    let mut current = Some(hit);
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let node = current?;
        if DEFAULT_TAGS.contains(&node.tag.as_str()) {
            return Some(node);
        }
        current = node
            .parent
            .and_then(|id| document.nodes.iter().find(|node| node.local_id == id));
    }
    None
}

/** Inline handlers participate in the same bubbling path as registered listeners; V9 still enforces CSP. */
pub(super) fn inline_handler(document: &Document, hit: &Node) -> bool {
    let mut current = Some(hit);
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some(node) = current else {
            return document.formatting.body.attributes.contains_key("onclick")
                || document.formatting.html.attributes.contains_key("onclick");
        };
        if node.handler.is_some() || node.attributes.contains_key("onclick") {
            return true;
        }
        current = node
            .parent
            .and_then(|id| document.nodes.iter().find(|node| node.local_id == id));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Native activation follows the containing link without replacing the selected DOM node. */
    #[test]
    fn nested_painted_content_resolves_the_nearest_supported_default() {
        let document =
            crate::document::parse("<a id=link href='/next'><span id=icon>Go</span></a>").unwrap();
        let hit = document
            .nodes
            .iter()
            .find(|node| node.html_id == "icon")
            .unwrap();
        assert_eq!(target(&document, hit).unwrap().html_id, "link");
        assert_eq!(hit.html_id, "icon");
    }

    /** A painted child cannot turn an inert supported ancestor into an enabled action. */
    #[test]
    fn disabled_and_pointer_disabled_controls_cannot_be_activated_through_children() {
        for sheet in [
            "button{pointer-events:none}span{pointer-events:auto}",
            "button{display:none}",
        ] {
            let document = crate::document::compute_styles(
                &crate::document::parse("<button><span id=child>Go</span></button>").unwrap(),
                &[sheet.into()],
            )
            .unwrap();
            let hit = document
                .nodes
                .iter()
                .find(|node| node.html_id == "child")
                .unwrap();
            assert!(target(&document, hit).is_err());
        }
        let document =
            crate::document::parse("<button disabled><span id=child>Go</span></button>").unwrap();
        let hit = document
            .nodes
            .iter()
            .find(|node| node.html_id == "child")
            .unwrap();
        assert!(target(&document, hit).is_err());
    }

    /** Generic descendants still reach inline listeners on their ancestors. */
    #[test]
    fn inline_ancestor_handlers_admit_original_target_dispatch() {
        let document =
            crate::document::parse("<div onclick='void 0'><span id=child>Go</span></div>").unwrap();
        let hit = document
            .nodes
            .iter()
            .find(|node| node.html_id == "child")
            .unwrap();
        assert!(inline_handler(&document, hit));
        assert_eq!(target(&document, hit).unwrap().html_id, "child");
    }
}
