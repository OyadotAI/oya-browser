/** Iterator cleanup protects an acquired iteration value while preserving the pending abrupt completion. */
use super::*;
use crate::v9::syntax::ForHead;

impl Compiler {
    /** The RHS runs with all lexical head names uninitialized, before the per-iteration records exist. */
    pub(super) fn iteration_rhs(
        &mut self,
        head: &ForHead,
        right: &Expr,
        captured: bool,
    ) -> EngineResult<()> {
        let ForHead::Declare(kind, pattern) = head else {
            return self.expr(right);
        };
        if *kind == crate::v9::syntax::VarKind::Var {
            return self.expr(right);
        }
        let mark = self.f().registers;
        self.push_scope(captured, false);
        let push = captured.then(|| {
            self.f().envs += 1;
            self.emit(Op::PushEnv(0))
        });
        let mut names = Vec::new();
        scan::bound_names(pattern, &mut names);
        for name in names {
            let place = self.declare(&name, Declared::Let)?;
            self.emit(Op::Hole);
            self.set_place(place, 0);
            self.emit(Op::Pop);
        }
        self.expr(right)?;
        let scope = self.scopes.pop().expect("iteration RHS scope");
        if let Some(push) = push {
            self.f().ops[push] = Op::PushEnv(scope.slots);
            self.emit(Op::PopEnv);
            self.f().envs -= 1;
        }
        self.release(mark);
        Ok(())
    }

    /** Only abrupt body exits close an iterator; exhaustion and a continue to this loop do not. */
    pub(super) fn iterator_scope(
        &mut self,
        iterator: Slot,
        next: u32,
        asynchronous: bool,
        body: impl FnOnce(&mut Self) -> EngineResult<()>,
    ) -> EngineResult<()> {
        let mark = self.f().registers;
        let completion = self.temp()?;
        self.temp()?;
        self.temp()?;
        self.f().handlers += 1;
        let handler = self.emit(Op::PushTry(0, 0, completion));
        body(self)?;
        self.emit(Op::PopTry);
        self.f().handlers -= 1;
        self.emit(Op::Normal(completion));
        let finalizer = self.here() as u32;
        self.f().ops[handler] = Op::PushTry(0, finalizer, completion);
        let skip = self.emit(Op::IterCloseCheck(completion, next, 0));
        self.iterator_close(iterator, completion, asynchronous)?;
        self.f().ops[skip] = Op::IterCloseCheck(completion, next, self.here() as u32);
        self.emit(Op::EndFinally(completion));
        self.release(mark);
        Ok(())
    }
    /** A close error replaces return/break completion but cannot replace an existing body throw. */
    fn iterator_close(
        &mut self,
        iterator: Slot,
        completion: Slot,
        asynchronous: bool,
    ) -> EngineResult<()> {
        self.f().handlers += 1;
        let handler = self.emit(Op::PushTry(0, 0, 0));
        self.emit(Op::GetReg(iterator));
        if asynchronous {
            let skip = self.emit(Op::AsyncIterClose(0));
            self.emit(Op::Await);
            self.emit(Op::AsyncCloseCheck);
            self.patch(skip);
        } else {
            self.emit(Op::IterClose);
        }
        self.emit(Op::PopTry);
        self.f().handlers -= 1;
        let done = self.emit(Op::Jump(0));
        let catch = self.here() as u32;
        self.f().ops[handler] = Op::PushTry(catch, 0, 0);
        self.emit(Op::IterCloseError(completion));
        self.patch(done);
        Ok(())
    }
}
