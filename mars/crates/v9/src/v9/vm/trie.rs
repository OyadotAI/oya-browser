/**
 * A persistent map from small integer identities to values. Cloning it is O(1) and shares
 * structure, so a step runs on a copy of the page heap that costs nothing to take and nothing
 * to free, and only the objects the step writes are copied: before, YouTube's half-million
 * objects were cloned and freed whole on every step. A key indexes a 64-ary radix tree six
 * levels deep; a read walks those six nodes, a write copies only the nodes and the value it
 * still shares with another map.
 */
use std::sync::Arc;

/** Key bits one level consumes. */
const BITS: u32 = 6;
/** Children per node. */
const WIDTH: usize = 1 << BITS;
/** The bits of one level. */
const MASK: u32 = WIDTH as u32 - 1;
/** Levels for 32-bit keys: six of six bits cover them. */
const LEVELS: u32 = 6;
/** The shift of the root level. */
const ROOT_SHIFT: u32 = BITS * (LEVELS - 1);

/** A leaf's slots: the key's low bits select one, which holds the full key and its value. */
type LeafSlots<V> = Box<[Option<(u32, Arc<V>)>; WIDTH]>;

/** One level: branches above, leaves holding keys and shared values at the bottom. */
enum Node<V> {
    Branch(Box<[Option<Arc<Node<V>>>; WIDTH]>),
    Leaf(LeafSlots<V>),
}

impl<V> Clone for Node<V> {
    /** A shallow copy: children and values stay shared until written. */
    fn clone(&self) -> Self {
        match self {
            Self::Branch(children) => {
                Self::Branch(Box::new(std::array::from_fn(|i| children[i].clone())))
            }
            Self::Leaf(slots) => Self::Leaf(Box::new(std::array::from_fn(|i| slots[i].clone()))),
        }
    }
}

impl<V> Node<V> {
    /** An empty node for the level at `shift`. */
    fn empty(shift: u32) -> Self {
        if shift == 0 {
            Self::Leaf(Box::new(std::array::from_fn(|_| None)))
        } else {
            Self::Branch(Box::new(std::array::from_fn(|_| None)))
        }
    }
}

/** The map: a shared root, its entry count and its height. The root covers only as many levels
as the largest key needs (`shift` is its level's shift), growing when a larger key arrives: a
page's half-million objects take four hops per lookup instead of six, and every object access
in the interpreter is one. */
pub(in crate::v9) struct Trie<V> {
    root: Arc<Node<V>>,
    len: usize,
    shift: u32,
}

impl<V> Clone for Trie<V> {
    /** O(1): the whole tree stays shared until written. */
    fn clone(&self) -> Self {
        Self {
            root: Arc::clone(&self.root),
            len: self.len,
            shift: self.shift,
        }
    }
}

impl<V> Default for Trie<V> {
    /** No entries. */
    fn default() -> Self {
        Self {
            root: Arc::new(Node::empty(0)),
            len: 0,
            shift: 0,
        }
    }
}

impl<V: std::fmt::Debug + Clone> std::fmt::Debug for Trie<V> {
    /** The entries, as a map. */
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<V> Trie<V> {
    /** Entries held. */
    pub fn len(&self) -> usize {
        self.len
    }
    /** Whether the root's levels reach `key`. */
    #[inline]
    fn covers(&self, key: u32) -> bool {
        self.shift >= ROOT_SHIFT || key >> (self.shift + BITS) == 0
    }
    /** The value under `key`. */
    pub fn get(&self, key: &u32) -> Option<&V> {
        if !self.covers(*key) {
            return None;
        }
        let mut node = &*self.root;
        let mut shift = self.shift;
        loop {
            match node {
                Node::Branch(children) => {
                    node = children[((key >> shift) & MASK) as usize].as_deref()?;
                    shift -= BITS;
                }
                Node::Leaf(slots) => {
                    return slots[(key & MASK) as usize]
                        .as_ref()
                        .filter(|(held, _)| held == key)
                        .map(|(_, value)| &**value);
                }
            }
        }
    }
    /** Whether `key` is held. */
    pub fn contains_key(&self, key: &u32) -> bool {
        self.get(key).is_some()
    }
    /** Add a level above the root: the old root becomes the new one's first child, holding
    the same keys at the same paths below. */
    fn grow(&mut self) {
        let shift = self.shift + BITS;
        let mut root = Node::empty(shift);
        if let Node::Branch(children) = &mut root {
            children[0] = Some(self.root.clone());
        }
        self.root = Arc::new(root);
        self.shift = shift;
    }
    /** Store an already shared value. */
    fn insert_shared(&mut self, key: u32, value: Arc<V>) -> Option<Arc<V>> {
        while !self.covers(key) {
            self.grow();
        }
        let mut node = Arc::make_mut(&mut self.root);
        let mut shift = self.shift;
        loop {
            match node {
                Node::Branch(children) => {
                    let child = children[((key >> shift) & MASK) as usize]
                        .get_or_insert_with(|| Arc::new(Node::empty(shift - BITS)));
                    node = Arc::make_mut(child);
                    shift -= BITS;
                }
                Node::Leaf(slots) => {
                    let old = slots[(key & MASK) as usize].replace((key, value));
                    if old.is_none() {
                        self.len += 1;
                    }
                    return old.map(|(_, value)| value);
                }
            }
        }
    }
    /** Keep the entries `keep` accepts, rebuilding the tree without the rest. */
    pub fn retain(&mut self, mut keep: impl FnMut(&u32, &V) -> bool) {
        let mut next = Self::default();
        for (key, value) in self.iter_shared() {
            if keep(key, value) {
                next.insert_shared(*key, Arc::clone(value));
            }
        }
        *self = next;
    }
    /** Entries in key order. */
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &V)> {
        self.iter_shared().map(|(key, value)| (key, &**value))
    }
    /** Keys in order. */
    pub fn keys(&self) -> impl Iterator<Item = &u32> {
        self.iter().map(|(key, _)| key)
    }
    /** Values in key order. */
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }
    /** Entries with their shared values, in key order. */
    fn iter_shared(&self) -> Iter<'_, V> {
        Iter {
            stack: vec![(&*self.root, 0)],
        }
    }
}

impl<V: Clone> Trie<V> {
    /** The value under `key` for writing: the path and the value are copied if shared. */
    pub fn get_mut(&mut self, key: &u32) -> Option<&mut V> {
        if !self.covers(*key) {
            return None;
        }
        let mut node = Arc::make_mut(&mut self.root);
        let mut shift = self.shift;
        loop {
            match node {
                Node::Branch(children) => {
                    let child = children[((key >> shift) & MASK) as usize].as_mut()?;
                    node = Arc::make_mut(child);
                    shift -= BITS;
                }
                Node::Leaf(slots) => {
                    let (held, value) = slots[(key & MASK) as usize].as_mut()?;
                    if held != key {
                        return None;
                    }
                    return Some(Arc::make_mut(value));
                }
            }
        }
    }
    /** Store `value` under `key`, returning what was there. */
    pub fn insert(&mut self, key: u32, value: V) -> Option<V> {
        self.insert_shared(key, Arc::new(value))
            .map(|old| Arc::try_unwrap(old).unwrap_or_else(|shared| (*shared).clone()))
    }
    /** Remove `key`, returning its value. Emptied nodes stay; a sweep rebuilds the tree. */
    #[cfg(test)]
    pub fn remove(&mut self, key: &u32) -> Option<V> {
        if !self.covers(*key) {
            return None;
        }
        let mut node = Arc::make_mut(&mut self.root);
        let mut shift = self.shift;
        loop {
            match node {
                Node::Branch(children) => {
                    let child = children[((key >> shift) & MASK) as usize].as_mut()?;
                    node = Arc::make_mut(child);
                    shift -= BITS;
                }
                Node::Leaf(slots) => {
                    let slot = &mut slots[(key & MASK) as usize];
                    if slot.as_ref().is_none_or(|(held, _)| held != key) {
                        return None;
                    }
                    let (_, value) = slot.take()?;
                    self.len -= 1;
                    return Some(Arc::try_unwrap(value).unwrap_or_else(|shared| (*shared).clone()));
                }
            }
        }
    }
    /** Every value for writing: copies the whole tree if shared, so callers that write a few
    values read first and `get_mut` those. */
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.iter_mut().map(|(_, value)| value)
    }
    /** Every entry for writing; see `values_mut`. */
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&u32, &mut V)> {
        let mut out: Vec<(&u32, &mut V)> = Vec::with_capacity(self.len);
        fn walk<'a, V: Clone>(node: &'a mut Node<V>, out: &mut Vec<(&'a u32, &'a mut V)>) {
            match node {
                Node::Branch(children) => {
                    for child in children.iter_mut().flatten() {
                        walk(Arc::make_mut(child), out);
                    }
                }
                Node::Leaf(slots) => {
                    for (key, value) in slots.iter_mut().flatten() {
                        out.push((&*key, Arc::make_mut(value)));
                    }
                }
            }
        }
        walk(Arc::make_mut(&mut self.root), &mut out);
        out.into_iter()
    }
}

/** A depth-first walk: each stacked node with the next child index to visit. */
pub(in crate::v9) struct Iter<'a, V> {
    stack: Vec<(&'a Node<V>, usize)>,
}

impl<'a, V> Iterator for Iter<'a, V> {
    type Item = (&'a u32, &'a Arc<V>);
    /** The next entry in key order. */
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (node, index) = self.stack.last_mut()?;
            match node {
                Node::Branch(children) => {
                    let Some(position) = (*index..WIDTH).find(|i| children[*i].is_some()) else {
                        self.stack.pop();
                        continue;
                    };
                    *index = position + 1;
                    let child = children[position].as_deref().expect("found");
                    self.stack.push((child, 0));
                }
                Node::Leaf(slots) => {
                    let Some(position) = (*index..WIDTH).find(|i| slots[*i].is_some()) else {
                        self.stack.pop();
                        continue;
                    };
                    *index = position + 1;
                    let (key, value) = slots[position].as_ref().expect("found");
                    return Some((key, value));
                }
            }
        }
    }
}

impl<V: Clone> FromIterator<(u32, V)> for Trie<V> {
    /** A map of the given entries. */
    fn from_iter<I: IntoIterator<Item = (u32, V)>>(entries: I) -> Self {
        let mut trie = Self::default();
        for (key, value) in entries {
            trie.insert(key, value);
        }
        trie
    }
}

impl<'a, V: Clone> IntoIterator for &'a Trie<V> {
    type Item = (&'a u32, &'a V);
    type IntoIter = Box<dyn Iterator<Item = (&'a u32, &'a V)> + 'a>;
    /** Entries in key order. */
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Reads, writes, removals and order hold across sparse keys, including the intrinsic range. */
    #[test]
    fn entries_round_trip_in_key_order() {
        let mut trie = Trie::default();
        let keys = [
            0u32,
            1,
            63,
            64,
            4095,
            70_000,
            0x4000_0000,
            0x6000_0001,
            u32::MAX,
        ];
        for (index, key) in keys.iter().enumerate() {
            assert_eq!(trie.insert(*key, index), None);
        }
        assert_eq!(trie.len(), keys.len());
        assert_eq!(trie.get(&4095), Some(&4));
        assert_eq!(trie.get(&4096), None);
        assert_eq!(trie.insert(64, 99), Some(3));
        *trie.get_mut(&70_000).unwrap() += 10;
        assert_eq!(trie.remove(&63), Some(2));
        assert_eq!(trie.remove(&63), None);
        let order: Vec<(u32, usize)> = trie.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(
            order,
            [
                (0, 0),
                (1, 1),
                (64, 99),
                (4095, 4),
                (70_000, 15),
                (0x4000_0000, 6),
                (0x6000_0001, 7),
                (u32::MAX, 8)
            ]
        );
        trie.retain(|key, _| *key < 100);
        assert_eq!(trie.keys().copied().collect::<Vec<_>>(), [0, 1, 64]);
    }

    /** A clone shares everything: a write to one leaves the other as it was, and the untouched
    values stay the same allocation. */
    #[test]
    fn the_root_grows_to_the_largest_key_and_keeps_every_entry() {
        let mut trie = Trie::default();
        trie.insert(5, "five");
        let small = trie.clone();
        assert_eq!(
            trie.get(&70),
            None,
            "a key past the root's reach is absent, not a panic"
        );
        for key in [70, 5_000, 1 << 20, 0x3fff_ffff] {
            trie.insert(key, "big");
        }
        assert_eq!(trie.get(&5), Some(&"five"));
        for key in [70, 5_000, 1 << 20, 0x3fff_ffff] {
            assert_eq!(trie.get(&key), Some(&"big"));
        }
        assert_eq!(trie.len(), 5);
        assert_eq!(small.get(&5), Some(&"five"));
        assert_eq!(small.get(&5_000), None);
        assert_eq!(trie.remove(&5_000), Some("big"));
        assert_eq!(trie.get(&5_000), None);
        let keys: Vec<u32> = trie.keys().copied().collect();
        assert_eq!(keys, vec![5, 70, 1 << 20, 0x3fff_ffff]);
    }

    #[test]
    fn clones_share_until_written() {
        let mut first: Trie<Vec<u8>> = (0..1000u32).map(|k| (k, vec![k as u8])).collect();
        let second = first.clone();
        first.get_mut(&500).unwrap().push(1);
        first.insert(5000, vec![]);
        assert_eq!(first.get(&500), Some(&vec![244, 1]));
        assert_eq!(second.get(&500), Some(&vec![244]));
        assert_eq!(second.get(&5000), None);
        assert_eq!(first.len(), 1001);
        assert_eq!(second.len(), 1000);
        assert!(std::ptr::eq(
            first.get(&7).unwrap(),
            second.get(&7).unwrap()
        ));
    }
}
