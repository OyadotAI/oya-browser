/** Fetch snapshots are owned by the initiating context and bounded across queued host turns. */
use super::*;
use crate::v9::platform::{BlobUrlSource, PageState};
use crate::v9::vm::heap::Heap;

/** Match the registry's aggregate ceiling independently of revocation and later replacement URLs. */
pub(super) const SNAPSHOT_BYTES: usize = crate::constants::VM_STATE_BYTES / 16;

/** Read no author code here: request conversion and signal getters must already be complete. */
pub(super) fn capture(vm: &Vm, request: &mut HostRequest) -> Result<(), String> {
    request.blob_source = None;
    if parse_url(&request.url).is_none_or(|url| url.scheme() != "blob") {
        return Ok(());
    }
    let source = super::super::blob_urls::resolve(&vm.page, &request.url)
        .ok_or_else(|| "Failed to fetch".to_owned())?;
    if request.method != "GET" || source.bytes.len() > VM_RESPONSE_BYTES {
        return Err("Failed to fetch".into());
    }
    request.blob_source = Some(source);
    Ok(())
}

/** Count immutable body and metadata once per request even when Arc shares physical storage. */
pub(super) fn charge(request: &HostRequest) -> usize {
    request
        .blob_source
        .as_ref()
        .map_or(0, |source| storage(&request.url, source))
}

/** Source metadata also consumes the pending snapshot budget, including empty Blob requests. */
fn storage(address: &str, source: &BlobUrlSource) -> usize {
    source
        .bytes
        .len()
        .saturating_add(address.len())
        .saturating_add(source.mime_type.len())
        .saturating_add(source.origin.len())
}

/** Snapshot storage is released only when the corresponding pending request leaves the VM. */
pub(super) fn admits(page: &PageState, request: &HostRequest) -> bool {
    page.pending.iter().fold(charge(request), |total, pending| {
        total.saturating_add(pending.blob_bytes)
    }) <= SNAPSHOT_BYTES
}

/** Saved Requests cannot carry capabilities; only actual fetch effects contain captured sources. */
pub(super) fn validate(page: &PageState, heap: &Heap) -> crate::protocol::EngineResult<()> {
    validate_summary(page)?;
    for object in heap.objects.values() {
        if let Class::Host(host) = &object.class
            && let HostData::Request(request) = host.as_ref()
            && request.blob_source.is_some()
        {
            return Err(EngineError::invalid(
                "Request cannot retain a Blob fetch snapshot.",
            ));
        }
    }
    Ok(())
}

/** Queued native capture accounting remains present in a resident page summary. */
pub(super) fn validate_summary(page: &PageState) -> crate::protocol::EngineResult<()> {
    let mut total = 0usize;
    for pending in &page.pending {
        if pending.xhr && pending.blob_bytes != 0 {
            return Err(EngineError::invalid("Invalid Blob fetch snapshot state."));
        }
        total = total.saturating_add(pending.blob_bytes);
    }
    if total > SNAPSHOT_BYTES {
        return Err(EngineError::limit(
            "Blob fetch queued bytes",
            SNAPSHOT_BYTES,
        ));
    }
    Ok(())
}
