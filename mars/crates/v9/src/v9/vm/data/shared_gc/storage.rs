/** Shared graph mutation is synchronized and checked before any record or field becomes visible. */
use super::*;
impl State {
    /** Intern equal shared string contents while retaining only weak table references. */
    fn intern(&mut self, value: SharedGcValue) -> EngineResult<SharedGcValue> {
        use std::hash::{Hash, Hasher};
        let SharedGcValue::String(text) = value else {
            return Ok(value);
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(existing) = self.interned.get(&hash).and_then(|entries| {
            entries
                .iter()
                .filter_map(Weak::upgrade)
                .find(|existing| **existing == *text)
        }) {
            return Ok(SharedGcValue::String(existing));
        }
        self.intern_operations += 1;
        if self.intern_operations >= INTERN_PURGE_INTERVAL
            || self.interned.len() >= MAX_INTERN_ENTRIES
        {
            self.interned.retain(|_, entries| {
                entries.retain(|entry| entry.strong_count() != 0);
                !entries.is_empty()
            });
            self.intern_operations = 0;
            self.intern_entries = self.interned.values().map(Vec::len).sum();
        }
        if self.intern_entries >= MAX_INTERN_ENTRIES {
            return Err(EngineError::limit(
                "shared interned strings",
                MAX_INTERN_ENTRIES,
            ));
        }
        self.interned
            .entry(hash)
            .or_default()
            .push(Arc::downgrade(&text));
        self.intern_entries += 1;
        Ok(SharedGcValue::String(text))
    }

    /** Rebuild the weak table when inert snapshots are decoded or merged. */
    pub(super) fn intern_all(&mut self) -> EngineResult<()> {
        let mut records = std::mem::take(&mut self.records);
        self.interned.clear();
        self.intern_entries = 0;
        self.intern_operations = 0;
        for record in records.values_mut() {
            for value in &mut record.values {
                *value = self.intern(value.clone())?;
            }
        }
        self.records = records;
        Ok(())
    }
    /** Root reachability collects cyclic records without retaining them through Rust Arc field edges. */
    pub(super) fn sweep(&mut self) {
        self.roots.retain(|_, root| root.strong_count() != 0);
        let mut pending: Vec<_> = self.roots.keys().copied().collect();
        let mut marked = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if marked.insert(id)
                && let Some(record) = self.records.get(&id)
            {
                for value in &record.values {
                    if let SharedGcValue::Reference(id) = value {
                        pending.push(*id);
                    }
                }
            }
        }
        self.records.retain(|id, _| marked.contains(id));
        if self.records.is_empty() {
            self.modules.clear();
        }
        self.bytes = self.computed_bytes();
        self.dirty = false;
    }
    pub(super) fn allocated(&self) -> usize {
        self.bytes
    }
    pub(super) fn computed_bytes(&self) -> usize {
        self.modules
            .iter()
            .map(|m| m.len() + std::mem::size_of::<Arc<[u8]>>())
            .sum::<usize>()
            + self
                .records
                .values()
                .map(|r| {
                    NODE_BYTES
                        + r.values.len() * std::mem::size_of::<SharedGcValue>()
                        + r.values
                            .iter()
                            .map(SharedGcValue::dynamic_bytes)
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
    pub(super) fn valid_value(&self, value: &SharedGcValue) -> bool {
        match value {
            SharedGcValue::Reference(id) => self.records.contains_key(id),
            SharedGcValue::String(s) => s.len() <= crate::constants::JS_STRING_UNITS,
            #[cfg(feature = "conformance")]
            SharedGcValue::SharedString(s) => s.len() <= crate::constants::JS_STRING_UNITS,
            _ => true,
        }
    }
}
impl SharedGcArena {
    /** A record is fully checked and charged before insertion; no script runs while this mutex is held. */
    pub(in crate::v9) fn allocate(
        &self,
        module: &[u8],
        ty: u32,
        array: bool,
        mut values: Vec<SharedGcValue>,
    ) -> EngineResult<SharedGc> {
        let mut state = self.lock();
        if state.dirty {
            state.sweep();
        }
        for value in &mut values {
            *value = state.intern(value.clone())?;
        }
        if module.len() > MAX_BYTES
            || values.len() > crate::constants::VM_ARRAY_LENGTH
            || state.records.len() >= crate::constants::VM_OBJECTS
            || !values.iter().all(|v| state.valid_value(v))
        {
            return Err(invalid());
        }
        let new_module = !state.modules.iter().any(|m| &**m == module);
        let extra = NODE_BYTES
            + values.len() * std::mem::size_of::<SharedGcValue>()
            + values
                .iter()
                .map(SharedGcValue::dynamic_bytes)
                .sum::<usize>()
            + if new_module {
                module.len() + std::mem::size_of::<Arc<[u8]>>()
            } else {
                0
            };
        if state
            .allocated()
            .checked_add(extra)
            .is_none_or(|n| n > MAX_BYTES)
        {
            return Err(limit());
        }
        let module_index = state
            .modules
            .iter()
            .position(|m| &**m == module)
            .unwrap_or_else(|| {
                state.modules.push(Arc::from(module));
                state.modules.len() - 1
            }) as u32;
        let node = state.next;
        state.next = node
            .checked_add(1)
            .filter(|n| *n < u64::MAX / 2)
            .ok_or_else(invalid)?;
        state.revision += 1;
        state.bytes += extra;
        let version = state.revision;
        state.records.insert(
            node,
            SharedGcRecord {
                module: module_index,
                version,
                ty,
                array,
                values,
            },
        );
        self.root_locked(&mut state, node, false)
    }
}
impl SharedGc {
    /** One closure observes and mutates one field while preserving a common atomic event ordering. */
    pub(in crate::v9) fn update(
        &self,
        index: usize,
        step: impl FnOnce(&SharedGcValue) -> SharedGcValue,
    ) -> EngineResult<(SharedGcValue, Option<SharedGc>)> {
        self.update_with_number(index, &[], |old, _| step(old))
    }
    /** Resolve an immutable numeric node under the same lock as compare-exchange. */
    pub(in crate::v9) fn update_with_number(
        &self,
        index: usize,
        marker: &[u8],
        step: impl FnOnce(&SharedGcValue, Option<u64>) -> SharedGcValue,
    ) -> EngineResult<(SharedGcValue, Option<SharedGc>)> {
        super::super::SharedBytes::ordered(|| {
            let mut state = self.root.arena.lock();
            let old = state
                .records
                .get(&self.node())
                .and_then(|r| r.values.get(index))
                .cloned()
                .ok_or_else(invalid)?;
            let number = if !marker.is_empty()
                && let SharedGcValue::Reference(node) = old
            {
                state
                    .records
                    .get(&node)
                    .filter(|record| {
                        state
                            .modules
                            .get(record.module as usize)
                            .is_some_and(|module| module.as_ref() == marker)
                    })
                    .and_then(|record| match record.values.first() {
                        Some(SharedGcValue::Number(bits)) => Some(*bits),
                        _ => None,
                    })
            } else {
                None
            };
            let next = state.intern(step(&old, number))?;
            if !state.valid_value(&next) {
                return Err(invalid());
            }
            let old_units = old.dynamic_bytes();
            let new_units = next.dynamic_bytes();
            if state
                .allocated()
                .saturating_sub(old_units)
                .checked_add(new_units)
                .is_none_or(|n| n > MAX_BYTES)
            {
                return Err(limit());
            }
            state.bytes = state.bytes - old_units + new_units;
            state.revision += 1;
            let version = state.revision;
            let record = state.records.get_mut(&self.node()).unwrap();
            record.values[index] = next;
            record.version = version;
            let reference = if let SharedGcValue::Reference(node) = old {
                Some(self.root.arena.root_locked(&mut state, node, false)?)
            } else {
                None
            };
            Ok((old, reference))
        })
    }
    /** Reads acquire a root under the same lock, so another agent cannot collect the referenced node. */
    pub(in crate::v9) fn read(
        &self,
        index: usize,
    ) -> EngineResult<(SharedGcValue, Option<SharedGc>)> {
        super::super::SharedBytes::ordered(|| {
            let mut state = self.root.arena.lock();
            let value = state
                .records
                .get(&self.node())
                .and_then(|r| r.values.get(index))
                .cloned()
                .ok_or_else(invalid)?;
            let reference = if let SharedGcValue::Reference(node) = value {
                Some(self.root.arena.root_locked(&mut state, node, false)?)
            } else {
                None
            };
            Ok((value, reference))
        })
    }
}

#[cfg(all(test, feature = "conformance"))]
mod tests {
    use super::*;

    /** Explicit collection preserves live roots and reclaims an unreachable cycle. */
    #[test]
    fn explicit_collection_traces_live_roots_and_reclaims_cycles() {
        let arena = SharedGcArena::new().unwrap();
        let first = arena
            .allocate(b"shared", 0, false, vec![SharedGcValue::Null])
            .unwrap();
        let second = arena
            .allocate(
                b"shared",
                0,
                false,
                vec![SharedGcValue::Reference(first.node())],
            )
            .unwrap();
        first
            .update(0, |_| SharedGcValue::Reference(second.node()))
            .unwrap();
        let first_id = first.node();
        let second_id = second.node();
        drop(second);
        arena.collect();
        assert!(arena.contains(first_id) && arena.contains(second_id));
        drop(first);
        arena.collect();
        assert!(!arena.contains(first_id) && !arena.contains(second_id));
    }

    /** Equal live strings share native storage without the intern table keeping dead strings alive. */
    #[test]
    fn equal_strings_are_interned_with_weak_lifetimes() {
        let arena = SharedGcArena::new().unwrap();
        let left: Arc<[u16]> = Arc::from([65, 66]);
        let right: Arc<[u16]> = Arc::from([65, 66]);
        let first = arena
            .allocate(b"shared", 0, false, vec![SharedGcValue::String(left)])
            .unwrap();
        let second = arena
            .allocate(b"shared", 0, false, vec![SharedGcValue::String(right)])
            .unwrap();
        let (SharedGcValue::String(a), _) = first.read(0).unwrap() else {
            panic!("string")
        };
        let (SharedGcValue::String(b), _) = second.read(0).unwrap() else {
            panic!("string")
        };
        assert!(Arc::ptr_eq(&a, &b));
        let weak = Arc::downgrade(&a);
        drop((a, b, first, second));
        assert!(arena.is_empty());
        assert!(weak.upgrade().is_none());
    }

    /** Restoring a graph reconstructs its shared string table rather than duplicating live backing. */
    #[test]
    fn snapshot_restore_reinterns_shared_string_values() {
        let arena = SharedGcArena::new().unwrap();
        let first = arena
            .allocate(
                b"shared",
                0,
                false,
                vec![SharedGcValue::String(Arc::from([65, 66]))],
            )
            .unwrap();
        let second = arena
            .allocate(
                b"shared",
                0,
                false,
                vec![SharedGcValue::String(Arc::from([65, 66]))],
            )
            .unwrap();
        let parent = arena
            .allocate(
                b"shared",
                0,
                false,
                vec![
                    SharedGcValue::Reference(first.node()),
                    SharedGcValue::Reference(second.node()),
                ],
            )
            .unwrap();
        let bytes = serde_json::to_vec(&parent).unwrap();
        let restored: SharedGc = serde_json::from_slice(&bytes).unwrap();
        let (_, Some(first)) = restored.read(0).unwrap() else {
            panic!("first child")
        };
        let (_, Some(second)) = restored.read(1).unwrap() else {
            panic!("second child")
        };
        let (SharedGcValue::String(a), _) = first.read(0).unwrap() else {
            panic!("first string")
        };
        let (SharedGcValue::String(b), _) = second.read(0).unwrap() else {
            panic!("second string")
        };
        assert!(Arc::ptr_eq(&a, &b));
    }
}
