/** One context's native observer storage has explicit resident and wire bounds inside the VM envelope. */
use super::*;
use std::collections::BTreeMap;

/** Reserve at most one sixteenth of a serialized VM for observer-owned native state. */
pub(super) const STORAGE_BYTES: usize = crate::constants::VM_STATE_BYTES / 16;
/** Conservative bytes include the map entry, vector slack, option keys and all four margins. */
const OBSERVER_BYTES: usize = 512;
/** A finite JSON double and its separator fit this charge; native vector slack is smaller. */
const THRESHOLD_BYTES: usize = 32;
/** Target identities and previous state fit this wire charge even with maximal integer spellings. */
const TARGET_BYTES: usize = 256;
/** Fourteen finite doubles, identities, keys and vector slack fit this record charge. */
const RECORD_BYTES: usize = 768;
/** Count ceilings follow memory capacity rather than an independent feature-specific object quota. */
pub(super) const OBSERVERS: usize = STORAGE_BYTES / OBSERVER_BYTES;
pub(super) const TARGETS: usize = STORAGE_BYTES / TARGET_BYTES;
pub(super) const RECORDS: usize = STORAGE_BYTES / RECORD_BYTES;
const _: () = assert!(OBSERVERS <= crate::constants::VM_OBJECTS);
const _: () = assert!(TARGETS <= crate::constants::NODES + crate::constants::VM_DETACHED_NODES);
const _: () = assert!(std::mem::size_of::<State>() <= OBSERVER_BYTES);
const _: () = assert!(2 * std::mem::size_of::<Observation>() <= TARGET_BYTES);
const _: () = assert!(2 * std::mem::size_of::<Entry>() <= RECORD_BYTES);

/** Saturating arithmetic keeps malformed saved counts from bypassing combined admission. */
#[derive(Clone, Copy, Default)]
pub(super) struct Usage {
    observers: usize,
    targets: usize,
    records: usize,
    bytes: usize,
}

impl Usage {
    /** Immutable options contribute their real threshold count, not the per-observer maximum. */
    pub fn observer(thresholds: usize) -> Self {
        Self {
            observers: 1,
            bytes: OBSERVER_BYTES.saturating_add(thresholds.saturating_mul(THRESHOLD_BYTES)),
            ..Self::default()
        }
    }
    /** A registration adds one retained node identity and its threshold history. */
    pub fn target() -> Self {
        Self {
            targets: 1,
            bytes: TARGET_BYTES,
            ..Self::default()
        }
    }
    /** Enqueue one complete record or fail before allocation; observations are never silently dropped. */
    pub fn record(&mut self) -> Res<()> {
        let next = self.plus(Self {
            records: 1,
            bytes: RECORD_BYTES,
            ..Self::default()
        });
        next.validate()?;
        *self = next;
        Ok(())
    }
    /** Used for both candidate admission and restored page/summary validation. */
    fn plus(self, other: Self) -> Self {
        Self {
            observers: self.observers.saturating_add(other.observers),
            targets: self.targets.saturating_add(other.targets),
            records: self.records.saturating_add(other.records),
            bytes: self.bytes.saturating_add(other.bytes),
        }
    }
    /** Combined memory and individual cardinality ceilings are enforced together. */
    pub fn validate(self) -> crate::protocol::EngineResult<()> {
        if self.observers > OBSERVERS
            || self.targets > TARGETS
            || self.records > RECORDS
            || self.bytes > STORAGE_BYTES
        {
            return Err(EngineError::limit(
                "IntersectionObserver native bytes",
                STORAGE_BYTES,
            ));
        }
        Ok(())
    }
}

/** Counts are derived from actual retained vectors so GC, takeRecords and disconnect release their charges. */
pub(super) fn usage(states: &BTreeMap<ObjectId, State>) -> Usage {
    states.values().fold(Usage::default(), |total, state| {
        total.plus(Usage {
            observers: 1,
            targets: state.targets.len(),
            records: state.queued.len(),
            bytes: OBSERVER_BYTES
                .saturating_add(
                    state
                        .options
                        .thresholds
                        .len()
                        .saturating_mul(THRESHOLD_BYTES),
                )
                .saturating_add(state.targets.len().saturating_mul(TARGET_BYTES))
                .saturating_add(state.queued.len().saturating_mul(RECORD_BYTES)),
        })
    })
}

/** Dead inactive wrappers are reclaimed before pressure becomes a refusal; callers pin native temporaries. */
pub(super) fn admit(vm: &mut Vm, added: Usage) -> Res<()> {
    vm.charge(vm.page.state.intersection_observers.len().saturating_add(1))?;
    let mut retained = usage(&vm.page.state.intersection_observers);
    if retained.plus(added).validate().is_err() {
        vm.collect_now()?;
        vm.charge(vm.page.state.intersection_observers.len().saturating_add(1))?;
        retained = usage(&vm.page.state.intersection_observers);
    }
    retained.plus(added).validate().map_err(Into::into)
}

/** Reclaim dead inactive options before a changed-geometry cohort could exhaust record storage. */
pub(super) fn sampling(vm: &mut Vm) -> Res<Usage> {
    let mut retained = usage(&vm.page.state.intersection_observers);
    let possible = Usage {
        records: retained.targets,
        bytes: retained.targets.saturating_mul(RECORD_BYTES),
        ..Usage::default()
    };
    if retained.plus(possible).validate().is_err() {
        vm.collect_now()?;
        vm.charge(vm.page.state.intersection_observers.len().saturating_add(1))?;
        retained = usage(&vm.page.state.intersection_observers);
    }
    retained.validate()?;
    Ok(retained)
}
