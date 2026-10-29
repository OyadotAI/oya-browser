/** Weak per-VM registrations observe live string backings without extending their lifetimes. */
use super::heap::{FastMap, FastSet};
use crate::v9::value::{JsString, WeakString};

/** Reclaim stale weak registrations periodically without scanning on every stack push. */
const PRUNE_INTERVAL: usize = 4096;
/** Headers, string metadata and alignment for temporary and final backing allocations. */
const MATERIALIZATION_OVERHEAD_UNITS: usize = 128;

/** Registrations are derived runtime state and are never persisted in a page snapshot. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct StringBudget {
    nodes: FastMap<(u8, usize), WeakString>,
    added: usize,
    observed_epoch: u64,
    upper_units: usize,
    reserved_units: usize,
}

impl StringBudget {
    /** Check both temporary and final contiguous allocations before materializing a rope. */
    pub(in crate::v9) fn preflight(
        &mut self,
        text: &JsString,
        limit: usize,
    ) -> crate::protocol::EngineResult<usize> {
        self.register(text);
        if text.flat_units().is_none() {
            let materialization = text
                .len()
                .saturating_mul(2)
                .saturating_add(MATERIALIZATION_OVERHEAD_UNITS);
            self.reserve_against(materialization, limit)?;
            return Ok(materialization);
        }
        Ok(0)
    }
    /** Reserve peak temporary and final backing before allocating a new UTF-16 copy. */
    #[cfg(feature = "conformance")]
    pub(super) fn reserve_copy(
        &mut self,
        source: &JsString,
        limit: usize,
    ) -> crate::protocol::EngineResult<usize> {
        self.register(source);
        let units = source
            .len()
            .saturating_mul(2)
            .saturating_add(MATERIALIZATION_OVERHEAD_UNITS);
        self.reserve_against(units, limit)?;
        Ok(units)
    }
    /** Include existing in-flight reservations in every resident-limit decision. */
    fn reserve_against(&mut self, units: usize, limit: usize) -> crate::protocol::EngineResult<()> {
        self.refresh_materialization();
        if self
            .upper_units
            .saturating_add(self.reserved_units)
            .saturating_add(units)
            > limit
        {
            let resident = self.resident_units();
            if resident
                .saturating_add(self.reserved_units)
                .saturating_add(units)
                > limit
            {
                return Err(crate::protocol::EngineError::limit(
                    "V9 resident string materialization units",
                    limit,
                ));
            }
        }
        self.reserved_units = self.reserved_units.saturating_add(units);
        Ok(())
    }
    /** Reserve a bounded native temporary while keeping its input in the resident ledger. */
    pub(in crate::v9) fn reserve_output(
        &mut self,
        source: &JsString,
        units: usize,
        limit: usize,
    ) -> crate::protocol::EngineResult<usize> {
        self.register(source);
        let units = units.saturating_add(MATERIALIZATION_OVERHEAD_UNITS);
        self.reserve_against(units, limit)?;
        Ok(units)
    }
    /** Reserve native string construction when no input string object exists yet. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn reserve_new(
        &mut self,
        units: usize,
        limit: usize,
    ) -> crate::protocol::EngineResult<usize> {
        let units = units.saturating_add(MATERIALIZATION_OVERHEAD_UNITS);
        self.reserve_against(units, limit)?;
        Ok(units)
    }
    /** Release a completed or failed temporary reservation. */
    pub(in crate::v9) fn release(&mut self, units: usize) {
        self.reserved_units = self.reserved_units.saturating_sub(units);
    }
    /** Track a root node by identity; duplicate aliases keep one non-owning entry. */
    pub(in crate::v9) fn register(&mut self, text: &JsString) {
        let (identity, weak) = text.weak_registration();
        if self.nodes.insert(identity, weak).is_none() {
            self.added += 1;
            let mut bytes = 0usize;
            text.resident_allocations(|_, _, _, upper| bytes = bytes.saturating_add(upper));
            self.upper_units = self.upper_units.saturating_add(bytes.div_ceil(2));
        }
        if self.added >= PRUNE_INTERVAL {
            self.prune();
        }
    }

    /** Remove dead registrations without keeping their backing allocations alive. */
    fn prune(&mut self) {
        self.nodes.retain(|_, weak| weak.is_alive());
        self.added = 0;
    }

    /** Re-observe live nodes, including flat and external caches created since registration. */
    pub(super) fn resident_units(&mut self) -> usize {
        let epoch = crate::v9::value::materialization_epoch();
        let mut seen = FastSet::default();
        let mut bytes = 0usize;
        self.nodes.retain(|_, weak| {
            weak.allocations(|node, identity, _, upper| {
                if seen.insert((node, identity)) {
                    bytes = bytes.saturating_add(upper);
                }
            })
        });
        self.added = 0;
        self.observed_epoch = epoch;
        self.upper_units = bytes.div_ceil(2);
        self.upper_units
    }

    /** Recount after any cache publication; a concurrent later publication remains observable. */
    pub(super) fn refresh_materialization(&mut self) -> Option<usize> {
        (self.observed_epoch != crate::v9::value::materialization_epoch())
            .then(|| self.resident_units())
    }

    /** This conservative value charges new nodes until the next deduplicating observation. */
    pub(super) fn upper_units(&self) -> usize {
        self.upper_units
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** An output copy must fit alongside every already reserved temporary buffer. */
    #[cfg(feature = "conformance")]
    #[test]
    fn copy_reservation_counts_in_flight_buffers_and_releases_them() {
        let mut budget = StringBudget::default();
        let text = JsString::from_utf8("a bounded copied string").unwrap();
        let resident = text.resident_units();
        let peak = text.len() * 2 + MATERIALIZATION_OVERHEAD_UNITS;
        assert!(budget.reserve_copy(&text, resident + peak - 1).is_err());
        let reserved = budget.reserve_copy(&text, resident + peak).unwrap();
        assert!(budget.reserve_copy(&text, resident + peak).is_err());
        budget.release(reserved);
        assert_eq!(budget.reserve_copy(&text, resident + peak).unwrap(), peak);
    }

    /** Native byte buffers share the same resident budget as string copies. */
    #[test]
    fn native_output_reservations_include_live_string_backing() {
        let mut budget = StringBudget::default();
        let text = JsString::from_utf8("bounded output").unwrap();
        let resident = text.resident_units();
        let peak = text.len() * 3 + MATERIALIZATION_OVERHEAD_UNITS;
        assert!(
            budget
                .reserve_output(&text, text.len() * 3, resident + peak - 1)
                .is_err()
        );
        let reserved = budget
            .reserve_output(&text, text.len() * 3, resident + peak)
            .unwrap();
        assert_eq!(reserved, peak);
        budget.release(reserved);
    }

    /** Weak registrations deduplicate aliases, observe later caches, and release dead strings. */
    #[test]
    fn live_allocations_follow_materialization_without_retaining_roots() {
        let text: JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
        let alias = text.clone();
        let mut budget = StringBudget::default();
        budget.register(&text);
        assert!(budget.upper_units() > 0);
        let compact = budget.resident_units();
        budget.register(&alias);
        assert_eq!(budget.resident_units(), compact);
        text.try_as_units().unwrap();
        assert!(budget.refresh_materialization().unwrap() >= 200000);
        drop(text);
        assert!(budget.resident_units() >= 200000);
        drop(alias);
        assert_eq!(budget.resident_units(), 0);
    }

    /** Two distinct rope roots sharing a child count its allocation only once after observation. */
    #[test]
    fn overlapping_roots_share_child_charges() {
        let base: JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
        let left = base.concat(&JsString::from_utf8("x").unwrap()).unwrap();
        let right = base.concat(&JsString::from_utf8("y").unwrap()).unwrap();
        let mut budget = StringBudget::default();
        budget.register(&left);
        budget.register(&right);
        let conservative = budget.upper_units();
        let deduplicated = budget.resident_units();
        assert!(deduplicated < conservative);
        assert!(deduplicated < left.resident_units() + right.resident_units());
    }

    /** Native environment writes register string backing without relying on an operand push. */
    #[test]
    fn environment_writes_register_backing_and_release_it() {
        let mut heap = super::super::heap::Heap::default();
        let text: JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
        let id = heap
            .new_env(None, vec![crate::v9::value::JsValue::String(text.clone())])
            .unwrap();
        assert!(heap.string_budget.upper_units() > 0);
        assert!(heap.set_slot(id, 0, crate::v9::value::JsValue::Undefined));
        drop(text);
        assert_eq!(heap.string_budget.resident_units(), 0);
    }

    /** Failed preflight never creates a flat cache; an adequate budget permits materialization. */
    #[test]
    fn materialization_preflight_respects_resident_limit() {
        let text: JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
        let mut budget = StringBudget::default();
        let before = text.resident_units();
        assert!(budget.preflight(&text, text.len()).is_err());
        assert_eq!(text.resident_units(), before);
        let reserved = budget.preflight(&text, text.len() * 3).unwrap();
        assert!(budget.preflight(&text, text.len() * 3).is_err());
        assert_eq!(text.try_as_units().unwrap().len(), text.len());
        budget.release(reserved);
        assert!(budget.refresh_materialization().unwrap() > before);
    }
}
