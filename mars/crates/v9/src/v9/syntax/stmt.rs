/** Statement grammar: declarations, control flow, exceptions and labels. */
use super::expr::{Cover, IMPORT_OUTSIDE_MODULE, INVALID_FOR_TARGET};
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
        if let Some(kind) = self.resource_declaration(false) {
            // V8 reads `using` as a name here, so the binding after it is the unexpected token.
            if !list_item || !self.context.using_allowed {
                self.next();
                return Err(self.unexpected());
            }
            self.resource_keyword(kind)?;
            let list = self.declarations(kind, false)?;
            self.semicolon()?;
            return Ok(StmtKind::Var(kind, list));
        }
        // An escaped name is never a keyword, so it takes the identifier arm.
        match if self.peek().escaped {
            ""
        } else {
            name.as_str()
        } {
            "var" => {
                self.next();
                let list = self.declarations(VarKind::Var, false)?;
                self.semicolon()?;
                Ok(StmtKind::Var(VarKind::Var, list))
            }
            "const" | "class" if !list_item => Err(self.unexpected()),
            // A single statement only refuses `let [`; `let` before a new line is a name.
            "let"
                if !list_item
                    && (matches!(self.peek_at(1).kind, Kind::Symbol('['))
                        || self.let_declaration() && !self.peek_at(1).line_terminator_before) =>
            {
                Err(self.error_at(
                    self.pos(),
                    "Lexical declaration cannot appear in a single-statement context",
                ))
            }
            "const" => {
                self.next();
                let list = self.declarations(VarKind::Const, false)?;
                self.semicolon()?;
                Ok(StmtKind::Var(VarKind::Const, list))
            }
            // Outside a statement list `let` is only ever a name (`let` then a new line then `{`).
            "let" if list_item && self.let_declaration() => {
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
                    return Err(self.error_at(self.pos(), "Illegal newline after throw"));
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
                    return Err(
                        self.error_at(pos, "Strict mode code may not include a with statement")
                    );
                }
                let object = self.parenthesized()?;
                let body = self.sub_statement(false)?;
                Ok(StmtKind::With(object, Box::new(body)))
            }
            "import" if !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '.')) => {
                Err(self.error_at(self.pos(), IMPORT_OUTSIDE_MODULE))
            }
            "export" => Err(self.unexpected()),
            _ if matches!(self.peek_at(1).kind, Kind::Symbol(':')) && self.at_identifier() => {
                self.labeled(list_item)
            }
            _ => self.expression_statement(),
        }
    }
    /** Contextual using starts a declaration only before an identifier without intervening line breaks. */
    fn resource_declaration(&self, for_head: bool) -> Option<VarKind> {
        let asynchronous = self.at_name("await")
            && matches!(&self.peek_at(1).kind, Kind::Name(n) if n == "using")
            && !self.peek_at(1).escaped
            && !self.peek_at(1).line_terminator_before;
        let offset = usize::from(asynchronous) + 1;
        if !asynchronous && !self.at_name("using") {
            return None;
        }
        let token = self.peek_at(offset);
        let Kind::Name(name) = &token.kind else {
            return None;
        };
        if reserved(name, self.strict) && name != "let" {
            return None;
        }
        if token.line_terminator_before
            || for_head
                && !asynchronous
                && name == "of"
                && !matches!(self.peek_at(offset + 1).kind, Kind::Symbol('='))
        {
            return None;
        }
        Some(if asynchronous {
            VarKind::AwaitUsing
        } else {
            VarKind::Using
        })
    }
    /** Awaited resource declarations use the enclosing async context rather than treating await as an identifier. */
    fn resource_keyword(&mut self, kind: VarKind) -> EngineResult<()> {
        if kind == VarKind::AwaitUsing {
            if !self.context.asynchronous || self.context.parameters {
                return Err(self.error_at(self.pos(), AWAIT_OUTSIDE_ASYNC));
            }
            self.expect_name("await")?;
        }
        self.expect_name("using")
    }
    /** `let` starts a declaration only before a binding name or pattern. */
    fn let_declaration(&self) -> bool {
        match &self.peek_at(1).kind {
            Kind::Symbol('[' | '{') => true,
            Kind::Name(name) => {
                !(["in", "instanceof"].contains(&name.as_str())
                    || (self.peek_at(1).line_terminator_before
                        && ["in", "instanceof", "of"].contains(&name.as_str())))
            }
            _ => false,
        }
    }
    /** `{ ... }` */
    pub(super) fn block(&mut self) -> EngineResult<Vec<Stmt>> {
        self.expect("{")?;
        let allowed = std::mem::replace(&mut self.context.using_allowed, true);
        let body = self.statement_list(false)?;
        self.context.using_allowed = allowed;
        self.expect("}")?;
        self.check_redeclarations(&body)?;
        Ok(body)
    }
    /** True at the start of an async function declaration or expression (`async` then
    `function` on the same line). */
    fn at_async_function(&self) -> bool {
        self.at_name("async")
            && matches!(&self.peek_at(1).kind, Kind::Name(n) if n == "function")
            && !self.peek_at(1).line_terminator_before
    }
    /** The body of `if`, a loop or `with`: never a declaration, except that sloppy code may
    give `if` a plain function declaration (Annex B.3.3). */
    fn sub_statement(&mut self, if_branch: bool) -> EngineResult<Stmt> {
        let async_function = self.at_async_function();
        let generator = self.at_name("function")
            && matches!(self.peek_at(1).kind, Kind::Operator(Operator::Multiply));
        let function = self.at_name("function") && (!if_branch || self.strict || generator);
        if async_function || function {
            // V8 names the generator only where Annex B would otherwise admit the function.
            let annex_b = if_branch && !self.strict;
            return Err(self.error_at(
                self.pos(),
                self.single_statement_function(async_function, generator && annex_b),
            ));
        }
        let declares = if_branch && self.at_name("function");
        let statement = self.statement(false)?;
        if !declares {
            return Ok(statement);
        }
        // B.3.4: the function stands as if it were the only statement in a block.
        let pos = statement.pos;
        Ok(Stmt {
            kind: StmtKind::Block(vec![statement]),
            pos,
        })
    }
    /** V8's reason for a function declaration where only a statement may stand. */
    fn single_statement_function(&self, asynchronous: bool, generator: bool) -> &'static str {
        if asynchronous {
            "Async functions can only be declared at the top level or inside a block."
        } else if self.strict {
            "In strict mode code, functions can only be declared at top level or inside a block."
        } else if generator {
            "Generators can only be declared at the top level or inside a block."
        } else {
            "In non-strict mode code, functions can only be declared at top level, inside a \
block, or as the body of an if statement."
        }
    }
    /** A block's (or case block's) lexical names must be unique, apart from sloppy plain
    function pairs, and no `var` in the same list may reuse one. */
    pub(super) fn check_redeclarations<'a>(
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
                    return Err(self.error_at(stmt.pos, &redeclared(&name)));
                }
                lexical.push((name, plain, stmt.pos));
            }
        }
        match lexical.iter().find(|(name, _, _)| vars.contains(name)) {
            Some((name, _, pos)) => Err(self.error_at(*pos, &redeclared(name))),
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
            if kind.resource() && !matches!(target, Pattern::Ident(..)) {
                return Err(
                    self.error_at(pos, "Resource declarations require a binding identifier")
                );
            }
            if kind != VarKind::Var {
                bound_names(&target, &mut names);
                if names.iter().any(|n| &**n == "let") {
                    return Err(self.error_at(pos, "let is disallowed as a lexically bound name"));
                }
                let duplicate = names
                    .iter()
                    .enumerate()
                    .find(|(i, n)| names[..*i].contains(n));
                if let Some((_, name)) = duplicate {
                    return Err(self.error_at(pos, &redeclared(name)));
                }
            }
            let init = if self.eat("=") {
                let mut value = self.assignment(for_head)?;
                if let Pattern::Ident(name, _) = &target {
                    infer_name(&mut value, name);
                }
                Some(value)
            } else {
                if !for_head && (kind.immutable() || !matches!(target, Pattern::Ident(..))) {
                    return Err(self.error_at(self.pos(), &missing_initializer(kind, &target)));
                }
                None
            };
            list.push(Declarator { target, init });
            if !self.eat(",") {
                return Ok(list);
            }
        }
    }
    /** V8's reason for a `for await` outside async code: `await` is just an unexpected word. */
    fn for_await_outside_async(&self) -> EngineError {
        self.unexpected()
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
        if self.at_name("await") && !self.context.asynchronous {
            return Err(self.for_await_outside_async());
        }
        let is_await = self.eat_name("await");
        self.expect("(")?;
        let init = if self.at(";") {
            None
        } else if self.at_name("var")
            || self.at_name("const")
            || self.at_name("let") && self.let_declaration()
            || self.resource_declaration(true).is_some()
        {
            let resource = self.resource_declaration(true);
            let kind = if let Some(kind) = resource {
                self.resource_keyword(kind)?;
                kind
            } else {
                match self.next().kind {
                    Kind::Name(name) if name == "var" => VarKind::Var,
                    Kind::Name(name) if name == "let" => VarKind::Let,
                    _ => VarKind::Const,
                }
            };
            let list = self.declarations(kind, true)?;
            let each = self.at_name("in") || self.at_name("of");
            if each && list.len() > 1 {
                return Err(self.error_at(
                    self.pos(),
                    &format!(
                        "Invalid left-hand side in {} loop: Must have a single binding.",
                        self.loop_kind()
                    ),
                ));
            }
            if each {
                let declarator = list.into_iter().next().expect("one declarator");
                // Only sloppy `for (var x = init in ...)` keeps an initializer (Annex B.3.5).
                let legacy = self.at_name("in")
                    && !self.strict
                    && kind == VarKind::Var
                    && matches!(declarator.target, Pattern::Ident(..));
                if declarator.init.is_some() && !legacy {
                    return Err(self.error_at(
                        self.pos(),
                        &format!(
                            "{} loop variable declaration may not have an initializer.",
                            self.loop_kind()
                        ),
                    ));
                }
                let assign = match (&declarator.target, declarator.init) {
                    (Pattern::Ident(name, at), Some(init)) => {
                        let target = Box::new(Pattern::Ident(name.clone(), *at));
                        let kind = ExprKind::Assign(AssignOp::Assign, target, Box::new(init));
                        Some(Expr { kind, pos: *at })
                    }
                    _ => None,
                };
                let mut each =
                    self.for_each(ForHead::Declare(kind, declarator.target), is_await)?;
                // B.3.5: the initializer runs once, before the object expression.
                if let (Some(assign), StmtKind::ForIn(_, right, _)) = (assign, &mut each) {
                    let pos = right.pos;
                    let object = std::mem::replace(
                        right,
                        Expr {
                            kind: ExprKind::Sequence(Vec::new()),
                            pos,
                        },
                    );
                    *right = Expr {
                        kind: ExprKind::Sequence(vec![assign, object]),
                        pos,
                    };
                }
                return Ok(each);
            }
            // Only a for-in/of head may omit initializers; a plain for head follows the
            // ordinary declaration rule.
            let missing = list.iter().find(|declarator| {
                declarator.init.is_none()
                    && (kind.immutable() || !matches!(declarator.target, Pattern::Ident(..)))
            });
            if let Some(declarator) = missing {
                return Err(
                    self.error_at(self.pos(), &missing_initializer(kind, &declarator.target))
                );
            }
            Some(ForInit::Declare(kind, list))
        } else {
            let start = self.position;
            let value = self.expression(true)?;
            if self.at_name("of") || self.at_name("in") {
                self.check_for_each_head(&value, start, is_await)?;
                let target = self.legacy_target_as(value, Cover::ForEach)?;
                return self.for_each(ForHead::Target(target), is_await);
            }
            Some(ForInit::Expr(value))
        };
        if is_await {
            return Err(self.unexpected());
        }
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
    /** "for-in" or "for-of", by the keyword at the cursor, for V8's loop-specific wording. */
    fn loop_kind(&self) -> &'static str {
        if self.at_name("of") {
            "for-of"
        } else {
            "for-in"
        }
    }
    /** An expression head of a for-in/of loop: an assignment is no target, a for-of head may
    not start with `let` (that would have been a declaration) nor be the bare name `async`. */
    fn check_for_each_head(&self, value: &Expr, start: usize, is_await: bool) -> EngineResult<()> {
        if matches!(value.kind, ExprKind::Assign(..)) {
            return Err(self.error_at(value.pos, INVALID_FOR_TARGET));
        }
        if !self.at_name("of") {
            return Ok(());
        }
        let first = &self.tokens[start];
        if !first.escaped && matches!(&first.kind, Kind::Name(name) if name == "let") {
            return Err(self.error_at(
                value.pos,
                "The left-hand side of a for-of loop may not start with 'let'.",
            ));
        }
        let bare_async = !is_await
            && matches!(&value.kind, ExprKind::Ident(n) if &**n == "async")
            && self.position == start + 1;
        if bare_async {
            return Err(self.error_at(
                value.pos,
                "The left-hand side of a for-of loop may not be 'async'.",
            ));
        }
        Ok(())
    }
    /** The rest of a for-in or for-of loop after its head. */
    fn for_each(&mut self, head: ForHead, is_await: bool) -> EngineResult<StmtKind> {
        let of = self.at_name("of");
        if is_await && !of {
            return Err(self.unexpected());
        }
        if !of && matches!(head, ForHead::Declare(kind, _) if kind.resource()) {
            return Err(self.error_at(self.pos(), "Invalid 'using' in for-in loop"));
        }
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
            self.check_label_jump(pos, &name, is_break)?;
            Some(name)
        } else {
            self.check_unlabeled_jump(pos, is_break)?;
            None
        };
        self.semicolon()?;
        Ok(if is_break {
            StmtKind::Break(label)
        } else {
            StmtKind::Continue(label)
        })
    }
    /** A labelled jump needs an enclosing label; `continue` further needs that label to name a loop. */
    fn check_label_jump(&self, pos: Pos, name: &Name, is_break: bool) -> EngineResult<()> {
        let found = self.labels.iter().rev().find(|(label, _)| label == name);
        match found {
            None => Err(self.error_at(pos, &format!("Undefined label '{name}'"))),
            Some((_, is_loop)) if is_break || *is_loop => Ok(()),
            Some(_) => Err(self.error_at(
                pos,
                &format!(
                    "Illegal continue statement: '{name}' does not denote an iteration statement"
                ),
            )),
        }
    }
    /** An unlabelled jump must sit inside a breakable or continuable statement, and V8 says which is missing. */
    fn check_unlabeled_jump(&self, pos: Pos, is_break: bool) -> EngineResult<()> {
        if is_break && !self.context.breakable {
            return Err(self.error_at(pos, "Illegal break statement"));
        }
        if !is_break && !self.context.continuable {
            return Err(self.error_at(
                pos,
                "Illegal continue statement: no surrounding iteration statement",
            ));
        }
        Ok(())
    }
    /** `label: statement` */
    fn labeled(&mut self, list_item: bool) -> EngineResult<StmtKind> {
        let pos = self.pos();
        let name = self.identifier()?;
        self.expect(":")?;
        if self.labels.iter().any(|(label, _)| *label == name) {
            return Err(self.error_at(pos, &format!("Label '{name}' has already been declared")));
        }
        let is_loop = self.labeled_loop();
        self.labels.push((name.clone(), is_loop));
        // A labelled function is sloppy Annex B grammar for a plain function only, and only
        // where a declaration may stand: never a generator or an async function.
        let body = if self.at_name("function") || self.at_async_function() {
            let asynchronous = self.at_async_function();
            let generator = matches!(self.peek_at(1).kind, Kind::Operator(Operator::Multiply));
            if !list_item || self.strict || asynchronous || generator {
                self.labels.pop();
                let reason = self.single_statement_function(asynchronous, generator && list_item);
                return Err(self.error_at(self.pos(), reason));
            }
            self.statement(list_item)
        } else {
            // A nested label keeps the declaration context: `a: b: function f() {}`.
            let nested = matches!(self.peek_at(1).kind, Kind::Symbol(':')) && self.at_identifier();
            self.statement(list_item && nested)
        };
        self.labels.pop();
        Ok(StmtKind::Labeled(name, Box::new(body?)))
    }
    /** Direct chains share the loop's continue target; a block, conditional or function stops that chain. */
    fn labeled_loop(&self) -> bool {
        let mut offset = 0;
        while matches!(self.peek_at(offset).kind, Kind::Name(_))
            && matches!(self.peek_at(offset + 1).kind, Kind::Symbol(':'))
        {
            offset += 2;
        }
        let token = self.peek_at(offset);
        !token.escaped
            && matches!(&token.kind, Kind::Name(name) if matches!(name.as_str(), "for" | "while" | "do"))
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
            return Err(self.error_at(self.pos(), "Missing catch or finally after try"));
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
        self.context.using_allowed = false;
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

/** V8's reason for a declaration that needs an initializer and has none: a pattern first, then
the declaration kind by name. */
fn missing_initializer(kind: VarKind, target: &Pattern) -> String {
    let what = match (kind, target) {
        (_, Pattern::Ident(..)) => kind.keyword(),
        _ => "destructuring",
    };
    format!("Missing initializer in {what} declaration")
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
