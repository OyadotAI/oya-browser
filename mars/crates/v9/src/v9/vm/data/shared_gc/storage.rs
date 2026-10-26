/** Shared graph mutation is synchronized and checked before any record or field becomes visible. */
use super::*;
impl State {
    /** Root reachability collects cyclic records without retaining them through Rust Arc field edges. */
    pub(super) fn sweep(&mut self) {
        self.roots.retain(|_, root| root.strong_count() != 0);
        let mut pending: Vec<_> = self.roots.keys().copied().collect();
        let mut marked = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if marked.insert(id) {
                if let Some(record) = self.records.get(&id) {
                    for value in &record.values {
                        if let SharedGcValue::Reference(id) = value {
                            pending.push(*id);
                        }
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
        values: Vec<SharedGcValue>,
    ) -> EngineResult<SharedGc> {
        let mut state = self.lock();
        if state.dirty {
            state.sweep();
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
        super::super::SharedBytes::ordered(|| {
            let mut state = self.root.arena.lock();
            let old = state
                .records
                .get(&self.node())
                .and_then(|r| r.values.get(index))
                .cloned()
                .ok_or_else(invalid)?;
            let next = step(&old);
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
