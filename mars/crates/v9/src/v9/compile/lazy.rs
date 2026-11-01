/**
 * Deferred code generation: a function compiles on its first call, not when its script loads.
 *
 * YouTube's bundle declares about 69,000 functions and calls under a fifth of them in its first
 * minute; generating bytecode for the rest was a third of every page's compile time and most of
 * its code memory. A deferred function's `Code` is a stub: its name, length, flags and source
 * span answer everything that reads a function without running it, and it keeps the function's
 * syntax tree and the scopes it was written in, so the first call compiles exactly what an eager
 * compile at that point would have. The result is kept process-wide beside the script's cached
 * unit, so later pages and other tabs share it.
 */
use super::*;
use std::sync::OnceLock;

/** `MARS_V9_LAZY=0` compiles every function when its script loads, the old behavior, for
comparing or for bisecting a deferred-compile bug. */
const LAZY_SWITCH: &str = "MARS_V9_LAZY";

/** Whether functions may defer their compile, read once per process. */
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var(LAZY_SWITCH).map_or(true, |value| value != "0"))
}

/** A stub's deferred compile, at the page offset its stub sits at. */
pub(in crate::v9) struct Lazy {
    shared: Arc<Deferred>,
    /// Bytes this copy sits past the offset `shared` was parsed at (see `Code::rebased`).
    delta: i64,
    unit: OnceLock<EngineResult<Arc<cache::Unit>>>,
}

/** What compiling a deferred function needs, captured where its literal was compiled. */
struct Deferred {
    func: Arc<Func>,
    self_name: bool,
    scopes: Vec<Arc<Scope>>,
    classes: Vec<(u32, Vec<Name>)>,
    eval_grammar: u8,
    /// The compile at the parsed offset, shared by every rebased copy.
    unit: OnceLock<EngineResult<Arc<cache::Unit>>>,
}

impl Lazy {
    /** The function's compiled unit: its own code as the entry, its nested functions (stubs
    themselves, mostly) as the codes. `source` is the stub's source, which the compiled codes
    share. A compile error is kept and returned on every call. */
    pub(super) fn unit(
        &self,
        source: Option<&(Arc<str>, usize)>,
    ) -> EngineResult<Arc<cache::Unit>> {
        self.unit
            .get_or_init(|| {
                let base =
                    source.map(|(text, at)| (text.clone(), (*at as i64 - self.delta) as usize));
                let unit = self.shared.unit(base)?;
                Ok(match self.delta {
                    0 => unit,
                    delta => Arc::new(unit.rebased(delta)),
                })
            })
            .clone()
    }
    /** The compiled unit when the first call has already compiled it. */
    pub(super) fn compiled(&self) -> Option<&Arc<cache::Unit>> {
        self.unit.get().and_then(|unit| unit.as_ref().ok())
    }
    /** The same deferred compile for a copy `delta` bytes further along a page. */
    pub fn rebased(&self, delta: i64) -> Lazy {
        Lazy {
            shared: self.shared.clone(),
            delta: self.delta + delta,
            unit: OnceLock::new(),
        }
    }
}

impl Deferred {
    /** Compile at the parsed offset, once per process. */
    fn unit(&self, source: Option<(Arc<str>, usize)>) -> EngineResult<Arc<cache::Unit>> {
        self.unit
            .get_or_init(|| {
                let mut compiler = Compiler {
                    scopes: self.scopes.clone(),
                    classes: self.classes.clone(),
                    checked: true,
                    ..Compiler::default()
                };
                compiler.functions.push(FnState {
                    scope_base: self.scopes.len(),
                    eval_grammar: self.eval_grammar,
                    ..FnState::default()
                });
                compiler.compile_now(&self.func, self.self_name)?;
                let entry = compiler.done.pop().expect("compiled function");
                Ok(Arc::new(cache::Unit::sharing(compiler.done, entry, source)))
            })
            .clone()
    }
}

impl Compiler {
    /** Whether `func` may wait for its first call. Constructors, field initializers and static
    blocks run as soon as their class is defined, and module code shares its scope with the
    module record, so those compile now. */
    pub(super) fn deferrable(&self, func: &Func) -> bool {
        enabled()
            && !self.eager
            && self.module.is_none()
            && !func.flags.constructor
            && !func.flags.fields
    }
    /** A stub for `func` that compiles on first call in the scopes in effect here, after the
    early errors its compile would raise (see `early`). */
    pub(super) fn defer(&self, func: &Arc<Func>, self_name: bool) -> EngineResult<Code> {
        if !self.checked {
            super::early::function(func)?;
        }
        let mut code = Code::stub(func);
        code.lazy = Some(Arc::new(Lazy {
            shared: Arc::new(Deferred {
                func: func.clone(),
                self_name,
                scopes: self.scopes.clone(),
                classes: self.classes.clone(),
                eval_grammar: self.functions.last().map_or(0, |state| state.eval_grammar),
                unit: OnceLock::new(),
            }),
            delta: 0,
            unit: OnceLock::new(),
        }));
        Ok(code)
    }
}

impl Code {
    /** A function's code before it compiles: what reading the function needs, nothing to run. */
    fn stub(func: &Func) -> Code {
        Code {
            site: func.site,
            flags: func.flags,
            strict: func.strict,
            strict_ranges: Vec::new(),
            name: JsString::from_utf8(&func.display.borrow()).unwrap_or_else(|_| empty()),
            source: None,
            source_span: *func.source_span.borrow(),
            length: func.length,
            ops: Vec::new(),
            registers: 0,
            env: None,
            arguments: None,
            strings: Vec::new(),
            privates: Vec::new(),
            templates: Vec::new(),
            positions: Vec::new(),
            callees: Vec::new(),
            caches: crate::v9::vm::shape::SiteCaches::new(0),
            lazy: None,
            jit: crate::v9::jit::JitSlot::default(),
        }
    }
}
