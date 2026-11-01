/**
 * Lowers V9's syntax tree to a compact stack bytecode. Code is rebuilt from page source on every
 * request (or taken from the process cache of code the same source produced before) and
 * identified by its source site, so serialized state never carries executable code.
 */
mod cache;
mod class_fields;
mod eval;
pub(in crate::v9) use eval::{env_sizes, private_sites as eval_private_sites};
mod catch_bindings;
mod early;
mod expr;
mod function;
mod iteration;
mod lazy;
mod member_reference;
mod module;
mod parameters;
mod pattern;
mod resources;
mod scan;
mod site_map;
mod site_text;
use pattern::Source;
use site_text::{CATCH_VALUE, FOR_VALUE, YIELD_STAR_VALUE};
mod stmt;
mod strict;
mod super_update;

use super::errors;
use super::syntax::{Body, Expr, Func, FuncFlags, Name, Pos, Program, Stmt};
use super::value::JsString;
use super::vm::heap::FastMap;
use crate::protocol::{EngineError, EngineResult};
pub(in crate::v9) use module::{Export, ModuleInfo};
use std::sync::Arc;

/** Register and slot indexes. */
pub(in crate::v9) type Slot = u16;

/** One bytecode instruction; operands index the owning code's string table. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::v9) enum Op {
    Undefined,
    Null,
    True,
    False,
    Hole,
    Number(f64),
    /** A BigInt literal, its decimal text in the string table. */
    BigInt(u32),
    String(u32),
    Pop,
    Dup,
    /** a b -> a b a b */
    Dup2,
    /** a b -> b a */
    Swap,
    /** a b c -> c a b */
    Rot3,
    /** a b c d -> d a b c */
    Rot4,
    GetReg(Slot),
    /** Throws a ReferenceError naming the string if the register is in its dead zone. */
    GetRegChecked(Slot, u32),
    /** Store the top of stack and keep it. */
    SetReg(Slot),
    /** Store, failing if the binding is still in its dead zone. */
    SetRegChecked(Slot, u32),
    GetEnv(Slot, Slot),
    GetEnvChecked(Slot, Slot, u32),
    SetEnv(Slot, Slot),
    SetEnvChecked(Slot, Slot, u32),
    /** Push a fresh block environment with this many slots, all in their dead zone. */
    PushEnv(Slot),
    /** Mark the current environment as the scope of the class at this source offset, so its
    private names are fresh for this evaluation of the class. */
    ClassScope(u32),
    PopEnv,
    /** Replace the current environment with a copy, for per-iteration loop bindings. */
    CopyEnv,
    GetGlobal(u32),
    TypeofGlobal(u32),
    SetGlobal(u32),
    DeleteGlobal(u32),
    /** Top-level `var`: define the global property if it is missing; the flag marks a sloppy
    direct eval's var, which stays deletable. */
    DeclareVar(u32, bool),
    /** Script Annex B compatibility declarations are optional when the global cannot acquire a property. */
    DeclareOptionalVar(u32),
    /** Check global var admission before any binding is created; the flag selects eval's TypeError. */
    CheckGlobalVar(u32, bool),
    /** Eval's Annex B global declaration or copy is suppressed by an existing global lexical. */
    AnnexGlobal(u32, bool),
    /** Top-level `function`: define or overwrite the global property with the closure on the
    stack; the flag marks a sloppy direct eval's function, which stays deletable. */
    DeclareFunction(u32, bool),
    /** Check global function admission before any binding is created; the flag selects eval's TypeError. */
    CheckGlobalFunction(u32, bool),
    /** callee this args... -> result: `eval(...)`, a direct eval when the callee is the
    intrinsic eval, run in the caller's scopes that the string operand describes. */
    DirectEval(u16, u32),
    /** A checked native intrinsic from the explicitly enabled upstream fixture grammar. */
    #[cfg(feature = "conformance")]
    NativeIntrinsic(u8),
    /** Look a name up in the eval-var object in env slot (hops, slot): pushes the value and
    true when it holds the name, else false. */
    GetExt(u32, Slot, Slot),
    /** value -> value bool: store into the eval-var object when it holds the name. */
    SetExt(u32, Slot, Slot),
    /** Create a sloppy eval's var in the eval-var object (undefined) unless it is there. */
    DeclareExt(u32, Slot, Slot),
    /** Delete an existing sloppy eval binding, pushing result and a found flag. */
    DeleteExt(u32, Slot, Slot),
    /** Look a name up in the `with` object in env slot (hops, slot), honoring
    Symbol.unscopables: pushes the value (and the object as `this` when the flag says the
    name is called) and true when the object holds it, else false. */
    GetWith(u32, Slot, Slot, bool),
    /** value -> value bool: store into the `with` object when it holds the name. */
    SetWith(u32, Slot, Slot),
    /** -> result true, or false: `delete name` on the `with` object when it holds the name. */
    DeleteWith(u32, Slot, Slot),
    /** The reference step of an assignment or update: resolve the name against the eval-var
    or `with` object (the flag) in env slot (hops, slot) before the right-hand side runs,
    pushing the object and true when it holds the name, else false. */
    ResolveRef(u32, Slot, Slot, bool),
    /** base -> base value true, or base false when the base is not a resolved object. */
    GetRef(u32),
    /** base value -> value true, or value false when the base is not a resolved object: the
    store goes to the object the reference resolved to, whether or not it still has the name. */
    SetRef(u32),
    /** value -> object, throwing a TypeError for null and undefined (spec ToObject). */
    ToObject,
    /** Throw a SyntaxError with this message, for eval's runtime declaration conflicts. */
    ThrowSyntax(u32),
    /** A legacy invalid call reference raises only if evaluation reaches this instruction. */
    ThrowReference(u32),
    /** Top-level `let`/`const`/`class`: create the dead-zone global binding; flag is const. */
    DeclareLexical(u32, bool),
    /** The collision checks of `DeclareLexical` alone, run for every top-level lexical name
    before a script creates any binding, so a script that fails them leaves no `var` behind. */
    CheckGlobalLexical(u32),
    /** Initialize a top-level lexical binding with the value on the stack (popped). */
    InitLexical(u32),
    /** Assignment to a constant. */
    ThrowConst(u32),
    LoadThis,
    LoadNewTarget,
    LoadHome,
    LoadCallee,
    CreateArguments,
    GlobalThis,
    NewObject,
    /** Pop `count` values into a new array; Hole values become holes. */
    NewArray(u32),
    /** array value -> array */
    ArrayPush,
    /** array iterable -> array */
    ArraySpread,
    /** obj value -> obj */
    DefineField(u32),
    /** obj key value -> obj */
    DefineFieldComputed,
    /** obj function -> obj; kind 0 method, 1 getter, 2 setter; enumerable flag for literals. */
    DefineMethod(u32, u8, bool),
    /** obj key function -> obj */
    DefineMethodComputed(u8, bool),
    /** Define a class accessor pair backed by the specified private key; the home remains on stack. */
    AutoAccessor(u32, u32),
    /** A computed public key is consumed once while the home remains on stack. */
    AutoAccessorComputed(u32),
    /** function key -> function, capturing a computed field's named evaluation before its body executes. */
    InferFunctionName,
    /** function home -> function home, assigning the initializer's lexical home before invocation. */
    SetInitializerHome,
    /** Derived instance elements initialize only after the lexical this binding has been assigned. */
    InitializeDerivedFields,
    /** obj source -> obj */
    CopyData,
    /** source key1..keyN -> rest */
    CopyRest(u32),
    /** Throw the destructuring TypeError for a nullish source, naming the pattern's first key
     * (an index into the strings, `u32::MAX` when there is none). The source stays on the stack. */
    RequireCoercible(u32),
    /** obj proto -> obj, for a literal `__proto__: value`. */
    SetProtoLiteral,
    GetProp(u32),
    GetElem,
    /** obj value -> value */
    SetProp(u32),
    /** obj key value -> value */
    SetElem,
    DeleteProp(u32),
    DeleteElem,
    In,
    /** obj -> bool: `#name in obj`, the name a private-key string index. */
    PrivateIn(u32),
    InstanceOf,
    /** this home -> value, looking the name up from the home object's prototype. */
    GetSuper(u32),
    /** this key home -> value */
    GetSuperElem,
    /** this home value -> value */
    SetSuper(u32),
    /** this home key value -> value */
    SetSuperElem,
    /** home -> its captured prototype or null, before computed-key conversion. */
    CaptureSuperBase,
    /** receiver captured-base key -> same reference plus its value. */
    ReadSuperReference,
    /** receiver captured-base key value -> value, writing through the retained reference. */
    WriteSuperReference,
    ToPropertyKey,
    /** object raw-key -> object canonical-key, checking the receiver before authored key coercion. */
    ToElementKey,
    /** callee this args... -> result */
    Call(u16),
    /** callee this array -> result */
    CallSpread,
    /** callee args... -> object */
    New(u16),
    /** callee array -> object */
    NewSpread,
    /** args... -> this; calls the parent constructor */
    SuperCall(u16),
    /** array -> this */
    SuperCallSpread,
    /** Lexical constructor and new.target followed by arguments; binding happens after construction. */
    LexicalSuperCall(u16),
    /** Lexical constructor and new.target followed by the spread arguments array. */
    LexicalSuperCallSpread,
    /** Bind a captured derived receiver once and then initialize its instance elements. */
    BindLexicalSuper(Slot, Slot),
    Return,
    /** value this -> return from a derived constructor */
    ReturnDerived,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Le,
    Gt,
    Ge,
    Not,
    Neg,
    ToNumber,
    ToNumeric,
    BitNot,
    Typeof,
    Inc,
    Dec,
    ToStr,
    Jump(u32),
    JumpIfFalse(u32),
    JumpIfTrue(u32),
    /** Jump keeping the value if falsy; otherwise pop it. */
    JumpIfFalseKeep(u32),
    JumpIfTrueKeep(u32),
    /** Jump keeping the value if it is neither null nor undefined; otherwise pop it. */
    JumpIfNotNullishKeep(u32),
    /** If the value is null or undefined, replace it with undefined and jump; else keep it. */
    JumpIfNullish(u32),
    /** Leave handlers and environments down to the given depths, running finally blocks. */
    JumpOut(u32, u16, u16),
    Throw,
    /** Throw an unsupported-feature error naming this string. */
    Unsupported(u32),
    /** Install a handler: catch address, finally address (0 means none) and the finally
    block's three completion registers. */
    PushTry(u32, u32, Slot),
    PopTry,
    /** Record a normal completion in a finally block's registers before entering it. */
    Normal(Slot),
    /** Resume the completion a finally block's registers hold. */
    EndFinally(Slot),
    /** Create a private lexical disposal stack, with no authored global lookup. */
    ResourceCreate,
    /** Register the stack top for eager disposal-method capture and leave the original value. */
    ResourceAdd(Slot, bool),
    /** Dispose a synchronous region using its pending finally completion registers. */
    ResourceDispose(Slot, Slot),
    /** Begin an async disposal region without adding an extra promise or job boundary. */
    ResourceBegin(Slot, Slot),
    /** Store one awaited disposal result or jump when the region has finished. */
    ResourceStep(Slot, Slot, u32),
    /** Fold a rejected disposal await into the region's pending suppressed-error chain. */
    ResourceRejected(Slot),
    /** Push argument `n`, or undefined. */
    Arg(Slot),
    /** Push an array of the arguments from `n` on. */
    RestArgs(Slot),
    Closure(u32),
    /** superclass? ctor-site -> ctor prototype; flag says a superclass is on the stack. */
    Class(u32, bool),
    GetIterator,
    /** iter -> iter value, or jump with iter when done. */
    IterNext(u32),
    /** iter -> iter value (undefined once done), for destructuring. */
    IterStep,
    /** iter -> undefined, advancing an elision without observing the result's value property. */
    IterSkip,
    /** iter -> array of the remaining values */
    IterRest,
    /** iter -> (closes it unless already done) */
    IterClose,
    /** Skip closure on normal iteration or continue to this same loop. */
    IterCloseCheck(Slot, u32, u32),
    /** A caught close error yields to an already pending body throw. */
    IterCloseError(Slot),
    ForIn,
    /** iter -> iter key, or jump with iter when done. */
    ForInNext(u32),
    Yield,
    /** iterable -> result, delegating with yield*. */
    YieldStar,
    Await,
    /** value -> async iterator record (GetIterator with kind async, wrapping a sync iterator in
    an AsyncFromSyncIterator when there is no Symbol.asyncIterator). */
    GetAsyncIterator,
    /** record -> what its next() returned, for an Await to settle. */
    AsyncIterNext,
    /** result -> its value, or pop and jump when it is done; a non-object is a TypeError. */
    AsyncIterValue(u32),
    /** record -> what its return() returned, or pop and jump when it has none (AsyncIteratorClose). */
    AsyncIterClose(u32),
    /** awaited return() result -> (a TypeError unless it is an object). */
    AsyncCloseCheck,
    /** record received -> one step of `yield*` in an async generator; suspends to await. */
    AsyncYieldStar,
    /** record mode result -> the awaited inner result: done ends the delegation, else yields. */
    AsyncYieldStarResult,
    /** Suspend a fresh generator until its first next(); a module body suspends the same way
    until its graph evaluates it. */
    Initial,
    /** specifier -> promise: `import()`, resolved against the running code's module or script. */
    Import,
    Regex(u32, u32),
    /** Push the frozen strings object for tagged template `index` of this code. */
    TemplateObject(u32),
}

/** How a recorded site text reads in the message for a value that was not what the site
needed. V8 prints the expression as written for most sites, names the callee when the value came
straight out of a call (`f is not a function or its return value is not iterable`), and has a
form of its own for a spread argument. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum SiteKind {
    /** The text is the operand expression as the source wrote it. */
    Printed,
    /** The operand was a call; the text is its callee. */
    CallResult,
    /** The operand was spread into a call's arguments. */
    SpreadArgument,
    /** The operand was spread into an array literal. */
    ArraySpread,
}

/** One compiled function or script. */
pub(in crate::v9) struct Code {
    /// Source site identity; scripts use their start offset with the source tag.
    pub site: u32,
    /// Function kind.
    pub flags: FuncFlags,
    /// Function-level strict mode controls activation semantics and stack privacy.
    pub strict: bool,
    /// Half-open instruction ranges whose references are strict inside an otherwise sloppy function.
    pub strict_ranges: Vec<(u32, u32)>,
    /// Observable function name.
    pub name: JsString,
    /// Exact source is shared by all functions compiled from one source unit.
    pub source: Option<(Arc<str>, usize)>,
    /// The caller scopes a direct eval compiled under, as the compiler's context text; None for
    /// every other code. A retained eval is reused only under the same scopes.
    pub scopes: Option<Arc<str>>,
    /// Absolute byte boundaries of this function or class literal.
    pub source_span: Option<(u32, u32)>,
    /// Observable function length.
    pub length: u32,
    /// The instructions.
    pub ops: Vec<Op>,
    /// Register count.
    pub registers: Slot,
    /// Slots in the function-scope environment, retained by closures or mapped arguments.
    pub env: Option<Slot>,
    /// Simple sloppy parameters share these environment slots with their arguments indices.
    pub arguments: Option<Vec<Option<Slot>>>,
    /// Strings and names the instructions reference.
    pub strings: Vec<JsString>,
    /// Indexes into `strings` that name private keys (class `#names`). Only the compiler adds
    /// one, so a script string with the same text is a different entry and an ordinary key.
    pub privates: Vec<u32>,
    /// Tagged template chunks: cooked (None when invalid) and raw.
    pub templates: Vec<Vec<(Option<JsString>, JsString)>>,
    /// Instruction index to source position, sorted.
    pub positions: Vec<(u32, Pos)>,
    /// Instructions whose operand has a written form that V8 prints in its message (`o.f is not
    /// a function`, `x.y is not iterable`): instruction index, how the text reads, and the text,
    /// sorted by index.
    pub callees: Vec<(u32, SiteKind, Arc<str>)>,
    /// Inline caches of the property loads and stores, one per entry of `strings`.
    pub caches: crate::v9::vm::shape::SiteCaches,
    /// A stub's deferred compile: None for compiled code (see `lazy`).
    pub lazy: Option<Arc<lazy::Lazy>>,
    /// Call and loop heat, and the compiled form once the JIT has produced one.
    pub jit: crate::v9::jit::JitSlot,
}

impl Code {
    /** The written form of the operand of the instruction at `pc`, found by binary search. */
    pub fn callee_at(&self, pc: usize) -> Option<(SiteKind, &str)> {
        let index = self
            .callees
            .binary_search_by_key(&(pc as u32), |(at, ..)| *at)
            .ok()?;
        let (_, kind, text) = &self.callees[index];
        Some((*kind, text))
    }
    /** Code from the platform bootstrap, which pages must see as native, as Chrome's own
    interfaces are: its functions print `[native code]`, never the bootstrap's text. */
    pub fn platform(&self) -> bool {
        self.source
            .as_ref()
            .is_some_and(|(_, base)| *base == crate::constants::SCRIPT_TOTAL_BYTES)
    }
    /** Preserve comments and whitespace without interpreting the function's public name. */
    pub fn original_source(&self) -> Option<&str> {
        let (source, base) = self.source.as_ref()?;
        let (start, end) = self.source_span?;
        source.get((start as usize).checked_sub(*base)?..(end as usize).checked_sub(*base)?)
    }
    /** The source position of an instruction, for diagnostics. */
    pub fn position(&self, pc: usize) -> Pos {
        let index = self
            .positions
            .partition_point(|(start, _)| *start as usize <= pc);
        index
            .checked_sub(1)
            .map(|index| self.positions[index].1)
            .unwrap_or_default()
    }
}

impl Code {
    /** This code as compiled `delta` bytes further along the page. A script's code depends on
    where it sits only through its page offsets (its site, span, positions and the sites its
    closures and classes name), so moving them is the same code a fresh compile at the new
    offset gives, without parsing again: YouTube's 10 MB bundle sits at a different offset on
    every page, because the inline scripts before it differ. */
    fn rebased(&self, delta: i64) -> Code {
        let shift = |offset: u32| (i64::from(offset) + delta) as u32;
        let site =
            |site: u32| (i64::from(site) + delta * i64::from(super::syntax::SITE_TAGS)) as u32;
        Code {
            site: site(self.site),
            flags: self.flags,
            strict: self.strict,
            strict_ranges: self.strict_ranges.clone(),
            name: self.name.clone(),
            source: self
                .source
                .as_ref()
                .map(|(text, base)| (text.clone(), (*base as i64 + delta) as usize)),
            scopes: self.scopes.clone(),
            source_span: self
                .source_span
                .map(|(start, end)| (shift(start), shift(end))),
            length: self.length,
            ops: self
                .ops
                .iter()
                .map(|op| match *op {
                    Op::Closure(target) => Op::Closure(site(target)),
                    Op::Class(target, derived) => Op::Class(site(target), derived),
                    Op::ClassScope(offset) => Op::ClassScope(shift(offset)),
                    other => other,
                })
                .collect(),
            registers: self.registers,
            env: self.env,
            arguments: self.arguments.clone(),
            strings: self
                .strings
                .iter()
                .enumerate()
                .map(|(index, text)| {
                    if self.privates.contains(&(index as u32)) {
                        rebased_private(text, delta)
                    } else {
                        text.clone()
                    }
                })
                .collect(),
            privates: self.privates.clone(),
            templates: self.templates.clone(),
            callees: self.callees.clone(),
            positions: self
                .positions
                .iter()
                .map(|(pc, pos)| {
                    // A generated instruction has the default position, which stays put.
                    let moved = *pos != Pos::default();
                    (
                        *pc,
                        Pos {
                            offset: if moved { shift(pos.offset) } else { pos.offset },
                            ..*pos
                        },
                    )
                })
                .collect(),
            caches: crate::v9::vm::shape::SiteCaches::new(self.strings.len()),
            lazy: self.lazy.as_ref().map(|lazy| Arc::new(lazy.rebased(delta))),
            jit: crate::v9::jit::JitSlot::default(),
        }
    }
    /** Bytes this code holds, for the process cache's bound. */
    fn footprint(&self) -> usize {
        let strings = |strings: &[JsString]| strings.iter().map(|s| s.len() * 2).sum::<usize>();
        let templates = self
            .templates
            .iter()
            .flatten()
            .map(|(cooked, raw)| cooked.as_ref().map_or(0, |s| s.len() * 2) + raw.len() * 2)
            .sum::<usize>();
        std::mem::size_of::<Self>()
            + self.ops.len() * std::mem::size_of::<Op>()
            + self
                .arguments
                .as_ref()
                .map_or(0, |slots| slots.len() * std::mem::size_of::<Option<Slot>>())
            + strings(&self.strings)
            + templates
            + self.positions.len() * std::mem::size_of::<(u32, Pos)>()
            + self
                .callees
                .iter()
                .map(|(_, _, name)| name.len() * 2)
                .sum::<usize>()
            + self.strict_ranges.len() * std::mem::size_of::<(u32, u32)>()
            + self.caches.bytes()
            + self.scopes.as_ref().map_or(0, |scopes| scopes.len())
    }
}

/** Every code of one page, keyed by site. A persistent map: the registry travels with the
page's state from step to step (cloning it is O(1) and shares structure), so a step no longer
re-files the page's ~69,000 functions before it runs, and a retried step starts from its base
state's registry. */
#[derive(Clone, Default)]
pub(in crate::v9) struct Registry {
    pub codes: site_map::SiteMap,
    /// Entry sites adopted during the current step (`begin_step` clears it). Two sources on one
    /// site within a step are a fault; a site filed by an earlier step belongs to whatever
    /// source is compiled there now (a later devtools evaluation reuses an offset, say).
    adopted: crate::v9::vm::heap::FastSet<u32>,
}

impl std::fmt::Debug for Registry {
    /** The count of filed codes; the codes themselves are large and printed nowhere. */
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("codes", &self.codes.len())
            .finish()
    }
}

/** How a unit stands against what the registry already holds at its entry site. */
enum Filing {
    /** Nothing there yet: file every code, refusing any site already taken. */
    Fresh,
    /** This very unit: a later step re-adopting its scripts, nothing to do. */
    Filed,
    /** The same source at the same offset, compiled again (after the process cache let the
    old unit go): its codes replace the old ones. */
    Recompiled,
    /** Another source at an offset an earlier step's source held: the old source's codes go
    first, then the new ones are filed as fresh. */
    Replaced,
    /** Another source claims the site. */
    Conflict,
}

/** The content key of a classic script, wherever it sits on a page. */
pub(in crate::v9) fn source_key(source: &str) -> cache::Key {
    cache::text_key(cache::Grammar::Script, source)
}

/** SHA-256 of a module's text, the part of its cache key that does not depend on its offset. */
pub(in crate::v9) fn module_source_key(source: &str) -> cache::Key {
    cache::text_key(cache::Grammar::Module, source)
}

/** Native-fixture grammar has its own cache namespace, even for identical source bytes. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn native_fixture_key(source: &str) -> cache::Key {
    cache::text_key(cache::Grammar::NativeFixture, source)
}

impl Registry {
    /** Compile a script and its nested functions; returns the script's top-level code. */
    pub fn add(&mut self, program: &Program) -> EngineResult<Arc<Code>> {
        let unit = compile_script(program, None)?;
        self.adopt(&unit)?;
        Ok(unit.entry.clone())
    }
    /**
     * The code `source` compiles to as a classic script at page offset `offset`: the process
     * cache's unit when this exact text compiled there before, else `parse` and compile it and
     * keep the unit for the next request. Either way its codes join this registry.
     */
    pub fn script(
        &mut self,
        source: &str,
        offset: usize,
        parse: impl FnOnce() -> EngineResult<Program>,
    ) -> EngineResult<Arc<Code>> {
        self.script_keyed(source_key(source), source, offset, parse)
    }
    /** `script` for a source whose `source_key` the caller already holds: a service that keeps
    a page's sources resident hashes each once, not on every step (YouTube's 10 MB of scripts
    took longer to hash than many steps took to run). */
    pub fn script_keyed(
        &mut self,
        text: cache::Key,
        source: &str,
        offset: usize,
        parse: impl FnOnce() -> EngineResult<Program>,
    ) -> EngineResult<Arc<Code>> {
        let key = cache::at(&text, offset);
        let unit = match cache::get(&key) {
            Some(unit) => unit,
            None => {
                let unit = match cache::elsewhere(&text) {
                    Some((at, unit)) => Arc::new(unit.rebased(offset as i64 - at as i64)),
                    None => compile_script(&parse()?, Some((source, offset)))?,
                };
                cache::put(key, unit.clone());
                cache::remember(text, offset, key);
                unit
            }
        };
        self.adopt(&unit)?;
        Ok(unit.entry.clone())
    }
    /**
     * Direct eval code at `offset`, compiled in the caller scopes `context` describes. The
     * context is the compiler's own text, handed over beside the source and part of the cache
     * key; the source itself is never searched for one.
     */
    pub fn eval(
        &mut self,
        source: &str,
        offset: usize,
        context: &str,
        parse: impl FnOnce(bool, &[Name], u8) -> EngineResult<Program>,
    ) -> EngineResult<Arc<Code>> {
        let scopes = eval::context(context)
            .ok_or_else(|| EngineError::invalid("Malformed V9 eval context."))?;
        let key = cache::eval_key(offset, context, source);
        let unit = match cache::get(&key) {
            Some(unit) => unit,
            None => {
                let unit = compile_eval(
                    &parse(scopes.strict, &scopes.private_names(), scopes.grammar)?,
                    scopes,
                    (source, offset),
                    context,
                )?;
                cache::put(key, unit.clone());
                unit
            }
        };
        self.adopt(&unit)?;
        Ok(unit.entry.clone())
    }
    /**
     * The code and link facts `source` compiles to as a module at `offset`, cached like
     * `script`. A parse or compile failure is the module's own outcome, not a request failure,
     * and is not cached; the same text fails the same way next time.
     */
    pub fn module(
        &mut self,
        source: &str,
        offset: usize,
        parse: impl FnOnce() -> EngineResult<Program>,
    ) -> EngineResult<(Arc<Code>, Arc<ModuleInfo>)> {
        self.module_keyed(module_source_key(source), source, offset, parse)
    }
    /**
     * `module` for a source whose `module_source_key` the caller already holds: a service that
     * keeps a page's modules resident hashes each once, not on every request, and the key at
     * an offset is one cheap hash of that text key.
     */
    pub fn module_keyed(
        &mut self,
        text: cache::Key,
        source: &str,
        offset: usize,
        parse: impl FnOnce() -> EngineResult<Program>,
    ) -> EngineResult<(Arc<Code>, Arc<ModuleInfo>)> {
        let key = cache::at(&text, offset);
        let unit = match cache::get(&key) {
            Some(unit) => unit,
            None => {
                let unit = compile_module(parse()?, (source, offset))?;
                cache::put(key, unit.clone());
                unit
            }
        };
        if matches!(self.filing(&unit), Filing::Conflict) {
            return Err(EngineError::invalid("Duplicate V9 module site."));
        }
        self.adopt(&unit)?;
        let info = unit
            .info
            .clone()
            .ok_or_else(|| EngineError::invalid("V9 lost a module's code."))?;
        Ok((unit.entry.clone(), info))
    }
    /**
     * The code already filed at `site` when it is exactly this dynamic source: the same text at
     * the same offset, under the same caller scopes for direct eval. It answers with no parse
     * and no hash, so a step re-adopting a retained source costs one text comparison. Any other
     * source at the site is None, and the keyed path tells the cases apart.
     */
    pub fn refiled(
        &mut self,
        site: u32,
        source: &str,
        offset: usize,
        scopes: Option<&str>,
    ) -> Option<Arc<Code>> {
        let entry = self.codes.get(&site)?;
        if !is_same_source(entry, source, offset, scopes) {
            return None;
        }
        let entry = entry.clone();
        self.adopted.insert(site);
        Some(entry)
    }
    /** Start a step: what earlier steps filed stays, but no longer counts as this step's. */
    pub fn begin_step(&mut self) {
        self.adopted.clear();
    }
    /** The entry code at `site` when this step adopted it. */
    pub fn adopted_this_step(&self, site: u32) -> Option<Arc<Code>> {
        self.adopted
            .contains(&site)
            .then(|| self.codes.get(&site).cloned())
            .flatten()
    }
    /** Drop every code filed for the source `entry` came from: a different source now
    compiles at that offset, and none of the old one's functions may answer for the new one's
    sites. Runs only when a later step reuses an offset (an unretained evaluation's). */
    fn evict_source(&mut self, entry: &Code) {
        let Some((text, base)) = &entry.source else {
            let site = entry.site;
            self.codes.retain(|filed, _| *filed != site);
            return;
        };
        let tags = super::syntax::SITE_TAGS as usize;
        let range = base * tags..(base + text.len() + 1) * tags;
        self.codes
            .retain(|site, _| !range.contains(&(*site as usize)));
    }
    /** Where `unit` stands: see `Filing`. */
    fn filing(&self, unit: &cache::Unit) -> Filing {
        let Some(filed) = self.codes.get(&unit.entry.site) else {
            return Filing::Fresh;
        };
        if Arc::ptr_eq(filed, &unit.entry) {
            return Filing::Filed;
        }
        let same = match (&filed.source, &unit.entry.source) {
            (Some((old, at)), Some((new, base))) => {
                at == base && (Arc::ptr_eq(old, new) || old == new)
            }
            _ => false,
        };
        if same {
            Filing::Recompiled
        } else if !self.adopted.contains(&unit.entry.site) {
            Filing::Replaced
        } else {
            Filing::Conflict
        }
    }
    /** Files a unit's codes under their sites; two sources on one site in a step are refused.
    A unit already filed (a later step of the same page) costs one lookup. */
    fn adopt(&mut self, unit: &cache::Unit) -> EngineResult<()> {
        let fresh = match self.filing(unit) {
            Filing::Filed => {
                self.adopted.insert(unit.entry.site);
                return Ok(());
            }
            Filing::Conflict => return Err(EngineError::invalid("Duplicate V9 code site.")),
            Filing::Fresh => true,
            Filing::Recompiled => false,
            Filing::Replaced => {
                if let Some(old) = self.codes.get(&unit.entry.site).cloned() {
                    self.evict_source(&old);
                }
                true
            }
        };
        self.adopted.insert(unit.entry.site);
        for code in &unit.codes {
            if fresh && self.codes.contains_key(&code.site) {
                return Err(EngineError::invalid("Duplicate V9 code site."));
            }
            self.file(code);
        }
        self.file(&unit.entry);
        Ok(())
    }
    /** Files one code; a stub whose first call already compiled it (on this page or any other)
    files its compiled code and nested functions instead, so a rebuilt registry runs it at once. */
    fn file(&mut self, code: &Arc<Code>) {
        match code.lazy.as_ref().and_then(|lazy| lazy.compiled()).cloned() {
            Some(unit) => {
                for nested in unit.codes.iter().chain([&unit.entry]) {
                    self.file(nested);
                }
            }
            None => {
                self.codes.insert(code.site, code.clone());
            }
        }
    }
    /** The code to run at `site`: filed code as it is, or a stub compiled now and filed, with
    its nested functions, in its place. None for an unknown site; an error when the deferred
    compile fails, the early error an eager compile would have raised at load. */
    pub fn compiled(&mut self, site: u32) -> EngineResult<Option<Arc<Code>>> {
        let Some(code) = self.codes.get(&site).cloned() else {
            return Ok(None);
        };
        let Some(lazy) = &code.lazy else {
            return Ok(Some(code));
        };
        let unit = lazy.unit(code.source.as_ref())?;
        for nested in unit.codes.iter().chain([&unit.entry]) {
            self.file(nested);
        }
        Ok(Some(unit.entry.clone()))
    }
    /** Look up compiled code by site. */
    pub fn get(&self, site: u32) -> Option<&Arc<Code>> {
        self.codes.get(&site)
    }
}

/** Whether `code` is the entry of exactly this dynamic source: its text, its offset and, for
direct eval, its caller scopes. Scripts carry no scopes, so they match only scripts. */
fn is_same_source(code: &Code, source: &str, offset: usize, scopes: Option<&str>) -> bool {
    code.scopes.as_deref() == scopes
        && code
            .source
            .as_ref()
            .is_some_and(|(text, at)| *at == offset && **text == *source)
}

/** Compile a classic script and its nested functions into one shareable unit. */
fn compile_script(
    program: &Program,
    source: Option<(&str, usize)>,
) -> EngineResult<Arc<cache::Unit>> {
    let mut compiler = Compiler::default();
    let script = compiler.script(program)?;
    Ok(Arc::new(cache::Unit::new(
        compiler.done,
        script,
        None,
        source,
    )))
}

/** Compile direct eval code in its caller's scopes. */
fn compile_eval(
    program: &Program,
    context: eval::Context,
    source: (&str, usize),
    scopes: &str,
) -> EngineResult<Arc<cache::Unit>> {
    let mut compiler = Compiler::default();
    let mut script = compiler.eval_script(program, context)?;
    script.scopes = Some(Arc::from(scopes));
    Ok(Arc::new(cache::Unit::new(
        compiler.done,
        script,
        None,
        Some(source),
    )))
}

/** Compile a module and its nested functions into one shareable unit with its link facts. */
fn compile_module(program: Program, source: (&str, usize)) -> EngineResult<Arc<cache::Unit>> {
    let mut compiler = Compiler::default();
    let (site, info) = compiler.module(program)?;
    let position = compiler
        .done
        .iter()
        .position(|code| code.site == site)
        .ok_or_else(|| EngineError::invalid("V9 lost a module's code."))?;
    let body = compiler.done.remove(position);
    Ok(Arc::new(cache::Unit::new(
        compiler.done,
        body,
        Some(info),
        Some(source),
    )))
}

/** Where a binding lives. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Register(Slot),
    Env(Slot),
    /** Top-level bindings live on the global object or the global lexical map, by name. */
    Global,
}

/** How a binding was declared, which decides dead-zone checks and assignability. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Declared {
    Var,
    Let,
    Const,
    /** A name only assignment cannot change silently: a class's own name inside its body. */
    ClassInner,
    /** A named function expression's own name: a sloppy write is dropped, a strict one throws. */
    FunctionName,
    /** Hidden bindings: this, arguments, new.target, the home object. */
    Hidden,
    /** A plain `catch (name)` parameter, which a `var` or block function may cross (B.3.4). */
    Catch,
    /** A module's imported name: a live read of another module's export, never assignable. */
    Import,
}

/** One binding in a scope. */
#[derive(Clone, Debug)]
struct Binding {
    name: Name,
    declared: Declared,
    place: Place,
    /// Ordinary block-function bindings allow Annex B copies through them to an existing var.
    block_function: bool,
}

/** One lexical scope during compilation. Shared copy-on-write: a deferred function keeps the
scopes it was written in, so taking them costs a pointer per scope, and only a scope changed
after that is copied. */
#[derive(Clone)]
struct Scope {
    bindings: Vec<Binding>,
    /// Position of each name in `bindings`: a minified bundle declares thousands of names in one
    /// function scope and reads them millions of times, so lookups cannot scan the vector.
    index: std::collections::HashMap<Name, usize>,
    /// Whether this scope has a runtime environment record.
    env: bool,
    /// Slots allocated in that environment.
    slots: Slot,
    /// First scope of a function: lookups crossing it leave the function.
    boundary: bool,
    /// The top-level script scope: names resolve globally.
    global: bool,
}

/** What a break or continue target is. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Loop,
    Switch,
    /** A labeled non-loop statement: only a labeled break leaves it. */
    Block,
}

/** A break or continue target. */
struct Target {
    labels: Vec<Name>,
    kind: TargetKind,
    breaks: Vec<usize>,
    continues: Vec<usize>,
    /// Handlers active outside the target.
    handlers: u16,
    /// Environments pushed outside the target.
    envs: u16,
}

/** Per-function emission state. */
#[derive(Default)]
struct FnState {
    ops: Vec<Op>,
    positions: Vec<(u32, Pos)>,
    callees: Vec<(u32, SiteKind, Arc<str>)>,
    strings: Vec<JsString>,
    /// Strong mixing avoids clustered UTF16 identifier hashes; keys share the pool's immutable string storage.
    string_index: std::collections::HashMap<JsString, u32>,
    /// Private names interned apart from `string_index`, so they never share an entry.
    private_index: FastMap<Vec<u16>, u32>,
    templates: Vec<Vec<(Option<JsString>, JsString)>>,
    registers: Slot,
    max_registers: Slot,
    /// Runtime environments pushed inside this function, beyond its own.
    envs: u16,
    /// Active exception handlers.
    handlers: u16,
    /// Lexical disposal stacks active in this function, independent of nested function compilation.
    resources: Vec<Slot>,
    targets: Vec<Target>,
    /// Labels waiting for the next loop or statement.
    pending_labels: Vec<Name>,
    /// Script and eval code: the register holding the completion value (spec 8.? UpdateEmpty).
    /// Every expression statement stores into it and every statement whose empty completion
    /// becomes undefined resets it first, as V8's Rewriter does.
    completion: Option<Slot>,
    last_pos: Option<Pos>,
    /// Index of the first scope of this function in the shared stack.
    scope_base: usize,
    strict: bool,
    /// A strict expression being emitted within this function, never an executing VM mode.
    lexical_strict: bool,
    /// Sorted strict instruction ranges for an otherwise sloppy activation.
    strict_ranges: Vec<(u32, u32)>,
    flags: FuncFlags,
    /// Open optional chains.
    chains: Vec<expr::Chain>,
    /// Compiling the parameter list, where a sloppy direct eval may not declare `arguments`.
    in_params: bool,
    /// Direct eval grammar follows this function's actual lexical receiver environment.
    eval_grammar: u8,
    /// Sloppy eval code: its var scope among the caller's scopes (None for the global scope),
    /// where Annex B block functions store their value.
    eval_vars: Option<Option<usize>>,
    /// Parameter names, excluding an implicit `arguments` object binding: an
    /// Annex B block function of such a name never updates the function-level binding (B.3.2.1).
    annex_skip: Vec<Name>,
    /// The last occurrence of each simple sloppy parameter is its mapped arguments index.
    arguments: Option<Vec<Option<Slot>>>,
}

/** The compiler: a stack of functions over one shared scope stack. */
#[derive(Default)]
struct Compiler {
    scopes: Vec<Arc<Scope>>,
    functions: Vec<FnState>,
    done: Vec<Code>,
    /// Enclosing classes, innermost last, for private names: each one's site and declared names.
    classes: Vec<(u32, Vec<Name>)>,
    /// The module body being compiled, if any.
    module: Option<module::ModuleScope>,
    /// Compiling a deferred function, whose subtree `early` already checked when it deferred.
    checked: bool,
    /// Compile every function now, as the deferred-compile oracle test does.
    eager: bool,
}

/** Most registers any one function may use. */
const MAX_REGISTERS: usize = Slot::MAX as usize;

impl Compiler {
    /** The function being emitted. */
    fn f(&mut self) -> &mut FnState {
        self.functions.last_mut().expect("function state")
    }
    /** Append an instruction, returning its index. */
    fn emit(&mut self, op: Op) -> usize {
        let state = self.f();
        state.record_strict(state.ops.len() as u32);
        state.ops.push(op);
        state.ops.len() - 1
    }
    /** The next instruction index. */
    fn here(&mut self) -> usize {
        self.f().ops.len()
    }
    /** Record the source position for the following instructions. */
    fn at(&mut self, pos: Pos) {
        let state = self.f();
        if state.last_pos != Some(pos) {
            state.last_pos = Some(pos);
            let pc = state.ops.len() as u32;
            if state.positions.last().is_some_and(|(last, _)| *last == pc) {
                state.positions.pop();
            }
            state.positions.push((pc, pos));
        }
    }
    /** Point the jump at `index` to the current instruction. */
    fn patch(&mut self, index: usize) {
        let target = self.here() as u32;
        self.patch_to(index, target);
    }
    /** Point the jump at `index` to `target`. */
    fn patch_to(&mut self, index: usize, target: u32) {
        let op = &mut self.f().ops[index];
        *op = match *op {
            Op::Jump(_) => Op::Jump(target),
            Op::JumpIfFalse(_) => Op::JumpIfFalse(target),
            Op::JumpIfTrue(_) => Op::JumpIfTrue(target),
            Op::JumpIfFalseKeep(_) => Op::JumpIfFalseKeep(target),
            Op::JumpIfTrueKeep(_) => Op::JumpIfTrueKeep(target),
            Op::JumpIfNotNullishKeep(_) => Op::JumpIfNotNullishKeep(target),
            Op::JumpIfNullish(_) => Op::JumpIfNullish(target),
            Op::JumpOut(_, handlers, envs) => Op::JumpOut(target, handlers, envs),
            Op::IterNext(_) => Op::IterNext(target),
            Op::AsyncIterValue(_) => Op::AsyncIterValue(target),
            Op::AsyncIterClose(_) => Op::AsyncIterClose(target),
            Op::ForInNext(_) => Op::ForInNext(target),
            other => other,
        };
    }
    /** Intern a string constant. */
    fn string(&mut self, value: &JsString) -> u32 {
        let state = self.f();
        if let Some(index) = state.string_index.get(value) {
            return *index;
        }
        let index = state.strings.len() as u32;
        state.strings.push(value.clone());
        state.string_index.insert(value.clone(), index);
        index
    }
    /** Intern a private name in its own table: its index means a private key, never a string. */
    fn private_string(&mut self, value: &JsString) -> u32 {
        let state = self.f();
        if let Some(index) = state.private_index.get(value.as_units()) {
            return *index;
        }
        let index = state.strings.len() as u32;
        state.strings.push(value.clone());
        state.private_index.insert(value.as_units().to_vec(), index);
        index
    }
    /** Intern a name. */
    fn name(&mut self, name: &str) -> EngineResult<u32> {
        let value = JsString::from_utf8(name)?;
        Ok(self.string(&value))
    }
    /** Allocate a temporary register. */
    fn temp(&mut self) -> EngineResult<Slot> {
        let state = self.f();
        if state.registers as usize >= MAX_REGISTERS {
            return Err(EngineError::limit("V9 registers", MAX_REGISTERS));
        }
        let slot = state.registers;
        state.registers += 1;
        state.max_registers = state.max_registers.max(state.registers);
        Ok(slot)
    }
    /** Release temporaries down to `mark`. */
    fn release(&mut self, mark: Slot) {
        self.f().registers = mark;
    }
    /** Push a scope; `env` gives it a runtime environment. */
    fn push_scope(&mut self, env: bool, boundary: bool) {
        self.scopes.push(Arc::new(Scope {
            bindings: Vec::new(),
            index: std::collections::HashMap::default(),
            env,
            slots: 0,
            boundary,
            global: false,
        }));
    }
    /** Declare a binding in the innermost scope; redeclaring a name reuses its place. */
    fn declare(&mut self, name: &Name, declared: Declared) -> EngineResult<Place> {
        let scope = Arc::make_mut(self.scopes.last_mut().expect("scope"));
        if let Some(existing) = scope.index.get(name).map(|&at| &mut scope.bindings[at]) {
            if declared != Declared::Var && declared != Declared::Hidden {
                existing.declared = declared;
            }
            return Ok(existing.place);
        }
        let place = if scope.global {
            Place::Global
        } else if scope.env {
            let slot = scope.slots;
            scope.slots = scope
                .slots
                .checked_add(1)
                .ok_or_else(|| EngineError::limit("V9 scope bindings", MAX_REGISTERS))?;
            Place::Env(slot)
        } else {
            let place = Place::Register(self.temp()?);
            return Ok(self.push_binding(name, declared, place));
        };
        Ok(self.push_binding(name, declared, place))
    }
    /** Add a binding record to the innermost scope. */
    fn push_binding(&mut self, name: &Name, declared: Declared, place: Place) -> Place {
        let scope = Arc::make_mut(self.scopes.last_mut().expect("scope"));
        scope.index.insert(name.clone(), scope.bindings.len());
        scope.bindings.push(Binding {
            name: name.clone(),
            declared,
            place,
            block_function: false,
        });
        place
    }
    /** Find a binding: its place, declaration kind, and how many environments lie between. */
    fn resolve(&self, name: &str) -> Option<(Place, Declared, Slot)> {
        let mut hops: Slot = 0;
        let base = self.functions.last().map_or(0, |f| f.scope_base);
        for (index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(binding) = scope.index.get(name).map(|&at| &scope.bindings[at]) {
                return match binding.place {
                    // A register belongs to its own function only.
                    Place::Register(_) if index < base => None,
                    place => Some((place, binding.declared, hops)),
                };
            }
            if scope.env {
                hops += 1;
            }
        }
        None
    }
    /** Start a new function's emission state. */
    fn begin_function(&mut self, strict: bool, flags: FuncFlags) {
        let scope_base = self.scopes.len();
        self.functions.push(FnState {
            scope_base,
            strict,
            flags,
            ..FnState::default()
        });
    }
    /** Finish a function into code. */
    fn end_function(&mut self, func: Option<&Func>, site: u32, env: Option<Slot>) -> Code {
        let state = self.functions.pop().expect("function state");
        let (name, length, flags, strict) = match func {
            Some(func) => (
                JsString::from_utf8(&func.display.borrow()).unwrap_or_else(|_| empty()),
                func.length,
                func.flags,
                func.strict,
            ),
            None => (empty(), 0, FuncFlags::default(), state.strict),
        };
        Code {
            site,
            flags,
            strict,
            strict_ranges: state.strict_ranges,
            name,
            source: None,
            scopes: None,
            source_span: func.and_then(|func| *func.source_span.borrow()),
            length,
            ops: state.ops,
            registers: state.max_registers,
            env,
            arguments: state.arguments,
            caches: crate::v9::vm::shape::SiteCaches::new(state.strings.len()),
            jit: crate::v9::jit::JitSlot::default(),
            strings: state.strings,
            privates: state.private_index.into_values().collect(),
            templates: state.templates,
            positions: state.positions,
            callees: state.callees,
            lazy: None,
        }
    }
    /** Compile a script's top level. */
    fn script(&mut self, program: &Program) -> EngineResult<Code> {
        self.begin_function(program.strict, FuncFlags::default());
        self.scopes.push(Arc::new(Scope {
            bindings: Vec::new(),
            index: std::collections::HashMap::default(),
            env: false,
            slots: 0,
            boundary: true,
            global: true,
        }));
        self.at(program.pos);
        self.global_declarations(&program.body)?;
        let result = self.temp()?;
        self.emit(Op::Undefined);
        self.emit(Op::SetReg(result));
        self.emit(Op::Pop);
        self.statements_with_result(&program.body, Some(result))?;
        self.emit(Op::GetReg(result));
        self.emit(Op::Return);
        self.scopes.pop();
        let site = super::syntax::site_of(program.pos);
        Ok(self.end_function(None, site, None))
    }
    /** A syntax or limit error at a position, for checks only the compiler can make. */
    fn error(&self, pos: Pos, reason: &str) -> EngineError {
        errors::syntax(pos.location(), reason)
    }
}

/** The empty string. */
fn empty() -> JsString {
    JsString::from_units(Vec::new()).expect("empty string fits")
}

/** Whether a function body is a block. */
fn block_body(func: &Func) -> Option<&[Stmt]> {
    match &func.body {
        Body::Block(body) => Some(body),
        Body::Expr(_) => None,
    }
}

/** A private name's key, `<class offset>:<name>`, for the class `delta` bytes further along. */
fn rebased_private(key: &JsString, delta: i64) -> JsString {
    let text = key.lossy();
    match text.split_once(':') {
        Some((offset, name)) if offset.parse::<i64>().is_ok() => {
            let offset = offset.parse::<i64>().unwrap_or_default() + delta;
            // Identifiers are whole code points, so the lossy text is exact.
            JsString::from_units(format!("{offset}:{name}").encode_utf16().collect())
                .unwrap_or_else(|_| key.clone())
        }
        _ => key.clone(),
    }
}

#[cfg(test)]
mod rebase_tests {
    use super::{Code, compile_script};

    /** Every field of `code` that a page can observe or the VM reads, as text to compare. */
    pub(super) fn shape(code: &Code) -> String {
        format!(
            "{} {:?} {} {:?} {:?} {:?} {:?} {} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            code.site,
            code.flags,
            code.strict,
            code.strict_ranges,
            code.name,
            code.source.as_ref().map(|(text, base)| (text.len(), *base)),
            code.source_span,
            code.length,
            code.ops,
            code.registers,
            code.env,
            code.arguments,
            code.strings,
            code.privates,
            code.positions,
        )
    }

    /** Shapes of a unit's codes with every deferred function compiled and expanded in place,
    so a comparison covers what the first calls will run, not only the stubs. */
    pub(super) fn shapes(unit: &super::cache::Unit) -> Vec<String> {
        let mut all = Vec::new();
        for code in unit.codes.iter().chain([&unit.entry]) {
            match &code.lazy {
                Some(lazy) => all.extend(shapes(&lazy.unit(code.source.as_ref()).unwrap())),
                None => all.push(shape(code)),
            }
        }
        all
    }

    /** Every deferred function compiles to exactly the code the eager compile gives it: the
    scopes it captured are the scopes the eager compile saw at that point. */
    #[test]
    fn deferred_functions_compile_to_the_eager_code() {
        let source = "var top=1;function outer(a,b){const f=(x)=>x+a+top;class K extends Array{#p=1;\
            static s(){return 2}get g(){return this.#p}m(){return ()=>super.g}}\
            return [f(b),new K().g,K.s(),eval('a'),arguments[0]]}\
            const o={m(){return `t${top}`},*gen(){yield 1},async am(){await 0}};\
            {let blocked=2;function inner(){return blocked}}\
            label:for(let i=0;i<2;i++){try{throw new Error('e'+i)}catch(e){(()=>e)();continue label}}\
            outer(1,2);";
        let program = crate::v9::parse_at(source, 0, Some(0)).unwrap();
        let lazy = compile_script(&program, Some((source, 0))).unwrap();
        let mut compiler = super::Compiler {
            eager: true,
            ..super::Compiler::default()
        };
        let entry = compiler.script(&program).unwrap();
        let eager = super::cache::Unit::new(compiler.done, entry, None, Some((source, 0)));
        let sorted = |mut all: Vec<String>| {
            all.sort();
            all
        };
        assert!(lazy.codes.iter().any(|code| code.lazy.is_some()));
        assert_eq!(sorted(shapes(&lazy)), sorted(shapes(&eager)));
    }

    /** A script compiled at one offset and moved to another is exactly the script compiled
    there: the cache may hand out the moved one instead of parsing again. */
    #[test]
    fn rebased_code_matches_a_fresh_compile_at_the_new_offset() {
        let source = "var top=1;function outer(a,b){const f=(x)=>x+a;class K extends Array{#p=1;\
            static s(){return 2}get g(){return this.#p}}return [f(b),new K().g,K.s()]}\
            const o={m(){return `t${top}`},*gen(){yield 1},async am(){await 0}};\
            label:for(let i=0;i<2;i++){try{throw new Error('e'+i)}catch(e){continue label}}\
            outer(1,2);";
        let compile = |offset: usize| {
            let program = crate::v9::parse_at(source, offset, Some(0)).unwrap();
            compile_script(&program, Some((source, offset))).unwrap()
        };
        let (early, late) = (compile(100), compile(5_000));
        let moved = early.rebased(4_900);
        assert_eq!(shapes(&moved), shapes(&late));
        assert_ne!(shapes(&early), shapes(&late));
    }
}

#[cfg(test)]
mod rebase_real_page {
    /** A real page's scripts (a JSON array of sources in MARS_REBASE_SOURCES, e.g. YouTube's
    42 scripts) rebase to exactly what compiling them at the new offset gives. */
    #[test]
    #[ignore = "needs MARS_REBASE_SOURCES"]
    fn real_page_scripts_rebase_exactly() {
        let path = std::env::var("MARS_REBASE_SOURCES").expect("MARS_REBASE_SOURCES");
        let sources: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for (index, source) in sources.iter().enumerate() {
            let compile = |offset: usize| {
                let program = crate::v9::parse_at(source, offset, Some(index)).ok()?;
                super::compile_script(&program, Some((source, offset))).ok()
            };
            let (Some(early), Some(late)) = (compile(1_000), compile(777_777)) else {
                continue;
            };
            let moved = early.rebased(776_777);
            let shapes = super::rebase_tests::shapes;
            assert!(shapes(&moved) == shapes(&late), "script {index} differs");
        }
    }

    /** A real page's scripts compile lazily to exactly their eager code, every function. */
    #[test]
    #[ignore = "needs MARS_REBASE_SOURCES"]
    fn real_page_functions_compile_lazily_to_their_eager_code() {
        let path = std::env::var("MARS_REBASE_SOURCES").expect("MARS_REBASE_SOURCES");
        let sources: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let sorted = |mut all: Vec<String>| {
            all.sort();
            all
        };
        for (index, source) in sources.iter().enumerate() {
            let Ok(program) = crate::v9::parse_at(source, 0, Some(index)) else {
                continue;
            };
            let Ok(lazy) = super::compile_script(&program, Some((source, 0))) else {
                continue;
            };
            let mut compiler = super::Compiler {
                eager: true,
                ..super::Compiler::default()
            };
            let entry = compiler.script(&program).unwrap();
            let eager = super::cache::Unit::new(compiler.done, entry, None, Some((source, 0)));
            let shapes = super::rebase_tests::shapes;
            assert!(
                sorted(shapes(&lazy)) == sorted(shapes(&eager)),
                "script {index} differs"
            );
        }
    }
}

#[cfg(test)]
mod retained_tests {
    use super::{Code, Registry, cache};
    use crate::v9::compile_dynamic;
    use std::sync::Arc;

    /** The caller context the Function constructor compiles under. */
    const INDIRECT: &str = "n|g";
    /** A strict caller's context: the same text compiles to other code under it. */
    const STRICT: &str = "s|g";
    /** The offset of the first dynamic source; a second one sits past it. */
    const BASE: usize = crate::v9::DYNAMIC_OFFSET;
    /** Where a second dynamic source starts, clear of the first. */
    const SECOND: usize = BASE + 100;

    /** The registry the next step starts from: the carried one, with nothing adopted yet. */
    fn next_step(carried: &Registry) -> Registry {
        let mut step = carried.clone();
        step.begin_step();
        step
    }

    /** The source text a code was compiled from. */
    fn text(code: &Code) -> &str {
        code.source
            .as_ref()
            .map(|(text, _)| &**text)
            .unwrap_or_default()
    }

    /** Classic and eval code kept by an earlier step is handed back as the same code to a later
    step that re-adopts it: nothing is parsed or compiled again. */
    #[test]
    fn retained_dynamic_code_is_reused_across_steps() {
        let mut first = Registry::default();
        let script = compile_dynamic(&mut first, "1+1", BASE, None, false).unwrap();
        let eval = compile_dynamic(&mut first, "2+2", SECOND, Some(INDIRECT), false).unwrap();
        let lookups = cache::lookups();
        let mut step = next_step(&first);
        let again = compile_dynamic(&mut step, "1+1", BASE, None, false).unwrap();
        assert!(Arc::ptr_eq(&script, &again));
        let again = compile_dynamic(&mut step, "2+2", SECOND, Some(INDIRECT), false).unwrap();
        assert!(Arc::ptr_eq(&eval, &again));
        let mut later = next_step(&step);
        let again = compile_dynamic(&mut later, "1+1", BASE, None, false).unwrap();
        assert!(Arc::ptr_eq(&script, &again));
        // No keyed lookup at all: the registry answers before the source is hashed.
        assert_eq!(cache::lookups(), lookups);
    }

    /** Another source at an offset an earlier step's source held compiles fresh, and the next
    step sees the new one, not the old one. */
    #[test]
    fn a_different_source_at_the_same_offset_recompiles() {
        let mut first = Registry::default();
        let old = compile_dynamic(&mut first, "1+1", BASE, None, false).unwrap();
        let mut step = next_step(&first);
        let new = compile_dynamic(&mut step, "2+2", BASE, None, false).unwrap();
        assert!(!Arc::ptr_eq(&old, &new));
        assert_eq!(text(&new), "2+2");
        let mut later = next_step(&step);
        let again = compile_dynamic(&mut later, "2+2", BASE, None, false).unwrap();
        assert!(Arc::ptr_eq(&new, &again));
    }

    /** The same text at the same offset under other caller scopes, or as a plain script, is
    other code: a retained eval answers only for its own scopes. */
    #[test]
    fn an_eval_with_a_different_context_recompiles() {
        let mut first = Registry::default();
        let indirect = compile_dynamic(&mut first, "x", BASE, Some(INDIRECT), false).unwrap();
        let mut step = next_step(&first);
        let strict = compile_dynamic(&mut step, "x", BASE, Some(STRICT), false).unwrap();
        assert!(!Arc::ptr_eq(&indirect, &strict));
        assert_eq!(strict.scopes.as_deref(), Some(STRICT));
        let mut again = next_step(&step);
        let script = compile_dynamic(&mut again, "x", BASE, None, false).unwrap();
        assert!(!Arc::ptr_eq(&strict, &script));
        assert_eq!(script.scopes, None);
        let mut back = next_step(&again);
        let indirect_again = compile_dynamic(&mut back, "x", BASE, Some(INDIRECT), false).unwrap();
        assert_eq!(indirect_again.scopes.as_deref(), Some(INDIRECT));
    }
}

#[cfg(test)]
mod filing_tests {
    use super::{Registry, compile_script};

    /** A script compiled at `offset`, as its own unit. */
    fn unit(source: &str, offset: usize) -> std::sync::Arc<super::cache::Unit> {
        let program = crate::v9::parse_at(source, offset, Some(0)).unwrap();
        compile_script(&program, Some((source, offset))).unwrap()
    }

    /** A later step re-adopting the same unit, or the same source compiled again, files it
    without a duplicate-site refusal; another source on the same site is refused within a
    step and takes the site over in the next one. */
    #[test]
    fn a_registry_carried_across_steps_refiles_only_what_changed() {
        let mut registry = Registry::default();
        let first = unit("function f(){return 1}f();", 10);
        registry.adopt(&first).unwrap();
        let filed = registry.codes.len();
        registry.adopt(&first).unwrap();
        assert_eq!(registry.codes.len(), filed);
        registry
            .adopt(&unit("function f(){return 1}f();", 10))
            .unwrap();
        assert_eq!(registry.codes.len(), filed);
        assert!(
            registry
                .adopt(&unit("function g(){return 2}g();", 10))
                .is_err()
        );
        // Only this step's adoptions answer a dynamic compile without the keyed cache.
        let entry = first.entry.site;
        assert!(registry.adopted_this_step(entry).is_some());
        registry.begin_step();
        assert!(registry.adopted_this_step(entry).is_none());
        // The next step may compile another source where an earlier step's was; none of the old
        // source's functions survive to answer for the new source's sites.
        let old: Vec<u32> = first.codes.iter().map(|code| code.site).collect();
        assert!(!old.is_empty());
        registry.adopt(&unit("var x=1;", 10)).unwrap();
        for site in old {
            assert!(
                registry.codes.get(&site).is_none(),
                "old function site {site} survived"
            );
        }
        assert!(registry.codes.get(&entry).is_some());
    }
}
