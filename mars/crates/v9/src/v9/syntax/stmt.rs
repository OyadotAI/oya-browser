/** Statement grammar: declarations, control flow, exceptions and labels. */
use super::func::bound_names;
use super::*;

impl Parser {
    /** Statements until `}` or the end of input. */
    pub(super) fn statement_list(&mut self, top: bool) -> EngineResult<Vec<Stmt>> {
        let mut body = Vec::new();
        while !matches!(self.peek().kind, Kind::End) && (top || !self.at("}")) {
            if top && self.at("}") {
                return Err(self.unexpected());
            }
            body.push(self.statement(true)?);
        }
        Ok(body)
    }
    /** One statement; `list_item` admits declarations. */
    pub(super) fn statement(&mut self, list_item: bool) -> EngineResult<Stmt> {
        let pos = self.pos();
        self.count_statement(pos)?;
        let kind = self.enter_statement(|parser| parser.statement_kind(list_item))?;
        Ok(Stmt { kind, pos })
    }
    /** Dispatch on the leading token. */
    fn statement_kind(&mut self, list_item: bool) -> EngineResult<StmtKind> {
        if self.at("{") {
            return Ok(StmtKind::Block(self.block()?));
        }
        if self.eat(";") {
            return Ok(StmtKind::Empty);
        }
        let Kind::Name(name) = &self.peek().kind else {
            return self.expression_statement();
        };
        if self.module_item() {
            return self.module_declaration();
        }
        match name.as_str() {
            "var" => {
                self.next();
                let list = self.declarations(VarKind::Var, false)?;
                self.semicolon()?;
                Ok(StmtKind::Var(VarKind::Var, list))
            }
            "const" | "class" if !list_item => Err(self.error_at(
                self.pos(),
                "Lexical declaration cannot appear in a single-statement context",
            )),
            "let" if !list_item && self.let_declaration() => Err(self.error_at(
                self.pos(),
                "Lexical declaration cannot appear in a single-statement context",
            )),
            "const" => {
                self.next();
                let list = self.declarations(VarKind::Const, false)?;
                self.semicolon()?;
                Ok(StmtKind::Var(VarKind::Const, list))
            }
            "let" if self.let_declaration() => {
                self.next();
                let list = self.declarations(VarKind::Let, false)?;
                self.semicolon()?;
                Ok(StmtKind::Var(VarKind::Let, list))
            }
            "function" => {
                let pos = self.pos();
                self.next();
                let func = self.function_rest(pos, false, true)?;
                Ok(StmtKind::Function(func))
            }
            "async"
                if matches!(&self.peek_at(1).kind, Kind::Name(n) if n == "function")
                    && !self.peek_at(1).line_terminator_before =>
            {
                let pos = self.pos();
                self.next();
                self.next();
                let func = self.function_rest(pos, true, true)?;
                Ok(StmtKind::Function(func))
            }
            "class" => {
                let class = self.class(true)?;
                Ok(StmtKind::Class(class))
            }
            "if" => self.if_statement(),
            "for" => self.for_statement(),
            "while" => {
                self.next();
                let test = self.parenthesized()?;
                let body = self.loop_body()?;
                Ok(StmtKind::While(test, Box::new(body)))
            }
            "do" => {
                self.next();
                let body = self.loop_body()?;
                self.expect_name("while")?;
                let test = self.parenthesized()?;
                self.eat(";");
                Ok(StmtKind::DoWhile(Box::new(body), test))
            }
            "return" => {
                let pos = self.pos();
                self.next();
                if !self.context.function {
                    return Err(self.error_at(pos, "Illegal return statement"));
                }
                let value = if self.at(";")
                    || self.at("}")
                    || self.peek().line_terminator_before
                    || matches!(self.peek().kind, Kind::End)
                {
                    None
                } else {
                    Some(self.expression(false)?)
                };
                self.semicolon()?;
                Ok(StmtKind::Return(value))
            }
            "break" | "continue" => self.jump(),
            "throw" => {
                self.next();
                if self.peek().line_terminator_before {
                    return Err(self.unexpected());
                }
                let value = self.expression(false)?;
                self.semicolon()?;
                Ok(StmtKind::Throw(value))
            }
            "try" => self.try_statement(),
            "switch" => self.switch_statement(),
            "debugger" => {
                self.next();
                self.semicolon()?;
                Ok(StmtKind::Empty)
            }
            "with" => {
                let pos = self.pos();
                self.next();
                if self.strict {
                    return Err(self.error_at(pos, "with is not allowed in strict mode"));
                }
                self.parenthesized()?;
                let body = self.sub_statement(false)?;
                Ok(StmtKind::With(Box::new(body)))
            }
            "import" | "export" if !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '.')) => {
                Err(self.unexpected())
            }
            _ if matches!(self.peek_at(1).kind, Kind::Symbol(':')) && self.at_identifier() => {
                self.labeled(list_item)
            }
            _ => self.expression_statement(),
        }
    }
    /** `let` starts a declaration only before a binding name or pattern. */
    fn let_declaration(&self) -> bool {
        match &self.peek_at(1).kind {
            Kind::Symbol('[' | '{') => true,
            Kind::Name(name) => {
                !(self.peek_at(1).line_terminator_before
                    && ["in", "instanceof", "of"].contains(&name.as_str()))
            }
            _ => false,
        }
    }
    /** `{ ... }` */
    pub(super) fn block(&mut self) -> EngineResult<Vec<Stmt>> {
        self.expect("{")?;
        let body = self.statement_list(false)?;
        self.expect("}")?;
        self.check_redeclarations(&body)?;
        Ok(body)
    }
    /** The body of `if`, a loop or `with`: never a declaration, except that sloppy code may
    give `if` a plain function declaration (Annex B.3.3). */
    fn sub_statement(&mut self, if_branch: bool) -> EngineResult<Stmt> {
        let async_function = self.at_name("async")
            && matches!(&self.peek_at(1).kind, Kind::Name(n) if n == "function")
            && !self.peek_at(1).line_terminator_before;
        let function = self.at_name("function")
            && (!if_branch
                || self.strict
                || matches!(self.peek_at(1).kind, Kind::Operator(Operator::Multiply)));
        if async_function || function {
            return Err(self.error_at(
                self.pos(),
                "In strict mode code, functions can only be declared at top level or inside a block.",
            ));
        }
        self.statement(false)
    }
    /** A block's (or case block's) lexical names must be unique, apart from sloppy plain
    function pairs, and no `var` in the same list may reuse one. */
    fn check_redeclarations<'a>(
        &self,
        body: impl IntoIterator<Item = &'a Stmt>,
    ) -> EngineResult<()> {
        let mut lexical: Vec<(Name, bool, Pos)> = Vec::new();
        let mut vars = Vec::new();
        for stmt in body {
            let mut names = Vec::new();
            let mut plain = false;
            match &stmt.kind {
                StmtKind::Var(VarKind::Var, list) => {
                    list.iter().for_each(|d| bound_names(&d.target, &mut vars));
                }
                StmtKind::Var(_, list) => {
                    list.iter().for_each(|d| bound_names(&d.target, &mut names))
                }
                StmtKind::Class(class) => names.extend(class.name.clone()),
                StmtKind::Function(func) => {
                    names.extend(func.name.clone());
                    plain = !self.strict && !func.flags.generator && !func.flags.asynchronous;
                }
                _ => {}
            }
            for name in names {
                let clash = lexical
                    .iter()
                    .any(|(seen, merges, _)| *seen == name && !(plain && *merges));
                if clash {
                    return Err(self.error_at(stmt.pos, "Identifier has already been declared"));
                }
                lexical.push((name, plain, stmt.pos));
            }
        }
        match lexical.iter().find(|(name, _, _)| vars.contains(name)) {
            Some((_, _, pos)) => Err(self.error_at(*pos, "Identifier has already been declared")),
            None => Ok(()),
        }
    }
    /** Expression statements, including the leading-`let` identifier case. */
    fn expression_statement(&mut self) -> EngineResult<StmtKind> {
        let value = self.expression(false)?;
        self.semicolon()?;
        Ok(StmtKind::Expr(value))
    }
    /** A comma-separated declaration list; `for_head` leaves `in`/`of` for the loop. */
    pub(super) fn declarations(
        &mut self,
        kind: VarKind,
        for_head: bool,
    ) -> EngineResult<Vec<Declarator>> {
        let mut list = Vec::new();
        let mut names = Vec::new();
        loop {
            let pos = self.pos();
            let target = self.binding_target()?;
            if kind != VarKind::Var {
                bound_names(&target, &mut names);
                let duplicate = names
                    .iter()
                    .enumerate()
                    .any(|(i, n)| names[..i].contains(n));
                if duplicate || names.iter().any(|n| &**n == "let") {
                    return Err(self.error_at(
                        pos,
                        "let is disallowed as a lexically bound name, and names may not repeat",
                    ));
                }
            }
            let init = if self.eat("=") {
                let mut value = self.assignment(for_head)?;
                if let Pattern::Ident(name, _) = &target {
                    infer_name(&mut value, name);
                }
                Some(value)
            } else {
                if !for_head && (kind == VarKind::Const || !matches!(target, Pattern::Ident(..))) {
                    return Err(self.error_at(
                        self.pos(),
                        "Missing initializer in destructuring or const declaration",
                    ));
                }
                None
            };
            list.push(Declarator { target, init });
            if !self.eat(",") {
                return Ok(list);
            }
        }
    }
    /** `( expression )` */
    fn parenthesized(&mut self) -> EngineResult<Expr> {
        self.expect("(")?;
        let value = self.expression(false)?;
        self.expect(")")?;
        Ok(value)
    }
    /** `if (test) yes else no` */
    fn if_statement(&mut self) -> EngineResult<StmtKind> {
        self.next();
        let test = self.parenthesized()?;
        let yes = self.sub_statement(true)?;
        let no = if self.eat_name("else") {
            Some(Box::new(self.sub_statement(true)?))
        } else {
            None
        };
        Ok(StmtKind::If(test, Box::new(yes), no))
    }
    /** A loop body sees `break` and `continue` as legal. */
    fn loop_body(&mut self) -> EngineResult<Stmt> {
        let saved = self.context;
        self.context.breakable = true;
        self.context.continuable = true;
        let body = self.sub_statement(false);
        self.context = saved;
        body
    }
    /** C-style, for-in, for-of and for-await loops. */
    fn for_statement(&mut self) -> EngineResult<StmtKind> {
        self.next();
        let is_await = self.eat_name("await");
        self.expect("(")?;
        let init = if self.at(";") {
            None
        } else if self.at_name("var")
            || self.at_name("const")
            || self.at_name("let") && self.let_declaration()
        {
            let kind = match self.next().kind {
                Kind::Name(name) if name == "var" => VarKind::Var,
                Kind::Name(name) if name == "let" => VarKind::Let,
                _ => VarKind::Const,
            };
            let list = self.declarations(kind, true)?;
            if list.len() == 1 && (self.at_name("in") || self.at_name("of")) {
                let declarator = list.into_iter().next().expect("one declarator");
                // Only sloppy `for (var x = init in ...)` keeps an initializer (Annex B.3.5).
                let legacy = self.at_name("in")
                    && !self.strict
                    && kind == VarKind::Var
                    && matches!(declarator.target, Pattern::Ident(..));
                if declarator.init.is_some() && !legacy {
                    return Err(self.error_at(
                        self.pos(),
                        "for-in/of loop variable declaration may not have an initializer.",
                    ));
                }
                return self.for_each(ForHead::Declare(kind, declarator.target), is_await);
            }
            // Only a for-in/of head may omit initializers; a plain for head follows the
            // ordinary declaration rule.
            if list.iter().any(|declarator| {
                declarator.init.is_none()
                    && (kind == VarKind::Const || !matches!(declarator.target, Pattern::Ident(..)))
            }) {
                return Err(self.error_at(
                    self.pos(),
                    "Missing initializer in destructuring or const declaration",
                ));
            }
            Some(ForInit::Declare(kind, list))
        } else {
            let start = self.position;
            let value = self.expression(true)?;
            if self.at_name("of")
                && !(matches!(&value.kind, ExprKind::Ident(n) if &**n == "async")
                    && self.position == start + 1)
                || self.at_name("in")
            {
                let target = self.pattern_of(value)?;
                return self.for_each(ForHead::Target(target), is_await);
            }
            Some(ForInit::Expr(value))
        };
        self.expect(";")?;
        let test = if self.at(";") {
            None
        } else {
            Some(self.expression(false)?)
        };
        self.expect(";")?;
        let update = if self.at(")") {
            None
        } else {
            Some(self.expression(false)?)
        };
        self.expect(")")?;
        let body = self.loop_body()?;
        Ok(StmtKind::For {
            init,
            test,
            update,
            body: Box::new(body),
        })
    }
    /** The rest of a for-in or for-of loop after its head. */
    fn for_each(&mut self, head: ForHead, is_await: bool) -> EngineResult<StmtKind> {
        let of = self.at_name("of");
        self.next();
        let right = if of {
            self.assignment(false)?
        } else {
            self.expression(false)?
        };
        self.expect(")")?;
        let body = self.loop_body()?;
        Ok(if of {
            StmtKind::ForOf(head, right, Box::new(body), is_await)
        } else {
            StmtKind::ForIn(head, right, Box::new(body))
        })
    }
    /** `break` and `continue`, with optional labels checked against enclosing ones. */
    fn jump(&mut self) -> EngineResult<StmtKind> {
        let pos = self.pos();
        let is_break = self.at_name("break");
        self.next();
        let label = if !self.peek().line_terminator_before && self.at_identifier() {
            let name = self.identifier()?;
            let found = self.labels.iter().rev().find(|(label, _)| *label == name);
            match found {
                Some((_, is_loop)) if is_break || *is_loop => {}
                _ => return Err(self.error_at(pos, "Undefined label")),
            }
            Some(name)
        } else {
            if is_break && !self.context.breakable || !is_break && !self.context.continuable {
                return Err(self.error_at(
                    pos,
                    if is_break {
                        "Illegal break statement"
                    } else {
                        "Illegal continue statement"
                    },
                ));
            }
            None
        };
        self.semicolon()?;
        Ok(if is_break {
            StmtKind::Break(label)
        } else {
            StmtKind::Continue(label)
        })
    }
    /** `label: statement` */
    fn labeled(&mut self, list_item: bool) -> EngineResult<StmtKind> {
        let name = self.identifier()?;
        self.expect(":")?;
        let is_loop = matches!(&self.peek().kind, Kind::Name(n) if ["for", "while", "do"].contains(&n.as_str()));
        self.labels.push((name.clone(), is_loop));
        // A labelled function is sloppy Annex B grammar, and only where a declaration may stand.
        let body = if self.at_name("function") {
            if !list_item
                || self.strict
                || matches!(self.peek_at(1).kind, Kind::Operator(Operator::Multiply))
            {
                self.labels.pop();
                return Err(self.error_at(self.pos(), "In strict mode code, functions can only be declared at top level or inside a block."));
            }
            self.statement(list_item)
        } else {
            self.statement(false)
        };
        self.labels.pop();
        Ok(StmtKind::Labeled(name, Box::new(body?)))
    }
    /** `try {} catch (e) {} finally {}` */
    fn try_statement(&mut self) -> EngineResult<StmtKind> {
        self.next();
        let block = self.block()?;
        let mut param = None;
        let mut handler = None;
        if self.eat_name("catch") {
            if self.eat("(") {
                param = Some(self.binding_target()?);
                self.expect(")")?;
            }
            handler = Some(self.block()?);
        }
        let finalizer = if self.eat_name("finally") {
            Some(self.block()?)
        } else {
            None
        };
        if handler.is_none() && finalizer.is_none() {
            return Err(self.unexpected());
        }
        Ok(StmtKind::Try {
            block,
            param,
            handler,
            finalizer,
        })
    }
    /** `switch (value) { case x: ... default: ... }` */
    fn switch_statement(&mut self) -> EngineResult<StmtKind> {
        self.next();
        let discriminant = self.parenthesized()?;
        self.expect("{")?;
        let saved = self.context;
        self.context.breakable = true;
        let mut cases = Vec::new();
        let mut seen_default = false;
        while !self.eat("}") {
            let test = if self.eat_name("case") {
                Some(self.expression(false)?)
            } else {
                let pos = self.pos();
                self.expect_name("default")?;
                if seen_default {
                    return Err(
                        self.error_at(pos, "More than one default clause in switch statement")
                    );
                }
                seen_default = true;
                None
            };
            self.expect(":")?;
            let mut body = Vec::new();
            while !self.at_name("case") && !self.at_name("default") && !self.at("}") {
                if matches!(self.peek().kind, Kind::End) {
                    self.context = saved;
                    return Err(self.unexpected());
                }
                body.push(self.statement(true)?);
            }
            cases.push(Case { test, body });
        }
        self.context = saved;
        self.check_redeclarations(cases.iter().flat_map(|case| &case.body))?;
        Ok(StmtKind::Switch(discriminant, cases))
    }
}

/** Anonymous functions and classes take the name of the binding they initialize. */
pub(super) fn infer_name(value: &mut Expr, name: &str) {
    match &value.kind {
        ExprKind::Function(func) if func.name.is_none() && func.display.borrow().is_empty() => {
            *func.display.borrow_mut() = Name::from(name);
        }
        ExprKind::Class(class) if class.name.is_none() => {
            let display = &class.constructor.display;
            if display.borrow().is_empty() {
                *display.borrow_mut() = Name::from(name);
            }
        }
        _ => {}
    }
}
