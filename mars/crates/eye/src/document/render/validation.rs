/** Tree and geometry bounds are checked before recursion and before native publication. */
use crate::{
    constants::{DOCUMENT_DEPTH, LAYOUT_EXTENT, NODES},
    document::Document,
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;
/** Parent-before-child ordering and a depth cap reject cycles before recursion. */
pub(super) fn tree(document: &Document) -> EngineResult<()> {
    crate::document::validate_content(document)?;
    if document.nodes.len() > NODES {
        return Err(EngineError::limit("layout nodes", NODES));
    }
    let mut seen = BTreeMap::new();
    for (position, node) in document.nodes.iter().enumerate() {
        if node.local_id >= NODES
            || seen.insert(node.local_id, position).is_some()
            || node
                .parent
                .is_some_and(|id| id == node.local_id || !seen.contains_key(&id))
        {
            return Err(EngineError::invalid("Invalid layout tree."));
        }
        let mut depth = 0;
        let mut parent = node.parent;
        while let Some(id) = parent {
            depth += 1;
            if depth >= DOCUMENT_DEPTH {
                return Err(EngineError::limit("layout depth", DOCUMENT_DEPTH));
            }
            let ancestor = seen.get(&id).map(|&position| &document.nodes[position]);
            parent = ancestor.and_then(|node| node.parent);
        }
    }
    Ok(())
}
/** Finite coordinate bounds are checked before a candidate reaches native painting. */
/** Offsets may be negative (relative, absolute and transformed boxes), but never unbounded. */
pub(super) fn offset(value: f64) -> EngineResult<()> {
    bounded(value.abs())
}
pub(super) fn bounded(value: f64) -> EngineResult<()> {
    if !(0.0..=LAYOUT_EXTENT).contains(&value) {
        return Err(EngineError::limit("layout extent", LAYOUT_EXTENT as usize));
    }
    Ok(())
}

/** Vector paint stays finite and inside the logical extent; coordinates may sit left of or above the origin. */
pub(super) fn vector(path: &super::VectorPath) -> EngineResult<()> {
    let coordinates = path.commands.iter().flat_map(|command| match *command {
        super::PathCommand::Move { x, y } | super::PathCommand::Line { x, y } => vec![x, y],
        super::PathCommand::Cubic {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => vec![x1, y1, x2, y2, x, y],
        super::PathCommand::Close => Vec::new(),
    });
    for value in coordinates.chain([path.stroke_width]) {
        if !value.is_finite() || value.abs() > LAYOUT_EXTENT {
            return Err(EngineError::limit("layout extent", LAYOUT_EXTENT as usize));
        }
    }
    Ok(())
}
