/** Native request queues release aborted snapshots before admitting the next bounded batch. */
use crate::constants::{VM_HOST_REQUESTS, VM_RESPONSE_BYTES, VM_STATE_BYTES};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::HostRequest;
use std::collections::BTreeSet;

/** Match the initiating context's pending snapshot quota without depending on its restored counters. */
pub(super) const SNAPSHOT_BYTES: usize = VM_STATE_BYTES / 16;

/** Cancellation is idempotent for completed IDs and always precedes new snapshot admission. */
pub(in crate::runtime) fn queue(
    queued: &mut Vec<HostRequest>,
    incoming: Vec<HostRequest>,
    cancelled: &[u32],
) -> EngineResult<()> {
    if cancelled.len() > VM_HOST_REQUESTS || cancelled.contains(&0) {
        return Err(EngineError::invalid("Invalid cancelled fetch identifiers."));
    }
    let cancelled: BTreeSet<u32> = cancelled.iter().copied().collect();
    let mut bytes = 0usize;
    for request in queued
        .iter()
        .filter(|request| !cancelled.contains(&request.id))
        .chain(&incoming)
    {
        bytes = bytes.saturating_add(snapshot_bytes(request)?);
    }
    if bytes > SNAPSHOT_BYTES {
        return Err(EngineError::limit(
            "Blob fetch queued bytes",
            SNAPSHOT_BYTES,
        ));
    }
    queued.retain(|request| !cancelled.contains(&request.id));
    queued.extend(incoming);
    Ok(())
}

/** A capability cannot be smuggled on another scheme, request kind, method or oversized response. */
fn snapshot_bytes(request: &HostRequest) -> EngineResult<usize> {
    let Some(source) = &request.blob_source else {
        return Ok(0);
    };
    source.validate(&request.url)?;
    if request.kind != "fetch" || request.method != "GET" || source.bytes.len() > VM_RESPONSE_BYTES
    {
        return Err(EngineError::invalid("Invalid Blob fetch snapshot."));
    }
    Ok(source
        .bytes
        .len()
        .saturating_add(request.url.len())
        .saturating_add(source.mime_type.len())
        .saturating_add(source.origin.len()))
}
