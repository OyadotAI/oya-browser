/** Shared snapshot aliases reconcile inside one VM; only broker grants can recover an existing live Arc. */
use super::*;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::vm::heap::Heap;
use std::collections::BTreeMap;

/** Untrusted snapshots cannot disagree about a store's immutable maximum or one revision's bytes. */
pub(in crate::v9) fn validate(heap: &Heap) -> EngineResult<()> {
    canonical(heap).map(|_| ())
}
/** Same-ID records are grouped locally and the newest internally consistent snapshot wins. */
fn canonical(heap: &Heap) -> EngineResult<BTreeMap<u64, SharedBuffer>> {
    let mut stores: BTreeMap<u64, SharedBuffer> = BTreeMap::new();
    if heap
        .objects
        .values()
        .filter_map(|object| object.function())
        .any(|function| !super::atomics::valid_function(heap, &function.kind))
    {
        return Err(EngineError::invalid("Invalid asynchronous shared capture."));
    }
    for object in heap.objects.values() {
        if let Class::SharedArrayBuffer(buffer) = &object.class {
            if buffer
                .async_tickets()
                .into_iter()
                .any(|(ticket, callback)| {
                    !super::atomics::valid_ticket(heap, callback, ticket, buffer.identity())
                })
            {
                return Err(EngineError::invalid("Invalid shared waiter callback."));
            }
            if let Some(previous) = stores.get(&buffer.identity()) {
                if !previous.compatible_snapshot(buffer) {
                    return Err(EngineError::invalid("Conflicting shared buffer snapshots."));
                }
                if previous.revision() > buffer.revision() {
                    continue;
                }
            }
            stores.insert(buffer.identity(), buffer.clone());
        }
    }
    Ok(stores)
}
/** Reattachment never grants access from a serialized ID and never overwrites concurrent bytes. */
pub(in crate::v9) fn initialize(vm: &mut Vm) -> EngineResult<()> {
    let agent = super::agent(vm);
    let snapshots = canonical(&vm.heap)?;
    if snapshots.is_empty() {
        // Nothing to reattach. The counts are already exact: `finish` leaves every state it
        // builds collected or recounted, and a decoded heap recounts as it is read.
        return Ok(());
    }
    let mut stores = BTreeMap::new();
    for (id, snapshot) in snapshots {
        let buffer = if snapshot.restored() {
            snapshot.canonical_snapshot_for_agent(agent)?
        } else {
            snapshot.for_agent(agent)
        };
        #[cfg(feature = "conformance")]
        let buffer = {
            let mut buffer = buffer;
            if let Some(host) = &vm.page.environment.conformance_workers {
                if let Some(live) =
                    host.restore_shared(vm.page.environment.conformance_worker_id, id)
                {
                    if live.backing_maximum() != snapshot.backing_maximum()
                        || snapshot.backing_maximum().is_none()
                            && live.backing_len() != snapshot.backing_len()
                        || snapshot.backing_maximum().is_some()
                            && live.backing_len() < snapshot.backing_len()
                    {
                        return Err(EngineError::invalid(
                            "Shared buffer grant metadata disagrees with its snapshot.",
                        ));
                    }
                    buffer = live;
                } else {
                    buffer = host.shared(vm.page.environment.conformance_worker_id, buffer)?;
                }
            }
            buffer
        };
        stores.insert(id, buffer);
    }
    for object in vm.heap.objects.values_mut() {
        if let Class::SharedArrayBuffer(buffer) = &mut object.class {
            *buffer = buffer
                .attach_to(&stores[&buffer.identity()])
                .for_agent(agent);
        }
    }
    vm.heap.recount();
    Ok(())
}
