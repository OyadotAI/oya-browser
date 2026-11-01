/**
 * Hidden classes and inline caches, after V8's maps and its load and store ICs. Objects that
 * gained the same keys in the same order share one immutable `Shape` that maps each key to a slot
 * index, so a property-load or store site can remember "shape N keeps this key in slot S" and
 * answer the next object of that shape with one integer compare and an indexed read. Objects with
 * many properties or a deleted one drop to dictionary mode, as V8's slow properties do, and are
 * never cached.
 */
use super::heap::{FastMap, Key, Prop};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

/** Keys a shape may hold before its objects go to dictionary mode (V8 caps fast properties
too); it also bounds the prefix copies a chain of shapes makes. */
const FAST_LIMIT: usize = 32;
/** Shapes and dictionaries with more keys than this keep a hash index instead of a scan. */
const INDEX_THRESHOLD: usize = 12;
/** Dead transitions are swept once a shape has this many. */
const SWEEP_AT: usize = 64;
/** Bits of a cache entry that hold the slot; the rest hold the shape identity. */
const SLOT_BITS: u32 = 16;
/** Shapes one cache site remembers before it replaces entries (a polymorphic IC). */
pub(in crate::v9) const WAYS: usize = 4;

/** Process-wide shape identities; never reused, so a cached identity can never name a
different shape (no ABA through a freed and reallocated shape). Zero means "no shape". */
static NEXT_SHAPE: AtomicU64 = AtomicU64::new(1);

/** An immutable key layout shared by every object that added the same keys in the same order. */
#[derive(Debug)]
pub(in crate::v9) struct Shape {
    id: u64,
    keys: Vec<Key>,
    index: Option<FastMap<Key, usize>>,
    /** The shape before the last key was added; deleting that key returns to it. */
    parent: Option<Arc<Shape>>,
    /** Shapes one key further on. Weak, so a layout nobody uses any more is freed. */
    transitions: Mutex<FastMap<Key, Weak<Shape>>>,
}

thread_local! {
    /** The empty shape a thread's objects grow from. Per thread, so pages running in parallel
    never contend on one transition lock or reference count. */
    static ROOT: Arc<Shape> = Shape::new(Vec::new(), None);
}

impl Shape {
    /** A fresh shape with its own identity. */
    fn new(keys: Vec<Key>, parent: Option<Arc<Shape>>) -> Arc<Self> {
        let index = (keys.len() > INDEX_THRESHOLD).then(|| index_of(&keys));
        Arc::new(Self {
            id: NEXT_SHAPE.fetch_add(1, Ordering::Relaxed),
            keys,
            index,
            parent,
            transitions: Mutex::default(),
        })
    }
    /** The slot of a key. */
    fn position(&self, key: &Key) -> Option<usize> {
        match &self.index {
            Some(index) => index.get(key).copied(),
            None => self.keys.iter().position(|found| found == key),
        }
    }
    /** The shared shape with `key` appended, made once and reused by every object after. */
    fn with(self: &Arc<Self>, key: &Key) -> Arc<Self> {
        let mut transitions = self.transitions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(next) = transitions.get(key).and_then(Weak::upgrade) {
            return next;
        }
        if transitions.len() >= SWEEP_AT {
            transitions.retain(|_, next| next.strong_count() > 0);
        }
        let mut keys = self.keys.clone();
        keys.push(key.clone());
        let next = Shape::new(keys, Some(self.clone()));
        transitions.insert(key.clone(), Arc::downgrade(&next));
        next
    }
}

/** A key-to-slot hash index. Keys compare by value and never change once indexed. */
#[allow(clippy::mutable_key_type)]
fn index_of(keys: &[Key]) -> FastMap<Key, usize> {
    keys.iter()
        .enumerate()
        .map(|(position, key)| (key.clone(), position))
        .collect()
}

/** Where an object's keys live. */
#[derive(Clone, Debug)]
enum Layout {
    /** No properties yet; holds no shape, so making and dropping empty objects is free. */
    Empty,
    /** Fast mode: a shared shape. */
    Shaped(Arc<Shape>),
    /** Dictionary mode: the object's own key list, indexed once it grows. */
    Dictionary(Vec<Key>, Option<FastMap<Key, usize>>),
}

/** Own properties in insertion order: a layout naming the keys and the slots holding them. */
#[derive(Clone, Debug)]
pub(in crate::v9) struct Props {
    layout: Layout,
    slots: Vec<Prop>,
}

impl Default for Props {
    /** No properties, on the shared empty shape. */
    fn default() -> Self {
        Self {
            layout: Layout::Empty,
            slots: Vec::new(),
        }
    }
}

impl Props {
    /** Shared immutable shape caches are engine metadata; these are this object's retained allocations. */
    #[cfg(feature = "conformance")]
    pub fn memory_bounds(&self) -> (usize, usize) {
        let mut lower = self.slots.capacity() * std::mem::size_of::<Prop>();
        let mut upper = lower;
        if let Layout::Dictionary(keys, index) = &self.layout {
            lower += keys.capacity() * std::mem::size_of::<Key>();
            upper = lower;
            if let Some(index) = index {
                let bounds = super::memory::hash_index_bounds::<(Key, usize)>(index.capacity());
                lower += bounds.0;
                upper += bounds.1;
            }
        }
        (lower, upper)
    }
    /** Number of own properties. */
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    /** No own properties. */
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    /** The shape identity inline caches key on; zero in dictionary mode, which is never cached. */
    pub fn shape_id(&self) -> u64 {
        match &self.layout {
            Layout::Shaped(shape) => shape.id,
            Layout::Empty | Layout::Dictionary(..) => 0,
        }
    }
    /** The slot holding a key. */
    pub fn position(&self, key: &Key) -> Option<usize> {
        match &self.layout {
            Layout::Empty => None,
            Layout::Shaped(shape) => shape.position(key),
            Layout::Dictionary(_, Some(index)) => index.get(key).copied(),
            Layout::Dictionary(keys, None) => keys.iter().position(|found| found == key),
        }
    }
    /** The property in a slot, for a cache hit. */
    pub fn slot_mut(&mut self, slot: usize) -> Option<&mut Prop> {
        self.slots.get_mut(slot)
    }
    /** The property in a slot, for a cache hit. */
    pub fn slot(&self, slot: usize) -> Option<&Prop> {
        self.slots.get(slot)
    }
    /** Look up a property. */
    pub fn get(&self, key: &Key) -> Option<&Prop> {
        self.position(key).map(|slot| &self.slots[slot])
    }
    /** Change a property in place: the interpreter's store fast path updates a value without
    the remove and insert a replacement would cost. */
    pub fn get_mut(&mut self, key: &Key) -> Option<&mut Prop> {
        self.position(key).map(|slot| &mut self.slots[slot])
    }
    /** Insert or replace, returning the old property. A new key moves a shaped object along its
    shape's transition, or to dictionary mode past `FAST_LIMIT`. */
    pub fn insert(&mut self, key: Key, prop: Prop) -> Option<Prop> {
        if let Some(slot) = self.position(&key) {
            return Some(std::mem::replace(&mut self.slots[slot], prop));
        }
        match &mut self.layout {
            Layout::Empty => self.layout = Layout::Shaped(ROOT.with(|root| root.with(&key))),
            Layout::Shaped(shape) if shape.keys.len() < FAST_LIMIT => *shape = shape.with(&key),
            Layout::Shaped(shape) => {
                let mut keys = shape.keys.clone();
                keys.push(key);
                let index = Some(index_of(&keys));
                self.layout = Layout::Dictionary(keys, index);
            }
            Layout::Dictionary(keys, index) => {
                if let Some(index) = index {
                    index.insert(key.clone(), keys.len());
                }
                keys.push(key);
                if index.is_none() && keys.len() > INDEX_THRESHOLD {
                    *index = Some(index_of(keys));
                }
            }
        }
        self.slots.push(prop);
        None
    }
    /** Remove a property. Removing the newest key of a shaped object steps back to the parent
    shape; any other removal goes to dictionary mode, as V8 does. */
    pub fn remove(&mut self, key: &Key) -> Option<Prop> {
        let slot = self.position(key)?;
        let prop = self.slots.remove(slot);
        let keys = match &self.layout {
            Layout::Shaped(shape) if slot + 1 == shape.keys.len() => {
                if let Some(parent) = &shape.parent {
                    self.layout = Layout::Shaped(parent.clone());
                    return Some(prop);
                }
                shape.keys[..slot].to_vec()
            }
            Layout::Shaped(shape) => shape.keys.clone(),
            Layout::Empty => Vec::new(),
            Layout::Dictionary(keys, _) => keys.clone(),
        };
        let mut keys = keys;
        if slot < keys.len() {
            keys.remove(slot);
        }
        let index = (keys.len() > INDEX_THRESHOLD).then(|| index_of(&keys));
        self.layout = Layout::Dictionary(keys, index);
        Some(prop)
    }
    /** The keys in insertion order. */
    fn keys(&self) -> &[Key] {
        match &self.layout {
            Layout::Empty => &[],
            Layout::Shaped(shape) => &shape.keys,
            Layout::Dictionary(keys, _) => keys,
        }
    }
    /** Entries in insertion order. */
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Prop)> {
        self.keys().iter().zip(&self.slots)
    }
    /** String units retained by data values. */
    pub fn units(&self) -> usize {
        self.slots.iter().map(Prop::units).sum()
    }
    /** Keys in spec order: integer indexes ascending, then strings, then symbols, as inserted. */
    pub fn ordered_keys(&self) -> Vec<Key> {
        let mut indexes: Vec<(u32, &Key)> = self
            .keys()
            .iter()
            .filter_map(|key| key.index().map(|index| (index, key)))
            .collect();
        indexes.sort_by_key(|(index, _)| *index);
        let mut keys: Vec<Key> = indexes.into_iter().map(|(_, key)| key.clone()).collect();
        let strings = |key: &&Key| matches!(key, Key::Str(_)) && key.index().is_none();
        keys.extend(self.keys().iter().filter(strings).cloned());
        let symbols = |key: &&Key| matches!(key, Key::Sym(_));
        keys.extend(self.keys().iter().filter(symbols).cloned());
        keys
    }
}

/** One code object's inline caches: per named-key constant, up to `WAYS` (shape, slot) pairs.
A site is keyed by the constant rather than the instruction, since "shape N keeps key K in slot
S" holds wherever K is read. Each entry is one atomic word packing both halves, so code shared
across threads never reads a torn pair; relaxed ordering suffices because a stale or missing
entry only costs a miss. */
#[derive(Debug, Default)]
pub(in crate::v9) struct SiteCaches(Box<[[AtomicU64; WAYS]]>);

impl SiteCaches {
    /** Empty caches for `constants` named keys. */
    pub fn new(constants: usize) -> Self {
        Self((0..constants).map(|_| Default::default()).collect())
    }
    /** Bytes held, for the code cache's bound. */
    pub fn bytes(&self) -> usize {
        std::mem::size_of_val(&*self.0)
    }
    /** The slot cached for `shape` at constant `site`. */
    #[inline]
    pub fn lookup(&self, site: u32, shape: u64) -> Option<usize> {
        if shape == 0 {
            return None;
        }
        let ways = self.0.get(site as usize)?;
        let mask = (1 << SLOT_BITS) - 1;
        let first = ways[0].load(Ordering::Relaxed);
        if first >> SLOT_BITS == shape {
            return Some((first & mask) as usize);
        }
        for entry in &ways[1..] {
            let val = entry.load(Ordering::Relaxed);
            if val >> SLOT_BITS == shape {
                return Some((val & mask) as usize);
            }
        }
        None
    }
    /** Remember a slot: in an empty way, else replacing one picked by the shape. */
    #[inline]
    pub fn record(&self, site: u32, shape: u64, slot: usize) {
        let Some(ways) = self.0.get(site as usize) else {
            return;
        };
        if shape == 0 || shape >> (u64::BITS - SLOT_BITS) != 0 || slot >> SLOT_BITS != 0 {
            return;
        }
        let entry = shape << SLOT_BITS | slot as u64;
        let way = ways
            .iter()
            .position(|way| way.load(Ordering::Relaxed) == 0)
            .unwrap_or(shape as usize % WAYS);
        ways[way].store(entry, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v9::value::{JsString, JsValue};
    use crate::v9::vm::heap::Prop;

    /** A numbered data property. */
    fn prop(n: f64) -> Prop {
        Prop::data(JsValue::Number(n))
    }

    /** Objects that add the same keys in the same order share a shape; another order does not. */
    #[test]
    fn same_keys_same_order_share_a_shape() {
        let mut a = Props::default();
        let mut b = Props::default();
        let mut c = Props::default();
        for (props, keys) in [
            (&mut a, ["x", "y"]),
            (&mut b, ["x", "y"]),
            (&mut c, ["y", "x"]),
        ] {
            for key in keys {
                props.insert(Key::from(key), prop(1.0));
            }
        }
        assert_ne!(a.shape_id(), 0);
        assert_eq!(a.shape_id(), b.shape_id());
        assert_ne!(a.shape_id(), c.shape_id());
        assert_eq!(a.position(&Key::from("y")), Some(1));
    }

    /** Deleting the newest key steps back; deleting another goes to dictionary mode in order. */
    #[test]
    fn deletes_step_back_or_go_to_dictionary() {
        let mut a = Props::default();
        a.insert(Key::from("x"), prop(1.0));
        let one = a.shape_id();
        a.insert(Key::from("y"), prop(2.0));
        a.remove(&Key::from("y"));
        assert_eq!(a.shape_id(), one);
        a.insert(Key::from("y"), prop(2.0));
        a.insert(Key::from("z"), prop(3.0));
        a.remove(&Key::from("x"));
        assert_eq!(a.shape_id(), 0);
        let keys: Vec<Key> = a.iter().map(|(key, _)| key.clone()).collect();
        assert_eq!(keys, [Key::from("y"), Key::from("z")]);
        assert_eq!(a.get(&Key::from("z")), Some(&prop(3.0)));
    }

    /** Past the fast limit an object is a dictionary and still finds every key. */
    #[test]
    fn many_keys_go_to_dictionary_mode() {
        let mut a = Props::default();
        for n in 0..=FAST_LIMIT {
            a.insert(Key::from(&format!("k{n}")), prop(n as f64));
        }
        assert_eq!(a.shape_id(), 0);
        for n in 0..=FAST_LIMIT {
            assert_eq!(a.get(&Key::from(&format!("k{n}"))), Some(&prop(n as f64)));
        }
    }

    /** A site keeps up to WAYS shapes, never answers for a shape it was not told, and ignores
    dictionary objects. */
    #[test]
    fn caches_are_polymorphic_and_exact() {
        let caches = SiteCaches::new(2);
        for shape in 1..=WAYS as u64 {
            caches.record(1, shape, shape as usize);
        }
        for shape in 1..=WAYS as u64 {
            assert_eq!(caches.lookup(1, shape), Some(shape as usize));
        }
        assert_eq!(caches.lookup(0, 1), None);
        assert_eq!(caches.lookup(1, 99), None);
        caches.record(1, 0, 3);
        assert_eq!(caches.lookup(1, 0), None);
        caches.record(5, 1, 1);
    }

    /** A rope key finds the slot of a flat key with the same text in an indexed shape, and a
    rope with other text finds nothing: the index hashes content, not the representation. */
    #[test]
    fn rope_key_finds_flat_key_in_indexed_shape() {
        let text: Vec<u16> = (0..1100).map(|n| 0x61 + (n % 26) as u16).collect();
        let rope_of = |units: &[u16]| {
            let (head, tail) = units.split_at(550);
            let head = JsString::from_units(head.to_vec()).unwrap();
            let tail = JsString::from_units(tail.to_vec()).unwrap();
            Key::Str(head.concat(&tail).unwrap())
        };
        let mut props = Props::default();
        for n in 0..INDEX_THRESHOLD + 1 {
            props.insert(Key::from(&format!("k{n}")), prop(n as f64));
        }
        let slot = INDEX_THRESHOLD + 1;
        props.insert(
            Key::Str(JsString::from_units(text.clone()).unwrap()),
            prop(9.0),
        );
        assert_eq!(props.position(&rope_of(&text)), Some(slot));
        let mut other = text.clone();
        other[1099] ^= 1;
        assert_eq!(props.position(&rope_of(&other)), None);
    }
}
