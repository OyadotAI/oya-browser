/* Weak-edge liveness is resolved after strong tracing, before targets or keys are swept. */
use super::Vm;
use super::data::{Class, TableKey};
use super::gc::Marker;
use super::heap::{FastMap, INTRINSIC_BASE};
use super::realm::symbol;
use crate::v9::value::{JsValue, ObjectId};
mod validation;

impl Vm<'_> {
    /** Registered symbols have process-wide identity and cannot be held weakly. */
    pub fn can_hold_weakly(&self, value: &JsValue) -> bool {
        match value {
            JsValue::Object(id) => !matches!(
                self.heap.get(*id).map(|o| &o.class),
                Some(Class::WasmShared(_))
            ),
            JsValue::Symbol(id) => !self
                .symbols
                .registry
                .values()
                .any(|registered| registered == id),
            _ => false,
        }
    }

    /** A successful dereference and construction retain a target through the current checkpoint. */
    pub fn keep_weak_target(&mut self, value: JsValue) {
        if self.can_hold_weakly(&value) {
            self.kept_weak.entry(TableKey::of(&value)).or_insert(value);
        }
    }

    /** ClearKeptObjects runs after the complete microtask checkpoint, never between its jobs. */
    pub fn clear_kept_objects(&mut self) {
        self.kept_weak.clear();
    }

    /** Snapshot compaction retains weak slots without pretending that it ran a language GC. */
    pub(super) fn trace_snapshot_weak(&self, marker: &mut Marker) {
        while let Some(identity) = marker.weak_live.pop() {
            let TableKey::Object(id) = identity else {
                continue;
            };
            match self.heap.get(id).map(|object| &object.class) {
                Some(Class::WeakRef(Some(target))) => marker.value(target),
                Some(Class::FinalizationRegistry(data)) => {
                    for cell in &data.cells {
                        for value in [&cell.target, &cell.token].into_iter().flatten() {
                            marker.value(value);
                        }
                    }
                }
                Some(Class::WeakMap(table) | Class::WeakSet(table)) => {
                    for (key, value) in table.entries.iter().flatten() {
                        marker.value(key);
                        marker.value(value);
                    }
                }
                _ => {}
            }
            self.trace(marker);
        }
    }

    /** Reachable weak maps retain values only after their keys are independently reachable.
    Table keys compare by identity and nothing writes them while the marker runs. */
    #[allow(clippy::mutable_key_type)]
    pub(super) fn trace_ephemerons(
        &self,
        marker: &mut Marker,
        waiting: &mut FastMap<TableKey, Vec<JsValue>>,
    ) {
        while let Some(identity) = marker.weak_live.pop() {
            if let Some(values) = waiting.remove(&identity) {
                for value in values {
                    marker.value(&value);
                }
            }
            if let TableKey::Object(id) = identity {
                self.mark_weak_map(id, marker, waiting);
            }
            self.trace(marker);
        }
    }

    /** Register conditional value edges once per live map; later key discoveries wake them. */
    #[allow(clippy::mutable_key_type)]
    fn mark_weak_map(
        &self,
        id: ObjectId,
        marker: &mut Marker,
        waiting: &mut FastMap<TableKey, Vec<JsValue>>,
    ) {
        let Some(Class::WeakMap(table)) = self.heap.get(id).map(|object| &object.class) else {
            return;
        };
        for (key, value) in table.entries.iter().flatten() {
            if alive(marker, key) {
                marker.value(value);
            } else {
                waiting
                    .entry(TableKey::of(key))
                    .or_default()
                    .push(value.clone());
            }
        }
    }

    /** Dead weak targets are cleared before sweep, while live registries keep pending holdings. */
    pub(super) fn clear_dead_weak(&mut self, marker: &Marker) -> Vec<ObjectId> {
        let mut cleanup = Vec::new();
        // Weak holders are found by a read pass: writing through every live object would copy
        // the whole heap the step shares with the committed state.
        let weak: Vec<ObjectId> = self
            .heap
            .objects
            .iter()
            .filter(|(id, object)| {
                marker.objects.contains(id)
                    && matches!(
                        object.class,
                        Class::WeakRef(_)
                            | Class::FinalizationRegistry(_)
                            | Class::WeakMap(_)
                            | Class::WeakSet(_)
                    )
            })
            .map(|(id, _)| *id)
            .collect();
        for id in weak {
            let Some(object) = self.heap.objects.get_mut(&id) else {
                continue;
            };
            let id = &id;
            match &mut object.class {
                Class::WeakRef(target) => clear(target, marker),
                Class::FinalizationRegistry(data) => {
                    for cell in &mut data.cells {
                        clear(&mut cell.target, marker);
                        clear(&mut cell.token, marker);
                    }
                    if !data.scheduled && data.cells.iter().any(|cell| cell.target.is_none()) {
                        data.scheduled = true;
                        cleanup.push(*id);
                    }
                }
                Class::WeakMap(table) | Class::WeakSet(table) => {
                    let dead: Vec<_> = table
                        .entries
                        .iter()
                        .flatten()
                        .filter(|(key, _)| !alive(marker, key))
                        .map(|(key, _)| key.clone())
                        .collect();
                    for key in dead {
                        table.delete(&key);
                    }
                }
                _ => {}
            }
        }
        cleanup
    }
}

/** Live contexts retain their intrinsic identities; user values need an independent strong mark. */
fn alive(marker: &Marker, value: &JsValue) -> bool {
    match value {
        JsValue::Object(id) => {
            marker.objects.contains(id)
                || *id >= INTRINSIC_BASE && marker.live_banks.contains(&owner(*id))
        }
        JsValue::Symbol(id) => *id < symbol::WELL_KNOWN || marker.symbols.contains(id),
        _ => false,
    }
}

/** Production has one identity bank; opt-in contexts encode their virtual intrinsic owner. */
fn owner(id: ObjectId) -> u32 {
    #[cfg(feature = "conformance")]
    {
        super::realm::banks::owner(id)
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = id;
        0
    }
}

/** A cleared weak slot preserves its brand while no longer naming a reclaimed target. */
fn clear(value: &mut Option<JsValue>, marker: &Marker) {
    if value.as_ref().is_some_and(|value| !alive(marker, value)) {
        *value = None;
    }
}
