/** Lexical strict expressions carry immutable instruction ranges without changing their enclosing function's activation mode. */
use super::*;

impl FnState {
    /** A class expression's eval and reference semantics inherit lexical strictness, unlike caller and arguments metadata. */
    pub(super) fn effective_strict(&self) -> bool {
        self.strict || self.lexical_strict
    }

    /** Emission order produces sorted, nonoverlapping ranges, merging adjacent strict instructions without extra bytecode. */
    pub(super) fn record_strict(&mut self, pc: u32) {
        if self.strict || !self.lexical_strict {
            return;
        }
        if let Some((_, end)) = self.strict_ranges.last_mut()
            && *end == pc
        {
            *end = pc + 1;
        } else {
            self.strict_ranges.push((pc, pc + 1));
        }
    }
}

impl Compiler {
    /** Compile-time context is restored even on an early error; no runtime flag must survive a throw, jump or suspension. */
    pub(super) fn strict_region<T>(
        &mut self,
        emit: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<T> {
        let outer = self.f().lexical_strict;
        self.f().lexical_strict = true;
        let result = emit(self);
        self.f().lexical_strict = outer;
        result
    }
}

impl Code {
    /** The instruction's immutable lexical mode is rebuilt from trusted source together with its validated program counter. */
    #[inline]
    pub(in crate::v9) fn strict_at(&self, pc: usize) -> bool {
        if self.strict {
            return true;
        }
        let index = self
            .strict_ranges
            .partition_point(|(_, end)| (*end as usize) <= pc);
        self.strict_ranges
            .get(index)
            .is_some_and(|(start, end)| (*start as usize) <= pc && pc < (*end as usize))
    }
}
