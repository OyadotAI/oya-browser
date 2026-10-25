/** Read-only syntax-tree walks the compiler uses to place bindings before emitting code. */
use super::super::syntax::*;
use super::super::vm::heap::FastSet;
use std::rc::Rc;

/** What a function body (including nested arrows) refers to. */
#[derive(Default, Clone, Copy)]
pub(super) struct Uses {
    pub this: bool,
    pub arguments: bool,
    pub new_target: bool,
    pub home: bool,
    /// Any nested function, arrow, method or class exists.
    pub closure: bool,
    /// A call spelled `eval(...)`: it may be a direct eval that sees every binding in scope.
    pub eval: bool,
}

/** A walker; `into_arrows` follows `this`-sharing arrows, never ordinary functions. */
struct Walker {
    uses: Uses,
    into_arrows: bool,
}

impl Walker {
    /** Visit statements. */
    fn stmts(&mut self, body: &[Stmt]) {
        for stmt in body {
            self.stmt(stmt);
        }
    }
    /** Visit one statement. */
    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Expr(value) | StmtKind::Throw(value) => self.expr(value),
            StmtKind::Var(_, list) => {
                for declarator in list {
                    self.pattern(&declarator.target);
                    if let Some(init) = &declarator.init {
                        self.expr(init);
                    }
                }
            }
            StmtKind::Function(func) => self.func(func),
            StmtKind::Class(class) => self.class(class),
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value)
                }
            }
            StmtKind::If(test, yes, no) => {
                self.expr(test);
                self.stmt(yes);
                if let Some(no) = no {
                    self.stmt(no);
                }
            }
            StmtKind::Block(body) => self.stmts(body),
            StmtKind::For {
                init,
                test,
                update,
                body,
            } => {
                match init {
                    Some(ForInit::Declare(_, list)) => {
                        for declarator in list {
                            self.pattern(&declarator.target);
                            if let Some(init) = &declarator.init {
                                self.expr(init);
                            }
                        }
                    }
                    Some(ForInit::Expr(value)) => self.expr(value),
                    None => {}
                }
                if let Some(test) = test {
                    self.expr(test);
                }
                if let Some(update) = update {
                    self.expr(update);
                }
                self.stmt(body);
            }
            StmtKind::ForIn(head, right, body) | StmtKind::ForOf(head, right, body, _) => {
                match head {
                    ForHead::Declare(_, pattern) | ForHead::Target(pattern) => {
                        self.pattern(pattern)
                    }
                }
                self.expr(right);
                self.stmt(body);
            }
            StmtKind::While(test, body) | StmtKind::DoWhile(body, test) => {
                self.expr(test);
                self.stmt(body);
            }
            StmtKind::Try {
                block,
                param,
                handler,
                finalizer,
            } => {
                self.stmts(block);
                if let Some(param) = param {
                    self.pattern(param);
                }
                if let Some(handler) = handler {
                    self.stmts(handler);
                }
                if let Some(finalizer) = finalizer {
                    self.stmts(finalizer);
                }
            }
            StmtKind::Switch(value, cases) => {
                self.expr(value);
                for case in cases {
                    if let Some(test) = &case.test {
                        self.expr(test);
                    }
                    self.stmts(&case.body);
                }
            }
            StmtKind::Labeled(_, body) => self.stmt(body),
            StmtKind::With(object, body) => {
                self.expr(object);
                self.stmt(body);
            }
            StmtKind::Field(key, value) => {
                self.key(key);
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
        }
    }
    /** A nested function: always a closure; arrows share `this` and `arguments`. */
    fn func(&mut self, func: &Rc<Func>) {
        self.uses.closure = true;
        if func.flags.arrow && self.into_arrows {
            for param in &func.params {
                self.pattern(param);
            }
            if let Some(rest) = &func.rest {
                self.pattern(rest);
            }
            match &func.body {
                Body::Block(body) => self.stmts(body),
                Body::Expr(value) => self.expr(value),
            }
        }
    }
    /** A class: its heritage and computed keys run in the enclosing context. */
    fn class(&mut self, class: &Class) {
        self.uses.closure = true;
        if let Some(superclass) = &class.superclass {
            self.expr(superclass);
        }
        for member in &class.members {
            self.key(&member.key);
        }
    }
    /** Computed keys are expressions. */
    fn key(&mut self, key: &PropKey) {
        if let PropKey::Computed(value) = key {
            self.expr(value);
        }
    }
    /** Destructuring defaults and computed keys are expressions. */
    fn pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ident(..) => {}
            Pattern::Expr(value) => self.expr(value),
            Pattern::Object(props, rest, _) => {
                for prop in props {
                    self.key(&prop.key);
                    self.pattern(&prop.value);
                }
                if let Some(rest) = rest {
                    self.pattern(rest);
                }
            }
            Pattern::Array(items, rest, _) => {
                for item in items.iter().flatten() {
                    self.pattern(item);
                }
                if let Some(rest) = rest {
                    self.pattern(rest);
                }
            }
            Pattern::Default(target, value) => {
                self.pattern(target);
                self.expr(value);
            }
        }
    }
    /** Visit list elements. */
    fn elements(&mut self, elements: &[Element]) {
        for element in elements {
            match element {
                Element::Value(value) | Element::Spread(value) => self.expr(value),
                Element::Hole => {}
            }
        }
    }
    /** Visit one expression. */
    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                if &**name == "arguments" {
                    self.uses.arguments = true;
                }
            }
            ExprKind::This => self.uses.this = true,
            ExprKind::NewTarget => self.uses.new_target = true,
            ExprKind::Super => {
                self.uses.home = true;
                self.uses.this = true;
            }
            ExprKind::Template(_, values) => {
                for value in values {
                    self.expr(value);
                }
            }
            ExprKind::Tagged(tag, _, _, values) => {
                self.expr(tag);
                for value in values {
                    self.expr(value);
                }
            }
            ExprKind::Array(elements) => self.elements(elements),
            ExprKind::Object(props) => {
                for prop in props {
                    match prop {
                        PropDef::Value(key, value) => {
                            self.key(key);
                            self.expr(value);
                        }
                        PropDef::Shorthand(name, _) => {
                            if &**name == "arguments" {
                                self.uses.arguments = true;
                            }
                        }
                        PropDef::ShorthandDefault(_, _, value) => self.expr(value),
                        PropDef::Method(key, func)
                        | PropDef::Getter(key, func)
                        | PropDef::Setter(key, func) => {
                            self.key(key);
                            self.func(func);
                        }
                        PropDef::Spread(value) => self.expr(value),
                    }
                }
            }
            ExprKind::Function(func) => self.func(func),
            ExprKind::Class(class) => self.class(class),
            ExprKind::Unary(_, value)
            | ExprKind::Update(_, _, value)
            | ExprKind::Await(value)
            | ExprKind::Chain(value)
            | ExprKind::PrivateIn(_, value) => self.expr(value),
            ExprKind::Binary(_, left, right) | ExprKind::Logical(_, left, right) => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Assign(_, target, value) => {
                // `arguments = x` rebinds this function's own arguments object, never an outer name.
                if matches!(&**target, Pattern::Ident(name, _) if &**name == "arguments") {
                    self.uses.arguments = true;
                }
                self.pattern(target);
                self.expr(value);
            }
            ExprKind::Conditional(test, yes, no) => {
                self.expr(test);
                self.expr(yes);
                self.expr(no);
            }
            ExprKind::Call(callee, args, _) | ExprKind::New(callee, args) => {
                if matches!(&expr.kind, ExprKind::Call(..))
                    && matches!(&callee.kind, ExprKind::Ident(name) if &**name == "eval")
                {
                    // Direct eval reads any binding, `this`, `arguments`, `new.target` and
                    // `super`, so every scope it can see lives in an environment.
                    self.uses = Uses {
                        this: true,
                        arguments: true,
                        new_target: true,
                        home: true,
                        closure: true,
                        eval: true,
                    };
                }
                self.expr(callee);
                self.elements(args);
            }
            ExprKind::Member(object, prop, _) => {
                self.expr(object);
                if let MemberProp::Computed(key) = prop {
                    self.expr(key);
                }
            }
            ExprKind::Sequence(values) => {
                for value in values {
                    self.expr(value);
                }
            }
            ExprKind::Yield(value, _) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            ExprKind::Import(specifier, options) => {
                self.expr(specifier);
                if let Some(options) = options {
                    self.expr(options);
                }
            }
            ExprKind::ImportMeta
            | ExprKind::Number(_)
            | ExprKind::BigInt(_)
            | ExprKind::String(_)
            | ExprKind::Opaque(_)
            | ExprKind::Regex(..)
            | ExprKind::Null
            | ExprKind::Bool(_) => {}
        }
    }
}

/** What a function's body and parameters use, looking through nested arrows. */
pub(super) fn function_uses(func: &Func) -> Uses {
    let mut walker = Walker {
        uses: Uses::default(),
        into_arrows: true,
    };
    for param in &func.params {
        walker.pattern(param);
    }
    if let Some(rest) = &func.rest {
        walker.pattern(rest);
    }
    match &func.body {
        Body::Block(body) => walker.stmts(body),
        Body::Expr(value) => walker.expr(value),
    }
    walker.uses
}

/** Whether statements contain any closure. */
pub(super) fn has_closure(body: &[Stmt]) -> bool {
    let mut walker = Walker {
        uses: Uses::default(),
        into_arrows: false,
    };
    walker.stmts(body);
    walker.uses.closure
}

/** Whether one statement contains any closure. */
pub(super) fn stmt_has_closure(stmt: &Stmt) -> bool {
    has_closure(std::slice::from_ref(stmt))
}

/** Names a pattern binds, in source order. */
pub(super) fn bound_names(pattern: &Pattern, out: &mut Vec<Name>) {
    match pattern {
        Pattern::Ident(name, _) => out.push(name.clone()),
        Pattern::Expr(_) => {}
        Pattern::Object(props, rest, _) => {
            for prop in props {
                bound_names(&prop.value, out);
            }
            if let Some(rest) = rest {
                bound_names(rest, out);
            }
        }
        Pattern::Array(items, rest, _) => {
            for item in items.iter().flatten() {
                bound_names(item, out);
            }
            if let Some(rest) = rest {
                bound_names(rest, out);
            }
        }
        Pattern::Default(target, _) => bound_names(target, out),
    }
}

/** `var` names hoisted to the function, including sloppy block functions (Annex B). */
pub(super) fn var_names(body: &[Stmt], strict: bool, top: bool, out: &mut Vec<Name>) {
    var_names_in(body, strict, top, &mut Vec::new(), out);
}

/**
 * `var` names of a statement list. `blocked` holds the names enclosing blocks declare
 * lexically: an Annex B block function of such a name is not hoisted, since replacing it with
 * `var` would be an early error (spec B.3.3.1 step ii).
 */
fn var_names_in(
    body: &[Stmt],
    strict: bool,
    top: bool,
    blocked: &mut Vec<Name>,
    out: &mut Vec<Name>,
) {
    let mark = blocked.len();
    if !strict {
        blocked.extend(lexical_names(body, !top).into_iter().map(|(name, _)| name));
    }
    for stmt in body {
        match &unlabeled(stmt).kind {
            StmtKind::Function(func) => {
                if !top
                    && !strict
                    && let Some(name) = &func.name
                    && !blocked[..mark].contains(name)
                {
                    out.push(name.clone());
                }
            }
            _ => var_names_stmt(stmt, strict, blocked, out),
        }
    }
    blocked.truncate(mark);
}

/** A statement under its labels. */
fn unlabeled(stmt: &Stmt) -> &Stmt {
    let mut stmt = stmt;
    while let StmtKind::Labeled(_, inner) = &stmt.kind {
        stmt = inner;
    }
    stmt
}

/** The names a loop head declares lexically. */
fn head_lexicals(kind: VarKind, patterns: &mut dyn Iterator<Item = &Pattern>) -> Vec<Name> {
    let mut names = Vec::new();
    if kind != VarKind::Var {
        for pattern in patterns {
            bound_names(pattern, &mut names);
        }
    }
    names
}

/** A nested statement's `var` names, with `extra` names blocked inside it. */
fn var_names_nested(
    stmt: &Stmt,
    strict: bool,
    extra: Vec<Name>,
    blocked: &mut Vec<Name>,
    out: &mut Vec<Name>,
) {
    let mark = blocked.len();
    blocked.extend(extra);
    var_names_in(std::slice::from_ref(stmt), strict, false, blocked, out);
    blocked.truncate(mark);
}

/** `var` names in one statement. */
fn var_names_stmt(stmt: &Stmt, strict: bool, blocked: &mut Vec<Name>, out: &mut Vec<Name>) {
    match &stmt.kind {
        StmtKind::Var(VarKind::Var, list) => {
            for declarator in list {
                bound_names(&declarator.target, out);
            }
        }
        StmtKind::If(_, yes, no) => {
            var_names_nested(yes, strict, Vec::new(), blocked, out);
            if let Some(no) = no {
                var_names_nested(no, strict, Vec::new(), blocked, out);
            }
        }
        StmtKind::Block(body) => var_names_in(body, strict, false, blocked, out),
        StmtKind::For { init, body, .. } => {
            let mut extra = Vec::new();
            if let Some(ForInit::Declare(kind, list)) = init {
                if *kind == VarKind::Var {
                    for declarator in list {
                        bound_names(&declarator.target, out);
                    }
                }
                extra = head_lexicals(*kind, &mut list.iter().map(|d| &d.target));
            }
            var_names_nested(body, strict, extra, blocked, out);
        }
        StmtKind::ForIn(head, _, body) | StmtKind::ForOf(head, _, body, _) => {
            let mut extra = Vec::new();
            if let ForHead::Declare(kind, pattern) = head {
                if *kind == VarKind::Var {
                    bound_names(pattern, out);
                }
                extra = head_lexicals(*kind, &mut std::iter::once(pattern));
            }
            var_names_nested(body, strict, extra, blocked, out);
        }
        StmtKind::While(_, body) | StmtKind::DoWhile(body, _) | StmtKind::With(_, body) => {
            var_names_nested(body, strict, Vec::new(), blocked, out)
        }
        StmtKind::Labeled(_, body) => var_names_stmt(body, strict, blocked, out),
        StmtKind::Try {
            block,
            param,
            handler,
            finalizer,
        } => {
            var_names_in(block, strict, false, blocked, out);
            if let Some(handler) = handler {
                // A plain catch name may be redeclared by var (B.3.4); a pattern may not.
                let mark = blocked.len();
                if let Some(param @ (Pattern::Object(..) | Pattern::Array(..))) = param {
                    bound_names(param, blocked);
                }
                var_names_in(handler, strict, false, blocked, out);
                blocked.truncate(mark);
            }
            if let Some(finalizer) = finalizer {
                var_names_in(finalizer, strict, false, blocked, out);
            }
        }
        StmtKind::Switch(_, cases) => {
            let mark = blocked.len();
            if !strict {
                for case in cases {
                    blocked.extend(lexical_names(&case.body, true).into_iter().map(|(n, _)| n));
                }
            }
            for case in cases {
                for stmt in &case.body {
                    if let StmtKind::Function(func) = &unlabeled(stmt).kind {
                        if !strict
                            && let Some(name) = &func.name
                            && !blocked[..mark].contains(name)
                        {
                            out.push(name.clone());
                        }
                    } else {
                        var_names_stmt(stmt, strict, blocked, out);
                    }
                }
            }
            blocked.truncate(mark);
        }
        _ => {}
    }
}

/** The direct lexical declarations of a statement list: let, const, class and block functions. */
pub(super) fn lexical_names(body: &[Stmt], functions_too: bool) -> Vec<(Name, VarKind)> {
    let mut out = Vec::new();
    for stmt in body {
        let mut stmt = stmt;
        while let StmtKind::Labeled(_, inner) = &stmt.kind {
            stmt = inner;
        }
        match &stmt.kind {
            StmtKind::Var(kind @ (VarKind::Let | VarKind::Const), list) => {
                for declarator in list {
                    let mut names = Vec::new();
                    bound_names(&declarator.target, &mut names);
                    out.extend(names.into_iter().map(|name| (name, *kind)));
                }
            }
            StmtKind::Class(class) => {
                if let Some(name) = &class.name {
                    out.push((name.clone(), VarKind::Let));
                }
            }
            StmtKind::Function(func) if functions_too => {
                if let Some(name) = &func.name {
                    out.push((name.clone(), VarKind::Let));
                }
            }
            _ => {}
        }
    }
    out
}

/** Function declarations directly in a statement list (through labels). */
pub(super) fn function_declarations(body: &[Stmt]) -> Vec<Rc<Func>> {
    let mut out = Vec::new();
    for stmt in body {
        let mut stmt = stmt;
        while let StmtKind::Labeled(_, inner) = &stmt.kind {
            stmt = inner;
        }
        if let StmtKind::Function(func) = &stmt.kind {
            out.push(func.clone());
        }
    }
    out
}

/**
 * The first name a statement list declares in conflicting ways, which is an early SyntaxError:
 * two lexical declarations (let, const, class) of one name, a lexical declaration and a function
 * declaration, a lexical declaration and a `var` that crosses this scope (including unreachable
 * branches), or a lexical declaration of a name in `outer` (parameters, or a loop's own lexical
 * head). Sloppy block functions may repeat each other (Annex B). `top` marks a function or script
 * body, where function declarations are var-scoped.
 */
pub(super) fn conflict(body: &[Stmt], strict: bool, top: bool, outer: &[Name]) -> Option<Name> {
    let lexical = lexical_names(body, false);
    let functions: Vec<Name> = function_declarations(body)
        .iter()
        .filter_map(|func| func.name.clone())
        .collect();
    let mut vars = Vec::new();
    // Strict scanning leaves out Annex B block functions, which never conflict.
    var_names(body, true, true, &mut vars);
    // Hashed, as a minified bundle declares thousands of names in one scope.
    let taken: FastSet<&Name> = functions.iter().chain(&vars).chain(outer).collect();
    let mut seen = FastSet::default();
    for (name, _) in &lexical {
        if taken.contains(name) || !seen.insert(name) {
            return Some(name.clone());
        }
    }
    if top {
        return None;
    }
    function_conflict(&functions, &vars, strict)
}

/** The first block function that repeats a `var`, or in strict code another block function. */
fn function_conflict(functions: &[Name], vars: &[Name], strict: bool) -> Option<Name> {
    let vars: FastSet<&Name> = vars.iter().collect();
    let mut seen = FastSet::default();
    functions
        .iter()
        .find(|name| vars.contains(name) || strict && !seen.insert(*name))
        .cloned()
}

/** `var` names a single statement declares, for loop heads that hold their own lexical names. */
pub(super) fn stmt_var_names(stmt: &Stmt) -> Vec<Name> {
    let mut out = Vec::new();
    var_names_stmt(stmt, true, &mut Vec::new(), &mut out);
    out
}
