/** Shared GC capabilities authorize exact roots inside an owned session arena, without a process registry. */
use super::*;
use crate::v9::vm::data::{SharedGc, SharedGcArena, WeakSharedGcArena};

/** Permission metadata is weak: it cannot retain a root, graph cycle or foreign arena by itself. */
struct Grant {
    arena: WeakSharedGcArena,
    endpoints: BTreeSet<Option<u64>>,
}
/** Session allocation shares one real arena; explicitly imported arenas keep their own identities and bounds. */
#[derive(Default)]
pub(super) struct Registry {
    arena: Option<SharedGcArena>,
    grants: BTreeMap<(u64, u64), Grant>,
    updates: usize,
    container_arena: Option<SharedGcArena>,
    container_grants: BTreeMap<(u64, u64), Grant>,
}
impl ConformanceWorkerHost {
    /** Sweep every granted shared arena once, including imported Wasm arenas and container storage. */
    pub(in crate::v9) fn collect_shared_arenas(&self) {
        let registry = self.shared_gc.lock().unwrap();
        let mut arenas: BTreeMap<u64, SharedGcArena> = BTreeMap::new();
        for arena in registry.arena.iter().chain(registry.container_arena.iter()).cloned()
            .chain(registry.grants.values().chain(registry.container_grants.values()).filter_map(|grant| grant.arena.upgrade()))
        {
            arenas.entry(arena.identity()).or_insert(arena);
        }
        drop(registry);
        for arena in arenas.into_values() { arena.collect(); }
    }
    /** JS containers and Wasm shared records use the session arena, with distinct root grants. */
    pub(in crate::v9) fn shared_container_arena(&self) -> EngineResult<SharedGcArena> {
        if self.closing.load(Ordering::Acquire) { return Err(cancelled()); }
        let mut registry = self.shared_gc.lock().unwrap();
        if registry.arena.is_none() { registry.arena = Some(SharedGcArena::new()?); }
        if registry.container_arena.is_none() { registry.container_arena = registry.arena.clone(); }
        Ok(registry.container_arena.as_ref().unwrap().clone())
    }
    /** Grant an actual container root to one live session endpoint; IDs alone never grant authority. */
    pub(in crate::v9) fn shared_container(&self, endpoint: Option<u64>, value: SharedGc) -> EngineResult<SharedGc> {
        if self.closing.load(Ordering::Acquire) { return Err(cancelled()); }
        if endpoint.is_some_and(|id| !self.workers.lock().unwrap().contains_key(&id)) { return Err(EngineError::invalid("Shared container endpoint is outside its worker session.")); }
        let mut registry = self.shared_gc.lock().unwrap();
        let arena = value.arena();
        if !registry.container_arena.as_ref().is_some_and(|owned| owned.same_store(&arena)) { return Err(EngineError::invalid("Shared container arena is outside its worker session.")); }
        let key = (arena.identity(), value.node());
        registry.container_grants.retain(|(_, node), grant| grant.arena.upgrade().is_some_and(|arena| arena.contains(*node)));
        if !registry.container_grants.contains_key(&key) && registry.container_grants.len() >= SHARED_STORE_LIMIT { return Err(EngineError::limit("Shared container root grants", SHARED_STORE_LIMIT)); }
        registry.container_grants.entry(key).or_insert_with(|| Grant { arena: arena.downgrade(), endpoints: BTreeSet::new() }).endpoints.insert(endpoint);
        Ok(value)
    }
    /** Restore only a live root explicitly granted to this endpoint. */
    pub(in crate::v9) fn restore_shared_container(&self, endpoint: Option<u64>, arena: u64, node: u64) -> Option<SharedGc> {
        let registry = self.shared_gc.lock().unwrap();
        let grant = registry.container_grants.get(&(arena, node))?;
        if !grant.endpoints.contains(&endpoint) { return None; }
        grant.arena.upgrade()?.root(node).ok()
    }
    /** All descendant isolates in this host session allocate compatible shared records in one arena. */
    pub(in crate::v9) fn shared_gc_arena(&self) -> EngineResult<SharedGcArena> {
        if self.closing.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        let mut registry = self.shared_gc.lock().unwrap();
        if registry.arena.is_none() {
            registry.arena = Some(match &registry.container_arena { Some(arena) => arena.clone(), None => SharedGcArena::new()? });
        }
        Ok(registry.arena.as_ref().unwrap().clone())
    }
    /** A native root transfer grants exactly that root; knowing another node's integer is insufficient. */
    pub(in crate::v9) fn shared_gc(
        &self,
        endpoint: Option<u64>,
        value: SharedGc,
    ) -> EngineResult<SharedGc> {
        if self.closing.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        if endpoint.is_some_and(|id| !self.workers.lock().unwrap().contains_key(&id)) {
            return Err(EngineError::invalid(
                "Shared GC endpoint is outside its worker session.",
            ));
        }
        let arena = value.arena();
        let key = (arena.identity(), value.node());
        let mut registry = self.shared_gc.lock().unwrap();
        if let Some(previous) = &registry.arena {
            if previous.identity() == arena.identity() && !previous.same_store(&arena) {
                if !previous.is_empty() {
                    return Err(EngineError::invalid("Shared GC arena identity collision."));
                }
                let shared_with_containers = registry.container_arena.as_ref().is_some_and(|container| container.same_store(previous));
                registry.arena = Some(arena.clone());
                if shared_with_containers { registry.container_arena = Some(arena.clone()); }
            }
        }
        if registry.grants.get(&key).is_some_and(|grant| {
            grant
                .arena
                .upgrade()
                .is_none_or(|arena| !arena.contains(key.1))
        }) {
            registry.grants.remove(&key);
        }
        if let Some(grant) = registry.grants.get_mut(&key) {
            if let Some(previous) = grant.arena.upgrade() {
                if !previous.same_store(&arena) {
                    return Err(EngineError::invalid("Shared GC arena identity collision."));
                }
                grant.endpoints.insert(endpoint);
                return Ok(value);
            }
        }
        if registry.updates >= SHARED_PURGE_INTERVAL || registry.grants.len() >= SHARED_STORE_LIMIT
        {
            registry.grants.retain(|(_, node), grant| {
                grant
                    .arena
                    .upgrade()
                    .is_some_and(|arena| arena.contains(*node))
            });
            registry.updates = 0;
        }
        if registry.grants.len() >= SHARED_STORE_LIMIT {
            return Err(EngineError::limit(
                "Shared GC session root grants",
                SHARED_STORE_LIMIT,
            ));
        }
        if !registry
            .arena
            .as_ref()
            .is_some_and(|owned| owned.same_store(&arena))
        {
            for ((_, node), grant) in registry
                .grants
                .range((arena.identity(), 0)..=(arena.identity(), u64::MAX))
            {
                if grant
                    .arena
                    .upgrade()
                    .is_some_and(|other| other.contains(*node) && !other.same_store(&arena))
                {
                    return Err(EngineError::invalid("Shared GC arena identity collision."));
                }
            }
        }
        registry.grants.insert(
            key,
            Grant {
                arena: arena.downgrade(),
                endpoints: BTreeSet::from([endpoint]),
            },
        );
        registry.updates += 1;
        Ok(value)
    }
    /** Saved data can reattach only an already granted, still reachable node from the same native arena. */
    pub(in crate::v9) fn restore_shared_gc(
        &self,
        endpoint: Option<u64>,
        arena: u64,
        node: u64,
    ) -> Option<SharedGc> {
        let registry = self.shared_gc.lock().unwrap();
        let grant = registry.grants.get(&(arena, node))?;
        if !grant.endpoints.contains(&endpoint) {
            return None;
        }
        grant.arena.upgrade()?.root(node).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /** Native records exercise actual arena ownership; the bytes are inert metadata in these capability tests. */
    fn record(arena: &SharedGcArena) -> SharedGc {
        arena.allocate(&[], 0, false, Vec::new()).unwrap()
    }
    #[test]
    /** Knowing the same session arena does not authorize a second, ungranted root or another endpoint. */
    fn restored_roots_require_exact_endpoint_grants() {
        let session = ConformanceWorkerSession::new();
        let host = session.host();
        let arena = host.shared_gc_arena().unwrap();
        let first = record(&arena);
        let second = record(&arena);
        host.shared_gc(None, first.clone()).unwrap();
        assert!(
            host.restore_shared_gc(None, arena.identity(), first.node())
                .is_some()
        );
        assert!(
            host.restore_shared_gc(None, arena.identity(), second.node())
                .is_none()
        );
        assert!(
            host.restore_shared_gc(Some(42), arena.identity(), first.node())
                .is_none()
        );
        assert!(host.shared_gc(Some(42), second).is_err());
    }
    #[test]
    /** A fresh host cannot recover an otherwise live arena from serialized integer identities. */
    fn root_permissions_do_not_cross_sessions() {
        let first = ConformanceWorkerSession::new();
        let second = ConformanceWorkerSession::new();
        let arena = first.host().shared_gc_arena().unwrap();
        let root = record(&arena);
        first.host().shared_gc(None, root.clone()).unwrap();
        assert!(
            second
                .host()
                .restore_shared_gc(None, arena.identity(), root.node())
                .is_none()
        );
        assert!(first.host().shared_gc_arena().unwrap().same_store(&arena));
    }
    #[test]
    /** Weak capability metadata does not retain the last root or its arena record. */
    fn root_permissions_do_not_keep_unreachable_records_alive() {
        let session = ConformanceWorkerSession::new();
        let host = session.host();
        let arena = host.shared_gc_arena().unwrap();
        let value = record(&arena);
        let node = value.node();
        host.shared_gc(None, value).unwrap();
        assert!(
            host.restore_shared_gc(None, arena.identity(), node)
                .is_none()
        );
        session.finish().unwrap();
        assert!(host.shared_gc_arena().is_err());
    }
    #[test]
    /** Sharing one arena never turns a Wasm grant into a container grant or vice versa. */
    fn shared_domains_use_one_arena_but_distinct_root_grants() {
        for container_first in [false, true] {
            let session = ConformanceWorkerSession::new();
            let host = session.host();
            let first = if container_first { host.shared_container_arena().unwrap() } else { host.shared_gc_arena().unwrap() };
            let wasm = host.shared_gc_arena().unwrap();
            let container = host.shared_container_arena().unwrap();
            assert!(first.same_store(&wasm) && wasm.same_store(&container));
            let wasm_root = record(&wasm);
            let container_root = record(&container);
            host.shared_gc(None, wasm_root.clone()).unwrap();
            host.shared_container(None, container_root.clone()).unwrap();
            assert!(host.restore_shared_gc(None, wasm.identity(), wasm_root.node()).is_some());
            assert!(host.restore_shared_container(None, container.identity(), container_root.node()).is_some());
            assert!(host.restore_shared_container(None, wasm.identity(), wasm_root.node()).is_none());
            assert!(host.restore_shared_gc(None, container.identity(), container_root.node()).is_none());
            session.finish().unwrap();
        }
    }
    #[test]
    /** Inert snapshots cannot replace a live native arena, but can recreate a fully released saved graph. */
    fn snapshots_replace_only_empty_native_arenas() {
        let session = ConformanceWorkerSession::new();
        let host = session.host();
        let arena = host.shared_gc_arena().unwrap();
        let root = record(&arena);
        host.shared_gc(None, root.clone()).unwrap();
        let restored: SharedGc =
            serde_json::from_value(serde_json::to_value(&root).unwrap()).unwrap();
        assert!(host.shared_gc(None, restored.clone()).is_err());
        drop(root);
        host.shared_gc(None, restored.clone()).unwrap();
        assert!(
            host.shared_gc_arena()
                .unwrap()
                .same_store(&restored.arena())
        );
    }
}
