/** Static source text has independent byte bounds while native controls and writes stay small. */
use super::{Document, Node, validate_text};
use crate::{
    constants::{DOCUMENT_DEPTH, NODES, STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Validate the actual document before recursive labels, projection, serialization or measurement. */
pub fn validate_content(document: &Document) -> EngineResult<()> {
    validate_structure(document)?;
    mars_core::dom::validate_metadata(
        &document.metadata,
        document.scripts.iter().map(|script| &script.attributes),
    )?;
    let mut canvas_bytes = 0usize;
    for node in &document.nodes {
        if let Some(bitmap) = &node.canvas {
            bitmap.validate()?;
            canvas_bytes = canvas_bytes.saturating_add(bitmap.pixels.len());
            if node.tag != "canvas" || canvas_bytes > mars_core::canvas::PAGE_BYTES {
                return Err(EngineError::invalid("Invalid document canvas storage."));
            }
        }
    }
    validate_text(&document.title)?;
    if document.csp.len() > super::CSP_POLICIES {
        return Err(EngineError::limit("meta policies", super::CSP_POLICIES));
    }
    for policy in &document.csp {
        validate_text(policy)?;
    }
    for wrapper in [&document.formatting.html, &document.formatting.body] {
        for value in wrapper.attributes.values() {
            validate_text(value)?;
        }
    }
    let mut total = 0;
    for node in &document.nodes {
        validate_node(node)?;
        total = add_static_bytes(total, node.text.len())?;
    }
    Ok(())
}

/** Parent-before-child ordering and bounded ancestry prevent forged cycles before label traversal. */
fn validate_structure(document: &Document) -> EngineResult<()> {
    if document.nodes.len() > NODES {
        return Err(EngineError::limit("document nodes", NODES));
    }
    let mut depths = BTreeMap::new();
    for node in &document.nodes {
        if node.local_id >= NODES || depths.contains_key(&node.local_id) {
            return Err(EngineError::invalid("Invalid document tree."));
        }
        let depth = match node.parent {
            Some(parent) => {
                depths
                    .get(&parent)
                    .copied()
                    .ok_or_else(|| EngineError::invalid("Invalid document tree."))?
                    + 1
            }
            None => 0,
        };
        if depth >= DOCUMENT_DEPTH {
            return Err(EngineError::limit("document depth", DOCUMENT_DEPTH));
        }
        depths.insert(node.local_id, depth);
    }
    Ok(())
}

/** Static text never relaxes initial field values, captions, metadata or decoded attributes. */
fn validate_node(node: &Node) -> EngineResult<()> {
    validate_node_text(node, node.text.len())?;
    if node.html_id.is_empty() || node.html_id.len() > mars_core::constants::HTML_ID_BYTES {
        return Err(EngineError::invalid("Invalid document identifier."));
    }
    for value in [&node.value, &node.placeholder, &node.image_alt] {
        validate_text(value)?;
    }
    for value in node.attributes.values() {
        validate_text(value)?;
    }
    for value in [&node.href, &node.handler, &node.validation, &node.image_src]
        .into_iter()
        .flatten()
    {
        validate_text(value)?;
    }
    Ok(())
}

/** The byte length is checked before parser appends allocate the combined source text. */
pub(super) fn validate_node_text(node: &Node, bytes: usize) -> EngineResult<()> {
    let limit = text_limit(node);
    if bytes > limit {
        return Err(EngineError::limit("node text bytes", limit));
    }
    Ok(())
}

/** Control text stays small for native widgets; static text has the larger per-node allowance. */
pub(super) fn text_limit(node: &Node) -> usize {
    if ["input", "textarea", "select", "button", "option"].contains(&node.tag.as_str()) {
        crate::constants::TEXT_BYTES
    } else {
        STATIC_TEXT_NODE_BYTES
    }
}

/** Checked aggregate arithmetic counts stored text once, without duplicated derived labels. */
pub(super) fn add_static_bytes(total: usize, bytes: usize) -> EngineResult<usize> {
    total
        .checked_add(bytes)
        .filter(|total| *total <= STATIC_TEXT_TOTAL_BYTES)
        .ok_or_else(|| EngineError::limit("document text bytes", STATIC_TEXT_TOTAL_BYTES))
}
