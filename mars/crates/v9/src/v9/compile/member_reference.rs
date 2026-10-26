/* Ordinary computed references evaluate their key expression once and canonicalize it only after checking the base. */
use super::*;

impl Compiler {
    /** Compound reads retain one canonical key for both GetValue and PutValue, including across RHS side effects. */
    pub(super) fn element_key(&mut self, key: &Expr) -> EngineResult<()> {
        self.expr(key)?;
        self.emit(Op::ToElementKey);
        Ok(())
    }
}
