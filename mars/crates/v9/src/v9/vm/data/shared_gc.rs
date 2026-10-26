/** Thread-shared GC arenas contain inert typed values and root handles, never another isolate's IDs. */
use crate::protocol::{EngineError, EngineResult};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
mod snapshot;
mod storage;

/** One arena stays within the same existing native byte, object and field budgets as ordinary heaps. */
const NODE_BYTES: usize = std::mem::size_of::<SharedGcRecord>()
    + std::mem::size_of::<u64>()
    + 16 * std::mem::size_of::<usize>();
const MAX_BYTES: usize = 2 * crate::constants::JS_STRING_UNITS;
/** A shared record's references identify nodes in this arena, not ordinary VM objects. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::v9) enum SharedGcValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(u64),
    BigInt(i128),
    BigIntWide(crate::v9::value::BigInteger),
    Vector([u8; 16]),
    String(Arc<[u16]>),
    Reference(u64),
}
impl SharedGcValue {
    /** Immutable payload bytes are retained once per field, independently of the fixed enum storage. */
    fn dynamic_bytes(&self) -> usize {
        match self {
            Self::String(v) => v.len() * 2,
            Self::BigIntWide(v) => v.units() * 2,
            _ => 0,
        }
    }
}
/** Typed records keep their validating wire module, exact type and owned fields. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::v9) struct SharedGcRecord {
    pub module: u32,
    pub version: u64,
    pub ty: u32,
    pub array: bool,
    pub values: Vec<SharedGcValue>,
}
/** All roots in an agent family share one synchronized arena; native clones retain actual ownership. */
#[derive(Clone)]
pub(in crate::v9) struct SharedGcArena {
    store: Arc<Store>,
}
/** Weak grants retain permission while allowing the final live arena owner to release its memory. */
#[derive(Clone)]
#[cfg(feature = "conformance")]
pub(in crate::v9) struct WeakSharedGcArena {
    store: Weak<Store>,
}
/** A wrapper or native packet holds a registered root; graph edges themselves are plain integers. */
#[derive(Clone)]
pub(in crate::v9) struct SharedGc {
    root: Arc<Root>,
    restored: bool,
}
struct Root {
    arena: SharedGcArena,
    node: u64,
}
struct Store {
    id: u64,
    state: Mutex<State>,
}
#[derive(Clone, Default)]
struct State {
    revision: u64,
    next: u64,
    modules: Vec<Arc<[u8]>>,
    records: BTreeMap<u64, SharedGcRecord>,
    roots: BTreeMap<u64, Weak<Root>>,
    bytes: usize,
    dirty: bool,
}
impl SharedGcArena {
    /** New arenas use unpredictable identities only as data; recovering authority always requires an Arc grant. */
    pub(in crate::v9) fn new() -> EngineResult<Self> {
        use ring::rand::SecureRandom;
        let mut bytes = [0; 8];
        ring::rand::SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| invalid())?;
        Ok(Self {
            store: Arc::new(Store {
                id: u64::from_le_bytes(bytes).max(1),
                state: Mutex::new(State {
                    next: 1,
                    ..State::default()
                }),
            }),
        })
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.store.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_empty(&self) -> bool {
        let mut state = self.lock();
        if state.dirty {
            state.sweep();
        }
        state.records.is_empty()
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn contains(&self, node: u64) -> bool {
        let mut state = self.lock();
        if state.dirty {
            state.sweep();
        }
        state.records.contains_key(&node)
    }
    pub(in crate::v9) fn identity(&self) -> u64 {
        self.store.id
    }
    pub(in crate::v9) fn same_store(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store)
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn downgrade(&self) -> WeakSharedGcArena {
        WeakSharedGcArena {
            store: Arc::downgrade(&self.store),
        }
    }
    /** Count live native storage once per arena, including modules, graph edges and string payloads. */
    pub(in crate::v9) fn allocated(&self) -> usize {
        let mut state = self.lock();
        if state.dirty {
            state.sweep();
        }
        state.allocated()
    }
    /** Root lookup cannot create a reference to a missing node or recover a root from an identity alone. */
    pub(in crate::v9) fn root(&self, node: u64) -> EngineResult<SharedGc> {
        let mut state = self.lock();
        if state.dirty {
            state.sweep();
        }
        self.root_locked(&mut state, node, false)
    }
    fn root_locked(&self, state: &mut State, node: u64, restored: bool) -> EngineResult<SharedGc> {
        if !state.records.contains_key(&node) {
            return Err(invalid());
        }
        let root = state
            .roots
            .get(&node)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let root = Arc::new(Root {
                    arena: self.clone(),
                    node,
                });
                state.roots.insert(node, Arc::downgrade(&root));
                root
            });
        Ok(SharedGc { root, restored })
    }
    /** Validation inspects copied inert metadata after releasing the arena mutex. */
    pub(in crate::v9) fn snapshot_records(
        &self,
    ) -> (Vec<Arc<[u8]>>, BTreeMap<u64, SharedGcRecord>) {
        let state = self.lock();
        (state.modules.clone(), state.records.clone())
    }
}
#[cfg(feature = "conformance")]
impl WeakSharedGcArena {
    pub(in crate::v9) fn upgrade(&self) -> Option<SharedGcArena> {
        self.store.upgrade().map(|store| SharedGcArena { store })
    }
}
impl SharedGc {
    pub(in crate::v9) fn arena(&self) -> SharedGcArena {
        self.root.arena.clone()
    }
    pub(in crate::v9) fn node(&self) -> u64 {
        self.root.node
    }
    pub(in crate::v9) fn restored(&self) -> bool {
        self.restored
    }
    pub(in crate::v9) fn allocated(&self) -> usize {
        self.root.arena.allocated()
    }
    pub(in crate::v9) fn metadata(&self) -> EngineResult<(Arc<[u8]>, u32, bool, usize)> {
        let state = self.root.arena.lock();
        let record = state.records.get(&self.node()).ok_or_else(invalid)?;
        Ok((
            state
                .modules
                .get(record.module as usize)
                .ok_or_else(invalid)?
                .clone(),
            record.ty,
            record.array,
            record.values.len(),
        ))
    }
    /** Broker-only attachment uses a live granted arena and verifies the root exists there. */
    pub(in crate::v9) fn attach_to(&self, arena: &SharedGcArena) -> EngineResult<Self> {
        arena.root(self.node())
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        self.arena.lock().dirty = true;
    }
}
impl std::fmt::Debug for SharedGcArena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedGcArena")
            .field("identity", &self.identity())
            .finish()
    }
}
impl std::fmt::Debug for SharedGc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedGc")
            .field("arena", &self.arena().identity())
            .field("node", &self.node())
            .finish()
    }
}
fn invalid() -> EngineError {
    EngineError::invalid("Invalid shared Wasm GC graph.")
}
fn limit() -> EngineError {
    EngineError::limit("V9 shared GC bytes", MAX_BYTES)
}
