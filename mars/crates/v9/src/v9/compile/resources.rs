/** Lexical resource regions reuse ordinary finally completion records for every abrupt exit. */
use super::*;
use crate::v9::syntax::{StmtKind, VarKind};

impl Compiler {
    /** Resource acquisition stays in the current region until all declarations and body effects finish. */
    pub(super) fn resource_add(&mut self, kind: VarKind) {
        if kind.resource() {
            let stack = *self
                .f()
                .resources
                .last()
                .expect("resource declaration region");
            self.emit(Op::ResourceAdd(stack, kind == VarKind::AwaitUsing));
        }
    }
    /** A region saves pending throws before disposal, then resumes the original completion unless disposal failed. */
    pub(super) fn resource_scope(
        &mut self,
        asynchronous: bool,
        body: impl FnOnce(&mut Self) -> EngineResult<()>,
    ) -> EngineResult<()> {
        let mark = self.f().registers;
        let stack = self.temp()?;
        let completion = self.temp()?;
        self.temp()?;
        self.temp()?;
        self.emit(Op::ResourceCreate);
        self.emit(Op::SetReg(stack));
        self.emit(Op::Pop);
        self.f().resources.push(stack);
        self.f().handlers += 1;
        let handler = self.emit(Op::PushTry(0, 0, completion));
        body(self)?;
        self.f().resources.pop();
        self.emit(Op::PopTry);
        self.f().handlers -= 1;
        self.emit(Op::Normal(completion));
        let finalizer = self.here() as u32;
        self.f().ops[handler] = Op::PushTry(0, finalizer, completion);
        if asynchronous {
            self.resource_async_finalizer(stack, completion)?;
        } else {
            self.emit(Op::ResourceDispose(stack, completion));
            self.emit(Op::Pop);
        }
        self.emit(Op::EndFinally(completion));
        self.release(mark);
        Ok(())
    }
    /** Each await resumes this frame directly, preserving the language's disposal job ordering. */
    fn resource_async_finalizer(&mut self, stack: Slot, completion: Slot) -> EngineResult<()> {
        let awaited = self.temp()?;
        self.emit(Op::ResourceBegin(stack, completion));
        let start = self.here() as u32;
        let step = self.emit(Op::ResourceStep(stack, awaited, 0));
        self.f().handlers += 1;
        let handler = self.emit(Op::PushTry(0, 0, 0));
        self.emit(Op::GetReg(awaited));
        self.emit(Op::Await);
        self.emit(Op::PopTry);
        self.f().handlers -= 1;
        self.emit(Op::Pop);
        self.emit(Op::Jump(start));
        let catch = self.here() as u32;
        self.f().ops[handler] = Op::PushTry(catch, 0, 0);
        self.emit(Op::ResourceRejected(stack));
        self.emit(Op::Jump(start));
        let end = self.here() as u32;
        self.f().ops[step] = Op::ResourceStep(stack, awaited, end);
        Ok(())
    }
}

/** Only direct declarations belong to this region; nested statement lists create their own regions. */
pub(super) fn resource_kind(body: &[Stmt]) -> Option<bool> {
    let mut result = None;
    for stmt in body {
        if let StmtKind::Var(kind, _) = stmt.kind
            && kind.resource()
        {
            result = Some(result.unwrap_or(false) || kind == VarKind::AwaitUsing);
        }
    }
    result
}
