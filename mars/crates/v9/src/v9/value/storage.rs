/** Bounded immutable UTF-16 concatenations flatten in bulk and publish stable slices before releasing child storage. */
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/** Short strings keep their original single flat allocation and efficient bulk-copy concatenation. */
const FLAT_CONCAT_UNITS: usize = 1_024;
/** Neither recursive destruction nor a balanced concat tree can grow without an independent bound. */
const MAX_CONCAT_DEPTH: u8 = 32;
/** Count occurrences, including repeated DAG children, so shared subtrees cannot evade this bound. */
const MAX_CONCAT_NODES: u8 = 32;

/** Flat strings preserve their old allocation; only deferred concatenations allocate a node. */
#[derive(Clone)]
pub(super) enum Storage {
    Flat(Arc<[u16]>),
    Cons(Arc<Concat>),
}

/** Logical content never changes; flattening only replaces child retention with its published equivalent buffer. */
pub(super) struct Concat {
    length: usize,
    depth: u8,
    nodes: u8,
    flat: OnceLock<Arc<[u16]>>,
    children: Mutex<Option<(Storage, Storage)>>,
}

impl Storage {
    /** Cardinality never traverses or materializes a concatenation. */
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Flat(units) => units.len(),
            Self::Cons(node) => node.length,
        }
    }

    /** A cached allocation is immutable and remains borrowed for the owning string's entire lifetime. */
    pub(super) fn units(&self) -> &[u16] {
        match self {
            Self::Flat(units) => units,
            Self::Cons(node) => {
                let flat = node.flat.get_or_init(|| {
                    let mut units = Vec::with_capacity(node.length);
                    self.append_to(&mut units);
                    units.into()
                });
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
            (Self::Cons(left), Self::Cons(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    /** Admission of code-unit length occurs before this helper; representation bounds choose when to copy in bulk. */
    pub(super) fn concat(&self, other: &Self) -> Self {
        let left = self.canonical();
        let right = other.canonical();
        let length = left.len() + right.len();
        let (left_depth, left_nodes) = left.shape();
        let (right_depth, right_nodes) = right.shape();
        let depth = left_depth.max(right_depth).saturating_add(1);
        let nodes = left_nodes.saturating_add(right_nodes).saturating_add(1);
        if length <= FLAT_CONCAT_UNITS || depth > MAX_CONCAT_DEPTH || nodes > MAX_CONCAT_NODES {
            return Self::join_flat(&left, &right, length);
        }
        Self::Cons(Arc::new(Concat {
            length,
            depth,
            nodes,
            flat: OnceLock::new(),
            children: Mutex::new(Some((left, right))),
        }))
    }

    /** Already flattened nodes become direct flat children instead of retaining historical graph depth. */
    fn canonical(&self) -> Self {
        match self {
            Self::Cons(node) if node.flat.get().is_some() => {
                Self::Flat(node.flat.get().expect("published UTF-16").clone())
            }
            _ => self.clone(),
        }
    }

    /** Cached heights may overestimate after compaction but can never admit an oversized tree. */
    fn shape(&self) -> (u8, u8) {
        match self {
            Self::Flat(_) => (0, 0),
            Self::Cons(node) => (node.depth, node.nodes),
        }
    }

    /** Keep the measured efficient bulk-copy path; iterator collection is deliberately avoided. */
    fn join_flat(left: &Self, right: &Self, length: usize) -> Self {
        let mut units = Vec::with_capacity(length);
        left.append_to(&mut units);
        right.append_to(&mut units);
        Self::Flat(units.into())
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
    #[cfg(feature = "conformance")]
    pub(super) fn allocations(&self, mut visit: impl FnMut(bool, usize, usize, usize)) {
        let mut pending = vec![self.clone()];
        while let Some(storage) = pending.pop() {
            match storage {
                Self::Flat(flat) => report_flat(&flat, &mut visit),
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
#[cfg(feature = "conformance")]
const ARC_HEADER_BYTES: usize = 2 * std::mem::size_of::<usize>();
/** Arc pads its payload to the counter alignment. */
#[cfg(feature = "conformance")]
const ARC_ALIGNMENT_SLACK: usize = std::mem::align_of::<usize>() - 1;

/** Flat buffers and node allocations have separate identity namespaces in the memory tracker. */
#[cfg(feature = "conformance")]
fn report_flat(flat: &Arc<[u16]>, visit: &mut impl FnMut(bool, usize, usize, usize)) {
    if !flat.is_empty() {
        let bytes = flat.len() * std::mem::size_of::<u16>() + ARC_HEADER_BYTES;
        visit(
            false,
            flat.as_ptr() as usize,
            bytes,
            bytes + ARC_ALIGNMENT_SLACK,
        );
    }
}

#[cfg(all(test, feature = "conformance"))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /** Create a real unobserved concat graph, without accidentally initializing its flat cache during setup. */
    fn tree() -> Storage {
        let mut value = Storage::Flat(vec![u16::from(b'x'); 2_048].into());
        let tail = Storage::Flat(vec![u16::from(b'y')].into());
        for _ in 0..24 {
            value = value.concat(&tail)
        }
        value
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
            appended = appended.concat(&Storage::Flat(vec![u16::from(b'z')].into()));
        }
        reader.join().unwrap();
        assert_eq!(appended.units().len(), 2_200);
    }
}
