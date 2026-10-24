/** Statement lowering: declarations, scopes, loops, switch, labels and exception handlers. */
use super::scan;
use super::*;
use crate::v9::syntax::{Case, Declarator, ForHead, ForInit, Pattern, StmtKind, VarKind};

impl Compiler {
    /** Top-level declarations: global vars and functions now, global lexical bindings in their dead zone. */
    pub(super) fn global_declarations(&mut self, body: &[Stmt]) -> EngineResult<()> {
        early_conflict(body, self.f().strict, true, &[])?;
        let mut vars = Vec::new();
        scan::var_names(body, self.f().strict, true, &mut vars);
        for name in &vars {
            let index = self.name(name)?;
            self.emit(Op::DeclareVar(index));
            self.declare(name, Declared::Var)?;
        }
        // Hoisted, yet a redeclaration error points at the declaration's own line.
        for (name, kind, pos) in lexical_declarations(body) {
            self.at(pos);
            let index = self.name(&name)?;
            self.emit(Op::DeclareLexical(index, kind == VarKind::Const));
            self.declare(
                &name,
                if kind == VarKind::Const {
                    Declared::Const
                } else {
                    Declared::Let
                },
            )?;
        }
        for func in scan::function_declarations(body) {
            let name = func.name.clone().expect("declaration name");
            self.closure(&func, false)?;
            let index = self.name(&name)?;
            self.emit(Op::DeclareFunction(index));
            self.declare(&name, Declared::Var)?;
        }
        Ok(())
    }
    /** Statements, with top-level expression values stored for eval's completion value. */
    pub(super) fn statements_with_result(
        &mut self,
        body: &[Stmt],
        result: Option<Slot>,
    ) -> EngineResult<()> {
        for stmt in body {
            match (&stmt.kind, result) {
                (StmtKind::Expr(value), Some(result)) => {
                    self.at(stmt.pos);
                    self.expr(value)?;
                    self.emit(Op::SetReg(result));
                    self.emit(Op::Pop);
                }
                _ => self.stmt(stmt)?,
            }
        }
        Ok(())
    }
    /** Statements in order. */
    pub(super) fn statements(&mut self, body: &[Stmt]) -> EngineResult<()> {
        for stmt in body {
            self.stmt(stmt)?;
        }
        Ok(())
    }
    /** One statement. */
    pub(super) fn stmt(&mut self, stmt: &Stmt) -> EngineResult<()> {
        self.at(stmt.pos);
        let labels = std::mem::take(&mut self.f().pending_labels);
        match &stmt.kind {
            StmtKind::Expr(value) => {
                self.expr(value)?;
                self.emit(Op::Pop);
            }
            StmtKind::Var(kind, list) => self.declarations(*kind, list)?,
            StmtKind::Function(func) => {
                // Hoisted; sloppy block functions also update the function-level var (Annex B).
                if !self.f().strict
                    && let Some(name) = &func.name
                    && self.scopes.last().is_some_and(|scope| !scope.boundary)
                {
                    self.load_name(name, stmt.pos)?;
                    self.store_var_outward(name, stmt.pos)?;
                    self.emit(Op::Pop);
                }
            }
            StmtKind::Class(class) => {
                self.class(class)?;
                let name = class.name.clone().expect("class declaration name");
                self.initialize(&name, stmt.pos)?;
                self.emit(Op::Pop);
            }
            StmtKind::Return(value) => self.return_statement(value.as_ref())?,
            StmtKind::If(test, yes, no) => {
                self.expr(test)?;
                let skip = self.emit(Op::JumpIfFalse(0));
                self.stmt(yes)?;
                if let Some(no) = no {
                    let end = self.emit(Op::Jump(0));
                    self.patch(skip);
                    self.stmt(no)?;
                    self.patch(end);
                } else {
                    self.patch(skip);
                }
            }
            StmtKind::Block(body) => self.block(body, labels)?,
            StmtKind::For {
                init,
                test,
                update,
                body,
            } => self.for_loop(
                stmt,
                init.as_ref(),
                test.as_ref(),
                update.as_ref(),
                body,
                labels,
            )?,
            StmtKind::ForIn(head, right, body) => {
                self.for_each(stmt, head, right, body, false, labels)?
            }
            StmtKind::ForOf(head, right, body, is_await) => {
                if *is_await {
                    let index = self.name("for await")?;
                    self.emit(Op::Unsupported(index));
                } else {
                    self.for_each(stmt, head, right, body, true, labels)?
                }
            }
            StmtKind::While(test, body) => {
                self.push_target(labels, TargetKind::Loop);
                let start = self.here();
                self.expr(test)?;
                let exit = self.emit(Op::JumpIfFalse(0));
                self.stmt(body)?;
                self.emit(Op::Jump(start as u32));
                self.patch(exit);
                self.pop_target(start as u32);
            }
            StmtKind::DoWhile(body, test) => {
                self.push_target(labels, TargetKind::Loop);
                let start = self.here();
                self.stmt(body)?;
                let next = self.here() as u32;
                self.expr(test)?;
                self.emit(Op::JumpIfTrue(start as u32));
                self.pop_target(next);
            }
            StmtKind::Break(label) => self.jump(label.as_ref(), true, stmt.pos)?,
            StmtKind::Continue(label) => self.jump(label.as_ref(), false, stmt.pos)?,
            StmtKind::Throw(value) => {
                self.expr(value)?;
                self.at(stmt.pos);
                self.emit(Op::Throw);
            }
            StmtKind::Try {
                block,
                param,
                handler,
                finalizer,
            } => self.try_statement(
                block,
                param.as_ref(),
                handler.as_deref(),
                finalizer.as_deref(),
            )?,
            StmtKind::Switch(value, cases) => self.switch(value, cases, labels)?,
            StmtKind::Labeled(name, body) => {
                let mut labels = labels;
                labels.push(name.clone());
                if is_loop(body) {
                    self.f().pending_labels = labels;
                    self.stmt(body)?;
                } else {
                    self.push_target(labels, TargetKind::Block);
                    self.stmt(body)?;
                    let end = self.here() as u32;
                    self.pop_target(end);
                }
            }
            StmtKind::Empty => {}
            StmtKind::With(..) => {
                let index = self.name("with statements")?;
                self.emit(Op::Unsupported(index));
            }
            StmtKind::Field(key, value) => self.field(key, value.as_ref())?,
        }
        Ok(())
    }
    /** A block with its own scope when it declares anything lexical. */
    fn block(&mut self, body: &[Stmt], labels: Vec<Name>) -> EngineResult<()> {
        let labeled = !labels.is_empty();
        if labeled {
            self.push_target(labels, TargetKind::Block);
        }
        let mark = self.enter_block(body)?;
        self.statements(body)?;
        self.leave_block(mark);
        if labeled {
            let end = self.here() as u32;
            self.pop_target(end);
        }
        Ok(())
    }
    /** Open a block scope for a statement list's lexical declarations; returns cleanup data. */
    pub(super) fn enter_block(
        &mut self,
        body: &[Stmt],
    ) -> EngineResult<Option<(Slot, bool, usize)>> {
        early_conflict(body, self.f().strict, false, &[])?;
        let lexical = scan::lexical_names(body, true);
        if lexical.is_empty() {
            return Ok(None);
        }
        let env = scan::has_closure(body);
        let mark = self.f().registers;
        self.push_scope(env, false);
        let push = if env {
            self.f().envs += 1;
            Some(self.emit(Op::PushEnv(0)))
        } else {
            None
        };
        for (name, kind) in lexical {
            let declared = if kind == VarKind::Const {
                Declared::Const
            } else {
                Declared::Let
            };
            if let Place::Register(slot) = self.declare(&name, declared)? {
                self.emit(Op::Hole);
                self.emit(Op::SetReg(slot));
                self.emit(Op::Pop);
            }
        }
        for func in scan::function_declarations(body) {
            let name = func.name.clone().expect("declaration name");
            self.closure(&func, false)?;
            self.initialize(&name, func.pos)?;
            self.emit(Op::Pop);
        }
        Ok(Some((mark, env, push.unwrap_or(usize::MAX))))
    }
    /** Close a block scope opened by `enter_block`. */
    pub(super) fn leave_block(&mut self, mark: Option<(Slot, bool, usize)>) {
        let Some((registers, env, push)) = mark else {
            return;
        };
        let scope = self.scopes.pop().expect("block scope");
        if env {
            self.f().ops[push] = Op::PushEnv(scope.slots);
            self.emit(Op::PopEnv);
            self.f().envs -= 1;
        }
        self.release(registers);
    }
    /** `var`, `let` and `const` declarations. */
    fn declarations(&mut self, kind: VarKind, list: &[Declarator]) -> EngineResult<()> {
        for declarator in list {
            match &declarator.init {
                Some(init) => {
                    self.expr(init)?;
                    self.bind_pattern(&declarator.target, kind != VarKind::Var)?;
                    self.emit(Op::Pop);
                }
                None if kind == VarKind::Let => {
                    self.emit(Op::Undefined);
                    self.bind_pattern(&declarator.target, true)?;
                    self.emit(Op::Pop);
                }
                None => {}
            }
        }
        Ok(())
    }
    /** `return`, which constructors and derived constructors finish specially. */
    fn return_statement(&mut self, value: Option<&Expr>) -> EngineResult<()> {
        match value {
            Some(value) => self.expr(value)?,
            None => {
                self.emit(Op::Undefined);
            }
        }
        if self.f().flags.derived {
            self.load_this_raw()?;
            self.emit(Op::ReturnDerived);
        } else {
            self.emit(Op::Return);
        }
        Ok(())
    }
    /** Push a break/continue target. */
    fn push_target(&mut self, labels: Vec<Name>, kind: TargetKind) {
        let (handlers, envs) = (self.f().handlers, self.f().envs);
        self.f().targets.push(Target {
            labels,
            kind,
            breaks: Vec::new(),
            continues: Vec::new(),
            handlers,
            envs,
        });
    }
    /** Pop a target, pointing continues at `next` and breaks here. */
    fn pop_target(&mut self, next: u32) {
        let target = self.f().targets.pop().expect("target");
        for index in target.continues {
            self.patch_to(index, next);
        }
        for index in target.breaks {
            self.patch(index);
        }
    }
    /** `break` and `continue`, leaving handlers and environments on the way. */
    fn jump(&mut self, label: Option<&Name>, is_break: bool, pos: Pos) -> EngineResult<()> {
        let targets = &self.f().targets;
        let index = match label {
            Some(label) => targets
                .iter()
                .rposition(|target| target.labels.contains(label)),
            None if is_break => targets
                .iter()
                .rposition(|target| target.kind != TargetKind::Block),
            None => targets
                .iter()
                .rposition(|target| target.kind == TargetKind::Loop),
        }
        .ok_or_else(|| self.error(pos, "Illegal break or continue target"))?;
        let (handlers, envs) = {
            let target = &self.f().targets[index];
            (target.handlers, target.envs)
        };
        let jump = if handlers != self.f().handlers {
            self.emit(Op::JumpOut(0, handlers, envs))
        } else {
            for _ in envs..self.f().envs {
                self.emit(Op::PopEnv);
            }
            self.emit(Op::Jump(0))
        };
        let target = &mut self.f().targets[index];
        if is_break {
            target.breaks.push(jump);
        } else {
            target.continues.push(jump);
        }
        Ok(())
    }
    /** A C-style for loop; `let` bindings are copied per iteration when closures can see them. */
    fn for_loop(
        &mut self,
        stmt: &Stmt,
        init: Option<&ForInit>,
        test: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
        labels: Vec<Name>,
    ) -> EngineResult<()> {
        let lexical = match init {
            Some(ForInit::Declare(kind @ (VarKind::Let | VarKind::Const), list)) => {
                Some((*kind, list))
            }
            _ => None,
        };
        let mut scope = None;
        if let Some((_, list)) = lexical {
            let mut names = Vec::new();
            for declarator in list {
                scan::bound_names(&declarator.target, &mut names);
            }
            head_conflict(stmt, &names, body)?;
        }
        if let Some((kind, list)) = lexical {
            let env = scan::stmt_has_closure(stmt);
            let mark = self.f().registers;
            self.push_scope(env, false);
            let push = env.then(|| {
                self.f().envs += 1;
                self.emit(Op::PushEnv(0))
            });
            for declarator in list {
                let mut names = Vec::new();
                scan::bound_names(&declarator.target, &mut names);
                for name in names {
                    let declared = if kind == VarKind::Const {
                        Declared::Const
                    } else {
                        Declared::Let
                    };
                    if let Place::Register(slot) = self.declare(&name, declared)? {
                        self.emit(Op::Hole);
                        self.emit(Op::SetReg(slot));
                        self.emit(Op::Pop);
                    }
                }
            }
            scope = Some((mark, env, push));
        }
        match init {
            Some(ForInit::Declare(kind, list)) => self.declarations(*kind, list)?,
            Some(ForInit::Expr(value)) => {
                self.expr(value)?;
                self.emit(Op::Pop);
            }
            None => {}
        }
        self.push_target(labels, TargetKind::Loop);
        let start = self.here();
        let exit = match test {
            Some(test) => {
                self.expr(test)?;
                Some(self.emit(Op::JumpIfFalse(0)))
            }
            None => None,
        };
        self.stmt(body)?;
        let next = self.here() as u32;
        if scope.as_ref().is_some_and(|(_, env, _)| *env) {
            self.emit(Op::CopyEnv);
        }
        if let Some(update) = update {
            self.expr(update)?;
            self.emit(Op::Pop);
        }
        self.emit(Op::Jump(start as u32));
        if let Some(exit) = exit {
            self.patch(exit);
        }
        self.pop_target(next);
        if let Some((mark, env, push)) = scope {
            let block = self.scopes.pop().expect("for scope");
            if env {
                if let Some(push) = push {
                    self.f().ops[push] = Op::PushEnv(block.slots);
                }
                self.emit(Op::PopEnv);
                self.f().envs -= 1;
            }
            self.release(mark);
        }
        Ok(())
    }
    /** for-in and for-of; the iterator lives in a register so jumps never strand stack values. */
    fn for_each(
        &mut self,
        stmt: &Stmt,
        head: &ForHead,
        right: &Expr,
        body: &Stmt,
        of: bool,
        labels: Vec<Name>,
    ) -> EngineResult<()> {
        let mark = self.f().registers;
        let iterator = self.temp()?;
        self.expr(right)?;
        self.emit(if of { Op::GetIterator } else { Op::ForIn });
        self.emit(Op::SetReg(iterator));
        self.emit(Op::Pop);
        self.push_target(labels, TargetKind::Loop);
        let start = self.here();
        self.emit(Op::GetReg(iterator));
        let exit = self.emit(if of {
            Op::IterNext(0)
        } else {
            Op::ForInNext(0)
        });
        // Each iteration gets fresh lexical bindings.
        let lexical = matches!(head, ForHead::Declare(VarKind::Let | VarKind::Const, _));
        let mut scope = None;
        if let ForHead::Declare(kind @ (VarKind::Let | VarKind::Const), pattern) = head {
            let env = scan::stmt_has_closure(stmt);
            let inner = self.f().registers;
            self.push_scope(env, false);
            let push = env.then(|| {
                self.f().envs += 1;
                self.emit(Op::PushEnv(0))
            });
            let mut names = Vec::new();
            scan::bound_names(pattern, &mut names);
            head_conflict(stmt, &names, body)?;
            for name in names {
                self.declare(
                    &name,
                    if *kind == VarKind::Const {
                        Declared::Const
                    } else {
                        Declared::Let
                    },
                )?;
            }
            scope = Some((inner, env, push));
        }
        match head {
            ForHead::Declare(_, pattern) => self.bind_pattern(pattern, lexical)?,
            ForHead::Target(pattern) => self.assign_pattern(pattern)?,
        }
        self.emit(Op::Pop);
        self.stmt(body)?;
        if let Some((inner, env, push)) = scope {
            let block = self.scopes.pop().expect("loop scope");
            if env {
                if let Some(push) = push {
                    self.f().ops[push] = Op::PushEnv(block.slots);
                }
                self.emit(Op::PopEnv);
                self.f().envs -= 1;
            }
            self.release(inner);
        }
        self.emit(Op::Jump(start as u32));
        // Breaks close the iterator; exhaustion has already consumed it.
        let target = self.f().targets.pop().expect("loop target");
        for index in target.continues {
            self.patch_to(index, start as u32);
        }
        let breaks = target.breaks;
        let done = self.emit(Op::Jump(0));
        for index in breaks {
            self.patch(index);
        }
        if of {
            self.emit(Op::GetReg(iterator));
            self.emit(Op::IterClose);
        }
        self.patch(done);
        self.patch(exit);
        self.release(mark);
        Ok(())
    }
    /** `switch`: one scope for all clauses, strict-equality dispatch, fallthrough bodies. */
    fn switch(&mut self, value: &Expr, cases: &[Case], labels: Vec<Name>) -> EngineResult<()> {
        let mark = self.f().registers;
        let discriminant = self.temp()?;
        self.expr(value)?;
        self.emit(Op::SetReg(discriminant));
        self.emit(Op::Pop);
        let all: Vec<&Stmt> = cases.iter().flat_map(|case| case.body.iter()).collect();
        let block_mark = self.enter_block_refs(&all)?;
        let mut jumps = Vec::new();
        for case in cases {
            if let Some(test) = &case.test {
                self.emit(Op::GetReg(discriminant));
                self.expr(test)?;
                self.emit(Op::StrictEq);
                jumps.push(Some(self.emit(Op::JumpIfTrue(0))));
            } else {
                jumps.push(None);
            }
        }
        let fallback = self.emit(Op::Jump(0));
        self.push_target(labels, TargetKind::Switch);
        let mut default = None;
        for (case, jump) in cases.iter().zip(jumps) {
            match jump {
                Some(jump) => self.patch(jump),
                None => default = Some(self.here()),
            }
            self.statements(&case.body)?;
        }
        match default {
            Some(default) => self.patch_to(fallback, default as u32),
            None => self.patch(fallback),
        }
        let end = self.here() as u32;
        self.pop_target(end);
        self.leave_block(block_mark);
        self.release(mark);
        Ok(())
    }
    /** `enter_block` over statements gathered from several clauses. */
    fn enter_block_refs(&mut self, body: &[&Stmt]) -> EngineResult<Option<(Slot, bool, usize)>> {
        // Lexical names and closures are found per statement, so a borrowed list suffices.
        let mut lexical = Vec::new();
        let mut functions = Vec::new();
        let mut env = false;
        for stmt in body {
            lexical.extend(scan::lexical_names(std::slice::from_ref(*stmt), true));
            functions.extend(scan::function_declarations(std::slice::from_ref(*stmt)));
            env |= scan::stmt_has_closure(stmt);
        }
        if lexical.is_empty() {
            return Ok(None);
        }
        let mark = self.f().registers;
        self.push_scope(env, false);
        let push = if env {
            self.f().envs += 1;
            self.emit(Op::PushEnv(0))
        } else {
            usize::MAX
        };
        for (name, kind) in lexical {
            let declared = if kind == VarKind::Const {
                Declared::Const
            } else {
                Declared::Let
            };
            if let Place::Register(slot) = self.declare(&name, declared)? {
                self.emit(Op::Hole);
                self.emit(Op::SetReg(slot));
                self.emit(Op::Pop);
            }
        }
        for func in functions {
            let name = func.name.clone().expect("declaration name");
            self.closure(&func, false)?;
            self.initialize(&name, func.pos)?;
            self.emit(Op::Pop);
        }
        Ok(Some((mark, env, push)))
    }
    /** try/catch/finally with handler records and a saved completion for finally. */
    fn try_statement(
        &mut self,
        block: &[Stmt],
        param: Option<&Pattern>,
        handler: Option<&[Stmt]>,
        finalizer: Option<&[Stmt]>,
    ) -> EngineResult<()> {
        let mark = self.f().registers;
        let completion = if finalizer.is_some() {
            let first = self.temp()?;
            self.temp()?;
            self.temp()?;
            Some(first)
        } else {
            None
        };
        let outer = match completion {
            Some(slot) => {
                self.f().handlers += 1;
                Some(self.emit(Op::PushTry(0, 0, slot)))
            }
            None => None,
        };
        let inner = handler.map(|_| {
            self.f().handlers += 1;
            self.emit(Op::PushTry(0, 0, 0))
        });
        self.block(block, Vec::new())?;
        if let (Some(handler), Some(inner)) = (handler, inner) {
            self.emit(Op::PopTry);
            self.f().handlers -= 1;
            let skip = self.emit(Op::Jump(0));
            let catch = self.here() as u32;
            self.f().ops[inner] = Op::PushTry(catch, 0, 0);
            self.catch_clause(param, handler)?;
            self.patch(skip);
        }
        if let (Some(finalizer), Some(outer), Some(slot)) = (finalizer, outer, completion) {
            self.emit(Op::PopTry);
            self.f().handlers -= 1;
            self.emit(Op::Normal(slot));
            let start = self.here() as u32;
            self.f().ops[outer] = Op::PushTry(0, start, slot);
            self.statements_scoped(finalizer)?;
            self.emit(Op::EndFinally(slot));
        }
        self.release(mark);
        Ok(())
    }
    /** A finally body as its own block. */
    fn statements_scoped(&mut self, body: &[Stmt]) -> EngineResult<()> {
        let mark = self.enter_block(body)?;
        self.statements(body)?;
        self.leave_block(mark);
        Ok(())
    }
    /** The catch clause: the thrown value is on the stack. */
    fn catch_clause(&mut self, param: Option<&Pattern>, body: &[Stmt]) -> EngineResult<()> {
        let Some(param) = param else {
            self.emit(Op::Pop);
            return self.block(body, Vec::new());
        };
        let mut names = Vec::new();
        scan::bound_names(param, &mut names);
        let env = scan::has_closure(body);
        let mark = self.f().registers;
        self.push_scope(env, false);
        let push = env.then(|| {
            self.f().envs += 1;
            self.emit(Op::PushEnv(0))
        });
        for name in &names {
            self.declare(name, Declared::Let)?;
        }
        self.bind_pattern(param, true)?;
        self.emit(Op::Pop);
        self.block(body, Vec::new())?;
        let scope = self.scopes.pop().expect("catch scope");
        if let Some(push) = push {
            self.f().ops[push] = Op::PushEnv(scope.slots);
            self.emit(Op::PopEnv);
            self.f().envs -= 1;
        }
        self.release(mark);
        Ok(())
    }
}

/** Loops take their labels directly so `continue label` reaches them. */
fn is_loop(stmt: &Stmt) -> bool {
    matches!(
        stmt.kind,
        StmtKind::For { .. }
            | StmtKind::ForIn(..)
            | StmtKind::ForOf(..)
            | StmtKind::While(..)
            | StmtKind::DoWhile(..)
    )
}

/** Top-level lexical names with the position of the statement declaring each. */
fn lexical_declarations(body: &[Stmt]) -> Vec<(Name, VarKind, Pos)> {
    body.iter()
        .flat_map(|stmt| {
            scan::lexical_names(std::slice::from_ref(stmt), false)
                .into_iter()
                .map(move |(name, kind)| (name, kind, stmt.pos))
        })
        .collect()
}

/** A conflicting declaration in a statement list is an early SyntaxError for the whole script. */
pub(super) fn early_conflict(
    body: &[Stmt],
    strict: bool,
    top: bool,
    outer: &[Name],
) -> EngineResult<()> {
    match (scan::conflict(body, strict, top, outer), body.first()) {
        (Some(_), Some(first)) => Err(errors::syntax(
            first.pos.location(),
            "SyntaxError: Identifier has already been declared",
        )),
        _ => Ok(()),
    }
}

/** A loop's lexical head names may not be redeclared by a `var` in its body. */
fn head_conflict(stmt: &Stmt, names: &[Name], body: &Stmt) -> EngineResult<()> {
    if scan::stmt_var_names(body)
        .iter()
        .any(|name| names.contains(name))
    {
        return Err(errors::syntax(
            stmt.pos.location(),
            "SyntaxError: Identifier has already been declared",
        ));
    }
    Ok(())
}
