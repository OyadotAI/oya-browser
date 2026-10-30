/** Image generations capture only their context's Blob registry and release replaced pending captures. */
use super::*;
use crate::constants::{IMAGE_BYTES, VM_STATE_BYTES};
use crate::v9::platform::BlobUrlSource;

/** Match the Blob registry ceiling while independently bounding outstanding image snapshots. */
const SNAPSHOT_BYTES: usize = VM_STATE_BYTES / 16;

/// Cancels this generation or an older one; a later generation of the same node stays live.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCancellation {
    /// Current node identity after any same-turn DOM adoptions.
    pub node: usize,
    /// Highest superseded source generation; identities never wrap or reset on mutations.
    pub generation: u64,
}

/** Admission runs after author source and mode mutations; URL text alone confers no bytes. */
pub(super) fn capture(page: &Page, address: &str) -> Option<BlobUrlSource> {
    let source = super::super::blob_urls::resolve(page, address)?;
    let used = page
        .state
        .images
        .values()
        .fold(charge(address, &source), |total, state| {
            total.saturating_add(state.blob_bytes)
        });
    (source.bytes.len() <= IMAGE_BYTES && used <= SNAPSHOT_BYTES).then_some(source)
}

/** Metadata counts even for zero-byte Blobs and repeated references to shared Arc storage. */
pub(super) fn charge(address: &str, source: &BlobUrlSource) -> usize {
    source
        .bytes
        .len()
        .saturating_add(address.len())
        .saturating_add(source.mime_type.len())
        .saturating_add(source.origin.len())
}

/** One cancellation per node per task suffices even for repeated A→B→A mutation. */
pub(super) fn cancel(page: &mut Page, node: usize) -> EngineResult<()> {
    let Some(state) = page.state.images.get(&node).filter(|state| state.requested) else {
        return Ok(());
    };
    let generation = state.generation;
    page.effects
        .image_requests
        .retain(|request| request.node != node);
    if let Some(existing) = page
        .effects
        .image_cancellations
        .iter_mut()
        .find(|entry| entry.node == node)
    {
        existing.generation = existing.generation.max(generation);
        return Ok(());
    }
    if page.effects.image_cancellations.len() >= NODES + VM_DETACHED_NODES {
        return Err(EngineError::limit(
            "Image cancellations",
            NODES + VM_DETACHED_NODES,
        ));
    }
    page.effects
        .image_cancellations
        .push(ImageCancellation { node, generation });
    Ok(())
}

/** Counters have no heap references and cannot hide retained snapshots in restored state. */
pub(super) fn validate(states: &BTreeMap<usize, State>) -> EngineResult<()> {
    let mut total = 0usize;
    for state in states.values() {
        if state.blob_bytes != 0 && (!state.requested || state.complete) {
            return Err(EngineError::invalid("Invalid pending Blob image state."));
        }
        total = total.saturating_add(state.blob_bytes);
    }
    if total > SNAPSHOT_BYTES {
        return Err(EngineError::limit(
            "Blob image queued bytes",
            SNAPSHOT_BYTES,
        ));
    }
    Ok(())
}
