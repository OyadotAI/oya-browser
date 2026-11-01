/** Weak per-VM registrations observe live string backings without extending their lifetimes. */
use super::heap::{FastMap, FastSet};
use crate::v9::value::{JsString, WeakString};

/** Reclaim stale weak registrations periodically without scanning on every stack push. */
const PRUNE_INTERVAL: usize = 4096;
/** Headers, string metadata and alignment for temporary and final backing allocations. */
const MATERIALIZATION_OVERHEAD_UNITS: usize = 128;

/** Slots in the recently registered identity cache. */
const RECENT_SLOTS: usize = 64;
/** Address bits below allocation alignment, skipped when picking a slot. */
const RECENT_SHIFT: u32 = 4;

/** The recently registered identities, by slot. */
#[derive(Clone, Debug)]
struct Recent([Option<(u8, usize)>; RECENT_SLOTS]);

impl Default for Recent {
    /** Nothing seen yet. */
    fn default() -> Self {
        Self([None; RECENT_SLOTS])
    }
}

/** Registrations are derived runtime state and are never persisted in a page snapshot. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct StringBudget {
    nodes: FastMap<(u8, usize), WeakString>,
    /** Identities registered recently, direct-mapped by address: most pushes repeat a few
    strings (constants, property names), and this answers them without hashing. An entry is
    only ever a registered identity, and registrations keep their allocations reserved, so a
    hit can never be a different string; a prune clears it. */
    recent: Recent,
    added: usize,
    /** `materialized_units()` when this budget last accounted for it. */
    observed_units: u64,
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
        // Every string the interpreter pushes comes through here, almost always one already
        // registered. A registered weak reference keeps its allocation reserved, so a matching
        // identity is this same live string: a lookup answers it without the weak reference's
        // two atomic operations and map write (most of this function's cost on YouTube).
        let identity = text.registration_identity();
        let slot = (identity.1 >> RECENT_SHIFT) % RECENT_SLOTS;
        if self.recent.0[slot] == Some(identity) {
            return;
        }
        if self.nodes.contains_key(&identity) {
            self.recent.0[slot] = Some(identity);
            return;
        }
        let (identity, weak) = text.weak_registration();
        if self.nodes.insert(identity, weak).is_none() {
            self.added += 1;
            let mut bytes = 0usize;
            // Registered parts were charged when they registered; only what is new counts.
            let nodes = &self.nodes;
            text.new_allocations(
                |part| nodes.contains_key(&part),
                |_, _, _, upper| bytes = bytes.saturating_add(upper),
            );
            self.upper_units = self.upper_units.saturating_add(bytes.div_ceil(2));
        }
        // Proportional, so each prune's walk over every registration is paid for by as many
        // new ones: a fixed interval made pruning quadratic as a page's strings grew.
        if self.added >= PRUNE_INTERVAL.max(self.nodes.len() / 2) {
            self.prune();
        }
    }

    /** Remove dead registrations without keeping their backing allocations alive. */
    fn prune(&mut self) {
        self.recent = Recent::default();
        self.nodes.retain(|_, weak| weak.is_alive());
        self.added = 0;
    }

    /** Re-observe live nodes, including flat and external caches created since registration. */
    pub(super) fn resident_units(&mut self) -> usize {
        let mut seen = FastSet::default();
        let mut bytes = 0usize;
        // Removing dead registrations frees their reserved addresses for new strings.
        self.recent = Recent::default();
        self.nodes.retain(|_, weak| {
            weak.allocations(|node, identity, _, upper| {
                if seen.insert((node, identity)) {
                    bytes = bytes.saturating_add(upper);
                }
            })
        });
        self.added = 0;
        self.observed_units = crate::v9::value::materialized_units();
        self.upper_units = bytes.div_ceil(2);
        self.upper_units
    }

    /** Add the backing that flattening and external publications allocated since this budget
    last looked: an upper bound in O(1). The exact recount runs only when that bound would
    refuse a reservation (`reserve_against`) or the heap's own bound check asks for it. */
    pub(super) fn refresh_materialization(&mut self) -> Option<usize> {
        let now = crate::v9::value::materialized_units();
        let grown = now
            .checked_sub(self.observed_units)
            .filter(|grown| *grown > 0)?;
        self.observed_units = now;
        self.upper_units = self.upper_units.saturating_add(grown as usize);
        Some(self.upper_units)
    }

    /** Whether nothing is registered yet: a decoded heap, or one that never held a string. */
    pub(super) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
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

    /** Building a string by repeated `+=` charges each piece once: each new rope node adds
    its own allocation, not every piece below it again, so the estimate stays near the exact
    count instead of growing with the square of the steps (which made every later concat on
    YouTube recount every live string). */
    #[test]
    fn appending_to_a_rope_charges_each_piece_once() {
        let piece = JsString::from_utf8(&"x".repeat(64)).unwrap();
        let mut built = JsString::from_utf8("start").unwrap();
        let mut budget = StringBudget::default();
        budget.register(&built);
        // Every step stays alive, so the exact count includes each one the estimate charged.
        let mut steps = Vec::new();
        for _ in 0..2000 {
            budget.register(&piece);
            built = built.concat(&piece).unwrap();
            budget.register(&built);
            steps.push(built.clone());
        }
        let estimate = budget.upper_units();
        let exact = budget.resident_units();
        assert!(estimate <= exact * 2, "estimate {estimate} exact {exact}");
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
