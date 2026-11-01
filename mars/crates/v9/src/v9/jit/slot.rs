/**
 * Per-function JIT state, carried on each compiled `Code`: a heat counter that calls and taken
 * backward jumps bump, and the compiled code once the function has been compiled or refused.
 * The counter is one relaxed atomic add, so the interpreter pays almost nothing for tier-up.
 */
use super::compiled::{CompiledCode, compile};
use super::policy;
use crate::v9::compile::Code;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/** Heat and compiled state of one function. */
pub(in crate::v9) struct JitSlot {
    /** Calls plus taken backward jumps seen so far. */
    heat: AtomicU32,
    /** Set once: `Some` when compiled, `None` when the function was tried and refused. */
    state: OnceLock<Option<CompiledCode>>,
}

impl Default for JitSlot {
    /** A function that has not been called yet: no heat and nothing compiled. */
    fn default() -> Self {
        Self {
            heat: AtomicU32::new(0),
            state: OnceLock::new(),
        }
    }
}

impl JitSlot {
    /** Counts one call or taken backward jump. Returns true on the exact step that reaches the
    threshold, which tells the interpreter to look for compiled code at its next boundary. */
    pub(in crate::v9) fn warm(&self) -> bool {
        if !policy::enabled() {
            return false;
        }
        let heat = self.heat.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        heat == policy::threshold()
    }
    /** The compiled code for `code`, compiling it now if it is hot and has not been tried. */
    pub(in crate::v9) fn compiled(&self, code: &Code) -> Option<&CompiledCode> {
        if let Some(state) = self.state.get() {
            return state.as_ref();
        }
        if !policy::enabled() || self.heat.load(Ordering::Relaxed) < policy::threshold() {
            return None;
        }
        self.state.get_or_init(|| compile(code)).as_ref()
    }
}
