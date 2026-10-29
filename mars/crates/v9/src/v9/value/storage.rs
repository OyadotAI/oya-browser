#[cfg(feature = "conformance")]
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize};
use std::sync::atomic::{AtomicU64, Ordering};
/** Bounded immutable UTF-16 concatenations flatten in bulk and publish stable slices before releasing child storage. */
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};

/** Completed cache publications invalidate weak VM allocation observations. */
static MATERIALIZATION_EPOCH: AtomicU64 = AtomicU64::new(0);

/** Read the epoch after observing allocations to detect concurrent publication. */
pub(in crate::v9) fn materialization_epoch() -> u64 {
    MATERIALIZATION_EPOCH.load(Ordering::Acquire)
}

/** Publish a new epoch only after the backing resource is visible. */
fn materialized() {
    MATERIALIZATION_EPOCH.fetch_add(1, Ordering::Release);
}

/** Short strings keep their original single flat allocation and efficient bulk-copy concatenation. */
#[cfg(not(feature = "conformance"))]
const FLAT_CONCAT_UNITS: usize = 1_024;
/** V8 uses real cons strings from its minimum non-flat length. */
#[cfg(feature = "conformance")]
const FLAT_CONCAT_UNITS: usize = 12;
/** Neither recursive destruction nor a balanced concat tree can grow without an independent bound. */
const MAX_CONCAT_DEPTH: u8 = 32;
/** Count occurrences, including repeated DAG children, so shared subtrees cannot evade this bound. */
const MAX_CONCAT_NODES: u8 = 32;
/** External buffers are bounded independently of ordinary VM logical string limits. */
#[cfg(feature = "conformance")]
const EXTERNAL_STORAGE_BYTES: usize = 32 * 1024 * 1024;
/** Cached external objects need native pointer space on this uncompressed 64-bit target. */
#[cfg(feature = "conformance")]
const EXTERNAL_ONE_BYTE_CACHED_LENGTH: usize = 9;
#[cfg(feature = "conformance")]
const EXTERNAL_TWO_BYTE_CACHED_LENGTH: usize = 5;
#[cfg(feature = "conformance")]
static EXTERNAL_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "conformance")]
static NEXT_STRING_ID: AtomicU64 = AtomicU64::new(1);

/** Flat strings preserve their old allocation; only deferred concatenations allocate a node. */
#[derive(Clone)]
pub(super) enum Storage {
    #[cfg(not(feature = "conformance"))]
    Flat(Arc<[u16]>),
    #[cfg(feature = "conformance")]
    Flat(Arc<FlatData>),
    #[cfg(feature = "conformance")]
    Shared(Arc<FlatData>),
    Cons(Arc<Concat>),
    Repeat(Arc<Repeat>),
    #[cfg(feature = "conformance")]
    Slice(Arc<Slice>),
}

/** A weak node registration observes later cache publication without keeping strings alive. */
#[derive(Clone, Debug)]
pub(super) enum WeakStorage {
    #[cfg(not(feature = "conformance"))]
    Flat(Weak<[u16]>),
    #[cfg(feature = "conformance")]
    Flat(Weak<FlatData>),
    #[cfg(feature = "conformance")]
    Shared(Weak<FlatData>),
    Cons(Weak<Concat>),
    Repeat(Weak<Repeat>),
    #[cfg(feature = "conformance")]
    Slice(Weak<Slice>),
}

impl WeakStorage {
    /** Upgrade only for one observation, so the registry cannot extend storage lifetime. */
    pub(super) fn upgrade(&self) -> Option<Storage> {
        match self {
            Self::Flat(value) => value.upgrade().map(Storage::Flat),
            #[cfg(feature = "conformance")]
            Self::Shared(value) => value.upgrade().map(Storage::Shared),
            Self::Cons(value) => value.upgrade().map(Storage::Cons),
            Self::Repeat(value) => value.upgrade().map(Storage::Repeat),
            #[cfg(feature = "conformance")]
            Self::Slice(value) => value.upgrade().map(Storage::Slice),
        }
    }
}

/** An external allocation is published once and lives as long as any alias of the string. */
#[cfg(feature = "conformance")]
pub(super) struct FlatData {
    pub(super) units: Arc<[u16]>,
    pub(super) one_byte: bool,
    pub(super) external: ExternalSlot,
    pub(super) old: AtomicBool,
    pub(super) survivals: AtomicU8,
    pub(super) id: u64,
    pub(super) interned: AtomicBool,
    pub(super) read_only: bool,
    pub(super) forward: OnceLock<Storage>,
}
/** Synchronize publication so competing aliases do not consume duplicate quota. */
#[cfg(feature = "conformance")]
#[derive(Default)]
pub(super) struct ExternalSlot {
    value: Mutex<Option<Arc<External>>>,
}
#[cfg(feature = "conformance")]
impl ExternalSlot {
    pub(super) fn get(&self) -> Option<Arc<External>> {
        self.value
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    fn take(&self) -> Option<Arc<External>> {
        self.value
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }
}
/** The host owns the exact external encoding and its immutable UTF-16 view. */
#[cfg(feature = "conformance")]
pub(super) struct External {
    pub(super) one_byte: Option<Arc<[u8]>>,
    pub(super) units: Arc<[u16]>,
    reserved: usize,
}
#[cfg(feature = "conformance")]
impl External {
    fn new(units: &[u16], one_byte: bool) -> crate::protocol::EngineResult<Self> {
        let size = units
            .len()
            .checked_mul(if one_byte { 3 } else { 2 })
            .ok_or_else(|| {
                crate::protocol::EngineError::limit("external string bytes", EXTERNAL_STORAGE_BYTES)
            })?;
        reserve_external(&EXTERNAL_ALLOCATED, size, EXTERNAL_STORAGE_BYTES)?;
        let one_byte = one_byte.then(|| {
            units
                .iter()
                .map(|unit| *unit as u8)
                .collect::<Vec<_>>()
                .into()
        });
        Ok(Self {
            one_byte,
            units: Arc::from(units),
            reserved: size,
        })
    }
}
/** Admission uses one atomic update so concurrent publications cannot exceed the budget. */
#[cfg(feature = "conformance")]
fn reserve_external(
    counter: &AtomicUsize,
    size: usize,
    limit: usize,
) -> crate::protocol::EngineResult<()> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current.checked_add(size).filter(|next| *next <= limit)
        })
        .map(|_| ())
        .map_err(|_| crate::protocol::EngineError::limit("external string bytes", limit))
}
#[cfg(feature = "conformance")]
impl Drop for External {
    fn drop(&mut self) {
        EXTERNAL_ALLOCATED.fetch_sub(self.reserved, Ordering::AcqRel);
    }
}
#[cfg(feature = "conformance")]
impl std::ops::Deref for FlatData {
    type Target = [u16];
    fn deref(&self) -> &[u16] {
        &self.units
    }
}

/** Repetition retains one bounded child and materializes only when a slice consumer demands it. */
pub(super) struct Repeat {
    length: usize,
    count: usize,
    depth: u8,
    nodes: u8,
    child: Storage,
    flat: OnceLock<Arc<[u16]>>,
    #[cfg(feature = "conformance")]
    flat_identity: OnceLock<Arc<FlatData>>,
    #[cfg(feature = "conformance")]
    external: ExternalSlot,
    #[cfg(feature = "conformance")]
    old: AtomicBool,
    #[cfg(feature = "conformance")]
    survivals: AtomicU8,
    #[cfg(feature = "conformance")]
    id: u64,
    #[cfg(feature = "conformance")]
    interned: AtomicBool,
    #[cfg(feature = "conformance")]
    forward: OnceLock<Storage>,
}

/** A sliced string retains its parent and exposes a bounded suffix without copying. */
#[cfg(feature = "conformance")]
pub(super) struct Slice {
    parent: Storage,
    start: usize,
    length: usize,
    flat: OnceLock<Arc<[u16]>>,
    external: ExternalSlot,
    old: AtomicBool,
    survivals: AtomicU8,
    id: u64,
    interned: AtomicBool,
    forward: OnceLock<Storage>,
}

/** Logical content never changes; flattening only replaces child retention with its published equivalent buffer. */
pub(super) struct Concat {
    length: usize,
    depth: u8,
    nodes: u8,
    flat: OnceLock<Arc<[u16]>>,
    #[cfg(feature = "conformance")]
    flat_identity: OnceLock<Arc<FlatData>>,
    #[cfg(feature = "conformance")]
    external: ExternalSlot,
    #[cfg(feature = "conformance")]
    old: AtomicBool,
    #[cfg(feature = "conformance")]
    survivals: AtomicU8,
    #[cfg(feature = "conformance")]
    id: u64,
    #[cfg(feature = "conformance")]
    interned: AtomicBool,
    #[cfg(feature = "conformance")]
    forward: OnceLock<Storage>,
    children: Mutex<Option<(Storage, Storage)>>,
}

impl Storage {
    /** Register this node by allocation identity without retaining its backing. */
    pub(super) fn downgrade(&self) -> ((u8, usize), WeakStorage) {
        match self {
            Self::Flat(value) => (
                (0, Arc::as_ptr(value) as *const () as usize),
                WeakStorage::Flat(Arc::downgrade(value)),
            ),
            #[cfg(feature = "conformance")]
            Self::Shared(value) => (
                (1, Arc::as_ptr(value) as usize),
                WeakStorage::Shared(Arc::downgrade(value)),
            ),
            Self::Cons(value) => (
                (2, Arc::as_ptr(value) as usize),
                WeakStorage::Cons(Arc::downgrade(value)),
            ),
            Self::Repeat(value) => (
                (3, Arc::as_ptr(value) as usize),
                WeakStorage::Repeat(Arc::downgrade(value)),
            ),
            #[cfg(feature = "conformance")]
            Self::Slice(value) => (
                (4, Arc::as_ptr(value) as usize),
                WeakStorage::Slice(Arc::downgrade(value)),
            ),
        }
    }
    #[cfg(feature = "conformance")]
    pub(super) fn snapshot_id(&self) -> u64 {
        match self {
            Self::Flat(cell) | Self::Shared(cell) => cell.id,
            Self::Cons(node) => node.id,
            Self::Repeat(node) => node.id,
            Self::Slice(node) => node.id,
        }
    }
    #[cfg(feature = "conformance")]
    pub(super) fn next_id() -> u64 {
        NEXT_STRING_ID.fetch_add(1, Ordering::Relaxed)
    }
    /** Native allocation identity deduplicates aliases during one collection. */
    #[cfg(feature = "conformance")]
    pub(super) fn identity(&self) -> usize {
        match self {
            Self::Flat(cell) | Self::Shared(cell) => Arc::as_ptr(cell) as usize,
            Self::Cons(node) => Arc::as_ptr(node) as usize,
            Self::Repeat(node) => Arc::as_ptr(node) as usize,
            Self::Slice(node) => Arc::as_ptr(node) as usize,
        }
    }
    /** A collected or explicitly prepared string retains old-generation eligibility. */
    #[cfg(feature = "conformance")]
    pub(super) fn mark_old(&self) {
        match self {
            Self::Flat(cell) | Self::Shared(cell) => cell.old.store(true, Ordering::Release),
            Self::Cons(node) => node.old.store(true, Ordering::Release),
            Self::Repeat(node) => node.old.store(true, Ordering::Release),
            Self::Slice(node) => node.old.store(true, Ordering::Release),
        }
    }
    #[cfg(feature = "conformance")]
    pub(super) fn is_old(&self) -> bool {
        match self {
            Self::Flat(cell) | Self::Shared(cell) => cell.old.load(Ordering::Acquire),
            Self::Cons(node) => node.old.load(Ordering::Acquire),
            Self::Repeat(node) => node.old.load(Ordering::Acquire),
            Self::Slice(node) => node.old.load(Ordering::Acquire),
        }
    }
    #[cfg(feature = "conformance")]
    pub(super) fn survived_minor(&self) {
        if self.is_old() {
            return;
        }
        let count = match self {
            Self::Flat(cell) | Self::Shared(cell) => &cell.survivals,
            Self::Cons(node) => &node.survivals,
            Self::Repeat(node) => &node.survivals,
            Self::Slice(node) => &node.survivals,
        };
        if count.fetch_add(1, Ordering::AcqRel) >= 1 {
            self.mark_old();
        }
    }
    #[cfg(feature = "conformance")]
    pub(super) fn survived_once(&self) -> bool {
        match self {
            Self::Flat(cell) | Self::Shared(cell) => cell.survivals.load(Ordering::Acquire) > 0,
            Self::Cons(node) => node.survivals.load(Ordering::Acquire) > 0,
            Self::Repeat(node) => node.survivals.load(Ordering::Acquire) > 0,
            Self::Slice(node) => node.survivals.load(Ordering::Acquire) > 0,
        }
    }
    /** Trace retained string nodes once per actual collection, including cached flat children. */
    #[cfg(feature = "conformance")]
    pub(super) fn age_reachable(&self, major: bool, seen: &mut std::collections::BTreeSet<usize>) {
        let mut pending = vec![self.clone()];
        while let Some(value) = pending.pop() {
            if !seen.insert(value.identity()) {
                continue;
            }
            if let Some(target) = value.forwarding_target() {
                pending.push(target);
            }
            if major {
                value.mark_old();
            } else {
                value.survived_minor();
            }
            match value {
                Self::Slice(node) => pending.push(node.parent.clone()),
                Self::Repeat(node) => {
                    pending.push(node.child.clone());
                    if let Some(flat) = node.flat_identity.get() {
                        pending.push(Self::Flat(flat.clone()));
                    }
                }
                Self::Cons(node) => {
                    if let Some((left, right)) = node.children().clone() {
                        pending.extend([left, right]);
                    }
                    if let Some(flat) = node.flat_identity.get() {
                        pending.push(Self::Flat(flat.clone()));
                    }
                }
                Self::Flat(_) | Self::Shared(_) => {}
            }
        }
    }
    /** A thin string holds its canonical target strongly, preserving identity of the original cell. */
    #[cfg(feature = "conformance")]
    fn forwarding_slot(&self) -> Option<&OnceLock<Storage>> {
        match self {
            Self::Flat(cell) => Some(&cell.forward),
            Self::Cons(node) => Some(&node.forward),
            Self::Repeat(node) => Some(&node.forward),
            Self::Slice(node) => Some(&node.forward),
            Self::Shared(_) => None,
        }
    }
    #[cfg(feature = "conformance")]
    pub(super) fn forwarding_target(&self) -> Option<Self> {
        self.forwarding_slot()?.get().cloned()
    }
    #[cfg(feature = "conformance")]
    pub(super) fn forward_to(&self, target: &Self) -> bool {
        if self.is_read_only_root() {
            return self.same(target);
        }
        let Some(slot) = self.forwarding_slot() else {
            return false;
        };
        if !target.interned() || target.forwarding_target().is_some() {
            return false;
        }
        if self.same(target) {
            return true;
        }
        if let Some(existing) = slot.get() {
            return existing.same(target);
        }
        let published = match slot.set(target.clone()) {
            Ok(()) => true,
            Err(_) => slot.get().is_some_and(|existing| existing.same(target)),
        };
        if published {
            let external = match self {
                Self::Flat(cell) => &cell.external,
                Self::Cons(node) => &node.external,
                Self::Repeat(node) => &node.external,
                Self::Slice(node) => &node.external,
                Self::Shared(_) => return false,
            };
            if external.take().is_some() {
                materialized();
            }
        }
        published
    }
    #[cfg(feature = "conformance")]
    pub(super) fn interned(&self) -> bool {
        match self {
            Self::Flat(cell) | Self::Shared(cell) => &cell.interned,
            Self::Cons(node) => &node.interned,
            Self::Repeat(node) => &node.interned,
            Self::Slice(node) => &node.interned,
        }
        .load(Ordering::Acquire)
    }
    #[cfg(feature = "conformance")]
    pub(super) fn mark_interned(&self) -> bool {
        if self.forwarding_target().is_some()
            || (!matches!(self, Self::Flat(_) | Self::Shared(_)) && !self.is_external())
        {
            return false;
        }
        let flag = match self {
            Self::Flat(cell) | Self::Shared(cell) => &cell.interned,
            Self::Cons(node) => &node.interned,
            Self::Repeat(node) => &node.interned,
            Self::Slice(node) => &node.interned,
        };
        self.mark_old();
        flag.store(true, Ordering::Release);
        true
    }
    #[cfg(feature = "conformance")]
    pub(super) fn can_intern_in_place(&self) -> bool {
        if self.is_external() {
            self.external_is_cached()
        } else {
            matches!(self, Self::Flat(_) | Self::Shared(_))
        }
    }
    #[cfg(feature = "conformance")]
    fn external_is_cached(&self) -> bool {
        let minimum = if self.one_byte_representation() {
            EXTERNAL_ONE_BYTE_CACHED_LENGTH
        } else {
            EXTERNAL_TWO_BYTE_CACHED_LENGTH
        };
        self.len() >= minimum
    }
    #[cfg(feature = "conformance")]
    pub(super) fn forwarded_one_byte(&self) -> Option<bool> {
        self.forwarding_target()
            .map(|target| target.one_byte_representation())
    }
    /** Rope representation follows actual child encodings, not just code-unit values. */
    #[cfg(feature = "conformance")]
    fn one_byte_representation(&self) -> bool {
        if let Some(target) = self.forwarding_target() {
            return target.one_byte_representation();
        }
        match self {
            Self::Flat(cell) | Self::Shared(cell) => cell.one_byte,
            Self::Repeat(node) => node.child.one_byte_representation(),
            Self::Slice(node) => node.parent.one_byte_representation(),
            Self::Cons(node) => node.children().as_ref().map_or_else(
                || {
                    node.flat.get().is_some_and(|units| {
                        units.iter().all(|unit| *unit <= super::LATIN1_MAX_UNIT)
                    })
                },
                |(left, right)| left.one_byte_representation() && right.one_byte_representation(),
            ),
        }
    }
    /** Expose contiguous data without flattening a compact rope. */
    pub(super) fn flat_units(&self) -> Option<&[u16]> {
        match self {
            Self::Flat(units) => Some(units),
            #[cfg(feature = "conformance")]
            Self::Shared(units) => Some(units),
            Self::Cons(node) => node.flat.get().map(AsRef::as_ref),
            #[cfg(feature = "conformance")]
            Self::Slice(node) => node.flat.get().map(AsRef::as_ref).or_else(|| {
                node.parent
                    .flat_units()
                    .map(|units| &units[node.start..node.start + node.length])
            }),
            Self::Repeat(node) => node.flat.get().map(AsRef::as_ref),
        }
    }
    /** New cells preserve alias identity while allowing the backing to be externalized once. */
    pub(super) fn flat(units: Arc<[u16]>) -> Self {
        #[cfg(feature = "conformance")]
        {
            let one_byte = units.iter().all(|unit| *unit <= super::LATIN1_MAX_UNIT);
            Self::flat_encoded(units, one_byte)
        }
        #[cfg(not(feature = "conformance"))]
        {
            Self::Flat(units)
        }
    }

    #[cfg(feature = "conformance")]
    pub(super) fn flat_encoded(units: Arc<[u16]>, one_byte: bool) -> Self {
        Self::Flat(Arc::new(FlatData {
            units,
            one_byte,
            external: ExternalSlot::default(),
            old: AtomicBool::new(false),
            survivals: AtomicU8::new(0),
            id: Self::next_id(),
            interned: AtomicBool::new(false),
            read_only: false,
            forward: OnceLock::new(),
        }))
    }

    #[cfg(feature = "conformance")]
    pub(super) fn shared(units: Arc<[u16]>, one_byte: bool) -> Self {
        Self::Shared(Arc::new(FlatData {
            units,
            one_byte,
            external: ExternalSlot::default(),
            old: AtomicBool::new(true),
            survivals: AtomicU8::new(0),
            id: Self::next_id(),
            interned: AtomicBool::new(false),
            read_only: false,
            forward: OnceLock::new(),
        }))
    }

    /** A permanent one-byte root has immutable backing and starts old and internalized. */
    #[cfg(feature = "conformance")]
    pub(super) fn character_root(unit: u8) -> Self {
        Self::Flat(Arc::new(FlatData {
            units: Arc::from([u16::from(unit)]),
            one_byte: true,
            external: ExternalSlot::default(),
            old: AtomicBool::new(true),
            survivals: AtomicU8::new(0),
            id: Self::next_id(),
            interned: AtomicBool::new(true),
            read_only: true,
            forward: OnceLock::new(),
        }))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn is_read_only_root(&self) -> bool {
        matches!(self, Self::Flat(cell) if cell.read_only)
    }

    /** Publish an actual owned external allocation without changing code units or alias identity. */
    #[cfg(feature = "conformance")]
    pub(super) fn externalize(&self, one_byte: bool) -> crate::protocol::EngineResult<()> {
        if self.is_read_only_root() {
            return Err(crate::protocol::EngineError::invalid(
                "Cannot externalize a read-only string.",
            ));
        }
        if let Some(target) = self.forwarding_target() {
            return target.externalize(one_byte);
        }
        let target = match self {
            Self::Flat(cell) | Self::Shared(cell) => &cell.external,
            Self::Cons(node) => &node.external,
            Self::Repeat(node) => &node.external,
            Self::Slice(node) => &node.external,
        };
        let mut slot = target
            .value
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot.is_some() {
            return Ok(());
        }
        let resource = External::new(self.units(), one_byte)?;
        *slot = Some(Arc::new(resource));
        drop(slot);
        materialized();
        if let Some(target) = self.forwarding_target() {
            target.externalize(target.one_byte_representation())?;
        }
        Ok(())
    }

    #[cfg(feature = "conformance")]
    pub(super) fn is_external(&self) -> bool {
        if let Some(target) = self.forwarding_target() {
            return target.is_external();
        }
        match self {
            Self::Flat(cell) | Self::Shared(cell) => cell.external.get().is_some(),
            Self::Cons(node) => node.external.get().is_some(),
            Self::Repeat(node) => node.external.get().is_some(),
            Self::Slice(node) => node.external.get().is_some(),
        }
    }
    /** Preserve concatenations only when they retain an unflattened compact repetition. */
    pub(super) fn compact_concat_parts(&self) -> Option<(Storage, Storage)> {
        let Self::Cons(node) = self else { return None };
        if node.flat.get().is_some() {
            return None;
        }
        let (left, right) = node.children().clone()?;
        (left.has_compact_repeat() || right.has_compact_repeat()).then_some((left, right))
    }

    /** Inspect a bounded rope without materializing its logical repeated content. */
    fn has_compact_repeat(&self) -> bool {
        let mut pending = vec![self.clone()];
        while let Some(item) = pending.pop() {
            match item {
                Self::Repeat(node) if node.flat.get().is_none() => return true,
                Self::Cons(node) if node.flat.get().is_none() => {
                    if let Some((left, right)) = node.children().clone() {
                        pending.extend([left, right]);
                    }
                }
                _ => {}
            }
        }
        false
    }

    /** Determine each child's Latin-1 flag without allocating its concatenated contents. */
    pub(super) fn latin1(&self) -> bool {
        let mut pending = vec![self.clone()];
        while let Some(item) = pending.pop() {
            match item {
                Self::Flat(units) => {
                    if units.iter().any(|unit| *unit > super::LATIN1_MAX_UNIT) {
                        return false;
                    }
                }
                #[cfg(feature = "conformance")]
                Self::Shared(units) => {
                    if units.iter().any(|unit| *unit > super::LATIN1_MAX_UNIT) {
                        return false;
                    }
                }
                #[cfg(feature = "conformance")]
                Self::Slice(node) => {
                    if (0..node.length).any(|index| {
                        node.parent
                            .unit_at(node.start + index)
                            .is_some_and(|unit| unit > super::LATIN1_MAX_UNIT)
                    }) {
                        return false;
                    }
                }
                Self::Repeat(node) => pending.push(node.child.clone()),
                Self::Cons(node) => {
                    if let Some(flat) = node.flat.get() {
                        if flat.iter().any(|unit| *unit > super::LATIN1_MAX_UNIT) {
                            return false;
                        }
                    } else if let Some((left, right)) = node.children().clone() {
                        pending.extend([left, right]);
                    } else if node
                        .flat
                        .get()
                        .is_some_and(|flat| flat.iter().any(|unit| *unit > super::LATIN1_MAX_UNIT))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
    /** Wire snapshots may preserve a repeat only while its original compact child is retained. */
    pub(super) fn repeat_parts(&self) -> Option<(Storage, usize)> {
        match self {
            Self::Repeat(node) if node.flat.get().is_none() => {
                Some((node.child.clone(), node.count))
            }
            _ => None,
        }
    }
    /** Read a code unit through bounded rope nodes without flattening the whole string. */
    pub(super) fn unit_at(&self, mut index: usize) -> Option<u16> {
        let mut current = self.clone();
        loop {
            match current {
                Self::Flat(units) => return units.get(index).copied(),
                #[cfg(feature = "conformance")]
                Self::Shared(units) => return units.get(index).copied(),
                #[cfg(feature = "conformance")]
                Self::Slice(node) => {
                    if index >= node.length {
                        return None;
                    }
                    index += node.start;
                    current = node.parent.clone();
                }
                Self::Repeat(node) => {
                    if index >= node.length {
                        return None;
                    }
                    if let Some(flat) = node.flat.get() {
                        return flat.get(index).copied();
                    }
                    index %= node.child.len();
                    current = node.child.clone();
                }
                Self::Cons(node) => {
                    if let Some(flat) = node.flat.get() {
                        return flat.get(index).copied();
                    }
                    let children = node.children().clone();
                    let Some((left, right)) = children else {
                        return node.flat.get().and_then(|flat| flat.get(index).copied());
                    };
                    if index < left.len() {
                        current = left;
                    } else {
                        index -= left.len();
                        current = right;
                    }
                }
            }
        }
    }
    /** Cardinality never traverses or materializes a concatenation. */
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Flat(units) => units.len(),
            #[cfg(feature = "conformance")]
            Self::Shared(units) => units.len(),
            Self::Cons(node) => node.length,
            Self::Repeat(node) => node.length,
            #[cfg(feature = "conformance")]
            Self::Slice(node) => node.length,
        }
    }

    /** A cached allocation is immutable and remains borrowed for the owning string's entire lifetime. */
    pub(super) fn units(&self) -> &[u16] {
        match self {
            Self::Flat(units) => units,
            #[cfg(feature = "conformance")]
            Self::Shared(units) => units,
            #[cfg(feature = "conformance")]
            Self::Slice(node) => {
                if let Some(units) = node.parent.flat_units() {
                    return &units[node.start..node.start + node.length];
                }
                let missing = node.flat.get().is_none();
                let flat = node.flat.get_or_init(|| {
                    (0..node.length)
                        .map(|index| {
                            node.parent
                                .unit_at(node.start + index)
                                .expect("bounded slice")
                        })
                        .collect::<Vec<_>>()
                        .into()
                });
                if missing {
                    materialized();
                }
                flat
            }
            Self::Repeat(node) => {
                let missing = node.flat.get().is_none();
                let flat = node.flat.get_or_init(|| {
                    let mut units = Vec::with_capacity(node.length);
                    node.child.append_to(&mut units);
                    let width = units.len();
                    for _ in 1..node.count {
                        units.extend_from_within(..width);
                    }
                    units.into()
                });
                if missing {
                    materialized();
                }
                flat
            }
            Self::Cons(node) => {
                let missing = node.flat.get().is_none();
                let flat = node.flat.get_or_init(|| {
                    let mut units = Vec::with_capacity(node.length);
                    self.append_to(&mut units);
                    units.into()
                });
                if missing {
                    materialized();
                }
                // OnceLock publishes first. Any competing traversal that finds no children
                // can therefore read flat; no guard or child borrow escapes this statement.
                let children = node.children().take();
                drop(children);
                flat
            }
        }
    }

    /** Known equal allocations avoid content work without making shape or pointer identity observable. */
    pub(super) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Flat(left), Self::Flat(right)) => Arc::ptr_eq(left, right),
            #[cfg(feature = "conformance")]
            (Self::Shared(left), Self::Shared(right)) => Arc::ptr_eq(left, right),
            (Self::Cons(left), Self::Cons(right)) => Arc::ptr_eq(left, right),
            (Self::Repeat(left), Self::Repeat(right)) => Arc::ptr_eq(left, right),
            #[cfg(feature = "conformance")]
            (Self::Slice(left), Self::Slice(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    /** Admission of code-unit length occurs before this helper; representation bounds choose when to copy in bulk. */
    pub(super) fn concat_will_flatten(&self, other: &Self) -> bool {
        let left = self.canonical();
        let right = other.canonical();
        let (left_depth, left_nodes) = left.shape();
        let (right_depth, right_nodes) = right.shape();
        left.len().saturating_add(right.len()) <= FLAT_CONCAT_UNITS
            || left_depth.max(right_depth).saturating_add(1) > MAX_CONCAT_DEPTH
            || left_nodes.saturating_add(right_nodes).saturating_add(1) > MAX_CONCAT_NODES
    }
    pub(super) fn concat(&self, other: &Self) -> Self {
        let left = self.canonical();
        let right = other.canonical();
        let length = left.len() + right.len();
        let (left_depth, left_nodes) = left.shape();
        let (right_depth, right_nodes) = right.shape();
        let depth = left_depth.max(right_depth).saturating_add(1);
        let nodes = left_nodes.saturating_add(right_nodes).saturating_add(1);
        if left.concat_will_flatten(&right) {
            return Self::join_flat(&left, &right, length);
        }
        Self::Cons(Arc::new(Concat {
            length,
            depth,
            nodes,
            flat: OnceLock::new(),
            #[cfg(feature = "conformance")]
            flat_identity: OnceLock::new(),
            #[cfg(feature = "conformance")]
            external: ExternalSlot::default(),
            #[cfg(feature = "conformance")]
            old: AtomicBool::new(false),
            #[cfg(feature = "conformance")]
            survivals: AtomicU8::new(0),
            #[cfg(feature = "conformance")]
            id: Self::next_id(),
            #[cfg(feature = "conformance")]
            interned: AtomicBool::new(false),
            #[cfg(feature = "conformance")]
            forward: OnceLock::new(),
            children: Mutex::new(Some((left, right))),
        }))
    }

    /** Keep repeated storage compact while retaining the existing logical and depth bounds. */
    pub(super) fn repeat_will_flatten(&self, count: usize) -> bool {
        let length = self.len().saturating_mul(count);
        length != 0
            && (length <= FLAT_CONCAT_UNITS
                || self.shape().0 >= MAX_CONCAT_DEPTH
                || self.shape().1 >= MAX_CONCAT_NODES)
    }
    pub(super) fn repeat(&self, count: usize) -> Self {
        let length = self.len() * count;
        if length == 0 {
            return Self::flat(Arc::from([]));
        }
        if self.repeat_will_flatten(count) {
            return Self::flat(self.units().repeat(count).into());
        }
        let (depth, nodes) = self.shape();
        Self::Repeat(Arc::new(Repeat {
            length,
            count,
            depth: depth + 1,
            nodes: nodes.saturating_add(1),
            child: self.canonical(),
            flat: OnceLock::new(),
            #[cfg(feature = "conformance")]
            flat_identity: OnceLock::new(),
            #[cfg(feature = "conformance")]
            external: ExternalSlot::default(),
            #[cfg(feature = "conformance")]
            old: AtomicBool::new(false),
            #[cfg(feature = "conformance")]
            survivals: AtomicU8::new(0),
            #[cfg(feature = "conformance")]
            id: Self::next_id(),
            #[cfg(feature = "conformance")]
            interned: AtomicBool::new(false),
            #[cfg(feature = "conformance")]
            forward: OnceLock::new(),
        }))
    }

    /** Keep the selected suffix tied to its immutable parent and its distinct identity. */
    #[cfg(feature = "conformance")]
    pub(super) fn sliced(&self, start: usize) -> Option<Self> {
        if start == 0 || start >= self.len() || self.len() - start < 13 {
            return None;
        }
        self.sliced_range(start, self.len())
    }
    /** Retain an arbitrary nonempty range while bounding the number of rope nodes. */
    #[cfg(feature = "conformance")]
    pub(super) fn sliced_range(&self, start: usize, end: usize) -> Option<Self> {
        if start >= end || end > self.len() || end - start < 13 {
            return None;
        }
        if let Self::Slice(node) = self {
            return node
                .parent
                .sliced_range(node.start + start, node.start + end);
        }
        let (depth, nodes) = self.shape();
        if depth >= MAX_CONCAT_DEPTH || nodes >= MAX_CONCAT_NODES {
            return None;
        }
        Some(Self::Slice(Arc::new(Slice {
            parent: self.clone(),
            start,
            length: end - start,
            flat: OnceLock::new(),
            external: ExternalSlot::default(),
            old: AtomicBool::new(false),
            survivals: AtomicU8::new(0),
            id: Self::next_id(),
            interned: AtomicBool::new(false),
            forward: OnceLock::new(),
        })))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn slice_parts(&self) -> Option<(Self, usize, usize)> {
        match self {
            Self::Slice(node) if node.external.get().is_none() => {
                Some((node.parent.clone(), node.start, node.length))
            }
            _ => None,
        }
    }
    /** V8 copies a nonflat, nonempty-first cons as a cons when preparing externalization. */
    #[cfg(feature = "conformance")]
    pub(super) fn externalizable_cons_parts(&self) -> Option<(Self, Self)> {
        let Self::Cons(node) = self else { return None };
        if node.flat.get().is_some() || node.external.get().is_some() {
            return None;
        }
        let (left, right) = node.children().clone()?;
        (left.len() != 0).then_some((left, right))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn cons_parts(&self, one_byte: bool) -> Option<(Self, Self)> {
        let Self::Cons(node) = self else { return None };
        if let Some(parts) = node.children().clone() {
            return Some(parts);
        }
        node.flat.get()?;
        Some((Self::flat(Arc::from([])), self.flatten_intrinsic(one_byte)))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn actual_one_byte(&self) -> bool {
        self.one_byte_representation()
    }
    #[cfg(feature = "conformance")]
    pub(super) fn is_cons(&self) -> bool {
        matches!(self, Self::Cons(_))
    }
    /** Identity tags cannot equate different representations or different retained slice parents. */
    #[cfg(feature = "conformance")]
    pub(super) fn same_representation(&self, other: &Self) -> bool {
        match (self.forwarding_target(), other.forwarding_target()) {
            (Some(left), Some(right)) => return left.same(&right),
            (Some(_), None) | (None, Some(_)) => return false,
            _ => {}
        }
        match (self, other) {
            (Self::Flat(left), Self::Flat(right)) => left.read_only == right.read_only,
            (Self::Shared(_), Self::Shared(_))
            | (Self::Cons(_), Self::Cons(_))
            | (Self::Repeat(_), Self::Repeat(_)) => true,
            (Self::Slice(left), Self::Slice(right)) => {
                left.start == right.start
                    && left.length == right.length
                    && left.parent.same(&right.parent)
            }
            _ => false,
        }
    }

    #[cfg(feature = "conformance")]
    pub(super) fn needs_intrinsic_flatten(&self) -> bool {
        matches!(self, Self::Cons(_) | Self::Repeat(_)) && !self.is_external()
    }
    #[cfg(feature = "conformance")]
    pub(super) fn flattened_cons_child(&self, one_byte: bool) -> Option<Self> {
        let Self::Cons(node) = self else { return None };
        node.flat.get()?;
        Some(self.flatten_intrinsic(one_byte))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn restore_flattened_cons(&self, child: &Self) -> bool {
        let (Self::Cons(node), Self::Flat(cell)) = (self, child) else {
            return false;
        };
        if node.flat.set(cell.units.clone()).is_err()
            || node.flat_identity.set(cell.clone()).is_err()
        {
            return false;
        }
        node.children().take();
        materialized();
        true
    }
    #[cfg(feature = "conformance")]
    pub(super) fn flattened_repeat_parts(&self, one_byte: bool) -> Option<(Self, usize, Self)> {
        let Self::Repeat(node) = self else {
            return None;
        };
        node.flat.get()?;
        Some((
            node.child.clone(),
            node.count,
            self.flatten_intrinsic(one_byte),
        ))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn restore_flattened_repeat(&self, child: &Self) -> bool {
        let (Self::Repeat(node), Self::Flat(cell)) = (self, child) else {
            return false;
        };
        if node.flat.set(cell.units.clone()).is_err()
            || node.flat_identity.set(cell.clone()).is_err()
        {
            return false;
        }
        materialized();
        true
    }
    /** Flatten indirect strings to their stable first child without changing direct or sliced identity. */
    #[cfg(feature = "conformance")]
    pub(super) fn flatten_intrinsic(&self, one_byte: bool) -> Self {
        if let Some(target) = self.forwarding_target() {
            return target;
        }
        if self.is_external() {
            return self.clone();
        }
        let identity = match self {
            Self::Cons(node) => &node.flat_identity,
            Self::Repeat(node) => &node.flat_identity,
            _ => return self.clone(),
        };
        let _ = self.units();
        let missing = identity.get().is_none();
        let flat = identity.get_or_init(|| {
            let units = match self {
                Self::Cons(node) => node.flat.get().expect("flattened"),
                Self::Repeat(node) => node.flat.get().expect("flattened"),
                _ => unreachable!(),
            };
            Arc::new(FlatData {
                units: units.clone(),
                one_byte,
                external: ExternalSlot::default(),
                old: AtomicBool::new(false),
                survivals: AtomicU8::new(0),
                id: Self::next_id(),
                interned: AtomicBool::new(false),
                read_only: false,
                forward: OnceLock::new(),
            })
        });
        if missing {
            materialized();
        }
        Self::Flat(flat.clone())
    }

    /** Already flattened nodes become direct flat children instead of retaining historical graph depth. */
    fn canonical(&self) -> Self {
        match self {
            Self::Cons(node) if node.flat.get().is_some() => {
                Self::flat(node.flat.get().expect("published UTF-16").clone())
            }
            Self::Repeat(node) if node.flat.get().is_some() => {
                Self::flat(node.flat.get().expect("published UTF-16").clone())
            }
            _ => self.clone(),
        }
    }

    /** Cached heights may overestimate after compaction but can never admit an oversized tree. */
    fn shape(&self) -> (u8, u8) {
        match self {
            Self::Flat(_) => (0, 0),
            #[cfg(feature = "conformance")]
            Self::Shared(_) => (0, 0),
            Self::Cons(node) => (node.depth, node.nodes),
            Self::Repeat(node) => (node.depth, node.nodes),
            #[cfg(feature = "conformance")]
            Self::Slice(node) => {
                let (depth, nodes) = node.parent.shape();
                (depth.saturating_add(1), nodes.saturating_add(1))
            }
        }
    }

    /** Keep the measured efficient bulk-copy path; iterator collection is deliberately avoided. */
    fn join_flat(left: &Self, right: &Self, length: usize) -> Self {
        let mut units = Vec::with_capacity(length);
        left.append_to(&mut units);
        right.append_to(&mut units);
        Self::flat(units.into())
    }

    /** Flatten leaves iteratively; initializing every child cache would retain copies of every prefix. */
    fn append_to(&self, units: &mut Vec<u16>) {
        if let Self::Flat(flat) = self {
            units.extend_from_slice(flat);
            return;
        }
        let mut pending = vec![self.clone()];
        while let Some(storage) = pending.pop() {
            match storage {
                Self::Flat(flat) => units.extend_from_slice(&flat),
                #[cfg(feature = "conformance")]
                Self::Shared(flat) => units.extend_from_slice(&flat),
                #[cfg(feature = "conformance")]
                Self::Slice(node) => units.extend((0..node.length).map(|index| {
                    node.parent
                        .unit_at(node.start + index)
                        .expect("bounded slice")
                })),
                Self::Repeat(node) => {
                    if let Some(flat) = node.flat.get() {
                        units.extend_from_slice(flat);
                    } else {
                        let start = units.len();
                        node.child.append_to(units);
                        let end = units.len();
                        for _ in 1..node.count {
                            units.extend_from_within(start..end);
                        }
                    }
                }
                Self::Cons(node) => {
                    if let Some(flat) = node.flat.get() {
                        units.extend_from_slice(flat);
                    } else if let Some((left, right)) = node.children().clone() {
                        pending.push(right);
                        pending.push(left);
                    } else {
                        // Compaction takes children only after publishing flat. The mutex
                        // acquisition synchronizes this recheck with the other thread.
                        units.extend_from_slice(node.flat.get().expect("compacted UTF-16"));
                    }
                }
            }
        }
    }

    /** Report real allocations without flattening; the caller retains content sharing and context attribution. */
    pub(super) fn allocations(&self, mut visit: impl FnMut(bool, usize, usize, usize)) {
        let mut pending = vec![self.clone()];
        while let Some(storage) = pending.pop() {
            #[cfg(feature = "conformance")]
            if let Some(target) = storage.forwarding_target() {
                pending.push(target);
            }
            match storage {
                #[cfg(not(feature = "conformance"))]
                Self::Flat(flat) => report_flat(&flat, &mut visit),
                #[cfg(feature = "conformance")]
                Self::Flat(flat) | Self::Shared(flat) => {
                    let bytes = std::mem::size_of::<FlatData>() + ARC_HEADER_BYTES;
                    visit(
                        true,
                        Arc::as_ptr(&flat) as usize,
                        bytes,
                        bytes + ARC_ALIGNMENT_SLACK,
                    );
                    report_flat(&flat.units, &mut visit);
                    if let Some(external) = flat.external.get() {
                        report_external(&external, &mut visit);
                    }
                }
                #[cfg(feature = "conformance")]
                Self::Slice(node) => {
                    let bytes = std::mem::size_of::<Slice>() + ARC_HEADER_BYTES;
                    visit(
                        true,
                        Arc::as_ptr(&node) as usize,
                        bytes,
                        bytes + ARC_ALIGNMENT_SLACK,
                    );
                    if let Some(flat) = node.flat.get() {
                        report_flat(flat, &mut visit);
                    }
                    if let Some(external) = node.external.get() {
                        report_external(&external, &mut visit);
                    }
                    pending.push(node.parent.clone());
                }
                Self::Repeat(node) => {
                    let bytes = std::mem::size_of::<Repeat>() + ARC_HEADER_BYTES;
                    visit(
                        true,
                        Arc::as_ptr(&node) as usize,
                        bytes,
                        bytes + ARC_ALIGNMENT_SLACK,
                    );
                    if let Some(flat) = node.flat.get() {
                        report_flat(flat, &mut visit);
                    }
                    #[cfg(feature = "conformance")]
                    if let Some(flat) = node.flat_identity.get() {
                        pending.push(Self::Flat(flat.clone()));
                    }
                    #[cfg(feature = "conformance")]
                    if let Some(external) = node.external.get() {
                        report_external(&external, &mut visit);
                    }
                    pending.push(node.child.clone());
                }
                Self::Cons(node) => {
                    let bytes = std::mem::size_of::<Concat>() + ARC_HEADER_BYTES;
                    visit(
                        true,
                        Arc::as_ptr(&node) as usize,
                        bytes,
                        bytes + ARC_ALIGNMENT_SLACK,
                    );
                    let (children, flat) = {
                        let children = node.children();
                        (children.clone(), node.flat.get().cloned())
                    };
                    if let Some(flat) = flat {
                        report_flat(&flat, &mut visit);
                    }
                    #[cfg(feature = "conformance")]
                    if let Some(flat) = node.flat_identity.get() {
                        pending.push(Self::Flat(flat.clone()));
                    }
                    #[cfg(feature = "conformance")]
                    if let Some(external) = node.external.get() {
                        report_external(&external, &mut visit);
                    }
                    if let Some((left, right)) = children {
                        pending.extend([right, left]);
                    }
                }
            }
        }
    }
}

impl Concat {
    /** No author code runs under this lock; poisoning cannot change the immutable child contents. */
    fn children(&self) -> MutexGuard<'_, Option<(Storage, Storage)>> {
        self.children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/** Arc holds strong and weak counters beside its payload; allocator bookkeeping is excluded like other estimates. */
const ARC_HEADER_BYTES: usize = 2 * std::mem::size_of::<usize>();
/** Arc pads its payload to the counter alignment. */
const ARC_ALIGNMENT_SLACK: usize = std::mem::align_of::<usize>() - 1;

/** Flat buffers and node allocations have separate identity namespaces in the memory tracker. */
fn report_flat(flat: &Arc<[u16]>, visit: &mut impl FnMut(bool, usize, usize, usize)) {
    let bytes = flat.len() * std::mem::size_of::<u16>() + ARC_HEADER_BYTES;
    visit(
        false,
        flat.as_ptr() as usize,
        bytes,
        bytes + ARC_ALIGNMENT_SLACK,
    );
}

/** Account for both external bytes and their immutable UTF-16 view. */
#[cfg(feature = "conformance")]
fn report_external(external: &Arc<External>, visit: &mut impl FnMut(bool, usize, usize, usize)) {
    let overhead = std::mem::size_of::<External>() + ARC_HEADER_BYTES;
    visit(
        true,
        Arc::as_ptr(external) as usize,
        overhead,
        overhead + ARC_ALIGNMENT_SLACK,
    );
    report_flat(&external.units, visit);
    if let Some(bytes) = &external.one_byte {
        let size = bytes.len() + ARC_HEADER_BYTES;
        visit(
            false,
            bytes.as_ptr() as usize,
            size,
            size + ARC_ALIGNMENT_SLACK,
        );
    }
}

#[cfg(all(test, feature = "conformance"))]
mod tests {
    /** Read-only character roots retain their explicit brand across standalone wire transport. */
    #[cfg(feature = "conformance")]
    #[test]
    fn character_root_is_old_internalized_and_cannot_be_externalized() {
        let value = super::super::JsString::character_root(b'a');
        assert!(value.is_read_only_root() && value.is_old() && value.is_interned());
        assert!(value.externalize().is_err());
        let wire = serde_json::to_string(&value).unwrap();
        let restored: super::super::JsString = serde_json::from_str(&wire).unwrap();
        assert!(restored.is_read_only_root() && restored.is_old() && restored.is_interned());
        assert_eq!(restored.unit_at(0), Some(u16::from(b'a')));
    }
    use super::*;
    use std::collections::BTreeMap;

    /** Externalization publishes a distinct allocation shared by all aliases and releases it with the last alias. */
    #[test]
    fn external_storage_is_real_shared_and_lifetime_bounded() {
        let value = Storage::flat(Arc::from([65, 0xd800, 66]));
        let alias = value.clone();
        value.externalize(false).unwrap();
        assert!(alias.is_external());
        let Storage::Flat(cell) = &value else {
            panic!("flat")
        };
        let external = cell.external.get().unwrap();
        assert!(!Arc::ptr_eq(&cell.units, &external.units));
        assert_eq!(alias.units(), &[65, 0xd800, 66]);
        let weak = Arc::downgrade(&external.units);
        drop(external);
        drop(value);
        assert!(weak.upgrade().is_some());
        drop(alias);
        assert!(weak.upgrade().is_none());
    }

    /** Reservations remain bounded and are released when external resources die. */
    #[test]
    fn external_storage_quota_is_bounded_and_released() {
        let counter = AtomicUsize::new(0);
        reserve_external(&counter, 6, 10).unwrap();
        assert_eq!(
            reserve_external(&counter, 5, 10).unwrap_err().code,
            "limit_exceeded"
        );
        assert_eq!(counter.load(Ordering::Acquire), 6);
        counter.fetch_sub(6, Ordering::AcqRel);
        reserve_external(&counter, 10, 10).unwrap();
    }

    /** Create a real unobserved concat graph, without accidentally initializing its flat cache during setup. */
    fn tree() -> Storage {
        let mut value = Storage::flat(vec![u16::from(b'x'); 2_048].into());
        let tail = Storage::flat(vec![u16::from(b'y')].into());
        for _ in 0..24 {
            value = value.concat(&tail)
        }
        value
    }

    /** Indexed reads traverse the actual rope and never allocate a full flatten buffer. */
    #[test]
    fn indexed_reads_do_not_flatten_rope_storage() {
        let value = tree();
        let Storage::Cons(node) = &value else {
            panic!("expected a rope")
        };
        assert_eq!(value.unit_at(0), Some(u16::from(b'x')));
        assert_eq!(value.unit_at(2_048), Some(u16::from(b'y')));
        assert_eq!(value.unit_at(value.len()), None);
        assert!(node.flat.get().is_none());
    }

    /** Large repetitions retain one child and flatten only for contiguous slice consumers. */
    #[test]
    fn repetition_shares_bounded_backing_until_flattened() {
        let child = Storage::flat(vec![65, 66].into());
        let repeated = child.repeat(20_000);
        let Storage::Repeat(node) = &repeated else {
            panic!("expected compact repeat")
        };
        assert_eq!(repeated.len(), 40_000);
        assert_eq!(repeated.unit_at(39_999), Some(66));
        assert!(node.flat.get().is_none());
        assert_eq!(repeated.units().len(), 40_000);
        assert!(node.flat.get().is_some());
    }

    /** Accounting must observe real children without flattening, and compaction must release those children after publication. */
    #[test]
    fn allocation_observation_does_not_materialize_and_compaction_releases_children() {
        let value = tree();
        let Storage::Cons(node) = &value else {
            panic!("expected deferred concatenation")
        };
        let mut before = BTreeMap::new();
        value.allocations(|kind, id, lower, upper| {
            assert!(upper >= lower);
            before.insert((kind, id), lower);
        });
        assert!(node.flat.get().is_none());
        assert!(before.keys().filter(|(kind, _)| *kind).count() > 1);
        assert_eq!(value.units().len(), 2_072);
        let mut after = BTreeMap::new();
        value.allocations(|kind, id, lower, upper| {
            assert!(upper >= lower);
            after.insert((kind, id), lower);
        });
        assert_eq!(after.keys().filter(|(kind, _)| *kind).count(), 1);
        assert_eq!(after.keys().filter(|(kind, _)| !*kind).count(), 1);
        assert!(node.children().is_none());
    }

    /** Memory snapshots racing flatten publication see only bounded valid allocations and never depend on child-cache initialization. */
    #[test]
    fn allocation_walks_race_safely_with_flatten_and_append() {
        let value = tree();
        let observed = value.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let ready = barrier.clone();
        let reader = std::thread::spawn(move || {
            ready.wait();
            for _ in 0..128 {
                let mut nodes = BTreeMap::new();
                observed.allocations(|kind, id, lower, upper| {
                    assert!(id > 0 && upper >= lower);
                    nodes.insert((kind, id), lower);
                });
                assert!(
                    nodes.keys().filter(|(kind, _)| *kind).count() <= usize::from(MAX_CONCAT_NODES)
                );
            }
        });
        barrier.wait();
        let mut appended = value;
        for _ in 0..128 {
            assert!(!appended.units().is_empty());
            appended = appended.concat(&Storage::flat(vec![u16::from(b'z')].into()));
        }
        reader.join().unwrap();
        assert_eq!(appended.units().len(), 2_200);
    }
    #[cfg(feature = "conformance")]
    #[test]
    fn constructed_slices_retain_parent_and_restore_their_shape() {
        use super::super::{JsString, with_string_identities, with_string_identity_encoding};
        let parent = JsString::from_utf8("abcdefghijklmnopqrstuvwxyz").unwrap();
        let slice = parent.construct_slice(3).unwrap().unwrap();
        assert_eq!(slice.lossy(), "defghijklmnopqrstuvwxyz");
        let Storage::Slice(node) = &slice.0 else {
            panic!("slice shape")
        };
        assert!(node.parent.same(&parent.0));
        assert!(parent.construct_slice(0).unwrap().is_none());
        assert!(parent.construct_slice(20).unwrap().is_none());
        let wire = with_string_identity_encoding(|| {
            serde_json::to_string(&(parent.clone(), slice.clone(), slice.clone())).unwrap()
        });
        let (restored_parent, a, b): (JsString, JsString, JsString) =
            with_string_identities(|| serde_json::from_str(&wire).unwrap());
        assert!(a.same_backing(&b));
        let Storage::Slice(node) = &a.0 else {
            panic!("restored slice shape")
        };
        assert!(node.parent.same(&restored_parent.0));
        drop(parent);
        drop(slice);
        drop(restored_parent);
        assert_eq!(a.lossy(), "defghijklmnopqrstuvwxyz");
    }
    #[cfg(feature = "conformance")]
    #[test]
    fn constructed_cons_preserves_empty_child_and_snapshot_shape() {
        use super::super::{JsString, with_string_identities, with_string_identity_encoding};
        let left = JsString::from_utf8("").unwrap();
        let right = JsString::from_utf8("abcdefghijklmn").unwrap();
        let cons = left.construct_cons(&right).unwrap().unwrap();
        assert!(cons.0.is_cons());
        assert!(!cons.same_backing(&right));
        let short = JsString::from_utf8("a").unwrap();
        assert!(short.construct_cons(&short).unwrap().is_none());
        let wire =
            with_string_identity_encoding(|| serde_json::to_string(&(cons.clone(), cons)).unwrap());
        let (a, b): (JsString, JsString) =
            with_string_identities(|| serde_json::from_str(&wire).unwrap());
        assert!(a.0.is_cons() && a.same_backing(&b));
        assert_eq!(a.lossy(), "abcdefghijklmn");
    }
    #[cfg(feature = "conformance")]
    #[test]
    fn flatten_returns_stable_flat_identity_and_round_trips() {
        use super::super::{JsString, with_string_identities, with_string_identity_encoding};
        let left = JsString::from_utf8("abcdefghijklm").unwrap();
        let right = JsString::from_utf8("nopqrstuvwxyz").unwrap();
        let cons = left.construct_cons(&right).unwrap().unwrap();
        let flat = cons.flatten_for_intrinsic().unwrap();
        assert!(!cons.same_backing(&flat));
        assert!(flat.same_backing(&cons.flatten_for_intrinsic().unwrap()));
        assert!(flat.same_backing(&flat.flatten_for_intrinsic().unwrap()));
        let slice = flat.construct_slice(3).unwrap().unwrap();
        assert!(slice.same_backing(&slice.flatten_for_intrinsic().unwrap()));
        let wire = with_string_identity_encoding(|| serde_json::to_string(&(cons, flat)).unwrap());
        let (cons, flat): (JsString, JsString) =
            with_string_identities(|| serde_json::from_str(&wire).unwrap());
        assert!(flat.same_backing(&cons.flatten_for_intrinsic().unwrap()));
        assert_eq!(flat.lossy(), "abcdefghijklmnopqrstuvwxyz");
        let repeated = JsString::from_utf8("abc").unwrap().repeat(9).unwrap();
        let repeated_flat = repeated.flatten_for_intrinsic().unwrap();
        let wire = with_string_identity_encoding(|| {
            serde_json::to_string(&(repeated, repeated_flat)).unwrap()
        });
        let (repeated, repeated_flat): (JsString, JsString) =
            with_string_identities(|| serde_json::from_str(&wire).unwrap());
        assert!(repeated_flat.same_backing(&repeated.flatten_for_intrinsic().unwrap()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn concurrent_flattening_publishes_one_child_identity() {
        use super::super::JsString;
        let left = JsString::from_utf8("abcdefghijklm").unwrap();
        let right = JsString::from_utf8("nopqrstuvwxyz").unwrap();
        let cons = left.construct_cons(&right).unwrap().unwrap();
        let mut threads = Vec::new();
        for _ in 0..8 {
            let value = cons.clone();
            threads.push(std::thread::spawn(move || {
                value.flatten_for_intrinsic().unwrap()
            }));
        }
        let children: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert!(
            children
                .iter()
                .all(|child| child.same_backing(&children[0]))
        );
        assert_eq!(children[0].lossy(), "abcdefghijklmnopqrstuvwxyz");
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn thin_alias_retains_canonical_external_resource_across_snapshot() {
        use super::super::{JsString, with_string_identities, with_string_identity_encoding};
        let source = JsString::from_utf8("thin external string").unwrap();
        let target = JsString::from_utf8("thin external string").unwrap();
        assert!(target.mark_interned());
        assert!(source.forward_to(&target));
        assert!(!source.same_backing(&target));
        target.externalize().unwrap();
        assert!(source.is_external());
        let wire = with_string_identity_encoding(|| {
            serde_json::to_string(&(source.clone(), target.clone())).unwrap()
        });
        let (source, target): (JsString, JsString) =
            with_string_identities(|| serde_json::from_str(&wire).unwrap());
        assert!(source.thin_target().unwrap().same_backing(&target));
        assert!(target.is_interned() && source.is_external());
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn thin_transition_releases_displaced_external_resource() {
        use super::super::JsString;
        let source = JsString::from_utf8("external migration source").unwrap();
        source.mark_old();
        source.externalize().unwrap();
        let Storage::Flat(cell) = &source.0 else {
            panic!("flat")
        };
        let resource = cell.external.get().unwrap();
        let weak = Arc::downgrade(&resource);
        drop(resource);
        let target = JsString::from_utf8("external migration source").unwrap();
        assert!(target.mark_interned());
        assert!(source.forward_to(&target));
        assert!(weak.upgrade().is_none());
        assert_eq!(source.lossy(), "external migration source");
        assert!(!source.is_external());
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn thin_transition_races_external_publication_without_stranding_a_resource() {
        use super::super::JsString;
        for _ in 0..64 {
            let source = JsString::from_utf8("concurrent external transition").unwrap();
            source.mark_old();
            let target = JsString::from_utf8("concurrent external transition").unwrap();
            assert!(target.mark_interned());
            let barrier = Arc::new(std::sync::Barrier::new(2));
            std::thread::scope(|scope| {
                let source1 = source.clone();
                let barrier1 = barrier.clone();
                scope.spawn(move || {
                    barrier1.wait();
                    source1.externalize().unwrap();
                });
                let source2 = source.clone();
                let target2 = target.clone();
                scope.spawn(move || {
                    barrier.wait();
                    assert!(source2.forward_to(&target2));
                });
            });
            assert!(source.thin_target().unwrap().same_backing(&target));
            assert_eq!(source.is_external(), target.is_external());
            assert_eq!(source.lossy(), "concurrent external transition");
        }
    }
}
