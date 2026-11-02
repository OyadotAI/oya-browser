/* Array indices have a logical uint32 length, independently of bounded resident element storage. */
mod codec;
mod iter;
use crate::v9::value::JsValue;
pub(super) use iter::Iter;
use std::collections::BTreeMap;
use std::ops::Index;
use std::sync::Arc;

/** A small prefix keeps common arrays fast without filling a large index gap with holes. */
const DENSE_PREFIX: usize = 4096;
/** Covers a B-tree leaf slot, node pointers, allocator metadata and slack. */
const MAP_ENTRY_SLOTS: usize = 8;
/** An immutable dense pattern with sparse copy-on-write edits. */
#[derive(Clone, Debug)]
struct Repeated {
    base: Arc<[JsValue]>,
    span: usize,
    overrides: BTreeMap<usize, JsValue>,
    covered: BTreeMap<usize, usize>,
    units: usize,
}

/** One-byte string positions refer to canonical, permanently rooted character values. */
#[derive(Clone, Debug)]
struct Characters {
    source: JsValue,
    roots: Arc<[JsValue]>,
    span: usize,
    overrides: BTreeMap<usize, JsValue>,
    units: usize,
}

impl Repeated {
    /** Charge sparse map nodes conservatively as well as the retained pattern. */
    fn physical_slots(&self) -> usize {
        self.base.len().saturating_add(1).saturating_add(
            self.overrides
                .len()
                .saturating_add(self.covered.len())
                .saturating_mul(MAP_ENTRY_SLOTS),
        )
    }

    /** Rebuild coverage after truncation without walking the logical array. */
    fn recount(&mut self) {
        self.covered.clear();
        for index in self.overrides.keys() {
            *self.covered.entry(index % self.base.len()).or_insert(0) += 1;
        }
    }

    /** Release a pattern value once every remaining occurrence is shadowed. */
    fn clear_shadowed(&mut self, offset: usize) -> Option<JsValue> {
        let occurrences = if offset < self.span {
            (self.span - 1 - offset) / self.base.len() + 1
        } else {
            0
        };
        if self.covered.get(&offset).copied().unwrap_or(0) != occurrences {
            return None;
        }
        let value = &mut Arc::make_mut(&mut self.base)[offset];
        (value != &JsValue::Hole).then(|| std::mem::replace(value, JsValue::Hole))
    }
}

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
    repeated: Option<Repeated>,
    characters: Option<Characters>,
}

impl ArrayElements {
    /** Freeze a fully present source and share it across a bounded number of repetitions. */
    pub(in crate::v9) fn repeated_copy(source: &Self, count: usize) -> Option<Self> {
        if source.is_empty() || !source.fully_present() {
            return None;
        }
        let length = source.len().checked_mul(count)?;
        if length > u32::MAX as usize {
            return None;
        }
        let base = match &source.repeated {
            Some(value) => value.base.clone(),
            None => Arc::from(source.dense.clone()),
        };
        let resident = base.len().checked_add(1)?;
        let units = base.iter().map(JsValue::string_units).sum();
        Some(Self {
            length,
            resident,
            dense: Vec::new(),
            sparse: BTreeMap::new(),
            repeated: Some(Repeated {
                base,
                span: length,
                overrides: BTreeMap::new(),
                covered: BTreeMap::new(),
                units,
            }),
            characters: None,
        })
    }
    /** Retain the source and actual character roots instead of one slot per yielded character. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn character_copy(source: JsValue, roots: Vec<JsValue>) -> Option<Self> {
        let JsValue::String(text) = &source else {
            return None;
        };
        let length = text.len();
        if length == 0 || length > u32::MAX as usize || roots.len() != 256 || !text.is_latin1() {
            return None;
        }
        let units = source.string_units() + roots.iter().map(JsValue::string_units).sum::<usize>();
        Some(Self {
            length,
            resident: roots.len() + 2,
            dense: Vec::new(),
            sparse: BTreeMap::new(),
            repeated: None,
            characters: Some(Characters {
                source,
                roots: Arc::from(roots),
                span: length,
                overrides: BTreeMap::new(),
                units,
            }),
        })
    }
    /** Fast construction is restricted to sources without holes or sparse overrides. */
    pub(in crate::v9) fn fully_present(&self) -> bool {
        if self.characters.is_some() {
            return false;
        }
        if let Some(repeated) = &self.repeated {
            repeated.span == self.length
                && repeated.overrides.is_empty()
                && self.sparse.is_empty()
                && self.length.is_multiple_of(repeated.base.len())
                && repeated.base.iter().all(|value| *value != JsValue::Hole)
        } else {
            self.dense.len() == self.length
                && self.sparse.is_empty()
                && self.dense.iter().all(|value| *value != JsValue::Hole)
        }
    }
    /** Reserve a new physical overlay even when the logical property already exists. */
    pub(super) fn new_slot_for_set(&self, index: usize) -> bool {
        if let Some(characters) = &self.characters
            && index < characters.span
        {
            return !characters.overrides.contains_key(&index);
        }
        if let Some(repeated) = &self.repeated
            && index < repeated.span
        {
            return !repeated.overrides.contains_key(&index);
        }
        self.get(index).is_none_or(|value| *value == JsValue::Hole)
    }
    /** Detect whether physical counts differ from logical element counts. */
    pub(super) fn is_repeated(&self) -> bool {
        self.repeated.is_some() || self.characters.is_some()
    }
    /** Keep repeated-array accounting constant-time across indexed mutations. */
    pub(super) fn repeated_units(&self) -> Option<usize> {
        self.repeated
            .as_ref()
            .map(|repeated| repeated.units)
            .or_else(|| self.characters.as_ref().map(|characters| characters.units))
    }
    /** Bound both sparse map records before writing an existing logical index. */
    pub(super) fn set_charge(&self, index: usize) -> usize {
        if !self.new_slot_for_set(index) {
            return 0;
        }
        if self.characters.is_some() {
            return MAP_ENTRY_SLOTS;
        }
        match &self.repeated {
            Some(repeated) if index < repeated.span => 2 * MAP_ENTRY_SLOTS,
            Some(_) => MAP_ENTRY_SLOTS,
            None => 1,
        }
    }
    /** Memory measurement visits physical dense capacity and sparse entries, never logical holes. */
    #[cfg(feature = "conformance")]
    pub fn memory_capacities(&self) -> (usize, usize) {
        if let Some(characters) = &self.characters {
            return (
                characters.roots.len() + 1,
                self.sparse.len() + characters.overrides.len(),
            );
        }
        match &self.repeated {
            Some(repeated) => (
                repeated.base.len(),
                self.sparse.len() + repeated.overrides.len() + repeated.covered.len(),
            ),
            None => (self.dense.capacity(), self.sparse.len()),
        }
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
        if let Some(characters) = &self.characters
            && index < characters.span
        {
            return Some(characters.overrides.get(&index).unwrap_or_else(|| {
                let JsValue::String(text) = &characters.source else {
                    unreachable!("all positions are overridden")
                };
                &characters.roots[text.unit_at(index).expect("validated character index") as usize]
            }));
        }
        if let Some(repeated) = &self.repeated
            && index < repeated.span
        {
            return Some(
                repeated
                    .overrides
                    .get(&index)
                    .unwrap_or_else(|| &repeated.base[index % repeated.base.len()]),
            );
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
        if let Some(characters) = &mut self.characters
            && index < characters.span
        {
            let added = value.string_units();
            let previous = characters.overrides.insert(index, value);
            characters.units = characters
                .units
                .saturating_add(added)
                .saturating_sub(previous.as_ref().map_or(0, JsValue::string_units));
            if characters.overrides.len() == characters.span {
                characters.units = characters
                    .units
                    .saturating_sub(characters.source.string_units());
                characters.source = JsValue::Hole;
            }
            self.resident = characters.roots.len()
                + 2
                + (characters.overrides.len() + self.sparse.len()) * MAP_ENTRY_SLOTS;
            return old;
        }
        if let Some(repeated) = &mut self.repeated
            && index < repeated.span
        {
            let added = value.string_units();
            let previous = repeated.overrides.insert(index, value);
            repeated.units = repeated
                .units
                .saturating_add(added)
                .saturating_sub(previous.as_ref().map_or(0, JsValue::string_units));
            if previous.is_none() {
                let offset = index % repeated.base.len();
                *repeated.covered.entry(offset).or_insert(0) += 1;
                if let Some(removed) = repeated.clear_shadowed(offset) {
                    repeated.units = repeated.units.saturating_sub(removed.string_units());
                }
            }
            self.resident = repeated.physical_slots() + self.sparse.len() * MAP_ENTRY_SLOTS;
            return old;
        }
        if let Some(characters) = &mut self.characters {
            characters.units = characters
                .units
                .saturating_add(value.string_units())
                .saturating_sub(old.string_units());
        }
        if let Some(repeated) = &mut self.repeated {
            repeated.units = repeated
                .units
                .saturating_add(value.string_units())
                .saturating_sub(old.string_units());
        }
        self.resident = self
            .resident
            .saturating_add(usize::from(value != JsValue::Hole))
            .saturating_sub(usize::from(old != JsValue::Hole));
        self.length = self.length.max(index + 1);
        if !self.is_repeated() && (index <= self.dense.len() || index < DENSE_PREFIX) {
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
        if let Some(repeated) = &self.repeated {
            self.resident = repeated.physical_slots() + self.sparse.len() * MAP_ENTRY_SLOTS;
        }
        if let Some(characters) = &self.characters {
            self.resident = characters.roots.len()
                + 2
                + (characters.overrides.len() + self.sparse.len()) * MAP_ENTRY_SLOTS;
        }
        old
    }

    /** Truncation visits actual storage only; sparse holes cost no work or allocation. */
    pub fn truncate_collect(&mut self, length: usize) -> Vec<JsValue> {
        if let Some(mut characters) = self.characters.take() {
            let mut removed: Vec<_> = characters
                .overrides
                .split_off(&length)
                .into_values()
                .collect();
            removed.extend(self.sparse.split_off(&length).into_values());
            if length == 0 {
                removed.push(characters.source);
                removed.extend(characters.roots.iter().cloned());
                self.resident = 0;
            } else {
                characters.span = characters.span.min(length);
                if characters.overrides.len() == characters.span {
                    removed.push(std::mem::replace(&mut characters.source, JsValue::Hole));
                }
                characters.units = std::iter::once(&characters.source)
                    .chain(characters.roots.iter())
                    .chain(characters.overrides.values())
                    .chain(self.sparse.values())
                    .map(JsValue::string_units)
                    .sum();
                self.resident = characters.roots.len()
                    + 2
                    + (characters.overrides.len() + self.sparse.len()) * MAP_ENTRY_SLOTS;
                self.characters = Some(characters);
            }
            self.length = length;
            return removed;
        }
        if let Some(mut repeated) = self.repeated.take() {
            let removed_overrides = repeated.overrides.split_off(&length);
            let mut removed: Vec<_> = removed_overrides.into_values().collect();
            removed.extend(self.sparse.split_off(&length).into_values());
            if length == 0 {
                removed.extend(
                    repeated
                        .base
                        .iter()
                        .filter(|&value| *value != JsValue::Hole)
                        .cloned(),
                );
                self.resident = 0;
            } else {
                repeated.span = repeated.span.min(length);
                repeated.recount();
                for index in 0..repeated.base.len() {
                    if let Some(value) = repeated.clear_shadowed(index) {
                        removed.push(value);
                    }
                }
                repeated.units = repeated
                    .base
                    .iter()
                    .chain(repeated.overrides.values())
                    .chain(self.sparse.values())
                    .map(JsValue::string_units)
                    .sum();
                self.resident = repeated.physical_slots() + self.sparse.len() * MAP_ENTRY_SLOTS;
                self.repeated = Some(repeated);
            }
            self.length = length;
            return removed;
        }
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
    pub fn resident_entries(&self) -> Box<dyn Iterator<Item = (usize, &JsValue)> + '_> {
        if let Some(characters) = &self.characters {
            Box::new(
                (0..characters.span)
                    .filter_map(|index| {
                        self.get(index)
                            .filter(|value| **value != JsValue::Hole)
                            .map(|value| (index, value))
                    })
                    .chain(self.sparse.iter().map(|(index, value)| (*index, value))),
            )
        } else if let Some(repeated) = &self.repeated {
            Box::new(
                (0..repeated.span)
                    .filter_map(|index| {
                        self.get(index)
                            .filter(|value| **value != JsValue::Hole)
                            .map(|value| (index, value))
                    })
                    .chain(self.sparse.iter().map(|(index, value)| (*index, value))),
            )
        } else {
            Box::new(
                self.dense
                    .iter()
                    .enumerate()
                    .chain(self.sparse.iter().map(|(index, value)| (*index, value)))
                    .filter(|(_, value)| **value != JsValue::Hole),
            )
        }
    }

    /** `resident_entries` with an index in `start..end`, read straight from that range of the
    dense part and the sparse map, so `slice(i, i + 1)` of a long array costs one element. */
    pub fn resident_range(
        &self,
        start: usize,
        end: usize,
    ) -> Box<dyn Iterator<Item = (usize, &JsValue)> + '_> {
        if start >= end {
            return Box::new(std::iter::empty());
        }
        if self.characters.is_some() || self.repeated.is_some() {
            return Box::new(
                self.resident_entries()
                    .skip_while(move |(index, _)| *index < start)
                    .take_while(move |(index, _)| *index < end),
            );
        }
        let dense = start.min(self.dense.len())..end.min(self.dense.len());
        Box::new(
            self.dense[dense.clone()]
                .iter()
                .enumerate()
                .map(move |(offset, value)| (dense.start + offset, value))
                .chain(
                    self.sparse
                        .range(start..end)
                        .map(|(index, value)| (*index, value)),
                )
                .filter(|(_, value)| **value != JsValue::Hole),
        )
    }

    /** Tracing and heap accounting inspect live values rather than a potentially huge length. */
    pub fn resident_values(&self) -> Box<dyn Iterator<Item = &JsValue> + '_> {
        if let Some(characters) = &self.characters {
            Box::new(
                std::iter::once(&characters.source)
                    .chain(characters.roots.iter())
                    .chain(characters.overrides.values())
                    .chain(self.sparse.values())
                    .filter(|value| **value != JsValue::Hole),
            )
        } else if let Some(repeated) = &self.repeated {
            Box::new(
                repeated
                    .base
                    .iter()
                    .chain(repeated.overrides.values())
                    .chain(self.sparse.values())
                    .filter(|value| **value != JsValue::Hole),
            )
        } else {
            Box::new(
                self.dense
                    .iter()
                    .chain(self.sparse.values())
                    .filter(|value| **value != JsValue::Hole),
            )
        }
    }

    /** Resident storage, rather than logical length, consumes the per-array budget. */
    pub fn resident_len(&self) -> usize {
        self.resident
    }

    /** Count enumerable indices without expanding a repeated pattern. */
    pub fn logical_present(&self) -> usize {
        if let Some(characters) = &self.characters {
            return characters
                .span
                .saturating_sub(
                    characters
                        .overrides
                        .values()
                        .filter(|value| **value == JsValue::Hole)
                        .count(),
                )
                .saturating_add(self.sparse.len());
        }
        match &self.repeated {
            Some(repeated) => repeated
                .span
                .saturating_sub(
                    repeated
                        .overrides
                        .values()
                        .filter(|value| **value == JsValue::Hole)
                        .count(),
                )
                .saturating_add(self.sparse.len()),
            None => self.resident,
        }
    }

    /** Language iteration remains logical; callers must charge work or bound materialization. */
    pub fn iter(&self) -> Iter<'_> {
        Iter::new(self)
    }

    /** A few engine-owned metadata arrays are guaranteed dense and expose that fact explicitly. */
    pub fn dense_slice(&self) -> Option<&[JsValue]> {
        (!self.is_repeated() && self.sparse.is_empty() && self.dense.len() == self.length)
            .then_some(self.dense.as_slice())
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
                repeated: None,
                characters: None,
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

#[cfg(test)]
mod repeated_tests {
    use super::*;

    #[cfg(feature = "conformance")]
    #[test]
    fn character_snapshot_rejects_forged_roots_and_missing_source() {
        use crate::v9::value::JsString;
        let roots = (0..=u8::MAX)
            .map(|unit| JsValue::String(JsString::character_root(unit)))
            .collect();
        let source = JsValue::String(JsString::from_utf8("abc").unwrap());
        let array = ArrayElements::character_copy(source, roots).unwrap();
        let mut wire = serde_json::to_value(&array).unwrap();
        wire["roots"][97] =
            serde_json::to_value(JsValue::String(JsString::from_utf8("a").unwrap())).unwrap();
        assert!(serde_json::from_value::<ArrayElements>(wire.clone()).is_err());
        wire = serde_json::to_value(&array).unwrap();
        wire["source"] = serde_json::to_value(JsValue::Hole).unwrap();
        assert!(serde_json::from_value::<ArrayElements>(wire).is_err());
    }

    #[test]
    fn repeated_edits_and_truncation_preserve_logical_values() {
        let source = ArrayElements::from(vec![JsValue::Number(1.0), JsValue::Number(2.0)]);
        let mut result = ArrayElements::repeated_copy(&source, 100_000).unwrap();
        assert_eq!(result.len(), 200_000);
        assert_eq!(result.get(199_999), Some(&JsValue::Number(2.0)));
        result.set(1, JsValue::Hole);
        result.set(2, JsValue::Number(7.0));
        assert_eq!(result.get(1), Some(&JsValue::Hole));
        assert_eq!(result.get(2), Some(&JsValue::Number(7.0)));
        assert_eq!(result.get(3), Some(&JsValue::Number(2.0)));
        result.truncate_collect(3);
        result.resize_holes(10);
        assert_eq!(result.get(3), Some(&JsValue::Hole));
        assert_eq!(source.get(1), Some(&JsValue::Number(2.0)));
    }

    #[test]
    fn fully_shadowed_pattern_values_are_not_traced() {
        let source = ArrayElements::from(vec![JsValue::Number(1.0)]);
        let mut result = ArrayElements::repeated_copy(&source, 3).unwrap();
        for index in 0..3 {
            result.set(index, JsValue::Hole);
        }
        assert_eq!(result.resident_values().count(), 0);
        assert_eq!(source.resident_values().count(), 1);
    }

    #[test]
    fn repeated_snapshot_preserves_compaction_and_holes() {
        let source = ArrayElements::from(vec![JsValue::Number(1.0), JsValue::Number(2.0)]);
        let mut result = ArrayElements::repeated_copy(&source, 10_000).unwrap();
        result.set(1, JsValue::Hole);
        let encoded = serde_json::to_vec(&result).unwrap();
        assert!(encoded.len() < 1_000);
        let restored: ArrayElements = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.len(), 20_000);
        assert_eq!(restored.get(1), Some(&JsValue::Hole));
        assert_eq!(restored.get(19_999), Some(&JsValue::Number(2.0)));
    }

    #[test]
    fn repeated_snapshot_rejects_uncovered_holes_and_duplicate_overrides() {
        let source = ArrayElements::from(vec![JsValue::Number(1.0)]);
        let result = ArrayElements::repeated_copy(&source, 3).unwrap();
        let mut wire = serde_json::to_value(&result).unwrap();
        let hole = serde_json::to_value(JsValue::Hole).unwrap();
        wire["base"][0] = hole.clone();
        assert!(serde_json::from_value::<ArrayElements>(wire.clone()).is_err());
        wire["base"][0] = serde_json::to_value(JsValue::Number(1.0)).unwrap();
        wire["overrides"] = serde_json::json!([[0, hole.clone()], [0, hole]]);
        assert!(serde_json::from_value::<ArrayElements>(wire).is_err());
    }
}
