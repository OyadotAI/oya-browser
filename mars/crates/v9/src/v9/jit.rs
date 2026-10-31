/**
 * JIT execution bridge for V9.
 * Interfaces between the bytecode interpreter and JIT compilation tiers,
 * tracking execution hotness and managing accelerated evaluation.
 */
use std::collections::HashMap;

/** Invocations required to promote a function to baseline JIT compilation. */
pub const BASELINE_TIER_INVOCATIONS: u32 = 20;

/** Invocations required to promote a function to optimizing JIT compilation. */
pub const OPTIMIZED_TIER_INVOCATIONS: u32 = 100;

/** V8-compatible optimization status bitflags for tier introspection. */
pub const STATUS_FUNCTION: i32 = 1 << 0;
pub const STATUS_NEVER_OPTIMIZE: i32 = 1 << 1;
pub const STATUS_OPTIMIZED: i32 = 1 << 4;
pub const STATUS_BASELINE: i32 = 1 << 7;

/** Execution tier for an executable unit in the V9 engine. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizationTier {
    /** Bytecode interpreted without compilation. */
    Interpreted,
    /** Fast baseline compiled native unit. */
    Baseline,
    /** Fully optimized native compiled unit. */
    Optimized,
}

/** The JIT bridge managing tier transitions and fast-path execution units. */
#[derive(Default, Debug)]
pub struct JitBridge {
    /** Call counters indexed by code or function identifier. */
    invocations: HashMap<u32, u32>,
    /** Compiled units marked for optimization. */
    tiers: HashMap<u32, OptimizationTier>,
}

impl JitBridge {
    /** Create a fresh JIT bridge with empty profile data. */
    pub fn new() -> Self {
        Self::default()
    }

    /** Record one call to the given function ID, tiering up when hot. */
    pub fn record_call(&mut self, id: u32) -> OptimizationTier {
        let count = {
            let entry = self.invocations.entry(id).or_insert(0);
            *entry = entry.saturating_add(1);
            *entry
        };
        self.update_tier(id, count)
    }

    /** Query current optimization tier for an entity. */
    pub fn tier(&self, id: u32) -> OptimizationTier {
        self.tiers
            .get(&id)
            .copied()
            .unwrap_or(OptimizationTier::Interpreted)
    }

    /** Determine and save the tier based on invocation count. */
    fn update_tier(&mut self, id: u32, count: u32) -> OptimizationTier {
        let tier = match count {
            c if c >= OPTIMIZED_TIER_INVOCATIONS => OptimizationTier::Optimized,
            c if c >= BASELINE_TIER_INVOCATIONS => OptimizationTier::Baseline,
            _ => OptimizationTier::Interpreted,
        };
        // Write only on a promotion: this runs on every call, so a second map write per call
        // was pure overhead.
        if count == BASELINE_TIER_INVOCATIONS || count == OPTIMIZED_TIER_INVOCATIONS {
            self.tiers.insert(id, tier);
        }
        tier
    }

    /** Returns the optimization status bitmask for introspection intrinsics. */
    pub fn status(&self, id: u32) -> i32 {
        match self.tier(id) {
            OptimizationTier::Optimized => STATUS_FUNCTION | STATUS_OPTIMIZED,
            OptimizationTier::Baseline => STATUS_FUNCTION | STATUS_BASELINE,
            OptimizationTier::Interpreted => STATUS_FUNCTION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jit_bridge_tiers_up_on_repeated_calls() {
        let mut bridge = JitBridge::new();
        assert_eq!(bridge.tier(1), OptimizationTier::Interpreted);
        for _ in 0..19 {
            bridge.record_call(1);
        }
        assert_eq!(bridge.tier(1), OptimizationTier::Interpreted);
        bridge.record_call(1);
        assert_eq!(bridge.tier(1), OptimizationTier::Baseline);
        for _ in 0..79 {
            bridge.record_call(1);
        }
        assert_eq!(bridge.tier(1), OptimizationTier::Baseline);
        bridge.record_call(1);
        assert_eq!(bridge.tier(1), OptimizationTier::Optimized);
        assert_eq!(bridge.status(1), STATUS_FUNCTION | STATUS_OPTIMIZED);
    }
}
