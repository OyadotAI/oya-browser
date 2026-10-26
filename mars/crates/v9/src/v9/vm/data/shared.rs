/** Shared backing stores own synchronized bytes; native clones share identity and snapshots carry no authority. */
use super::Bytes;
use crate::protocol::{EngineError, EngineResult};
#[cfg(feature = "conformance")]
use std::sync::Weak;
use std::sync::{Arc, Mutex, MutexGuard};
mod asynchronous;
mod snapshot;
mod waits;
pub(in crate::v9) use asynchronous::AsyncWait;
pub(in crate::v9) use waits::WaitResult;

/** Shared and ordinary buffers retain the same existing byte budget. */
pub(in crate::v9) const MAX_BYTES: usize = 2 * crate::constants::JS_STRING_UNITS;
/** Untrusted revisions leave ample headroom for every bounded execution's subsequent writes. */
const MAX_SNAPSHOT_REVISION: u64 = u64::MAX / 2;
/** Wasm buffer growth is always expressed in complete binary-format pages. */
const WASM_PAGE_BYTES: usize = 65_536;
/** Length metadata remains an exactly representable ECMAScript index. */
const MAX_INDEX: usize = ((1u64 << 53) - 1) as usize;
/** All agents and stores share one ordering point for ECMAScript sequentially consistent events. */
static SEQUENTIAL_ATOMICS: Mutex<()> = Mutex::new(());

/** One identity is shared by native clones, independently of JS wrapper identities or context IDs. */
#[derive(Clone)]
pub(in crate::v9) struct SharedBytes {
    store: Arc<Store>,
    /** Deserialized snapshots are reconciled only in their own VM or through explicit broker grants. */
    restored: bool,
    /** Wasm shared-memory buffers retain a fixed window while their backing can grow. */
    visible: Option<usize>,
    /** A Wasm growable wrapper exposes the declared maximum while the store retains its physical quota. */
    logical_maximum: Option<usize>,
    /** Callback identities are meaningful only in the VM agent that owns this wrapper. */
    agent: u64,
}
/** Broker grants cannot keep otherwise unreachable shared allocations alive indefinitely. */
#[derive(Clone, Debug)]
#[cfg(feature = "conformance")]
pub(in crate::v9) struct WeakSharedBytes {
    store: Weak<Store>,
    visible: Option<usize>,
    /** A Wasm growable wrapper exposes the declared maximum while the store retains its physical quota. */
    logical_maximum: Option<usize>,
    agent: u64,
}
#[cfg(feature = "conformance")]
impl WeakSharedBytes {
    /** A live JS wrapper, packet or worker wait supplies the actual ownership needed for reattachment. */
    pub(in crate::v9) fn upgrade(&self) -> Option<SharedBytes> {
        self.store.upgrade().map(|store| SharedBytes {
            store,
            restored: false,
            visible: self.visible,
            logical_maximum: self.logical_maximum,
            agent: self.agent,
        })
    }
}

/** Bytes, growth and wait registration are guarded by one mutex, preventing compare/sleep races. */
struct Store {
    id: u64,
    state: Mutex<State>,
}
/** Revisions make independently serialized aliases coherent even while another worker writes. */
struct State {
    bytes: Vec<u8>,
    maximum: Option<usize>,
    revision: u64,
    waiters: Vec<waits::Entry>,
    next_ticket: u64,
}

impl SharedBytes {
    /** Poison recovery cannot expose uninitialized data; no script runs under a backing-store lock. */
    fn lock(&self) -> MutexGuard<'_, State> {
        self.store
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
    /** Identity is opaque data; broker access additionally requires a previously granted endpoint. */
    pub(in crate::v9) fn identity(&self) -> u64 {
        self.store.id
    }
    /** Grant ledgers retain permission without retaining native memory after the final actual owner dies. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn downgrade(&self) -> WeakSharedBytes {
        WeakSharedBytes {
            store: Arc::downgrade(&self.store),
            visible: self.visible,
            logical_maximum: self.logical_maximum,
            agent: self.agent,
        }
    }
    /** The broker detects accidental ID collision instead of silently merging independent stores. */
    pub(in crate::v9) fn same_store(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store)
    }
    /** Current length is read with the same synchronization as byte access. */
    pub(in crate::v9) fn len(&self) -> usize {
        self.visible.unwrap_or_else(|| self.backing_len())
    }
    /** Broker validation and Wasm growth operate on the whole backing, independently of JS windows. */
    pub(in crate::v9) fn backing_len(&self) -> usize {
        self.lock().bytes.len()
    }
    /** A fixed Wasm wrapper hides growth capability but continues to share its original bytes. */
    pub(in crate::v9) fn fixed_view(&self, length: usize) -> Option<Self> {
        (length <= self.backing_len()).then(|| Self {
            store: self.store.clone(),
            restored: self.restored,
            visible: Some(length),
            logical_maximum: None,
            agent: self.agent,
        })
    }
    /** The Wasm engine's view can access the full reserved backing and its growth allowance. */
    pub(in crate::v9) fn engine_view(&self) -> Self {
        Self {
            store: self.store.clone(),
            restored: self.restored,
            visible: None,
            logical_maximum: None,
            agent: self.agent,
        }
    }
    /** Wasm conversions retain the actual store and make future growth visible through existing views. */
    pub(in crate::v9) fn growable_view(&self, maximum: usize) -> Option<Self> {
        if maximum < self.backing_len()
            || self
                .backing_maximum()
                .is_none_or(|physical| physical > maximum)
            || maximum > MAX_INDEX
            || maximum % WASM_PAGE_BYTES != 0
        {
            return None;
        }
        Some(Self {
            store: self.store.clone(),
            restored: self.restored,
            visible: None,
            logical_maximum: Some(maximum),
            agent: self.agent,
        })
    }
    /** Only engine-owned growable windows require page-aligned author growth. */
    pub(in crate::v9) fn wasm_maximum(&self) -> Option<usize> {
        self.logical_maximum
    }
    /** Reconciliation restores aliases without discarding an individual wrapper's fixed byte window. */
    pub(in crate::v9) fn attach_to(&self, backing: &Self) -> Self {
        Self {
            store: backing.store.clone(),
            restored: false,
            visible: self.visible,
            logical_maximum: self.logical_maximum,
            agent: self.agent,
        }
    }
    /** Only growable buffers carry a mutable-length allowance. */
    pub(in crate::v9) fn maximum(&self) -> Option<usize> {
        if self.visible.is_some() {
            None
        } else {
            self.logical_maximum.or_else(|| self.backing_maximum())
        }
    }
    /** Actual allocation metadata is immutable and shared by every fixed window. */
    pub(in crate::v9) fn backing_maximum(&self) -> Option<usize> {
        self.lock().maximum
    }
    /** Reserved bytes count once per backing identity in the VM heap budget. */
    pub(in crate::v9) fn allocated(&self) -> usize {
        let state = self.lock();
        state.maximum.unwrap_or(state.bytes.len())
    }
    /** Memory reporting includes real reserved bytes, waiter vectors and individual signal allocations. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn resident_size(&self) -> usize {
        let state = self.lock();
        let arc_header = 2 * std::mem::size_of::<usize>();
        std::mem::size_of::<Store>()
            + arc_header
            + state.bytes.capacity()
            + state.waiters.capacity() * std::mem::size_of::<waits::Entry>()
            + state.waiters.len() * (std::mem::size_of::<waits::Signal>() + arc_header)
    }
    /** Snapshot revisions order simultaneous writes without exposing synchronization state to scripts. */
    pub(in crate::v9) fn revision(&self) -> u64 {
        self.lock().revision
    }
    /** Whole-range copies never observe a partially written integer element. */
    pub(in crate::v9) fn read(&self, start: usize, length: usize) -> Option<Vec<u8>> {
        if self
            .visible
            .is_some_and(|visible| start.checked_add(length).is_none_or(|end| end > visible))
        {
            return None;
        }
        self.lock()
            .bytes
            .get(start..start.checked_add(length)?)
            .map(<[u8]>::to_vec)
    }
    /** A shared write is one lock-protected memory event and advances the snapshot revision. */
    pub(in crate::v9) fn write(&self, start: usize, bytes: &[u8]) -> bool {
        let mut state = self.lock();
        let Some(end) = start.checked_add(bytes.len()) else {
            return false;
        };
        if self.visible.is_some_and(|visible| end > visible) {
            return false;
        }
        let Some(target) = state.bytes.get_mut(start..end) else {
            return false;
        };
        target.copy_from_slice(bytes);
        state.revision = state.revision.wrapping_add(1);
        true
    }
    /** Byte-store and shared-GC atomics acquire one common order before locking their backing. */
    pub(in crate::v9) fn ordered<T>(step: impl FnOnce() -> T) -> T {
        let _order = SEQUENTIAL_ATOMICS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        step()
    }
    /** A fence participates in the same global order as every shared atomic memory event. */
    pub(in crate::v9) fn fence() {
        Self::ordered(|| std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst));
    }
    /** Atomic read/modify/write locks the global order before its backing; no author code runs here. */
    pub(in crate::v9) fn atomic<T>(
        &self,
        start: usize,
        length: usize,
        write: bool,
        operation: impl FnOnce(&mut [u8]) -> T,
    ) -> Option<T> {
        Self::ordered(|| {
            let mut state = self.lock();
            let result = operation(state.bytes.get_mut(start..start.checked_add(length)?)?);
            if write {
                state.revision = state.revision.wrapping_add(1)
            }
            Some(result)
        })
    }
    /** Growth only appends zero bytes; existing views and waiter locations keep the same backing. */
    pub(in crate::v9) fn grow(&self, length: usize) -> bool {
        if self
            .logical_maximum
            .is_some_and(|maximum| length > maximum || length % WASM_PAGE_BYTES != 0)
        {
            return false;
        }
        let mut state = self.lock();
        if self.visible.is_some()
            || state.maximum.is_none_or(|max| length > max)
            || length < state.bytes.len()
        {
            return false;
        }
        state.bytes.resize(length, 0);
        state.revision = state.revision.wrapping_add(1);
        true
    }
    /** Wasm memory growth adds pages atomically so simultaneous workers cannot lose an increment. */
    pub(in crate::v9) fn grow_by(&self, delta: usize, maximum: usize) -> Option<usize> {
        let mut state = self.lock();
        let old = state.bytes.len();
        let length = old.checked_add(delta)?;
        if self.visible.is_some()
            || length > maximum
            || state.maximum.is_none_or(|max| length > max)
        {
            return None;
        }
        state.bytes.resize(length, 0);
        state.revision = state.revision.wrapping_add(1);
        Some(old)
    }

    /** Native state transport already owns the actual Arc and needs no snapshot replacement. */
    pub(in crate::v9) fn restored(&self) -> bool {
        self.restored
    }
}
impl std::fmt::Debug for SharedBytes {
    /** Debug output reports bounded metadata, never a process pointer or wait queue. */
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedBytes")
            .field("id", &self.identity())
            .field("length", &self.len())
            .finish()
    }
}
