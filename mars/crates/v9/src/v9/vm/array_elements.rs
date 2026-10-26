/* Array indices have a logical uint32 length, independently of bounded resident element storage. */
mod codec;
mod iter;
use crate::v9::value::JsValue;
pub(super) use iter::Iter;
use std::collections::BTreeMap;
use std::ops::Index;

/** A small prefix keeps common arrays fast without filling a large index gap with holes. */
const DENSE_PREFIX: usize = 4096;

/** Missing logical indices are holes; only actual values contribute to retained heap budgets. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct ArrayElements {
    /** The JavaScript length can exceed the number of resident slots by billions. */
    length: usize,
    /** Updated by explicit stores so quota checks never rescan an entire array per append. */
    resident: usize,
    /** A contiguous prefix used for ordinary small arrays and explicitly dense input. */
    dense: Vec<JsValue>,
    /** Higher resident indices, always outside the dense prefix. */
    sparse: BTreeMap<usize, JsValue>,
}

impl ArrayElements {
    /** Memory measurement visits physical dense capacity and sparse entries, never logical holes. */
    #[cfg(feature = "conformance")]
    pub fn memory_capacities(&self) -> (usize, usize) {
        (self.dense.capacity(), self.sparse.len())
    }
    /** An array of absent indices requires no element allocation. */
    pub fn holes(length: usize) -> Self {
        Self {
            length,
            ..Self::default()
        }
    }

    /** Logical length is independent of both storage forms and resident value count. */
    pub fn len(&self) -> usize {
        self.length
    }

    /** Empty means no logical indices, matching JavaScript rather than resident storage. */
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /** A logical in-range hole is distinguishable from an out-of-range index. */
    pub fn get(&self, index: usize) -> Option<&JsValue> {
        if index >= self.length {
            return None;
        }
        Some(
            self.dense
                .get(index)
                .or_else(|| self.sparse.get(&index))
                .unwrap_or(&JsValue::Hole),
        )
    }

    /** Setting a high index grows logical length without allocating the intervening holes. */
    pub fn set(&mut self, index: usize, value: JsValue) -> JsValue {
        let old = self.get(index).cloned().unwrap_or(JsValue::Hole);
        self.resident = self
            .resident
            .saturating_add(usize::from(value != JsValue::Hole))
            .saturating_sub(usize::from(old != JsValue::Hole));
        self.length = self.length.max(index + 1);
        if index <= self.dense.len() || index < DENSE_PREFIX {
            if index >= self.dense.len() {
                self.dense.resize(index + 1, JsValue::Hole);
            }
            self.sparse.remove(&index);
            self.dense[index] = value;
        } else if value == JsValue::Hole {
            self.sparse.remove(&index);
        } else {
            self.sparse.insert(index, value);
        }
        old
    }

    /** Truncation visits actual storage only; sparse holes cost no work or allocation. */
    pub fn truncate_collect(&mut self, length: usize) -> Vec<JsValue> {
        let dense = self.dense.split_off(length.min(self.dense.len()));
        let sparse = self.sparse.split_off(&length);
        self.length = length;
        let removed: Vec<_> = dense
            .into_iter()
            .chain(sparse.into_values())
            .filter(|value| *value != JsValue::Hole)
            .collect();
        self.resident -= removed.len();
        removed
    }

    /** Length extension preserves holes and does not touch resident values. */
    pub fn resize_holes(&mut self, length: usize) {
        if length < self.length {
            self.truncate_collect(length);
        } else {
            self.length = length;
        }
    }

    /** Small internal FIFO arrays share the same representation as page arrays. */
    pub fn push(&mut self, value: JsValue) {
        self.set(self.length, value);
    }

    /** Removing a logical queue slot shifts only its resident successors. */
    pub fn remove(&mut self, index: usize) -> JsValue {
        let value = self.get(index).cloned().expect("queue index");
        if index < self.dense.len() {
            self.dense.remove(index);
        }
        self.sparse.remove(&index);
        let moved = self.sparse.split_off(&index);
        self.sparse
            .extend(moved.into_iter().map(|(index, value)| (index - 1, value)));
        self.length -= 1;
        self.resident -= usize::from(value != JsValue::Hole);
        value
    }

    /** Resident entries are sorted by index and never enumerate absent logical indices. */
    pub fn resident_entries(&self) -> impl Iterator<Item = (usize, &JsValue)> {
        self.dense
            .iter()
            .enumerate()
            .chain(self.sparse.iter().map(|(index, value)| (*index, value)))
            .filter(|(_, value)| **value != JsValue::Hole)
    }

    /** Tracing and heap accounting inspect live values rather than a potentially huge length. */
    pub fn resident_values(&self) -> impl Iterator<Item = &JsValue> {
        self.resident_entries().map(|(_, value)| value)
    }

    /** Resident storage, rather than logical length, consumes the per-array budget. */
    pub fn resident_len(&self) -> usize {
        self.resident
    }

    /** Language iteration remains logical; callers must charge work or bound materialization. */
    pub fn iter(&self) -> Iter<'_> {
        Iter::new(self)
    }

    /** A few engine-owned metadata arrays are guaranteed dense and expose that fact explicitly. */
    pub fn dense_slice(&self) -> Option<&[JsValue]> {
        (self.sparse.is_empty() && self.dense.len() == self.length).then_some(self.dense.as_slice())
    }
}

impl From<Vec<JsValue>> for ArrayElements {
    /** Existing dense constructors retain their fast representation and compact wire format. */
    fn from(dense: Vec<JsValue>) -> Self {
        if dense.len() <= DENSE_PREFIX || !dense.contains(&JsValue::Hole) {
            return Self {
                length: dense.len(),
                resident: dense
                    .iter()
                    .filter(|value| **value != JsValue::Hole)
                    .count(),
                dense,
                sparse: BTreeMap::new(),
            };
        }
        let mut elements = Self::holes(dense.len());
        for (index, value) in dense.into_iter().enumerate() {
            if value != JsValue::Hole {
                elements.set(index, value);
            }
        }
        elements
    }
}

impl Index<usize> for ArrayElements {
    type Output = JsValue;
    /** Indexing is reserved for paths that already checked logical bounds. */
    fn index(&self, index: usize) -> &JsValue {
        self.get(index).expect("checked array index")
    }
}
