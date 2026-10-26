/** DOM identity adoption preserves an image's outstanding decode and moves native association explicitly. */
use super::{Page, State};
use crate::constants::{NODES, STATEMENTS, VM_DETACHED_NODES};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A validated DOM adoption moves the same image from its old local identity to its new one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageAdoption {
    /// The prior identity may still name an outstanding backend decode.
    pub from: usize,
    /// The new document identity receives the same decoded pixels and event callback.
    pub to: usize,
}

impl crate::v9::VmState {
    /// The native host may retain decoded pixels for this exact saved detached image, without exposing them as painted content.
    pub fn is_detached_image(&self, node: usize) -> bool {
        self.page.images.contains_key(&node)
            && self
                .page
                .detached
                .get(&node)
                .is_some_and(|entry| entry.tag == "img")
    }
}

/** Called only after the DOM parser adopts a genuine image; a source generation never changes here. */
pub(super) fn adopt(page: &mut Page, from: usize, to: usize) -> EngineResult<()> {
    if from == to || !page.state.images.contains_key(&from) {
        return Ok(());
    }
    if page.effects.image_adoptions.len() >= STATEMENTS {
        return Err(EngineError::limit("Image adoption effects", STATEMENTS));
    }
    if page
        .state
        .images
        .values()
        .map(|s| s.previous_nodes.len())
        .sum::<usize>()
        >= NODES + VM_DETACHED_NODES
    {
        return Err(EngineError::limit(
            "Image adoption records",
            NODES + VM_DETACHED_NODES,
        ));
    }
    let mut state = page.state.images.remove(&from).expect("checked image");
    state.previous_nodes.push(from);
    page.state.images.insert(to, state);
    for request in &mut page.effects.image_requests {
        if request.node == from {
            request.node = to;
        }
    }
    page.effects
        .image_adoptions
        .push(ImageAdoption { from, to });
    Ok(())
}

/** A completed backend request may still name a pre-adoption identity from the same source generation. */
pub(super) fn target(states: &BTreeMap<usize, State>, node: usize) -> Option<usize> {
    if states.contains_key(&node) {
        return Some(node);
    }
    states
        .iter()
        .find_map(|(current, state)| state.previous_nodes.contains(&node).then_some(*current))
}

/** Saved alias lists remain bounded, acyclic and unique across current images and their histories. */
pub(super) fn validate(states: &BTreeMap<usize, State>) -> EngineResult<()> {
    let count: usize = states
        .values()
        .map(|state| state.previous_nodes.len())
        .sum();
    if count > NODES + VM_DETACHED_NODES {
        return Err(EngineError::limit(
            "Image adoption records",
            NODES + VM_DETACHED_NODES,
        ));
    }
    let mut seen: BTreeSet<usize> = states.keys().copied().collect();
    if states
        .values()
        .flat_map(|state| &state.previous_nodes)
        .any(|node| !seen.insert(*node))
    {
        return Err(EngineError::invalid("Invalid image adoption history."));
    }
    Ok(())
}
