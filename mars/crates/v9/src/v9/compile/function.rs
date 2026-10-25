/** Function and class lowering, plus how names load and store wherever they live. */
use super::scan;
use super::*;
use crate::v9::syntax::{
    Class, ExprKind, MemberKind, Pattern, PropKey, SITE_CONSTRUCTOR, SITE_TAGS, StmtKind, VarKind,
};

/** Method kinds for DefineMethod. */
pub(in crate::v9) const METHOD: u8 = 0;
/** A getter. */
pub(in crate::v9) const GETTER: u8 = 1;
/** A setter. */
pub(in crate::v9) const SETTER: u8 = 2;

impl Compiler {
    /** Compile a function literal and push a closure over the current environment. */
    pub(super) fn closure(&mut self, func: &Rc<Func>, self_name: bool) -> EngineResult<()> {
        self.compile_function(func, self_name)?;
        self.emit(Op::Closure(func.site));
        Ok(())
    }
    /** Compile one function into the registry. */
    pub(super) fn compile_function(&mut self, func: &Func, self_name: bool) -> EngineResult<()> {
        let uses = scan::function_uses(func);
        // A module's bindings live in its environment, where other modules read them.
        let env = uses.closure || self.module_body(func.site);
        self.begin_function(func.strict, func.flags);
        self.push_scope(env, true);
        self.at(func.pos);
        let result = self.function_body(func, uses, self_name);
        let scope = self.scopes.pop().expect("function scope");
        if self.module_body(func.site) {
            self.keep_module_bindings(scope.bindings.clone());
        }
        let mut code = self.end_function(Some(func), func.site, env.then_some(scope.slots));
        code.flags = func.flags;
        result?;
        self.done.push(code);
        Ok(())
    }
    /** Parameters, hidden bindings, hoisting and the body. */
    fn function_body(
        &mut self,
        func: &Func,
        uses: scan::Uses,
        self_name: bool,
    ) -> EngineResult<()> {
        let mut params = Vec::new();
        for param in &func.params {
            scan::bound_names(param, &mut params);
        }
        if let Some(rest) = &func.rest {
            scan::bound_names(rest, &mut params);
        }
        parameter_early_errors(func, &params)?;
        for name in &params {
            self.declare(name, Declared::Var)?;
        }
        let body: &[Stmt] = block_body(func).unwrap_or(&[]);
        super::stmt::early_conflict(body, func.strict, true, &params)?;
        let mut vars = Vec::new();
        scan::var_names(body, func.strict, true, &mut vars);
        self.hidden_bindings(func, uses, &params, &vars)?;
        if self_name
            && let Some(name) = &func.name
            && !params.contains(name)
            && !vars.contains(name)
        {
            self.declare(name, Declared::FunctionName)?;
            self.emit(Op::LoadCallee);
            self.initialize(name, func.pos)?;
            self.emit(Op::Pop);
        }
        self.parameters(func)?;
        if self.module_body(func.site) {
            self.module_imports(func.pos)?;
        }
        for name in &vars {
            if !params.contains(name) {
                self.declare(name, Declared::Var)?;
            }
        }
        for (name, kind) in scan::lexical_names(body, false) {
            let declared = if kind == VarKind::Const {
                Declared::Const
            } else {
                Declared::Let
            };
            let place = self.declare(&name, declared)?;
            self.emit(Op::Hole);
            self.set_place(place, 0);
            self.emit(Op::Pop);
        }
        let declarations = scan::function_declarations(body);
        for declaration in &declarations {
            let name = declaration.name.clone().expect("declaration name");
            self.declare(&name, Declared::Var)?;
        }
        for declaration in declarations {
            let name = declaration.name.clone().expect("declaration name");
            self.closure(&declaration, false)?;
            self.initialize(&name, declaration.pos)?;
            self.emit(Op::Pop);
        }
        // A module stops after hoisting, so every module of a graph is instantiated before any runs.
        if func.flags.generator || self.module_body(func.site) {
            self.emit(Op::Initial);
        }
        if func.site % SITE_TAGS == SITE_CONSTRUCTOR && func.flags.derived {
            self.emit(Op::RestArgs(0));
            self.emit(Op::SuperCallSpread);
            self.initialize(&Name::from("this"), func.pos)?;
            self.emit(Op::Pop);
        }
        match &func.body {
            Body::Block(body) => self.statements(body)?,
            Body::Expr(value) => {
                self.expr(value)?;
                self.emit(Op::Return);
                return Ok(());
            }
        }
        self.emit(Op::Undefined);
        if func.flags.derived {
            self.load_this_raw()?;
            self.emit(Op::ReturnDerived);
        } else {
            self.emit(Op::Return);
        }
        Ok(())
    }
    /** `this`, `arguments`, `new.target` and the home object as ordinary bindings. */
    fn hidden_bindings(
        &mut self,
        func: &Func,
        uses: scan::Uses,
        params: &[Name],
        vars: &[Name],
    ) -> EngineResult<()> {
        if func.flags.arrow {
            return Ok(());
        }
        if uses.this
            || func.flags.fields
            || func.flags.derived
            || func.flags.constructor
            || self.module_body(func.site)
        {
            let name = Name::from("this");
            if func.flags.derived {
                self.declare(&name, Declared::Let)?;
                self.emit(Op::Hole);
            } else {
                self.declare(&name, Declared::Hidden)?;
                self.emit(Op::LoadThis);
            }
            self.initialize(&name, func.pos)?;
            self.emit(Op::Pop);
        }
        let arguments = Name::from("arguments");
        if uses.arguments && !params.contains(&arguments) && !func.flags.fields {
            let _ = vars;
            self.declare(&arguments, Declared::Var)?;
            self.emit(Op::CreateArguments);
            self.initialize(&arguments, func.pos)?;
            self.emit(Op::Pop);
        }
        for (used, name, op) in [
            (uses.new_target, "new.target", Op::LoadNewTarget),
            (uses.home, "%home", Op::LoadHome),
        ] {
            if used {
                let name = Name::from(name);
                self.declare(&name, Declared::Hidden)?;
                self.emit(op);
                self.initialize(&name, func.pos)?;
                self.emit(Op::Pop);
            }
        }
        Ok(())
    }
    /** Bind arguments to parameters, evaluating defaults and patterns in order. */
    fn parameters(&mut self, func: &Func) -> EngineResult<()> {
        for (index, param) in func.params.iter().enumerate() {
            self.emit(Op::Arg(index as Slot));
            self.bind_pattern(param, false)?;
            self.emit(Op::Pop);
        }
        if let Some(rest) = &func.rest {
            self.emit(Op::RestArgs(func.params.len() as Slot));
            self.bind_pattern(rest, false)?;
            self.emit(Op::Pop);
        }
        Ok(())
    }
    /** A class: heritage, constructor, methods, the name binding and static initializers. */
    pub(super) fn class(&mut self, class: &Class) -> EngineResult<()> {
        self.at(class.pos);
        let mark = self.f().registers;
        let named = class.name.clone();
        self.push_scope(true, false);
        self.f().envs += 1;
        let push = self.emit(Op::PushEnv(0));
        if let Some(name) = &named {
            self.declare(name, Declared::ClassInner)?;
        }
        self.classes.push(class.pos.offset);
        let result = self.class_body(class);
        self.classes.pop();
        let scope = self.scopes.pop().expect("class scope");
        self.f().ops[push] = Op::PushEnv(scope.slots);
        self.emit(Op::PopEnv);
        self.f().envs -= 1;
        self.release(mark);
        result
    }
    /** Everything between the class's own scope push and pop; leaves the constructor. */
    fn class_body(&mut self, class: &Class) -> EngineResult<()> {
        let constructor = self.temp()?;
        let prototype = self.temp()?;
        if let Some(superclass) = &class.superclass {
            self.expr(superclass)?;
        }
        match &class.fields {
            Some(fields) => self.closure(fields, false)?,
            None => {
                self.emit(Op::Undefined);
            }
        }
        self.compile_function(&class.constructor, false)?;
        self.emit(Op::Class(
            class.constructor.site,
            class.superclass.is_some(),
        ));
        self.emit(Op::SetReg(prototype));
        self.emit(Op::Pop);
        self.emit(Op::SetReg(constructor));
        self.emit(Op::Pop);
        for member in &class.members {
            self.at(member.pos);
            self.emit(Op::GetReg(if member.is_static {
                constructor
            } else {
                prototype
            }));
            let (func, kind) = match &member.kind {
                MemberKind::Method(func) => (func, METHOD),
                MemberKind::Getter(func) => (func, GETTER),
                MemberKind::Setter(func) => (func, SETTER),
            };
            match &member.key {
                PropKey::Computed(key) => {
                    self.expr(key)?;
                    self.emit(Op::ToPropertyKey);
                    self.closure(func, false)?;
                    self.emit(Op::DefineMethodComputed(kind, false));
                }
                key => {
                    let index = self.key_index(key)?;
                    self.closure(func, false)?;
                    self.emit(Op::DefineMethod(index, kind, false));
                }
            }
            self.emit(Op::Pop);
        }
        if let Some(name) = &class.name {
            self.emit(Op::GetReg(constructor));
            self.initialize(name, class.pos)?;
            self.emit(Op::Pop);
        }
        if let Some(statics) = &class.statics {
            self.closure(statics, false)?;
            self.emit(Op::GetReg(constructor));
            self.emit(Op::Call(0));
            self.emit(Op::Pop);
        }
        self.emit(Op::GetReg(constructor));
        Ok(())
    }
    /** A class field definition inside a synthesized initializer. */
    pub(super) fn field(&mut self, key: &PropKey, value: Option<&Expr>) -> EngineResult<()> {
        self.load_this()?;
        match key {
            PropKey::Computed(key) => {
                self.expr(key)?;
                self.emit(Op::ToPropertyKey);
                self.field_value(value)?;
                self.emit(Op::DefineFieldComputed);
            }
            key => {
                let index = self.key_index(key)?;
                self.field_value(value)?;
                self.emit(Op::DefineField(index));
            }
        }
        self.emit(Op::Pop);
        Ok(())
    }
    /** A field's initial value. */
    fn field_value(&mut self, value: Option<&Expr>) -> EngineResult<()> {
        match value {
            Some(value) => self.expr(value),
            None => {
                self.emit(Op::Undefined);
                Ok(())
            }
        }
    }
    /** The string-table index of a non-computed key; private names get a hidden spelling. */
    pub(super) fn key_index(&mut self, key: &PropKey) -> EngineResult<u32> {
        match key {
            PropKey::Name(name) => self.name(name),
            PropKey::Private(name) => self.private_name(name),
            PropKey::Computed(_) => Err(EngineError::invalid("Computed key has no index.")),
        }
    }
    /** Private names become keys scripts cannot spell or enumerate. */
    pub(super) fn private_name(&mut self, name: &str) -> EngineResult<u32> {
        let class = self.classes.last().copied().unwrap_or(0);
        let mut units = vec![PRIVATE_MARK];
        units.extend(format!("{class}:{name}").encode_utf16());
        let key = JsString::from_units(units)?;
        Ok(self.string(&key))
    }
    /** Store the top of stack into a resolved place, keeping it. */
    fn set_place(&mut self, place: Place, hops: Slot) {
        match place {
            Place::Register(slot) => {
                self.emit(Op::SetReg(slot));
            }
            Place::Env(slot) => {
                self.emit(Op::SetEnv(hops, slot));
            }
            Place::Global => {}
        }
    }
    /** Push a name's value. */
    pub(super) fn load_name(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        self.at(pos);
        if let Some((_, Declared::Import, _)) = self.resolve(name) {
            return self.load_import(name, pos);
        }
        match self.resolve(name) {
            Some((Place::Register(slot), declared, _)) => {
                if checked(declared) {
                    let index = self.name(name)?;
                    self.emit(Op::GetRegChecked(slot, index));
                } else {
                    self.emit(Op::GetReg(slot));
                }
            }
            Some((Place::Env(slot), declared, hops)) => {
                if checked(declared) {
                    let index = self.name(name)?;
                    self.emit(Op::GetEnvChecked(hops, slot, index));
                } else {
                    self.emit(Op::GetEnv(hops, slot));
                }
            }
            Some((Place::Global, ..)) | None => match &**name {
                "undefined" => {
                    self.emit(Op::Undefined);
                }
                _ => {
                    let index = self.name(name)?;
                    self.emit(Op::GetGlobal(index));
                }
            },
        }
        Ok(())
    }
    /** Assign the top of stack to a name, keeping it; constants throw. */
    pub(super) fn assign_name(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        self.at(pos);
        let index = self.name(name)?;
        match self.resolve(name) {
            Some((_, Declared::Const | Declared::Import, _)) => {
                self.emit(Op::ThrowConst(index));
            }
            Some((_, Declared::FunctionName, _)) => {
                if self.f().strict {
                    self.emit(Op::ThrowConst(index));
                }
            }
            Some((Place::Register(slot), declared, _)) => {
                self.emit(if checked(declared) {
                    Op::SetRegChecked(slot, index)
                } else {
                    Op::SetReg(slot)
                });
            }
            Some((Place::Env(slot), declared, hops)) => {
                self.emit(if checked(declared) {
                    Op::SetEnvChecked(hops, slot, index)
                } else {
                    Op::SetEnv(hops, slot)
                });
            }
            Some((Place::Global, ..)) | None => {
                self.emit(Op::SetGlobal(index));
            }
        }
        Ok(())
    }
    /** Initialize a declared binding with the top of stack, keeping it. */
    pub(super) fn initialize(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        self.at(pos);
        match self.resolve(name) {
            Some((Place::Register(slot), ..)) => {
                self.emit(Op::SetReg(slot));
            }
            Some((Place::Env(slot), _, hops)) => {
                self.emit(Op::SetEnv(hops, slot));
            }
            Some((Place::Global, declared, _)) if declared != Declared::Var => {
                let index = self.name(name)?;
                self.emit(Op::InitLexical(index));
            }
            Some((Place::Global, ..)) | None => {
                let index = self.name(name)?;
                self.emit(Op::SetGlobal(index));
            }
        }
        Ok(())
    }
    /** Annex B: a sloppy block function also updates the function-level `var` of its name. */
    pub(super) fn store_var_outward(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        let base = self.functions.last().map_or(0, |f| f.scope_base);
        let mut hops: Slot = 0;
        for scope in self.scopes[base..].iter().rev() {
            if let Some(binding) = scope
                .index
                .get(name)
                .map(|&at| &scope.bindings[at])
                .filter(|b| b.declared == Declared::Var)
            {
                let place = binding.place;
                match place {
                    Place::Global => {
                        let index = self.name(name)?;
                        self.emit(Op::SetGlobal(index));
                    }
                    place => self.set_place(place, hops),
                }
                return Ok(());
            }
            if scope.env {
                hops += 1;
            }
        }
        let _ = pos;
        Ok(())
    }
    /** Push `this`, checking a derived constructor's dead zone. */
    pub(super) fn load_this(&mut self) -> EngineResult<()> {
        if self.resolve("this").is_some() {
            self.load_name(&Name::from("this"), Pos::default())
        } else {
            // Only arrows at the top level reach here: their `this` is the global object.
            self.emit(Op::GlobalThis);
            Ok(())
        }
    }
    /** Push `this` without a dead-zone check, for a derived constructor's return. */
    pub(super) fn load_this_raw(&mut self) -> EngineResult<()> {
        match self.resolve("this") {
            Some((Place::Register(slot), ..)) => {
                self.emit(Op::GetReg(slot));
            }
            Some((Place::Env(slot), _, hops)) => {
                self.emit(Op::GetEnv(hops, slot));
            }
            _ => {
                self.emit(Op::LoadThis);
            }
        }
        Ok(())
    }
}

/// Early errors a body directive can create after the head was parsed: strict code refuses
/// `eval` and `arguments` as a parameter or function name, and strict functions and arrows
/// refuse duplicate parameters. Whole-script, so an unreached branch fails the same way.
fn parameter_early_errors(func: &Func, params: &[Name]) -> EngineResult<()> {
    let restricted = |name: &Name| &**name == "eval" || &**name == "arguments";
    if func.strict && (params.iter().any(restricted) || func.name.as_ref().is_some_and(restricted))
    {
        return Err(crate::v9::errors::syntax(
            func.pos.location(),
            "Unexpected eval or arguments in strict mode",
        ));
    }
    let simple = func.rest.is_none() && func.params.iter().all(|p| matches!(p, Pattern::Ident(..)));
    if !simple && has_strict_directive(func) {
        return Err(crate::v9::errors::syntax(
            func.pos.location(),
            "Illegal 'use strict' directive in function with non-simple parameter list",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    if (func.strict || func.flags.arrow) && params.iter().any(|name| !seen.insert(name)) {
        return Err(crate::v9::errors::syntax(
            func.pos.location(),
            "Duplicate parameter name not allowed in this context",
        ));
    }
    Ok(())
}

/** Whether the body's own directive prologue says "use strict" (spec 15.2.1 early error:
ContainsUseStrict of a body with a non-simple parameter list). */
// ponytail: compares cooked strings, so an escaped "use\x20strict" also counts; the AST keeps
// no raw directive text. Thread the parser's exact_strict flag through Func if that matters.
fn has_strict_directive(func: &Func) -> bool {
    let Some(body) = block_body(func) else {
        return false;
    };
    body.iter()
        .map_while(|stmt| match &stmt.kind {
            StmtKind::Expr(Expr {
                kind: ExprKind::String(text),
                ..
            }) => Some(text),
            _ => None,
        })
        .any(|text| text.as_units() == "use strict".encode_utf16().collect::<Vec<_>>())
}

/** Private keys start with a noncharacter unit no page string contains by accident. */
pub(in crate::v9) const PRIVATE_MARK: u16 = 0xffff;

/** Bindings that need dead-zone checks. */
fn checked(declared: Declared) -> bool {
    matches!(
        declared,
        Declared::Let | Declared::Const | Declared::ClassInner
    )
}
