/**
 * Direct eval (spec PerformEval and EvalDeclarationInstantiation; V8's
 * Runtime_ResolvePossiblyDirectEval with its scope analysis). A call spelled `eval(...)` records
 * the caller's environment scopes as text; when the callee turns out to be the intrinsic eval the
 * VM compiles the argument in those scopes, so the code resolves the caller's bindings by
 * environment hops. The text travels beside the source (never inside it, where a page could
 * write it) and a restored page recompiles from the pair.
 * A sloppy eval's new `var`s land in the caller function's eval-var object (the `%ext` binding,
 * V8's context extension), which every name lookup that crosses that function checks first.
 */
use super::*;
use crate::v9::syntax::VarKind;

/** The hidden binding holding a sloppy function's eval-var object. */
pub(super) const EXT: &str = "%ext";

/** The hidden binding holding a `with` statement's object. */
pub(super) const WITH: &str = "%with";

/** Internal names (`this`, `new.target`, `%home`, ...) never resolve through a `with` object. */
fn internal(name: &str) -> bool {
    name == "this" || name.contains('.') || name.starts_with('%')
}

/** One dynamic check a lookup makes before its static place: env (hops, slot) and whether it
is a `with` object rather than an eval-var object. */
pub(super) type Check = (Slot, Slot, bool);

/** The caller's scopes as the eval code sees them. */
pub(super) struct Context {
    strict: bool,
    /// The call sits in a sloppy function's parameter list.
    params: bool,
    scopes: Vec<Scope>,
}

/** The one-letter code of each declaration kind in the context text. */
const KINDS: [(Declared, char); 7] = [
    (Declared::Var, 'V'),
    (Declared::Let, 'L'),
    (Declared::Const, 'C'),
    (Declared::ClassInner, 'K'),
    (Declared::FunctionName, 'F'),
    (Declared::Hidden, 'H'),
    (Declared::Catch, 'P'),
];

/** Parse the context text `eval_context` made; it travels beside the eval source, never in it. */
pub(super) fn context(text: &str) -> Option<Context> {
    let (strict, scopes) = text.split_once('|')?;
    let scopes = scopes
        .split(';')
        .filter(|scope| !scope.is_empty())
        .map(parse_scope)
        .collect::<Option<Vec<_>>>()?;
    Some(Context {
        strict: strict == "s",
        params: strict == "p",
        scopes,
    })
}

/** Slots each environment scope of a context needs, innermost first, so the VM can check them
against the caller's real environments before eval code indexes into them. */
pub(in crate::v9) fn env_sizes(text: &str) -> Option<Vec<Slot>> {
    let context = context(text)?;
    Some(
        context
            .scopes
            .iter()
            .rev()
            .filter(|scope| scope.env)
            .map(|scope| scope.slots)
            .collect(),
    )
}

/** One scope of the context text: `g` for the global scope, else `e` and its bindings. */
fn parse_scope(text: &str) -> Option<Scope> {
    let mut parts = text.split(' ');
    let global = parts.next()? == "g";
    let mut scope = Scope {
        bindings: Vec::new(),
        index: FastMap::default(),
        env: !global,
        slots: 0,
        boundary: false,
        global,
    };
    for part in parts {
        let mut fields = part.split(':');
        let name = Name::from(fields.next()?);
        let code = fields.next()?.chars().next()?;
        let declared = KINDS.iter().find(|(_, c)| *c == code)?.0;
        let slot: Slot = fields.next()?.parse().ok()?;
        scope.slots = scope.slots.max(slot.checked_add(1)?);
        scope.index.insert(name.clone(), scope.bindings.len());
        scope.bindings.push(Binding {
            name,
            declared,
            place: Place::Env(slot),
        });
    }
    Some(scope)
}

impl Compiler {
    /** The caller's environment scopes at a direct eval call, as context text. */
    pub(super) fn eval_context(&self) -> String {
        let state = self.f_ref();
        let mut text = String::from(match (state.strict, state.in_params) {
            (true, _) => "s|",
            (false, true) => "p|",
            (false, false) => "n|",
        });
        for scope in &self.scopes {
            if scope.global {
                text.push_str("g;");
                continue;
            }
            if !scope.env {
                continue;
            }
            text.push('e');
            for binding in &scope.bindings {
                let (Place::Env(slot), Some((_, code))) = (
                    binding.place,
                    KINDS.iter().find(|(kind, _)| *kind == binding.declared),
                ) else {
                    continue;
                };
                text.push_str(&format!(" {}:{code}:{slot}", binding.name));
            }
            text.push(';');
        }
        text
    }
    /** The function being emitted, read-only. */
    fn f_ref(&self) -> &FnState {
        self.functions.last().expect("function state")
    }
    /** Environment hops from the innermost scope to scope `index`. */
    fn hops_to(&self, index: usize) -> Slot {
        self.scopes[index + 1..].iter().filter(|s| s.env).count() as Slot
    }
    /**
     * The objects a lookup of `name` must check before its static place: each `%ext` of a
     * sloppy function with direct eval and each `with` object the lookup crosses, innermost
     * first.
     */
    pub(super) fn ext_checks(&self, name: &str) -> Vec<Check> {
        let mut checks = Vec::new();
        let mut hops: Slot = 0;
        for scope in self.scopes.iter().rev() {
            if scope.index.contains_key(name) {
                break;
            }
            for (hidden, with) in [(EXT, false), (WITH, true)] {
                if with && internal(name) {
                    continue;
                }
                if let Some(Binding {
                    place: Place::Env(slot),
                    ..
                }) = scope.index.get(hidden).map(|&at| &scope.bindings[at])
                {
                    checks.push((hops, *slot, with));
                }
            }
            if scope.env {
                hops += 1;
            }
        }
        checks
    }
    /** Emit eval-var checks for a load: each found value jumps past the static load. */
    pub(super) fn ext_loads(&mut self, name: &str) -> EngineResult<Vec<usize>> {
        let checks = self.ext_checks(name);
        if checks.is_empty() {
            return Ok(Vec::new());
        }
        let index = self.name(name)?;
        let mut jumps = Vec::new();
        for (hops, slot, with) in checks {
            self.emit(if with {
                Op::GetWith(index, hops, slot, false)
            } else {
                Op::GetExt(index, hops, slot)
            });
            jumps.push(self.emit(Op::JumpIfTrue(0)));
        }
        Ok(jumps)
    }
    /** Emit eval-var checks for a store of the top of stack, which stays. */
    pub(super) fn ext_stores(&mut self, name: &str) -> EngineResult<Vec<usize>> {
        let checks = self.ext_checks(name);
        if checks.is_empty() {
            return Ok(Vec::new());
        }
        let index = self.name(name)?;
        let mut jumps = Vec::new();
        for (hops, slot, with) in checks {
            self.emit(if with {
                Op::SetWith(index, hops, slot)
            } else {
                Op::SetExt(index, hops, slot)
            });
            jumps.push(self.emit(Op::JumpIfTrue(0)));
        }
        Ok(jumps)
    }
    /**
     * Push a called name's value and `this`: a `with` object that holds the name is the
     * receiver (spec 9.1.1.2.10 WithBaseObject); anything else calls with undefined.
     */
    pub(super) fn callee_name(&mut self, name: &Name, pos: Pos) -> EngineResult<()> {
        let checks = self.ext_checks(name);
        if !checks.iter().any(|check| check.2) {
            self.load_name(name, pos)?;
            self.emit(Op::Undefined);
            return Ok(());
        }
        self.at(pos);
        let index = self.name(name)?;
        let (mut found, mut done) = (Vec::new(), Vec::new());
        for (hops, slot, with) in checks {
            self.emit(if with {
                Op::GetWith(index, hops, slot, true)
            } else {
                Op::GetExt(index, hops, slot)
            });
            let jump = self.emit(Op::JumpIfTrue(0));
            if with {
                done.push(jump)
            } else {
                found.push(jump)
            }
        }
        self.load_static_name(name)?;
        self.patch_all(found);
        self.emit(Op::Undefined);
        self.patch_all(done);
        Ok(())
    }
    /** `delete name` checks each `with` object holding the name first; jumps land at the end. */
    pub(super) fn delete_checks(&mut self, name: &Name) -> EngineResult<Vec<usize>> {
        let mut jumps = Vec::new();
        for (hops, slot, with) in self.ext_checks(name) {
            if with {
                let index = self.name(name)?;
                self.emit(Op::DeleteWith(index, hops, slot));
                jumps.push(self.emit(Op::JumpIfTrue(0)));
            }
        }
        Ok(jumps)
    }
    /** Point eval-var check jumps here. */
    pub(super) fn patch_all(&mut self, jumps: Vec<usize>) {
        for jump in jumps {
            self.patch(jump);
        }
    }
    /** `eval(args)` with no spread: the VM decides at run time whether it is direct. */
    pub(super) fn direct_eval_call(&mut self, count: u16) -> EngineResult<()> {
        let context = self.eval_context();
        let index = self.name(&context)?;
        self.emit(Op::DirectEval(count, index));
        Ok(())
    }
    /** Compile direct eval code inside the caller's scopes. */
    pub(super) fn eval_script(
        &mut self,
        program: &Program,
        context: Context,
    ) -> EngineResult<Code> {
        let strict = program.strict || context.strict;
        let params = context.params;
        self.begin_function(strict, FuncFlags::default());
        self.scopes.extend(context.scopes);
        let base = self.scopes.len();
        self.f().scope_base = base;
        self.at(program.pos);
        let body = &program.body;
        super::stmt::early_conflict(body, strict, true, &[])?;
        let mut vars = Vec::new();
        scan::var_names(body, true, true, &mut vars);
        let functions = scan::function_declarations(body);
        let var_scope = self.scopes.iter().rposition(|s| s.index.contains_key(EXT));
        if !strict {
            self.f().eval_vars = Some(var_scope);
            let mut names = vars.clone();
            names.extend(functions.iter().filter_map(|f| f.name.clone()));
            if params && names.iter().any(|name| &**name == "arguments") {
                let index =
                    self.name("Cannot declare 'arguments' in a parameter initializer eval")?;
                self.emit(Op::ThrowSyntax(index));
            }
            self.eval_conflicts(&names, var_scope)?;
            let mut annex = Vec::new();
            scan::var_names(body, false, true, &mut annex);
            // A set, not `vars.contains`: an eval of n vars would otherwise cost n squared.
            let declared: super::super::vm::heap::FastSet<&Name> = vars.iter().collect();
            annex.retain(|name| !declared.contains(name) && !self.eval_blocked(name, var_scope));
            names.extend(annex);
            self.eval_vars(&names, var_scope)?;
        }
        let env = scan::has_closure(body);
        self.push_scope(env, true);
        let push = env.then(|| {
            self.f().envs += 1;
            self.emit(Op::PushEnv(0))
        });
        if strict {
            for name in &vars {
                let place = self.declare(name, Declared::Var)?;
                self.emit(Op::Undefined);
                self.set_place(place, 0);
                self.emit(Op::Pop);
            }
        }
        for (name, kind) in scan::lexical_names(body, false) {
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
            if strict {
                self.declare(&name, Declared::Var)?;
            }
            self.closure(&func, false)?;
            if strict {
                self.initialize(&name, func.pos)?;
            } else {
                self.eval_function_store(&name, var_scope)?;
            }
            self.emit(Op::Pop);
        }
        let result = self.temp()?;
        self.emit(Op::Undefined);
        self.emit(Op::SetReg(result));
        self.emit(Op::Pop);
        self.statements_with_result(body, Some(result))?;
        self.emit(Op::GetReg(result));
        self.emit(Op::Return);
        let scope = self.scopes.pop().expect("eval scope");
        if let Some(push) = push {
            self.f().ops[push] = Op::PushEnv(scope.slots);
        }
        self.scopes.clear();
        let site = crate::v9::syntax::site_of(program.pos);
        Ok(self.end_function(None, site, None))
    }
    /** A sloppy eval's `var` may not cross a lexical binding of its name up to the var scope. */
    fn eval_conflicts(&mut self, names: &[Name], var_scope: Option<usize>) -> EngineResult<()> {
        let floor = var_scope.unwrap_or(0);
        for name in names {
            let lexical = self.scopes[floor..].iter().any(|scope| {
                scope.index.get(name).is_some_and(|&at| {
                    matches!(
                        scope.bindings[at].declared,
                        Declared::Let | Declared::Const | Declared::ClassInner
                    )
                })
            });
            if lexical {
                let message = format!("Identifier '{name}' has already been declared");
                let index = self.name(&message)?;
                self.emit(Op::ThrowSyntax(index));
                return Ok(());
            }
        }
        Ok(())
    }
    /** Annex B (B.3.3.3): a block function in eval is not hoisted past a caller's lexical
    binding of its name between the eval and its var scope. */
    fn eval_blocked(&self, name: &Name, var_scope: Option<usize>) -> bool {
        self.scopes[var_scope.unwrap_or(0)..].iter().any(|scope| {
            scope.index.get(name).is_some_and(|&at| {
                !matches!(scope.bindings[at].declared, Declared::Var | Declared::Catch)
            })
        })
    }
    /** Annex B: store a sloppy eval's block function (on the stack, kept) into its var scope
    when no static binding took it: the eval-var object, or the global object. */
    pub(super) fn eval_block_function_store(
        &mut self,
        name: &Name,
        var_scope: Option<usize>,
    ) -> EngineResult<()> {
        let index = self.name(name)?;
        match var_scope {
            None => {
                self.emit(Op::SetGlobal(index));
            }
            Some(at) => {
                let (hops, slot) = self.ext_slot(at);
                self.emit(Op::SetExt(index, hops, slot));
                self.emit(Op::Pop);
            }
        }
        Ok(())
    }
    /** Create a sloppy eval's vars: global properties, or entries of the function's `%ext`. */
    fn eval_vars(&mut self, names: &[Name], var_scope: Option<usize>) -> EngineResult<()> {
        for name in names {
            let index = self.name(name)?;
            match var_scope {
                None => {
                    self.emit(Op::DeclareVar(index, true));
                }
                Some(at) if !self.scopes[at].index.contains_key(name) => {
                    let (hops, slot) = self.ext_slot(at);
                    self.emit(Op::DeclareExt(index, hops, slot));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
    /** Where scope `at`'s `%ext` lives, from here. */
    fn ext_slot(&self, at: usize) -> (Slot, Slot) {
        let scope = &self.scopes[at];
        let slot = match scope.index.get(EXT).map(|&i| scope.bindings[i].place) {
            Some(Place::Env(slot)) => slot,
            _ => 0,
        };
        (self.hops_to(at), slot)
    }
    /** Store a sloppy eval's function declaration (on the stack, kept) into its var scope. */
    fn eval_function_store(&mut self, name: &Name, var_scope: Option<usize>) -> EngineResult<()> {
        let index = self.name(name)?;
        let Some(at) = var_scope else {
            self.emit(Op::Dup);
            self.emit(Op::DeclareFunction(index, true));
            return Ok(());
        };
        let hops = self.hops_to(at);
        let scope = &self.scopes[at];
        match scope.index.get(name).map(|&i| scope.bindings[i].place) {
            Some(Place::Env(slot)) => {
                self.emit(Op::SetEnv(hops, slot));
            }
            _ => {
                let (hops, slot) = self.ext_slot(at);
                self.emit(Op::SetExt(index, hops, slot));
                self.emit(Op::Pop);
            }
        }
        Ok(())
    }
}
