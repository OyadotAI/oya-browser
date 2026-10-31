/** The instruction loop: one bytecode op at a time, with explicit unwinding and suspension. */
/** Activations a throw trace keeps; the innermost ones say where an error came from. */
const THROW_TRACE_FRAMES: usize = 8;
/** Short spreads cost less to store directly than the fixed character-root table. */
#[cfg(feature = "conformance")]
const MIN_COMPACT_STRING_LENGTH: usize = 4096;
use super::call::Invoked;
use super::data::{Class, Function, FunctionKind, Handler};
use super::heap::{CONFIGURABLE, ENUMERABLE, Key, Object, Prop, Slot, WRITABLE};
use super::property::Descriptor;
use super::realm::realm;
use super::{Exit, Res, Resume, Thrown, Vm};
use crate::constants::SCRIPT_CLOCK_STRIDE;
use crate::protocol::EngineError;
use crate::v9::binding::{Binding, BindingKind};
use crate::v9::builtins;
use crate::v9::compile::{Op, SiteKind};
use crate::v9::value::{JsString, JsValue, ObjectId, to_int32, to_uint32};

mod asyncgen;
mod declarations;
mod eval;
mod hole_spread;
mod super_update;
mod with;

/** Completion kinds a finally block's registers hold. */
const NORMAL: f64 = 0.0;
const THROW: f64 = 1.0;
const RETURN: f64 = 2.0;
const JUMP: f64 = 3.0;
/** V8's ReferenceError for `this` (or a return) in a derived constructor before `super()`. */
const SUPER_NOT_CALLED: &str = "Must call super constructor in derived class before accessing 'this' or returning from derived constructor";
/** Packs handler and environment depths into one completion register. */
const DEPTH_SHIFT: f64 = 65_536.0;
/** V8's `%GetOptimizationStatus` bits mjsunit reads: kIsFunction (1) and kNeverOptimize (2). V9
has no optimizer, so a function is "never optimized" and anything else just "never optimized". */
#[cfg(feature = "conformance")]
const OPTIMIZATION_STATUS_FUNCTION: f64 = 3.0;
#[cfg(feature = "conformance")]
const OPTIMIZATION_STATUS_NEVER: f64 = 2.0;

/** What one instruction did to control flow. */
enum Flow {
    Next,
    Exit(Exit),
}

impl Vm<'_> {
    /** Run until the innermost boundary frame returns, yields or awaits. */
    pub fn run(&mut self) -> Res<Exit> {
        let mut pending = match self.take_resume() {
            Ok(Some(exit)) => return Ok(exit),
            Ok(None) => None,
            Err(thrown) => Some(thrown),
        };
        loop {
            let result = match pending.take() {
                Some(thrown) => Err(thrown),
                None => self.execute(),
            };
            match result {
                Ok(exit) => return Ok(exit),
                Err(thrown) => {
                    if let Some(exit) = self.unwind(thrown)? {
                        return Ok(exit);
                    }
                }
            }
        }
    }
    /** Apply a generator's throw or return resumption before running. */
    fn take_resume(&mut self) -> Res<Option<Exit>> {
        let Some(frame) = self.frames.last_mut() else {
            return Ok(None);
        };
        let mode = std::mem::take(&mut frame.resume);
        if matches!(
            frame.code.ops.get(frame.pc),
            Some(Op::YieldStar | Op::AsyncYieldStar)
        ) {
            frame.resume = mode;
            return Ok(None);
        }
        match mode {
            Resume::Next => Ok(None),
            Resume::Throw => Err(Thrown::Value(
                frame.stack.pop().unwrap_or(JsValue::Undefined),
            )),
            Resume::Return => {
                let value = frame.stack.pop().unwrap_or(JsValue::Undefined);
                match self.do_return(value)? {
                    Flow::Next => Ok(None),
                    Flow::Exit(exit) => Ok(Some(exit)),
                }
            }
        }
    }
    /** Set how the top (just restored) frame resumes. */
    pub fn resume_with(&mut self, mode: Resume, value: JsValue) -> Res<()> {
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| EngineError::invalid("No V9 frame to resume."))?;
        frame.stack.push(value);
        frame.resume = mode;
        Ok(())
    }
    /** Find a handler for a thrown value; returns an exit if it left the boundary frame. */
    fn unwind(&mut self, thrown: Thrown) -> Res<Option<Exit>> {
        let value = match thrown {
            Thrown::Value(value) => value,
            Thrown::Fatal(error) => {
                let error = self.locate_fatal(error);
                while let Some(frame) = self.frames.pop() {
                    if frame.boundary {
                        break;
                    }
                }
                return Err(Thrown::Fatal(error));
            }
        };
        #[cfg(feature = "conformance")]
        if self.page.throw_location.is_none() {
            builtins::debug_event(self, builtins::DebugEvent::Exception);
        }
        self.note_throw_location();
        loop {
            let Some(frame) = self.frames.last_mut() else {
                return Err(Thrown::Value(value));
            };
            if let Some(handler) = frame.handlers.pop() {
                frame.stack.truncate(handler.stack as usize);
                frame.env = handler.env;
                frame.envs = handler.envs;
                if handler.catch != 0 {
                    frame.stack.push(value);
                    frame.pc = handler.catch as usize;
                    self.page.throw_location = None;
                    return Ok(None);
                }
                let slot = handler.slot as usize;
                frame.regs[slot] = JsValue::Number(THROW);
                frame.regs[slot + 1] = value;
                frame.pc = handler.finally as usize;
                return Ok(None);
            }
            let boundary = frame.boundary;
            self.frames.pop();
            if boundary {
                return Err(Thrown::Value(value));
            }
        }
    }
    /** Remember where an exception was first thrown, for diagnostics. */
    fn note_throw_location(&mut self) {
        if self.page.throw_location.is_some() {
            return;
        }
        if let Some(frame) = self.frames.last() {
            let mut location = frame.code.position(frame.pc.saturating_sub(1)).location();
            location.script_index = None;
            self.page.throw_location = Some(location);
            self.page.throw_trace = self.throw_trace();
        }
    }
    /** The innermost THROW_TRACE_FRAMES activations as "name (source line:column)" lines, for
    the console: without them an error in a bundle names only a position in megabytes of code. */
    pub(in crate::v9) fn throw_trace(&self) -> Vec<String> {
        self.frames
            .iter()
            .rev()
            .take(THROW_TRACE_FRAMES)
            .map(|frame| {
                let location = frame.code.position(frame.pc.saturating_sub(1)).location();
                let name = String::from_utf16_lossy(frame.code.name.as_units());
                let name = if name.is_empty() {
                    "<anonymous>"
                } else {
                    name.as_str()
                };
                format!(
                    "{name} ({} line {}, column {})",
                    self.modules.source_name(location.byte_offset),
                    location.line,
                    location.column
                )
            })
            .collect()
    }
    /** Give an engine error the current position. */
    fn locate_fatal(&self, error: EngineError) -> EngineError {
        match self.frames.last() {
            Some(frame) => super::error::locate(
                error,
                frame.code.position(frame.pc.saturating_sub(1)).location(),
            ),
            None => error,
        }
    }
    /** One spread is one instruction however long its source, so its loop checks the clock
    every SCRIPT_CLOCK_STRIDE elements: otherwise a huge copy overran the script deadline by
    more than the coordinator's RPC margin and the caller saw "timeout" in place of the
    script's own time limit. */
    pub(super) fn spread_tick(&self, count: usize) -> Res<()> {
        if count.is_multiple_of(SCRIPT_CLOCK_STRIDE) && self.expired() {
            return Err(Thrown::Fatal(super::super::platform::timeout()));
        }
        Ok(())
    }
        /** Execute instructions until an exit or an exception. */
    fn execute(&mut self) -> Res<Exit> {
        loop {
            self.remaining = match self.remaining.checked_sub(1) {
                Some(remaining) => remaining,
                None => return Err(self.charge(1).expect_err("budget exhausted")),
            };
            if self.remaining.is_multiple_of(SCRIPT_CLOCK_STRIDE) {
                if self.expired() {
                    return Err(Thrown::Fatal(super::super::platform::timeout()));
                }
                #[cfg(feature = "conformance")]
                crate::v9::builtins::profile_tick(self)?;
                self.collect_if_needed()?;
                #[cfg(feature = "conformance")]
                if let Some(host) = self.page.environment.conformance_workers.clone() {
                    host.collection_safepoint(self.page.environment.conformance_worker_id, self)?;
                }
            }
            if self.collect_due {
                self.collect_if_due()?;
            }
            let realm = self.frames.last().expect("running frame").realm;
            if realm != self.active_realm {
                self.select_realm(realm)?;
            }
            let frame = self.frames.last_mut().expect("running frame");
            let Some(op) = frame.code.ops.get(frame.pc).copied() else {
                return Err(EngineError::invalid("V9 ran past the end of its code.").into());
            };
            frame.pc += 1;
            if let Flow::Exit(exit) = self.op(op)? {
                return Ok(exit);
            }
        }
    }
    /** The running frame. */
    #[inline]
    fn frame(&mut self) -> &mut super::Frame {
        self.frames.last_mut().expect("running frame")
    }
    /** References use the current instruction lexical strictness; activation and stack privacy keep the function own mode. */
    #[inline]
    fn reference_strict(&self) -> bool {
        self.frames
            .last()
            .is_some_and(|frame| frame.code.strict_at(frame.pc.saturating_sub(1)))
    }
    /** Push onto the operand stack. */
    #[inline]
    pub(in crate::v9) fn push(&mut self, value: JsValue) {
        if let JsValue::String(text) = &value {
            self.track_string(text);
        }
        self.frame().stack.push(value);
    }
    /** Pop the operand stack. */
    #[inline]
    pub(in crate::v9) fn pop(&mut self) -> JsValue {
        self.frame().stack.pop().unwrap_or(JsValue::Undefined)
    }
    /** Read the value `depth` below the top. */
    #[inline]
    fn peek(&mut self, depth: usize) -> JsValue {
        let stack = &self.frame().stack;
        stack
            .get(stack.len().wrapping_sub(depth + 1))
            .cloned()
            .unwrap_or(JsValue::Undefined)
    }
    /** The two plain numbers on top of the stack, read in place: the arithmetic and comparison
    fast paths neither clone nor drop operand values, which the generic peek-and-drop sequence
    did on every operation. */
    #[cfg(not(feature = "conformance"))]
    fn number_pair(&mut self) -> Option<(f64, f64)> {
        let stack = &self.frame().stack;
        let top = stack.len().checked_sub(1)?;
        match (stack.get(top.checked_sub(1)?), stack.get(top)) {
            (Some(JsValue::Number(a)), Some(JsValue::Number(b))) => Some((*a, *b)),
            _ => None,
        }
    }
    /** Replace the two values on top of the stack with one result. */
    #[cfg(not(feature = "conformance"))]
    fn replace_pair(&mut self, value: JsValue) {
        let stack = &mut self.frame().stack;
        stack.pop();
        if let Some(top) = stack.last_mut() {
            *top = value;
        }
    }
    /** Drop `count` values. */
    fn drop_n(&mut self, count: usize) {
        let stack = &mut self.frame().stack;
        let length = stack.len().saturating_sub(count);
        stack.truncate(length);
    }
    /** A string constant of the running code. */
    fn constant(&mut self, index: u32) -> JsString {
        #[cfg(feature = "conformance")]
        {
            let (site, source) = {
                let code = &self.frame().code;
                (code.site, code.strings.get(index as usize).cloned())
            };
            let Some(source) = source else {
                return JsString::empty();
            };
            self.isolate_code_constant(site, index, &source)
        }
        #[cfg(not(feature = "conformance"))]
        self.frame()
            .code
            .strings
            .get(index as usize)
            .cloned()
            .unwrap_or_else(JsString::empty)
    }
    /** A named-key constant as a key: private only when the compiler interned it as one. */
    fn constant_key(&mut self, index: u32) -> Key {
        let text = self.constant(index);
        if self.frame().code.privates.contains(&index) {
            Key::Private(self.private_for_evaluation(text))
        } else {
            Key::Str(text)
        }
    }
    /** A compiled private name (`site:name`) made unique to the class evaluation in scope: the
    nearest environment marked for that class site adds its identity, so two evaluations of one
    class body never share a private name. */
    fn private_for_evaluation(&self, text: JsString) -> JsString {
        let units = text.as_units();
        let Some(colon) = units.iter().position(|&u| u == u16::from(b':')) else {
            return text;
        };
        let site = String::from_utf16_lossy(&units[..colon])
            .parse::<u32>()
            .ok();
        let mut env = self.frames.last().and_then(|frame| frame.env);
        for _ in 0..super::property::CHAIN_STEPS {
            let Some(id) = env else { break };
            let Some(record) = self.heap.env(id) else {
                break;
            };
            if record.class_site.is_some() && record.class_site == site {
                let name = String::from_utf16_lossy(&units[colon + 1..]);
                let spelled = format!("{}:{id}:{name}", site.unwrap_or(0));
                return JsString::from_utf8(&spelled).unwrap_or(text);
            }
            env = record.parent;
        }
        text
    }
    /** The environment `hops` levels out. */
    fn env_at(&mut self, hops: u16) -> Res<u32> {
        let mut env = self.frame().env;
        for _ in 0..hops {
            env = env
                .and_then(|id| self.heap.env(id))
                .and_then(|record| record.parent);
        }
        env.ok_or_else(|| EngineError::invalid("Invalid V9 environment chain.").into())
    }
    /** Execute one instruction. */
    fn op(&mut self, op: Op) -> Res<Flow> {
        match op {
            Op::Undefined => self.push(JsValue::Undefined),
            Op::Null => self.push(JsValue::Null),
            Op::True => self.push(JsValue::Boolean(true)),
            Op::False => self.push(JsValue::Boolean(false)),
            Op::Hole => self.push(JsValue::Hole),
            Op::Number(value) => {
                #[cfg(feature = "conformance")]
                if self
                    .page
                    .environment
                    .conformance_native_main_source
                    .is_some()
                    && value.is_finite()
                    && value.fract() == 0.0
                    && value >= -(1_i64 << 30) as f64
                    && value < (1_i64 << 30) as f64
                    && !(value == 0.0 && value.is_sign_negative())
                {
                    self.push(JsValue::Smi(value));
                } else if self
                    .page
                    .environment
                    .conformance_native_main_source
                    .is_some()
                {
                    let (site, source) = {
                        let code = &self.frame().code;
                        (
                            code.site,
                            code.source.as_ref().map(|(source, _)| source.clone()),
                        )
                    };
                    let literal = match source {
                        Some(source) => self.number_literal(site, &source, value)?,
                        None => JsValue::Number(value),
                    };
                    self.push(literal);
                } else {
                    self.push(JsValue::Number(value));
                }
                #[cfg(not(feature = "conformance"))]
                self.push(JsValue::Number(value));
            }
            Op::BigInt(index) => {
                let text = self.constant(index).lossy();
                self.charge(crate::v9::value::BigInteger::parse_work(&text))?;
                let value =
                    crate::v9::value::BigInteger::parse(&text).map_err(|error| match error {
                        crate::v9::value::BigIntParseError::Limit => {
                            self.range_error("Maximum BigInt size exceeded")
                        }
                        crate::v9::value::BigIntParseError::Syntax => {
                            EngineError::invalid("Invalid V9 BigInt literal.").into()
                        }
                    })?;
                self.push(JsValue::bigint(value));
            }
            Op::String(index) => {
                let value = self.constant(index);
                #[cfg(feature = "conformance")]
                let value = if self.page.environment.conformance_d8
                    && value.len() == 1
                    && value.unit_at(0).is_some_and(|unit| unit <= 255)
                {
                    self.character_root(value.unit_at(0).expect("one character") as u8)?
                } else if self.page.environment.conformance_shared_string_table {
                    self.share_string(&value)?
                } else {
                    value
                };
                #[cfg(feature = "conformance")]
                value.mark_old();
                self.push(JsValue::String(value));
            }
            Op::Pop => {
                self.pop();
            }
            Op::Dup => {
                let value = self.peek(0);
                self.push(value);
            }
            Op::Dup2 => {
                let (a, b) = (self.peek(1), self.peek(0));
                self.push(a);
                self.push(b);
            }
            Op::Swap => {
                let stack = &mut self.frame().stack;
                let length = stack.len();
                if length >= 2 {
                    stack.swap(length - 1, length - 2);
                }
            }
            Op::Rot3 => {
                let stack = &mut self.frame().stack;
                let length = stack.len();
                if length >= 3 {
                    stack[length - 3..].rotate_right(1);
                }
            }
            Op::Rot4 => {
                let stack = &mut self.frame().stack;
                let length = stack.len();
                if length >= 4 {
                    stack[length - 4..].rotate_right(1);
                }
            }
            Op::GetReg(slot) => {
                let value = self.frame().regs[slot as usize].clone();
                self.push(value);
            }
            Op::GetRegChecked(slot, name) => {
                let value = self.frame().regs[slot as usize].clone();
                if value == JsValue::Hole {
                    return Err(self.dead_zone(name));
                }
                self.push(value);
            }
            Op::SetReg(slot) => {
                let value = self.peek(0);
                self.frame().regs[slot as usize] = value;
            }
            Op::SetRegChecked(slot, name) => {
                if self.frame().regs[slot as usize] == JsValue::Hole {
                    return Err(self.dead_zone(name));
                }
                let value = self.peek(0);
                self.frame().regs[slot as usize] = value;
            }
            Op::GetEnv(hops, slot) => {
                let env = self.env_at(hops)?;
                let value = self.env_slot(env, slot)?;
                self.push(value);
            }
            Op::GetEnvChecked(hops, slot, name) => {
                let env = self.env_at(hops)?;
                let value = self.env_slot(env, slot)?;
                if value == JsValue::Hole {
                    return Err(self.dead_zone(name));
                }
                self.push(value);
            }
            Op::SetEnv(hops, slot) => {
                let env = self.env_at(hops)?;
                let value = self.peek(0);
                self.heap.set_slot(env, slot as usize, value);
            }
            Op::SetEnvChecked(hops, slot, name) => {
                let env = self.env_at(hops)?;
                if self.env_slot(env, slot)? == JsValue::Hole {
                    return Err(self.dead_zone(name));
                }
                let value = self.peek(0);
                self.heap.set_slot(env, slot as usize, value);
            }
            Op::PushEnv(slots) => {
                let parent = self.frame().env;
                let env = self
                    .heap
                    .new_env(parent, vec![JsValue::Hole; slots as usize])?;
                let frame = self.frame();
                frame.env = Some(env);
                frame.envs += 1;
            }
            Op::ClassScope(site) => {
                if let Some(env) = self.frame().env {
                    self.heap.mark_class_env(env, site);
                }
            }
            Op::PopEnv => {
                let env = self.frame().env;
                let parent = env
                    .and_then(|id| self.heap.env(id))
                    .and_then(|record| record.parent);
                let frame = self.frame();
                frame.env = parent;
                frame.envs = frame.envs.saturating_sub(1);
            }
            Op::CopyEnv => {
                if let Some(env) = self.frame().env
                    && let Some(record) = self.heap.env(env).cloned()
                {
                    let copy = self.heap.new_env(record.parent, record.slots)?;
                    self.frame().env = Some(copy);
                }
            }
            Op::GetGlobal(name) => {
                let name = self.constant(name);
                let value = self.global_get(&name, false)?;
                self.push(value);
            }
            Op::TypeofGlobal(name) => {
                let name = self.constant(name);
                let value = self.global_get(&name, true)?;
                let kind = self.type_of(&value);
                self.push(JsValue::string(kind)?);
            }
            Op::SetGlobal(name) => {
                let name = self.constant(name);
                let value = self.peek(0);
                self.global_set(&name, value)?;
            }
            Op::DeleteGlobal(name) => {
                let name = self.constant(name);
                let text = name.lossy();
                let deleted = if self.globals.contains_key(&text) {
                    false
                } else {
                    self.delete(self.global_object(), &Key::Str(name))?
                };
                self.push(JsValue::Boolean(deleted));
            }
            Op::AnnexGlobal(name, store) => {
                let name = self.constant(name);
                if !self.globals.contains_key(&name.lossy()) {
                    if store {
                        let value = self.peek(0);
                        self.global_set(&name, value)?;
                    } else {
                        self.declare_annex_global(name)?;
                    }
                }
            }
            Op::DeclareVar(name, deletable) => {
                let name = self.constant(name);
                self.declare_global_var(name, deletable)?;
            }
            Op::DeclareOptionalVar(name) => {
                let name = self.constant(name);
                self.declare_optional_global_var(name)?;
            }
            Op::CheckGlobalVar(name, eval) => {
                let name = self.constant(name);
                self.check_global_var(&name, eval)?;
            }
            Op::DeclareFunction(name, deletable) => {
                let name = self.constant(name);
                let value = self.pop();
                self.declare_global_function(name, value, deletable)?;
            }
            Op::CheckGlobalFunction(name, eval) => {
                let name = self.constant(name);
                self.check_global_function(&name, eval)?;
            }
            Op::DirectEval(count, context) => return self.direct_eval(count, context),
            #[cfg(feature = "conformance")]
            Op::NativeIntrinsic(intrinsic) => match intrinsic {
                0 => {
                    let value = self.pop();
                    let message = self.to_text(&value)?;
                    return Err(EngineError::new(
                        "conformance_abort_js",
                        &message,
                        "The upstream test invoked %AbortJS.",
                    )
                    .into());
                }
                1 => {
                    self.drain_jobs()?;
                    self.push(JsValue::Undefined);
                }
                2 | 12 => {
                    let detach_key = if intrinsic == 12 {
                        self.pop()
                    } else {
                        JsValue::Undefined
                    };
                    let value = self.pop();
                    let Some(id) = value.object() else {
                        return Err(self.type_error("ArrayBuffer expected"));
                    };
                    if !matches!(
                        self.heap.get(id).map(|object| &object.class),
                        Some(Class::ArrayBuffer(_))
                    ) || !builtins::buffer_detachable(self, id)
                        || detach_key != JsValue::Undefined
                    {
                        return Err(self.type_error("ArrayBuffer cannot be detached"));
                    }
                    builtins::detach_buffer(self, id);
                    self.push(JsValue::Undefined);
                }
                3 => self.push(JsValue::Boolean(cfg!(target_pointer_width = "64"))),
                4 => {
                    let value = self.pop();
                    let length = self
                        .to_integer(&value)?
                        .clamp(0.0, crate::constants::VM_MAX_SAFE_LENGTH);
                    self.push(JsValue::Number(length));
                }
                5 => {
                    let value = self.pop();
                    let count = builtins::synchronization_waiter_count(self, &value)?;
                    self.push(count);
                }
                14 | 15 => {
                    let index = self.peek(0);
                    let value = self.peek(1);
                    let count =
                        builtins::atomics_waiter_count(self, &value, &index, intrinsic == 15)?;
                    self.drop_n(2);
                    self.push(count);
                }
                16 => {
                    let allowed = self.pop().truthy();
                    self.page.state.atomics_wait_disallowed = !allowed;
                    self.push(JsValue::Undefined);
                }
                17 => self.push(JsValue::Boolean(!self.page.state.atomics_wait_disallowed)),
                18 => {
                    let value = self.pop();
                    self.push(JsValue::Boolean(
                        matches!(value, JsValue::String(value) if value.is_shared()),
                    ));
                }
                19 => {
                    let right = self.pop();
                    let left = self.pop();
                    let same = match (&left, &right) {
                        (JsValue::Object(a), JsValue::Object(b)) => a == b,
                        (JsValue::String(a), JsValue::String(b)) => a.same_backing(b),
                        (JsValue::SharedNumber(a), JsValue::SharedNumber(b))
                        | (JsValue::LocalNumber(a), JsValue::LocalNumber(b)) => a.root == b.root,
                        (
                            JsValue::LocalNumber(_) | JsValue::SharedNumber(_),
                            JsValue::Number(number),
                        )
                        | (
                            JsValue::Number(number),
                            JsValue::LocalNumber(_) | JsValue::SharedNumber(_),
                        ) if number.is_finite()
                            && number.fract() == 0.0
                            && *number >= -(1_i64 << 30) as f64
                            && *number < (1_i64 << 30) as f64
                            && !(*number == 0.0 && number.is_sign_negative()) =>
                        {
                            return Err(
                                self.unsupported_error("V8 heap identity for unboxed primitives")
                            );
                        }
                        (JsValue::LocalNumber(_) | JsValue::SharedNumber(_), JsValue::Smi(_))
                        | (JsValue::Smi(_), JsValue::LocalNumber(_) | JsValue::SharedNumber(_)) => {
                            return Err(
                                self.unsupported_error("V8 heap identity for small integers")
                            );
                        }
                        (JsValue::LocalNumber(_), _)
                        | (_, JsValue::LocalNumber(_))
                        | (JsValue::SharedNumber(_), JsValue::Number(_))
                        | (JsValue::Number(_), JsValue::SharedNumber(_)) => false,
                        (
                            JsValue::SharedNumber(_),
                            JsValue::Object(_) | JsValue::String(_) | JsValue::Symbol(_),
                        )
                        | (
                            JsValue::Object(_) | JsValue::String(_) | JsValue::Symbol(_),
                            JsValue::SharedNumber(_),
                        ) => false,
                        (
                            JsValue::Number(n),
                            JsValue::Undefined
                            | JsValue::Null
                            | JsValue::Boolean(_)
                            | JsValue::String(_)
                            | JsValue::Object(_)
                            | JsValue::Symbol(_),
                        )
                        | (
                            JsValue::Undefined
                            | JsValue::Null
                            | JsValue::Boolean(_)
                            | JsValue::String(_)
                            | JsValue::Object(_)
                            | JsValue::Symbol(_),
                            JsValue::Number(n),
                        ) if !n.is_finite()
                            || n.fract() != 0.0
                            || *n < -(1_i64 << 30) as f64
                            || *n >= (1_i64 << 30) as f64
                            || (*n == 0.0 && n.is_sign_negative()) =>
                        {
                            false
                        }
                        (JsValue::Symbol(a), JsValue::Symbol(b)) => a == b,
                        (
                            JsValue::Object(_) | JsValue::String(_) | JsValue::Symbol(_),
                            JsValue::Object(_) | JsValue::String(_) | JsValue::Symbol(_),
                        ) => false,
                        _ => {
                            return Err(
                                self.unsupported_error("V8 heap identity for unboxed primitives")
                            );
                        }
                    };
                    self.push(JsValue::Boolean(same));
                }
                20 => {
                    builtins::collect_shared(self)?;
                    self.push(JsValue::Undefined);
                }
                21 => {
                    let value = self.pop();
                    let initializer = value
                        .object()
                        .and_then(|id| self.heap.get(id))
                        .and_then(|object| object.function())
                        .and_then(|function| function.fields);
                    self.push(initializer.map_or(JsValue::Undefined, JsValue::Object));
                }
                22 => {
                    let value = self.peek(0);
                    let result = builtins::wasm_force_detach(self, &value)?;
                    self.pop();
                    self.push(result);
                }
                23 => {
                    let instance = self.peek(0);
                    let exception = self.peek(1);
                    let result = builtins::wasm_exception_tag_id(self, &exception, &instance)?;
                    self.drop_n(2);
                    self.push(result);
                }
                24 => {
                    let exception = self.peek(0);
                    let result = builtins::wasm_exception_values(self, &exception)?;
                    self.pop();
                    self.push(result);
                }
                25 => return Err(builtins::forced_stack_overflow(self)),
                26 => self.push(JsValue::Number(crate::constants::JS_STRING_UNITS as f64)),
                31 => {
                    let right = self.pop();
                    let left = self.pop();
                    let result = match (&left, &right) {
                        (JsValue::String(left), JsValue::String(right)) => {
                            left.construct_cons(right)?.map(JsValue::String)
                        }
                        _ => None,
                    };
                    if result.is_none() && !self.page.environment.conformance_fuzzing {
                        return Err(self.unsupported_error("ConstructConsString precondition"));
                    }
                    self.push(result.unwrap_or(JsValue::Undefined));
                }
                32 => {
                    const MAX_SMI: f64 = ((1u32 << 30) - 1) as f64;
                    let start = self.pop();
                    let text = self.pop();
                    let result = match (&text, &start) {
                        (JsValue::String(text), JsValue::Number(start) | JsValue::Smi(start))
                            if *start >= 0.0
                                && !start.is_sign_negative()
                                && *start <= MAX_SMI
                                && start.fract() == 0.0 =>
                        {
                            text.construct_slice(*start as usize)?.map(JsValue::String)
                        }
                        _ => None,
                    };
                    if result.is_none() && !self.page.environment.conformance_fuzzing {
                        return Err(self.unsupported_error("ConstructSlicedString precondition"));
                    }
                    self.push(result.unwrap_or(JsValue::Undefined));
                }
                33 | 34 => {
                    self.pop();
                    self.pop();
                    if !self.page.environment.conformance_fuzzing {
                        return Err(self.unsupported_error(
                            "string construction intrinsic with nonstandard arity",
                        ));
                    }
                    self.push(JsValue::Undefined);
                }
                35 => {
                    let value = self.pop();
                    let JsValue::String(text) = value else {
                        return Err(self.unsupported_error("FlattenString requires a string"));
                    };
                    self.push(JsValue::String(text.flatten_for_intrinsic()?));
                }
                38 => {
                    let value = self.pop();
                    let JsValue::String(source) = value else {
                        return if self.page.environment.conformance_fuzzing {
                            self.push(JsValue::Undefined);
                            Ok(Flow::Next)
                        } else {
                            Err(self
                                .unsupported_error("ConstructInternalizedString requires a string"))
                        };
                    };
                    let result = self.internalize_string(&source)?;
                    self.push(JsValue::String(result));
                }
                39 => {
                    let value = self.pop();
                    let JsValue::String(source) = value else {
                        return if self.page.environment.conformance_fuzzing {
                            self.push(JsValue::Undefined);
                            Ok(Flow::Next)
                        } else {
                            Err(self.unsupported_error("ConstructThinString requires a string"))
                        };
                    };
                    let existing = source.thin_target().is_some() || source.is_actual_cons();
                    let thin = if existing {
                        Some(source)
                    } else {
                        JsString::empty().construct_cons(&source)?
                    };
                    let Some(thin) = thin else {
                        return if self.page.environment.conformance_fuzzing {
                            self.push(JsValue::Undefined);
                            Ok(Flow::Next)
                        } else {
                            Err(self.unsupported_error("ConstructThinString precondition"))
                        };
                    };
                    if !existing {
                        thin.mark_old();
                    }
                    if thin.thin_target().is_none() {
                        self.internalize_string(&thin)?;
                    }
                    self.push(JsValue::String(thin));
                }
                40 | 41 => {
                    self.pop();
                    if !self.page.environment.conformance_fuzzing {
                        return Err(self.unsupported_error("string construction intrinsic arity"));
                    }
                    self.push(JsValue::Undefined);
                }
                42 => {
                    let id = self.alloc(super::heap::Object::new(
                        super::data::Class::Number(0.0),
                        None,
                    ))?;
                    self.push(JsValue::LocalNumber(std::sync::Arc::new(
                        crate::v9::value::SharedNumberData {
                            value: 0.0,
                            root: id,
                        },
                    )));
                }
                46 => self.push(JsValue::Smi(f64::from(crate::v9::value::SMI_MAX))),
                48 | 49 => {
                    let value = builtins::wasm_intrinsic_object(self, intrinsic == 49)?;
                    self.push(value);
                }
                50 => self.push(JsValue::Boolean(false)),
                51..=56 => {
                    let right = self.pop();
                    let left = self.pop();
                    let (JsValue::String(left), JsValue::String(right)) = (left, right) else {
                        return Err(self.unsupported_error(
                            "V8 string comparison intrinsic requires string operands",
                        ));
                    };
                    let ordering = left.cmp(&right);
                    let value = match intrinsic {
                        51 => JsValue::Boolean(ordering.is_lt()),
                        52 => JsValue::Boolean(!ordering.is_gt()),
                        53 => JsValue::Boolean(ordering.is_gt()),
                        54 => JsValue::Boolean(!ordering.is_lt()),
                        55 => JsValue::Boolean(ordering.is_eq()),
                        _ => JsValue::Smi(match ordering {
                            std::cmp::Ordering::Less => -1.0,
                            std::cmp::Ordering::Equal => 0.0,
                            std::cmp::Ordering::Greater => 1.0,
                        }),
                    };
                    self.push(value);
                }
                58 | 60 => {
                    let input = self.peek(0);
                    let primitive = self.to_primitive(&input, super::convert::Hint::Number)?;
                    let value = if primitive.number_value().is_some()
                        || (intrinsic == 60 && primitive.is_bigint())
                    {
                        primitive
                    } else {
                        JsValue::Number(self.to_number(&primitive)?)
                    };
                    self.pop();
                    self.push(value);
                }
                59 => {
                    let input = self.peek(0);
                    let value = self.to_string(&input)?;
                    self.pop();
                    self.push(JsValue::String(value));
                }
                61 => {
                    let low = self.pop();
                    let high = self.pop();
                    let (Some(high), Some(low)) = (high.number_value(), low.number_value()) else {
                        return Err(
                            self.unsupported_error("V8 ConstructDouble requires numeric operands")
                        );
                    };
                    let bits = (u64::from(to_uint32(high)) << 32) | u64::from(to_uint32(low));
                    let number = f64::from_bits(bits);
                    let value = if number.is_finite()
                        && number.fract() == 0.0
                        && number >= -((1_i64 << 30) as f64)
                        && number <= f64::from(crate::v9::value::SMI_MAX)
                        && !(number == 0.0 && number.is_sign_negative())
                    {
                        JsValue::Smi(number)
                    } else {
                        let root = self.alloc(super::heap::Object::new(
                            super::data::Class::Number(number),
                            None,
                        ))?;
                        JsValue::LocalNumber(std::sync::Arc::new(
                            crate::v9::value::SharedNumberData {
                                value: number,
                                root,
                            },
                        ))
                    };
                    self.push(value);
                }
                62 => {
                    let value = self.pop();
                    let result = match value {
                        JsValue::String(text) => text.thin_target().is_none() && text.is_interned(),
                        JsValue::Smi(_) | JsValue::Number(_) => {
                            return Err(self.unsupported_error(
                                "V8 IsInternalizedString requires a heap object",
                            ));
                        }
                        _ => false,
                    };
                    self.push(JsValue::Boolean(result));
                }
                63..=65 => {
                    if cfg!(target_arch = "mips64") {
                        return Err(self.unsupported_error("V8 MIPS hole NaN encoding"));
                    }
                    const HOLE_NAN_WORD: u32 = 0xfff7_ffff;
                    let number = if intrinsic == 63 {
                        f64::from_bits((u64::from(HOLE_NAN_WORD) << 32) | u64::from(HOLE_NAN_WORD))
                    } else {
                        f64::from(HOLE_NAN_WORD)
                    };
                    let root = self.alloc(super::heap::Object::new(
                        super::data::Class::Number(number),
                        None,
                    ))?;
                    self.push(JsValue::LocalNumber(std::sync::Arc::new(
                        crate::v9::value::SharedNumberData {
                            value: number,
                            root,
                        },
                    )));
                }
                68 => {
                    let value = self.pop();
                    let status = match value {
                        JsValue::Object(id) => self.jit.status(id) as f64,
                        _ => {
                            if self.is_callable(&value) {
                                OPTIMIZATION_STATUS_FUNCTION
                            } else {
                                OPTIMIZATION_STATUS_NEVER
                            }
                        }
                    };
                    self.push(JsValue::Smi(status));
                }
                66 => {
                    // V9 has no V8 maps; verify its real heap graph and object invariants instead.
                    self.charge(self.heap.objects.len())?;
                    self.validate_graph()?;
                    self.pop();
                    self.push(JsValue::Boolean(true));
                }
                47 => {
                    let maximum = builtins::array_buffer_max_allocation();
                    self.push(if maximum <= crate::v9::value::SMI_MAX as usize {
                        JsValue::Smi(maximum as f64)
                    } else {
                        JsValue::Number(maximum as f64)
                    });
                }
                44 => {
                    let input = self.peek(0);
                    let primitive = self.to_primitive(&input, super::convert::Hint::Number)?;
                    let result = match primitive {
                        value @ (JsValue::LocalNumber(_) | JsValue::SharedNumber(_)) => value,
                        JsValue::Number(_) => {
                            return Err(
                                self.unsupported_error("HeapNumber identity for untracked number")
                            );
                        }
                        value => {
                            let number = self.to_number(&value)?;
                            let id = self.alloc(super::heap::Object::new(
                                super::data::Class::Number(number),
                                None,
                            ))?;
                            JsValue::LocalNumber(std::sync::Arc::new(
                                crate::v9::value::SharedNumberData {
                                    value: number,
                                    root: id,
                                },
                            ))
                        }
                    };
                    self.pop();
                    self.push(result);
                }
                45 => {
                    self.pop();
                    if !self.page.environment.conformance_fuzzing {
                        return Err(self.unsupported_error("AllocateHeapNumberWithValue arity"));
                    }
                    self.push(JsValue::Undefined);
                }
                43 => {
                    let value = self.pop();
                    if matches!(value, JsValue::Number(number) if number.is_finite() && number.fract() == 0.0 && number >= -(1_i64 << 30) as f64 && number < (1_i64 << 30) as f64 && !(number == 0.0 && number.is_sign_negative()))
                    {
                        return Err(self.unsupported_error("Smi identity for untracked number"));
                    }
                    self.push(JsValue::Boolean(matches!(value, JsValue::Smi(_))));
                }
                37 => {
                    let value = self.pop();
                    let JsValue::String(source) = value else {
                        return Err(self.unsupported_error("InternalizeString requires a string"));
                    };
                    let result = self.internalize_string(&source)?;
                    self.push(JsValue::String(result));
                }
                36 => {
                    let value = self.pop();
                    let young = match &value {
                        JsValue::String(text) => !text.is_old(),
                        JsValue::Undefined | JsValue::Null | JsValue::Boolean(_) => false,
                        JsValue::Number(number)
                            if number.is_finite()
                                && number.fract() == 0.0
                                && *number >= -(1_i64 << 30) as f64
                                && *number < (1_i64 << 30) as f64
                                && !(*number == 0.0 && number.is_sign_negative()) =>
                        {
                            false
                        }
                        JsValue::Smi(_) | JsValue::SharedNumber(_) => false,
                        JsValue::LocalNumber(value) => self
                            .heap
                            .get(value.root)
                            .is_some_and(|object| object.age < super::gc::PROMOTION_AGE),
                        JsValue::Object(id) if builtins::is_shared_object(self, *id) => false,
                        JsValue::Object(id)
                            if *id < super::heap::INTRINSIC_BASE
                                && !builtins::is_shared_object(self, *id) =>
                        {
                            match self.heap.get(*id) {
                                Some(object)
                                    if !matches!(
                                        object.class,
                                        Class::Host(_)
                                            | Class::WasmShared(_)
                                            | Class::SharedContainerRoot(_)
                                    ) =>
                                {
                                    object.age < super::gc::PROMOTION_AGE
                                }
                                _ => {
                                    return Err(self.unsupported_error(
                                        "InYoungGeneration requires tracked ordinary storage",
                                    ));
                                }
                            }
                        }
                        _ => {
                            return Err(self.unsupported_error(
                                "InYoungGeneration requires tracked ordinary storage",
                            ));
                        }
                    };
                    self.push(JsValue::Boolean(young));
                }
                27 | 29 | 30 => {
                    let env = &self.page.environment;
                    if !(env.conformance_shared_heap
                        || env.conformance_shared_strings
                        || env.conformance_shared_string_table
                        || env.conformance_harmony_struct)
                        || !(env.conformance_shared_strings
                            || env.conformance_shared_string_table
                            || env.conformance_harmony_struct)
                    {
                        return Err(self.unsupported_error(
                            "ShareObject requires the shared heap and shared strings",
                        ));
                    }
                    if intrinsic != 27 && !self.page.environment.conformance_fuzzing {
                        return Err(self.unsupported_error("ShareObject with nonstandard arity"));
                    }
                    let value = self.pop();
                    let value = match value {
                        JsValue::Number(number)
                            if number.is_finite()
                                && number.fract() == 0.0
                                && number >= -(1_i64 << 30) as f64
                                && number < (1_i64 << 30) as f64
                                && !(number == 0.0 && number.is_sign_negative()) =>
                        {
                            JsValue::Number(number)
                        }
                        JsValue::Number(number) => builtins::new_shared_number(self, number)?,
                        JsValue::LocalNumber(value) => {
                            builtins::new_shared_number(self, value.value)?
                        }
                        JsValue::String(value) => JsValue::String(self.share_string(&value)?),
                        JsValue::Smi(_)
                        | JsValue::SharedNumber(_)
                        | JsValue::Boolean(_)
                        | JsValue::Null
                        | JsValue::Undefined => value,
                        JsValue::Object(id) if builtins::is_shared_object(self, id) => {
                            let mark = self.pins.len();
                            self.pin(JsValue::Object(id));
                            let result = builtins::materialize_shared_object(self, id);
                            self.pins.truncate(mark);
                            result?;
                            JsValue::Object(id)
                        }
                        JsValue::Object(id)
                            if builtins::conformance_local_mutex(self, &JsValue::Object(id))
                                || builtins::conformance_local_condition(
                                    self,
                                    &JsValue::Object(id),
                                ) =>
                        {
                            JsValue::Object(id)
                        }
                        _ if self.page.environment.conformance_fuzzing => JsValue::Undefined,
                        _ => return Err(self.type_error("Value cannot be shared")),
                    };
                    self.push(value);
                }
                28 => {
                    let value = self.pop();
                    let mark = self.pins.len();
                    self.pin(value.clone());
                    let result = if let Some(id) = value
                        .object()
                        .filter(|id| builtins::is_shared_object(self, *id))
                    {
                        builtins::materialize_shared_object(self, id)
                    } else {
                        Ok(())
                    };
                    self.pins.truncate(mark);
                    result?;
                    let shared = matches!(&value, JsValue::SharedNumber(_))
                        || matches!(&value, JsValue::String(s) if s.is_shared())
                        || value.object().is_some_and(|id| {
                            matches!(
                                self.heap.get(id).map(|object| &object.class),
                                Some(Class::WasmShared(_))
                            )
                        })
                        || value
                            .object()
                            .is_some_and(|id| builtins::is_shared_object(self, id))
                        || builtins::conformance_local_mutex(self, &value)
                        || builtins::conformance_local_condition(self, &value);
                    self.push(JsValue::Boolean(shared));
                }
                6 => {
                    let value = self.pop();
                    self.push(JsValue::Boolean(value.object().is_some()));
                }
                7 => {
                    let value = self.pop();
                    let is_array = value.object().is_some_and(|id| {
                        matches!(
                            self.heap.get(id).map(|object| &object.class),
                            Some(Class::Array(_))
                        ) || id == crate::v9::vm::realm::realm_for(id).ids.array_prototype
                    });
                    self.push(JsValue::Boolean(is_array));
                }
                8 => {
                    let value = self.pop();
                    self.push(JsValue::Boolean(value.truthy()));
                }
                9 => {
                    let value = self.pop();
                    self.push(JsValue::string(self.type_of(&value))?);
                }
                10 => {
                    let value = self.pop();
                    let result = match self.to_key(&value)? {
                        Key::Str(value) => JsValue::String(value),
                        Key::Sym(id) => JsValue::Symbol(id),
                        Key::Private(_) => {
                            return Err(EngineError::invalid("Invalid V9 property key.").into());
                        }
                    };
                    self.push(result);
                }
                11 => {
                    let done = self.pop().truthy();
                    let value = self.pop();
                    let result = self.new_object()?;
                    self.put(result, "value", value)?;
                    self.put(result, "done", JsValue::Boolean(done))?;
                    self.push(JsValue::Object(result));
                }
                13 => {
                    let value = self.pop();
                    let size = value
                        .object()
                        .and_then(|id| self.heap.get(id))
                        .and_then(|object| match &object.class {
                            Class::WeakMap(table) | Class::WeakSet(table) => Some(table.live),
                            _ => None,
                        })
                        .ok_or_else(|| self.type_error("Expected a weak collection"))?;
                    self.push(JsValue::Number(size as f64));
                }
                _ => return Err(self.unsupported_error("unknown V8 native intrinsic")),
            },
            Op::GetExt(name, hops, slot) => self.get_ext(name, hops, slot)?,
            Op::SetExt(name, hops, slot) => self.set_ext(name, hops, slot)?,
            Op::DeclareExt(name, hops, slot) => self.declare_ext(name, hops, slot)?,
            Op::DeleteExt(name, hops, slot) => self.delete_ext(name, hops, slot)?,
            Op::GetWith(name, hops, slot, call) => self.get_with(name, hops, slot, call)?,
            Op::SetWith(name, hops, slot) => self.set_with(name, hops, slot)?,
            Op::DeleteWith(name, hops, slot) => self.delete_with(name, hops, slot)?,
            Op::ResolveRef(name, hops, slot, with) => self.resolve_ref(name, hops, slot, with)?,
            Op::GetRef(name) => self.get_ref(name)?,
            Op::SetRef(name) => self.set_ref(name)?,
            Op::ToObject => {
                let value = self.pop();
                let id = self.to_object(&value)?;
                self.push(JsValue::Object(id));
            }
            Op::ThrowSyntax(message) => {
                let message = self.constant(message).lossy();
                return Err(self.syntax_error(&message));
            }
            Op::CheckGlobalLexical(name) => {
                let name = self.constant(name);
                self.check_global_lexical(&name)?;
            }
            Op::DeclareLexical(name, constant) => {
                let name = self.constant(name);
                self.check_global_lexical(&name)?;
                let text = name.lossy();
                self.globals.insert(
                    text,
                    Binding {
                        kind: if constant {
                            BindingKind::Const
                        } else {
                            BindingKind::Let
                        },
                        value: None,
                    },
                );
            }
            Op::InitLexical(name) => {
                let name = self.constant(name).lossy();
                let value = self.peek(0);
                let units = value.string_units();
                if let Some(binding) = self.globals.get_mut(&name) {
                    binding.value = Some(value);
                    self.heap.adjust(0, units as isize);
                }
            }
            Op::ThrowReference(message) => {
                let message = self.constant(message).lossy();
                return Err(self.reference_error(&message));
            }
            Op::ThrowConst(_) => return Err(self.type_error("Assignment to constant variable.")),
            Op::LoadThis => {
                let value = self.frame().this.clone();
                self.push(value);
            }
            Op::LoadNewTarget => {
                let value = self.frame().new_target.clone();
                self.push(value);
            }
            Op::LoadHome => {
                let callee = self.frame().callee.clone();
                let home = callee
                    .object()
                    .and_then(|id| self.heap.get(id))
                    .and_then(|object| object.function())
                    .and_then(|function| function.home);
                self.push(home.map_or(JsValue::Undefined, JsValue::Object));
            }
            Op::LoadCallee => {
                let value = self.frame().callee.clone();
                self.push(value);
            }
            Op::CreateArguments => {
                let args = self.frame().args.clone();
                let id = self.arguments_object(&args)?;
                self.push(JsValue::Object(id));
            }
            Op::GlobalThis => self.push(JsValue::Object(self.global_object())),
            Op::NewObject => {
                let id = self.new_object()?;
                self.push(JsValue::Object(id));
            }
            Op::NewArray(count) => {
                let stack = &mut self.frame().stack;
                let start = stack.len().saturating_sub(count as usize);
                let elements: Vec<JsValue> = stack.drain(start..).collect();
                let id = self.new_array(elements)?;
                self.push(JsValue::Object(id));
            }
            Op::ArrayPush => {
                let value = self.pop();
                let array = self.peek(0);
                self.array_append(&array, value)?;
            }
            Op::ArraySpread => {
                let iterable = self.peek(0);
                let array = self.peek(1);
                let roots = self.pins.len();
                let site = self.site_here();
                let iterator = match self.get_iterator_at(&iterable, site) {
                    Ok(iterator) => iterator,
                    Err(error) => {
                        if let Thrown::Value(value) = &error {
                            self.push(value.clone());
                        }
                        self.pins.truncate(roots);
                        return Err(error);
                    }
                };
                self.push(JsValue::Object(iterator));
                self.pins.truncate(roots);
                if self.compact_hole_spread(&array, iterator)? {
                    self.drop_n(2);
                    return Ok(Flow::Next);
                }
                #[cfg(feature = "conformance")]
                if self.compact_character_spread(&array, iterator)? {
                    self.drop_n(2);
                    return Ok(Flow::Next);
                }
                let mut appended = 0usize;
                while let Some(value) = self.iter_step(iterator)? {
                    self.array_append(&array, value)?;
                    appended += 1;
                    self.spread_tick(appended)?;
                }
                self.drop_n(2);
            }
            Op::DefineField(key) => {
                let key = self.constant_key(key);
                let value = self.peek(0);
                let object = self.peek(1);
                self.create_data(&object, key, value)?;
                self.pop();
            }
            Op::DefineFieldComputed => {
                let value = self.peek(0);
                let key = self.peek(1);
                let object = self.peek(2);
                let key = self.to_key(&key)?;
                self.create_data(&object, key, value)?;
                self.drop_n(2);
            }
            Op::DefineMethod(key, kind, enumerable) => {
                let key = self.constant_key(key);
                let function = self.peek(0);
                let object = self.peek(1);
                self.define_method(&object, key, function, kind, enumerable)?;
                self.pop();
            }
            Op::DefineMethodComputed(kind, enumerable) => {
                let function = self.peek(0);
                let key = self.peek(1);
                let object = self.peek(2);
                let key = self.to_key(&key)?;
                self.define_method(&object, key, function, kind, enumerable)?;
                self.drop_n(2);
            }
            Op::AutoAccessor(public, backing) => {
                let home = self.peek(0);
                let public = self.constant_key(public);
                let backing = self.constant_key(backing);
                self.define_auto_accessor(&home, public, backing)?;
            }
            Op::AutoAccessorComputed(backing) => {
                let home = self.peek(1);
                let public_value = self.peek(0);
                let public = self.to_key(&public_value)?;
                let backing = self.constant_key(backing);
                self.define_auto_accessor(&home, public, backing)?;
                self.pop();
            }
            Op::InferFunctionName => {
                let key_value = self.peek(0);
                let key = self.to_key(&key_value)?;
                let name = builtins::inferred_function_name(self, &key)?;
                let function = self
                    .peek(1)
                    .object()
                    .ok_or_else(|| EngineError::invalid("Invalid named field initializer."))?;
                self.set_name_snapshot(
                    function,
                    super::data::FunctionName::flat(JsValue::String(name)),
                )?;
                self.pop();
            }
            Op::SetInitializerHome => {
                let home = self.peek(0).object();
                let function = self
                    .peek(1)
                    .object()
                    .ok_or_else(|| EngineError::invalid("Invalid class initializer."))?;
                if let Some(Class::Function(data)) =
                    self.heap.get_mut(function).map(|object| &mut object.class)
                {
                    data.home = home;
                }
            }
            Op::InitializeDerivedFields => {
                let (callee, receiver) = self
                    .frames
                    .iter()
                    .rev()
                    .find(|frame| frame.code.flags.derived && frame.construct)
                    .map(|frame| (frame.callee.clone(), frame.this.clone()))
                    .ok_or_else(|| {
                        EngineError::invalid("Missing derived constructor initialization.")
                    })?;
                self.run_fields(&callee, &receiver)?;
            }
            Op::CopyData => {
                let source = self.peek(0);
                let target = self.peek(1);
                self.copy_data(&target, &source, &[])?;
                self.pop();
            }
            Op::RequireCoercible(name) => {
                let source = self.peek(0);
                if source.nullish() {
                    return Err(self.destructure_error(&source, name));
                }
            }
            Op::CopyRest(count) => {
                let stack = &mut self.frame().stack;
                let start = stack.len().saturating_sub(count as usize);
                let keys: Vec<JsValue> = stack.drain(start..).collect();
                let source = self.peek(0);
                if source.nullish() {
                    return Err(self.destructure_error(&source, u32::MAX));
                }
                let mut excluded = Vec::new();
                for key in &keys {
                    excluded.push(self.to_key(key)?);
                }
                let target = self.new_object()?;
                self.push(JsValue::Object(target));
                self.copy_data(&JsValue::Object(target), &source, &excluded)?;
                let rest = self.pop();
                self.pop();
                self.push(rest);
            }
            Op::SetProtoLiteral => {
                let proto = self.pop();
                let object = self.peek(0);
                if let JsValue::Object(id) = object {
                    match proto {
                        JsValue::Object(proto) => {
                            self.set_proto(id, Some(proto))?;
                        }
                        JsValue::Null => {
                            self.set_proto(id, None)?;
                        }
                        _ => {}
                    }
                }
            }
            Op::GetProp(site) => {
                let object = self.peek(0);
                let value = self.load_named(site, &object)?;
                self.pop();
                self.push(value);
            }
            Op::GetElem => {
                let key = self.peek(0);
                let object = self.peek(1);
                self.check_element_base(&object, &key, false)?;
                let value = match self.fast_element(&object, &key) {
                    Some(value) => value,
                    None => {
                        let key = self.to_key(&key)?;
                        self.get(&object, &key)?
                    }
                };
                self.drop_n(2);
                self.push(value);
            }
            Op::SetProp(site) => {
                let value = self.peek(0);
                let object = self.peek(1);
                self.store_named(site, &object, &value)?;
                self.drop_n(2);
                self.push(value);
            }
            Op::SetElem => {
                let value = self.peek(0);
                let key = self.peek(1);
                let object = self.peek(2);
                self.check_element_base(&object, &key, true)?;
                if !self.fast_store(&object, &key, &value) {
                    let key = self.to_key(&key)?;
                    let strict = self.reference_strict();
                    self.set(&object, key, value.clone(), strict)?;
                }
                self.drop_n(3);
                self.push(value);
            }
            Op::DeleteProp(key) => {
                let key = self.constant_key(key);
                let object = self.peek(0);
                let result = self.delete_value(&object, &key)?;
                self.pop();
                self.push(JsValue::Boolean(result));
            }
            Op::DeleteElem => {
                let key = self.peek(0);
                let object = self.peek(1);
                self.check_element_base(&object, &key, false)?;
                let key = self.to_key(&key)?;
                let result = self.delete_value(&object, &key)?;
                self.drop_n(2);
                self.push(JsValue::Boolean(result));
            }
            Op::In => {
                let object = self.peek(0);
                let key = self.peek(1);
                let JsValue::Object(id) = object else {
                    let key = self.v8_value_text(&key);
                    return Err(self.in_operator_error(&key, &object));
                };
                let key = self.to_key(&key)?;
                let result = self.has_property(id, &key)?;
                self.drop_n(2);
                self.push(JsValue::Boolean(result));
            }
            Op::PrivateIn(key) => {
                let key = self.constant_key(key);
                let JsValue::Object(id) = self.peek(0) else {
                    let object = self.peek(0);
                    let name = format!("#{}", super::private_elements::private_name(&key));
                    return Err(self.in_operator_error(&name, &object));
                };
                self.guard_realm_global(id)?;
                let result = self
                    .heap
                    .get(id)
                    .is_some_and(|object| object.props.get(&key).is_some());
                self.pop();
                self.push(JsValue::Boolean(result));
            }
            Op::InstanceOf => {
                let constructor = self.peek(0);
                let value = self.peek(1);
                let result = self.instance_of(&value, &constructor)?;
                self.drop_n(2);
                self.push(JsValue::Boolean(result));
            }
            Op::GetSuper(key) => {
                let key = self.constant_key(key);
                let home = self.peek(0);
                let this = self.peek(1);
                let value = self.super_get(&this, &home, &key)?;
                self.drop_n(2);
                self.push(value);
            }
            Op::GetSuperElem => {
                let home = self.peek(0);
                let key = self.peek(1);
                let this = self.peek(2);
                let home = home.object().ok_or_else(|| {
                    EngineError::invalid("Invalid V9 super reference home object.")
                })?;
                let base = builtins::object_get_prototype(self, home)?
                    .ok_or_else(|| self.type_error("Cannot read properties of null"))?;
                let key = self.to_key(&key)?;
                let value = self.get_from(base, &key, &this)?;
                self.drop_n(3);
                self.push(value);
            }
            Op::SetSuper(key) => {
                let key = self.constant_key(key);
                let value = self.peek(0);
                let home = self.peek(1);
                let this = self.peek(2);
                self.super_set(&this, &home, key, value.clone())?;
                self.drop_n(3);
                self.push(value);
            }
            Op::SetSuperElem => {
                let value = self.peek(0);
                let key = self.peek(1);
                let home = self.peek(2);
                let this = self.peek(3);
                let key = self.to_key(&key)?;
                self.super_set(&this, &home, key, value.clone())?;
                self.drop_n(4);
                self.push(value);
            }
            Op::CaptureSuperBase => self.capture_super_base()?,
            Op::ReadSuperReference => self.read_super_reference()?,
            Op::WriteSuperReference => self.write_super_reference()?,
            Op::ToElementKey => {
                let object = self.peek(1);
                let value = self.peek(0);
                self.check_element_base(&object, &value, false)?;
                let key = self.to_key(&value)?.value();
                self.pop();
                self.push(key);
            }
            Op::ToPropertyKey => {
                let value = self.peek(0);
                let key = self.to_key(&value)?.value();
                self.pop();
                self.push(key);
            }
            Op::Call(count) => {
                let stack = &mut self.frame().stack;
                let start = stack.len().saturating_sub(count as usize);
                let args: Vec<JsValue> = stack[start..].to_vec();
                let this = self.peek(count as usize);
                let function = self.peek(count as usize + 1);
                return self.call_op(function, this, args, count as usize + 2, None);
            }
            Op::CallSpread => {
                let array = self.peek(0);
                let args = self.array_values(&array)?;
                let this = self.peek(1);
                let function = self.peek(2);
                return self.call_op(function, this, args, 3, None);
            }
            Op::New(count) => {
                let stack = &mut self.frame().stack;
                let start = stack.len().saturating_sub(count as usize);
                let args: Vec<JsValue> = stack[start..].to_vec();
                let function = self.peek(count as usize);
                return self.call_op(
                    function.clone(),
                    JsValue::Undefined,
                    args,
                    count as usize + 1,
                    Some(function),
                );
            }
            Op::NewSpread => {
                let array = self.peek(0);
                let args = self.array_values(&array)?;
                let function = self.peek(1);
                return self.call_op(
                    function.clone(),
                    JsValue::Undefined,
                    args,
                    2,
                    Some(function),
                );
            }
            Op::SuperCall(count) => {
                let stack = &mut self.frame().stack;
                let start = stack.len().saturating_sub(count as usize);
                let args: Vec<JsValue> = stack[start..].to_vec();
                let this = self.super_call(args)?;
                self.drop_n(count as usize);
                self.push(this);
            }
            Op::SuperCallSpread => {
                let array = self.peek(0);
                let args = self.array_values(&array)?;
                let this = self.super_call(args)?;
                self.pop();
                self.push(this);
            }
            Op::LexicalSuperCall(count) => {
                let stack = &self.frame().stack;
                let start = stack.len().saturating_sub(count as usize + 2);
                let callee = stack[start].clone();
                let target = stack[start + 1].clone();
                let args = stack[start + 2..].to_vec();
                let this = self.construct_lexical_super(callee, target, args)?;
                self.drop_n(count as usize + 2);
                self.push(this);
            }
            Op::LexicalSuperCallSpread => {
                let array = self.peek(0);
                let args = self.array_values(&array)?;
                let target = self.peek(1);
                let callee = self.peek(2);
                let this = self.construct_lexical_super(callee, target, args)?;
                self.drop_n(3);
                self.push(this);
            }
            Op::BindLexicalSuper(hops, slot) => {
                let callee = self.pop();
                let this = self.peek(0);
                let env = self.env_at(hops)?;
                if self.env_slot(env, slot)? != JsValue::Hole {
                    return Err(self.reference_error("Super constructor may only be called once"));
                }
                self.heap.set_slot(env, slot as usize, this.clone());
                if let Some(index) = self.derived_frame_over(env) {
                    self.frames[index].this = this.clone();
                }
                self.run_fields(&callee, &this)?;
            }
            Op::Return => {
                let value = self.pop();
                return self.do_return(value);
            }
            Op::ReturnDerived => {
                // A derived constructor's return settles only when its frame exits: a `finally`
                // block or an iterator close still runs first and may be the super() call that
                // initializes `this`, and its exception outranks the constructor's own.
                let _ = self.pop();
                let value = self.pop();
                return self.do_return(value);
            }
            Op::Add => {
                #[cfg(not(feature = "conformance"))]
                if let Some((a, b)) = self.number_pair() {
                    self.replace_pair(JsValue::Number(a + b));
                    return Ok(Flow::Next);
                }
                let right = self.peek(0);
                let left = self.peek(1);
                let value = self.add(&left, &right)?;
                #[cfg(feature = "conformance")]
                let value = tagged_smi_result(&left, &right, value);
                self.drop_n(2);
                self.push(value);
            }
            Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Exp => {
                #[cfg(not(feature = "conformance"))]
                if let Some((a, b)) = self.number_pair() {
                    let value = match op {
                        Op::Sub => a - b,
                        Op::Mul => a * b,
                        Op::Div => a / b,
                        Op::Mod => js_remainder(a, b),
                        _ => js_power(a, b),
                    };
                    self.replace_pair(JsValue::Number(value));
                    return Ok(Flow::Next);
                }
                let right = self.peek(0);
                let left = self.peek(1);
                let (a, b) = match (&left, &right) {
                    (JsValue::Number(a), JsValue::Number(b)) => (*a, *b),
                    _ => match self.numeric_pair(op, &left, &right)? {
                        Ok(pair) => pair,
                        Err(value) => {
                            self.drop_n(2);
                            self.push(value);
                            return Ok(Flow::Next);
                        }
                    },
                };
                let value = match op {
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div => a / b,
                    Op::Mod => js_remainder(a, b),
                    _ => js_power(a, b),
                };
                self.drop_n(2);
                #[cfg(feature = "conformance")]
                let value = if matches!(op, Op::Sub | Op::Mul | Op::Div | Op::Mod) {
                    tagged_smi_result(&left, &right, JsValue::Number(value))
                } else {
                    JsValue::Number(value)
                };
                #[cfg(not(feature = "conformance"))]
                let value = JsValue::Number(value);
                self.push(value);
            }
            Op::Shl | Op::Shr | Op::UShr | Op::BitAnd | Op::BitOr | Op::BitXor => {
                let right = self.peek(0);
                let left = self.peek(1);
                // Minified code masks and shifts numbers constantly; they skip the conversions.
                let (a, b) = match (&left, &right) {
                    (JsValue::Number(a), JsValue::Number(b)) => (*a, *b),
                    _ => match self.numeric_pair(op, &left, &right)? {
                        Ok(pair) => pair,
                        Err(value) => {
                            self.drop_n(2);
                            self.push(value);
                            return Ok(Flow::Next);
                        }
                    },
                };
                let value = match op {
                    Op::Shl => f64::from(to_int32(a).wrapping_shl(to_uint32(b) & 31)),
                    Op::Shr => f64::from(to_int32(a) >> (to_uint32(b) & 31)),
                    Op::UShr => f64::from(to_uint32(a) >> (to_uint32(b) & 31)),
                    Op::BitAnd => f64::from(to_int32(a) & to_int32(b)),
                    Op::BitOr => f64::from(to_int32(a) | to_int32(b)),
                    _ => f64::from(to_int32(a) ^ to_int32(b)),
                };
                self.drop_n(2);
                #[cfg(feature = "conformance")]
                let value = tagged_smi_result(&left, &right, JsValue::Number(value));
                #[cfg(not(feature = "conformance"))]
                let value = JsValue::Number(value);
                self.push(value);
            }
            Op::Eq | Op::Ne => {
                let right = self.peek(0);
                let left = self.peek(1);
                let equal = self.loose_equal(&left, &right)?;
                self.drop_n(2);
                self.push(JsValue::Boolean(equal == (op == Op::Eq)));
            }
            Op::StrictEq | Op::StrictNe => {
                let right = self.pop();
                let left = self.pop();
                let equal = left.strict_equal(&right);
                self.push(JsValue::Boolean(equal == (op == Op::StrictEq)));
            }
            Op::Lt | Op::Gt | Op::Le | Op::Ge => {
                #[cfg(not(feature = "conformance"))]
                if let Some((a, b)) = self.number_pair() {
                    let result = match op {
                        Op::Lt => a < b,
                        Op::Gt => a > b,
                        Op::Le => a <= b,
                        _ => a >= b,
                    };
                    self.replace_pair(JsValue::Boolean(result));
                    return Ok(Flow::Next);
                }
                let right = self.peek(0);
                let left = self.peek(1);
                let result = match (&left, &right) {
                    (JsValue::Number(a), JsValue::Number(b)) => match op {
                        Op::Lt => a < b,
                        Op::Gt => a > b,
                        Op::Le => a <= b,
                        _ => a >= b,
                    },
                    _ => match op {
                        Op::Lt => self.less_than(&left, &right, true)? == Some(true),
                        Op::Gt => self.less_than(&right, &left, false)? == Some(true),
                        Op::Le => self.less_than(&right, &left, false)? == Some(false),
                        _ => self.less_than(&left, &right, true)? == Some(false),
                    },
                };
                self.drop_n(2);
                self.push(JsValue::Boolean(result));
            }
            Op::Not => {
                let value = self.pop();
                self.push(JsValue::Boolean(!value.truthy()));
            }
            Op::Neg | Op::BitNot | Op::Inc | Op::Dec | Op::ToNumeric
                if self.peek(0).number_value().is_none() =>
            {
                let value = self.peek(0);
                let numeric = self.to_numeric(&value)?;
                self.pop();
                match numeric {
                    value if value.is_bigint() && op != Op::ToNumeric => {
                        let result = self.bigint_unary(op, &value)?;
                        self.push(result);
                    }
                    other => {
                        // Numbers continue through the ordinary arm below on the next dispatch.
                        self.push(other);
                        if op != Op::ToNumeric {
                            return self.op(op);
                        }
                    }
                }
            }
            Op::Neg => {
                let value = self.peek(0);
                let number = self.to_number(&value)?;
                self.pop();
                #[cfg(feature = "conformance")]
                let result = tagged_smi_result(&value, &value, JsValue::Number(-number));
                #[cfg(not(feature = "conformance"))]
                let result = JsValue::Number(-number);
                self.push(result);
            }
            Op::ToNumber | Op::ToNumeric => {
                let value = self.peek(0);
                if value.number_value().is_none() {
                    let number = self.to_number(&value)?;
                    self.pop();
                    self.push(JsValue::Number(number));
                }
            }
            Op::BitNot => {
                let value = self.peek(0);
                let number = self.to_number(&value)?;
                self.pop();
                #[cfg(feature = "conformance")]
                let result = tagged_smi_result(
                    &value,
                    &value,
                    JsValue::Number(f64::from(!to_int32(number))),
                );
                #[cfg(not(feature = "conformance"))]
                let result = JsValue::Number(f64::from(!to_int32(number)));
                self.push(result);
            }
            Op::Typeof => {
                let value = self.pop();
                let kind = self.type_of(&value);
                self.push(JsValue::string(kind)?);
            }
            Op::Inc | Op::Dec => {
                let value = self.peek(0);
                let number = self.to_number(&value)?;
                self.pop();
                let result = if op == Op::Inc {
                    number + 1.0
                } else {
                    number - 1.0
                };
                #[cfg(feature = "conformance")]
                let result = tagged_smi_result(&value, &value, JsValue::Number(result));
                #[cfg(not(feature = "conformance"))]
                let result = JsValue::Number(result);
                self.push(result);
            }
            Op::ToStr => {
                let value = self.peek(0);
                if !matches!(value, JsValue::String(_)) {
                    let text = self.to_string(&value)?;
                    self.pop();
                    self.push(JsValue::String(text));
                }
            }
            Op::Jump(target) => self.frame().pc = target as usize,
            Op::JumpIfFalse(target) => {
                if !self.pop().truthy() {
                    self.frame().pc = target as usize;
                }
            }
            Op::JumpIfTrue(target) => {
                if self.pop().truthy() {
                    self.frame().pc = target as usize;
                }
            }
            Op::JumpIfFalseKeep(target) => {
                if self.peek(0).truthy() {
                    self.pop();
                } else {
                    self.frame().pc = target as usize;
                }
            }
            Op::JumpIfTrueKeep(target) => {
                if self.peek(0).truthy() {
                    self.frame().pc = target as usize;
                } else {
                    self.pop();
                }
            }
            Op::JumpIfNotNullishKeep(target) => {
                if self.peek(0).nullish() {
                    self.pop();
                } else {
                    self.frame().pc = target as usize;
                }
            }
            Op::JumpIfNullish(target) => {
                if self.peek(0).nullish() {
                    self.pop();
                    self.push(JsValue::Undefined);
                    self.frame().pc = target as usize;
                }
            }
            Op::JumpOut(target, handlers, envs) => self.jump_out(target, handlers, envs)?,
            Op::Throw => {
                let value = self.pop();
                return Err(Thrown::Value(value));
            }
            Op::Unsupported(feature) => {
                let feature = self.constant(feature).lossy();
                return Err(self.unsupported_error(&feature));
            }
            Op::PushTry(catch, finally, slot) => {
                let frame = self.frame();
                let handler = Handler {
                    catch,
                    finally,
                    slot,
                    stack: frame.stack.len() as u32,
                    env: frame.env,
                    envs: frame.envs,
                };
                frame.handlers.push(handler);
            }
            Op::PopTry => {
                self.frame().handlers.pop();
            }
            Op::Normal(slot) => self.frame().regs[slot as usize] = JsValue::Number(NORMAL),
            Op::EndFinally(slot) => return self.end_finally(slot),
            Op::ResourceCreate => {
                let stack = builtins::disposal_create(self)?;
                self.push(stack);
            }
            Op::ResourceAdd(slot, asynchronous) => {
                let stack = self.frame().regs[slot as usize].clone();
                let value = self
                    .frame()
                    .stack
                    .last()
                    .cloned()
                    .expect("resource initialization value");
                builtins::disposal_add(self, stack, value, asynchronous)?;
            }
            Op::ResourceDispose(slot, completion) => {
                let stack = self.frame().regs[slot as usize].clone();
                if self.frame().regs[completion as usize] == JsValue::Number(THROW) {
                    let error = self.frame().regs[completion as usize + 1].clone();
                    builtins::disposal_error(self, stack.clone(), error)?;
                }
                let result = builtins::disposal_dispose(self, stack, false)?;
                self.push(result);
            }
            Op::ResourceBegin(slot, completion) => {
                let stack = self.frame().regs[slot as usize].clone();
                if self.frame().regs[completion as usize] == JsValue::Number(THROW) {
                    let error = self.frame().regs[completion as usize + 1].clone();
                    builtins::disposal_error(self, stack.clone(), error)?;
                }
                builtins::disposal_begin(self, stack)?;
            }
            Op::ResourceStep(slot, awaited, done) => {
                let stack = self.frame().regs[slot as usize].clone();
                match builtins::disposal_step(self, stack)? {
                    Some(value) => self.frame().regs[awaited as usize] = value,
                    None => self.frame().pc = done as usize,
                }
            }
            Op::ResourceRejected(slot) => {
                let stack = self.frame().regs[slot as usize].clone();
                let error = self.pop();
                builtins::disposal_rejected(self, stack, error)?;
            }
            Op::Arg(index) => {
                let value = self
                    .frame()
                    .args
                    .get(index as usize)
                    .cloned()
                    .unwrap_or(JsValue::Undefined);
                self.push(value);
            }
            Op::RestArgs(index) => {
                let rest: Vec<JsValue> = self
                    .frame()
                    .args
                    .iter()
                    .skip(index as usize)
                    .cloned()
                    .collect();
                let id = self.new_array(rest)?;
                self.push(JsValue::Object(id));
            }
            Op::Closure(site) => {
                let env = self.frame().env;
                let id = self.make_closure(site, env, None)?;
                self.push(JsValue::Object(id));
            }
            Op::Class(site, has_super) => self.class_op(site, has_super)?,
            Op::GetIterator => {
                let value = self.peek(0);
                let roots = self.pins.len();
                let site = self.site_here();
                let iterator = match self.get_iterator_at(&value, site) {
                    Ok(iterator) => iterator,
                    Err(error) => {
                        if let Thrown::Value(value) = &error {
                            self.push(value.clone());
                        }
                        self.pins.truncate(roots);
                        return Err(error);
                    }
                };
                self.pop();
                self.push(JsValue::Object(iterator));
                self.pins.truncate(roots);
            }
            Op::IterNext(target) => {
                let iterator = self.peek(0);
                let JsValue::Object(id) = iterator else {
                    return Err(EngineError::invalid("Invalid V9 iterator.").into());
                };
                let next = self.iter_step(id)?;
                self.pop();
                match next {
                    Some(value) => self.push(value),
                    None => self.frame().pc = target as usize,
                }
            }
            Op::IterStep => {
                let iterator = self.peek(0);
                let JsValue::Object(id) = iterator else {
                    return Err(EngineError::invalid("Invalid V9 iterator.").into());
                };
                let next = self.iter_step(id)?;
                self.pop();
                self.push(next.unwrap_or(JsValue::Undefined));
            }
            Op::IterSkip => {
                let iterator = self.peek(0);
                let JsValue::Object(id) = iterator else {
                    return Err(EngineError::invalid("Invalid V9 iterator.").into());
                };
                self.iter_skip(id)?;
                self.pop();
                self.push(JsValue::Undefined);
            }
            Op::IterRest => {
                let iterator = self.peek(0);
                let JsValue::Object(id) = iterator else {
                    return Err(EngineError::invalid("Invalid V9 iterator.").into());
                };
                let array = self.new_array(Vec::new())?;
                self.push(JsValue::Object(array));
                let mut collected = 0usize;
                while let Some(value) = self.iter_step(id)? {
                    self.array_append(&JsValue::Object(array), value)?;
                    collected += 1;
                    self.spread_tick(collected)?;
                }
                self.drop_n(2);
                self.push(JsValue::Object(array));
            }
            Op::IterClose => {
                let iterator = self.peek(0);
                if let JsValue::Object(id) = iterator {
                    self.iter_close(id)?;
                }
                self.pop();
            }
            Op::IterCloseCheck(completion, next, done) => {
                let kind = self.frame().regs[completion as usize].clone();
                let target = self.frame().regs[completion as usize + 1].clone();
                if kind == JsValue::Number(NORMAL)
                    || kind == JsValue::Number(JUMP) && target == JsValue::Number(f64::from(next))
                {
                    self.frame().pc = done as usize;
                }
            }
            Op::IterCloseError(completion) => {
                let error = self.pop();
                if self.frame().regs[completion as usize] != JsValue::Number(THROW) {
                    return Err(Thrown::Value(error));
                }
            }
            Op::ForIn => {
                let value = self.peek(0);
                let iterator = self.for_in_iterator(&value)?;
                self.pop();
                self.push(JsValue::Object(iterator));
            }
            Op::ForInNext(target) => {
                let iterator = self.peek(0);
                let JsValue::Object(id) = iterator else {
                    return Err(EngineError::invalid("Invalid V9 iterator.").into());
                };
                let next = self.for_in_step(id)?;
                self.pop();
                match next {
                    Some(key) => self.push(key),
                    None => self.frame().pc = target as usize,
                }
            }
            Op::Yield => {
                let value = self.pop();
                return Ok(self.suspend(Exit::Yield(value)));
            }
            Op::YieldStar => return self.yield_star(),
            Op::Await => {
                let value = self.pop();
                if !self.frame().boundary || !self.frame().code.flags.asynchronous {
                    return Err(self.unsupported_error("await outside an async function"));
                }
                return Ok(self.suspend(Exit::Await(value)));
            }
            Op::Initial if !self.frame().code.flags.generator => {
                // A module body: instantiated, it waits for its graph to evaluate it.
                return Ok(self.suspend(Exit::Yield(JsValue::Undefined)));
            }
            Op::Import => {
                let options = self.pop();
                let specifier = self.pop();
                let site = self.frame().code.site;
                let promise = crate::v9::modules::dynamic_import(self, specifier, options, site)?;
                self.push(promise);
            }
            Op::Initial => {
                let frame = self.frames.pop().expect("generator frame");
                let callee = frame.callee.clone();
                let boundary = frame.boundary;
                let asynchronous = frame.code.flags.asynchronous;
                let saved = self.save_frame(frame);
                let generator = if asynchronous {
                    self.new_async_generator(saved, &callee)?
                } else {
                    self.new_generator(saved, &callee)?
                };
                return Ok(self.finish_frame(JsValue::Object(generator), boundary));
            }
            Op::GetAsyncIterator
            | Op::AsyncIterNext
            | Op::AsyncIterValue(_)
            | Op::AsyncIterClose(_)
            | Op::AsyncCloseCheck
            | Op::AsyncYieldStar
            | Op::AsyncYieldStarResult => return self.async_iteration_op(op),
            Op::Regex(pattern, flags) => {
                let pattern = self.constant(pattern);
                let flags = self.constant(flags).lossy();
                let id = self.new_regexp(pattern, &flags)?;
                self.push(JsValue::Object(id));
            }
            Op::TemplateObject(index) => {
                let id = self.cached_template(index)?;
                self.push(JsValue::Object(id));
            }
        }
        Ok(Flow::Next)
    }
    /** Read an environment slot. */
    fn env_slot(&self, env: u32, slot: u16) -> Res<JsValue> {
        self.heap
            .env(env)
            .and_then(|record| record.slots.get(slot as usize))
            .cloned()
            .ok_or_else(|| EngineError::invalid("Invalid V9 environment slot.").into())
    }
    /** The dead-zone ReferenceError for a name; `this` before `super()` has V8's own wording. */
    fn dead_zone(&mut self, name: u32) -> Thrown {
        let name = self.constant(name).lossy();
        if name == "this" {
            return self.reference_error(SUPER_NOT_CALLED);
        }
        self.reference_error(&format!("Cannot access '{name}' before initialization"))
    }
    /** Read a global name: lexical bindings first, then the global object. */
    pub fn global_get(&mut self, name: &JsString, typeof_only: bool) -> Res<JsValue> {
        let found = with_text(name, |text| {
            self.globals.get(text).map(|binding| binding.value.clone())
        });
        if let Some(value) = found {
            return value.ok_or_else(|| {
                let text = name.lossy();
                self.reference_error(&format!("Cannot access '{text}' before initialization"))
            });
        }
        let key = Key::Str(name.clone());
        // An own data property of the global object (var, function, builtins) answers in one
        // lookup; anything else takes the full [[HasProperty]] then [[Get]] path.
        if let Some(Prop {
            slot: Slot::Data(value),
            ..
        }) = self.own(self.global_object(), &key)?
        {
            return Ok(value);
        }
        if self.has_property(self.global_object(), &key)? {
            return self.get_from(
                self.global_object(),
                &key,
                &JsValue::Object(self.global_object()),
            );
        }
        let text = name.lossy();
        if let Some(element) = crate::v9::platform::named_element(self, &text)? {
            return Ok(element);
        }
        if typeof_only {
            return Ok(JsValue::Undefined);
        }
        Err(self.reference_error(&format!("{text} is not defined")))
    }
    /** Assign a global name. */
    pub fn global_set(&mut self, name: &JsString, value: JsValue) -> Res<()> {
        let found = with_text(name, |text| {
            self.globals
                .get(text)
                .map(|binding| (binding.value.is_none(), binding.kind == BindingKind::Const))
        });
        if let Some((uninitialized, constant)) = found {
            if uninitialized {
                let text = name.lossy();
                return Err(
                    self.reference_error(&format!("Cannot access '{text}' before initialization"))
                );
            }
            if constant {
                return Err(self.type_error("Assignment to constant variable."));
            }
            let units = value.string_units();
            with_text(name, |text| {
                self.globals.get_mut(text).expect("binding").value = Some(value);
            });
            self.heap.adjust(0, units as isize);
            return Ok(());
        }
        let key = Key::Str(name.clone());
        let strict = self.reference_strict();
        if strict {
            return self.set_global_context(key, value);
        }
        self.set(&JsValue::Object(self.global_object()), key, value, strict)
    }
    /** Start a call from an instruction; `consumed` stack values are dropped once it starts. */
    fn call_op(
        &mut self,
        function: JsValue,
        this: JsValue,
        args: Vec<JsValue>,
        consumed: usize,
        construct: Option<JsValue>,
    ) -> Res<Flow> {
        let constructing = construct.is_some();
        let invoked = match self.invoke(function.clone(), this, args, construct, false) {
            Err(_) if !self.callable_for(&function, constructing) => {
                Err(self.uncallable_error(&function, constructing))
            }
            other => other,
        };
        // A pushed frame sits above the caller; its operands are dropped from the caller.
        let depth = if matches!(invoked, Ok(Invoked::Frame)) {
            2
        } else {
            1
        };
        let caller = self.frames.len() - depth;
        let stack = &mut self.frames[caller].stack;
        let length = stack.len().saturating_sub(consumed);
        stack.truncate(length);
        if let Invoked::Value(value) = invoked? {
            self.push(value);
        }
        Ok(Flow::Next)
    }
    /** Whether a value can be called (or constructed) at all, ignoring the call's arguments. */
    fn callable_for(&mut self, function: &JsValue, constructing: bool) -> bool {
        if constructing {
            self.is_constructor(function)
        } else {
            self.is_callable(function)
        }
    }
    /** The TypeError for calling a non-function: the callee as the source wrote it, which the
     * compiler recorded for every call it compiled; a call it did not record is an intermediate
     * value to V8 too. */
    fn uncallable_error(&mut self, _function: &JsValue, constructing: bool) -> Thrown {
        let text = match self.site_here() {
            Some((_, text)) => text,
            None => "(intermediate value)".into(),
        };
        let noun = if constructing {
            "constructor"
        } else {
            "function"
        };
        self.type_error(&format!("{text} is not a {noun}"))
    }
    /** Return from the running frame, running finally blocks first. */
    fn do_return(&mut self, value: JsValue) -> Res<Flow> {
        let frame = self.frame();
        while let Some(handler) = frame.handlers.pop() {
            if handler.finally != 0 {
                frame.stack.truncate(handler.stack as usize);
                frame.env = handler.env;
                frame.envs = handler.envs;
                let slot = handler.slot as usize;
                frame.regs[slot] = JsValue::Number(RETURN);
                frame.regs[slot + 1] = value;
                frame.pc = handler.finally as usize;
                return Ok(Flow::Next);
            }
        }
        let frame = self.frames.pop().expect("returning frame");
        let value = if frame.construct {
            self.construct_result(&frame, value)?
        } else {
            value
        };
        let boundary = frame.boundary;
        self.recycle_frame(frame);
        Ok(self.finish_frame(value, boundary))
    }
    /** Keep a returned frame's vectors for the next call, emptied so no value outlives its frame. */
    fn recycle_frame(&mut self, frame: super::Frame) {
        if self.frame_pool.len() >= super::FRAME_POOL {
            return;
        }
        let (mut stack, mut regs) = (frame.stack, frame.regs);
        stack.clear();
        regs.clear();
        self.frame_pool.push((stack, regs));
    }
    /** The innermost active derived constructor frame whose scope chain holds `env`, the
    environment that owns the `this` slot super() just filled. The frame may be inside a block
    or a nested arrow when super() runs, so its current environment is walked up to `env`. */
    fn derived_frame_over(&self, env: super::heap::EnvId) -> Option<usize> {
        self.frames
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, frame)| {
                if !frame.construct || !frame.code.flags.derived {
                    return None;
                }
                let mut current = frame.env;
                for _ in 0..super::property::CHAIN_STEPS {
                    let found = current?;
                    if found == env {
                        return Some(index);
                    }
                    current = self.heap.env(found)?.parent;
                }
                None
            })
    }
    /** [[Construct]]'s result once the body has fully completed: an object is the result; a
    derived constructor returning any other non-undefined value is a TypeError; otherwise the
    result is `this`, which a derived constructor must have initialized by now. */
    fn construct_result(&mut self, frame: &super::Frame, value: JsValue) -> Res<JsValue> {
        if matches!(value, JsValue::Object(_)) {
            return Ok(value);
        }
        if frame.code.flags.derived && !matches!(value, JsValue::Undefined) {
            return Err(self.type_error("Derived constructors may only return object or undefined"));
        }
        if frame.this == JsValue::Hole {
            return Err(self.reference_error(SUPER_NOT_CALLED));
        }
        Ok(frame.this.clone())
    }
    /** Deliver a finished frame's value to its caller or to the run's caller. */
    fn finish_frame(&mut self, value: JsValue, boundary: bool) -> Flow {
        if boundary {
            return Flow::Exit(Exit::Return(value));
        }
        self.push(value);
        Flow::Next
    }
    /** Suspend the running frame for Yield or Await. */
    fn suspend(&mut self, exit: Exit) -> Flow {
        let frame = self.frames.pop().expect("suspending frame");
        let saved = self.save_frame(frame);
        self.suspended = Some(saved);
        Flow::Exit(exit)
    }
    /** Leave handlers and environments for a jump out of try blocks. */
    fn jump_out(&mut self, target: u32, handlers: u16, envs: u16) -> Res<()> {
        let frame = self.frame();
        while frame.handlers.len() > handlers as usize {
            let handler = frame.handlers.pop().expect("handler");
            if handler.finally != 0 {
                frame.stack.truncate(handler.stack as usize);
                frame.env = handler.env;
                frame.envs = handler.envs;
                let slot = handler.slot as usize;
                frame.regs[slot] = JsValue::Number(JUMP);
                frame.regs[slot + 1] = JsValue::Number(f64::from(target));
                frame.regs[slot + 2] =
                    JsValue::Number(f64::from(handlers) * DEPTH_SHIFT + f64::from(envs));
                frame.pc = handler.finally as usize;
                return Ok(());
            }
        }
        let env = frame.env;
        let pops = frame.envs.saturating_sub(envs);
        let mut env = env;
        for _ in 0..pops {
            env = env
                .and_then(|id| self.heap.env(id))
                .and_then(|record| record.parent);
        }
        let frame = self.frame();
        frame.env = env;
        frame.envs = envs;
        frame.pc = target as usize;
        Ok(())
    }
    /** Resume the completion a finally block saved. */
    fn end_finally(&mut self, slot: u16) -> Res<Flow> {
        let slot = slot as usize;
        let frame = self.frame();
        let kind = match frame.regs[slot] {
            JsValue::Number(kind) => kind,
            _ => NORMAL,
        };
        let value = frame.regs[slot + 1].clone();
        let aux = frame.regs[slot + 2].clone();
        if kind == THROW {
            return Err(Thrown::Value(value));
        }
        if kind == RETURN {
            return self.do_return(value);
        }
        if kind == JUMP {
            let (JsValue::Number(target), JsValue::Number(depths)) = (value, aux) else {
                return Err(EngineError::invalid("Invalid V9 completion.").into());
            };
            let handlers = (depths / DEPTH_SHIFT).trunc() as u16;
            let envs = (depths % DEPTH_SHIFT) as u16;
            self.jump_out(target as u32, handlers, envs)?;
        }
        Ok(Flow::Next)
    }
    /** `object.name`: the property-load inline cache. A hit is a shape compare and an indexed
    read; a miss on an ordinary object looks the key up once, caches an own data property under
    the object's shape, and walks on from the prototype when the key is absent. */
    fn load_named(&mut self, site: u32, base: &JsValue) -> Res<JsValue> {
        if let JsValue::Object(id) = base {
            self.guard_realm_global(*id)?;
            let key = self.constant_key(site);
            if let Some(value) = crate::v9::platform::frame_directory::lookup(self, *id, &key)? {
                return Ok(value);
            }
            #[cfg(feature = "conformance")]
            if crate::v9::builtins::is_shared_object(self, *id) {
                let key = self.constant_key(site);
                if key.hidden() {
                    self.private_brand(base, &key, false)?;
                }
                return self.get(base, &key);
            }
        }
        if let Some(value) = self.load_hit(site, base) {
            return Ok(value);
        }
        let key = self.constant_key(site);
        if key.hidden() {
            self.private_brand(base, &key, false)?;
            return self.get(base, &key);
        }
        let JsValue::Object(id) = base else {
            return self.get(base, &key);
        };
        let Some(object) = self.heap.get(*id).filter(|o| cacheable(&o.class)) else {
            return self.get(base, &key);
        };
        match object.props.position(&key) {
            Some(slot) => {
                if let Some(Slot::Data(value)) = object.props.slot(slot).map(|p| &p.slot) {
                    let value = value.clone();
                    let shape = object.props.shape_id();
                    self.frame().code.caches.record(site, shape, slot);
                    return Ok(value);
                }
            }
            None if object.function().is_none() => {
                return match object.proto {
                    Some(proto) => self.get_from(proto, &key, base),
                    None => Ok(JsValue::Undefined),
                };
            }
            None => {}
        }
        self.get(base, &key)
    }
    /** A cached own data property of an ordinary object, if the site knows its shape. */
    #[inline]
    fn load_hit(&self, site: u32, base: &JsValue) -> Option<JsValue> {
        let JsValue::Object(id) = base else {
            return None;
        };
        let object = self.heap.get(*id).filter(|o| cacheable(&o.class))?;
        let code = &self.frames.last()?.code;
        let slot = code.caches.lookup(site, object.props.shape_id())?;
        match &object.props.slot(slot)?.slot {
            Slot::Data(value) => Some(value.clone()),
            Slot::Accessor(..) => None,
        }
    }
    /** `object.name = value`: the property-store inline cache. It writes in place only an own
    writable data property of an ordinary object (attributes live in the slot, so a frozen or
    read-only property, a setter, or a new key always takes the full [[Set]]). */
    fn store_named(&mut self, site: u32, base: &JsValue, value: &JsValue) -> Res<()> {
        if let JsValue::Object(id) = base {
            self.guard_realm_global(*id)?;
        }
        if self.store_cached(site, base, value, None) {
            return Ok(());
        }
        let key = self.constant_key(site);
        if key.hidden() {
            self.private_brand(base, &key, true)?;
        } else if self.store_cached(site, base, value, Some(&key)) {
            return Ok(());
        }
        let strict = self.reference_strict();
        self.set(base, key, value.clone(), strict)
    }
    /** One store attempt: through the cache when `key` is None, else by looking the key up and
    caching the slot it finds. False means the full [[Set]] must run. */
    fn store_cached(
        &mut self,
        site: u32,
        base: &JsValue,
        value: &JsValue,
        key: Option<&Key>,
    ) -> bool {
        self.heap.track_value(value);
        let JsValue::Object(id) = base else {
            return false;
        };
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::is_shared_object(self, *id) {
            return false;
        }
        let Some(frame) = self.frames.last() else {
            return false;
        };
        let Some(object) = self
            .heap
            .objects
            .get_mut(id)
            .filter(|o| cacheable(&o.class))
        else {
            return false;
        };
        let shape = object.props.shape_id();
        let slot = match key {
            None => frame.code.caches.lookup(site, shape),
            Some(key) => object.props.position(key),
        };
        let Some(prop) = slot.and_then(|slot| object.props.slot_mut(slot)) else {
            return false;
        };
        let Slot::Data(old) = &mut prop.slot else {
            return false;
        };
        if prop.attrs & WRITABLE == 0 {
            return false;
        }
        let delta = value.string_units() as isize - old.string_units() as isize;
        *old = value.clone();
        if let (Some(slot), Some(_)) = (slot, key) {
            frame.code.caches.record(site, shape, slot);
        }
        self.heap.adjust(0, delta);
        true
    }
    /** Array element read without key conversion when possible. */
    fn fast_element(&self, object: &JsValue, key: &JsValue) -> Option<JsValue> {
        let (JsValue::Object(id), JsValue::Number(index)) = (object, key) else {
            return None;
        };
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::shared_container_active(self, *id) {
            return None;
        }
        if index.fract() != 0.0 || *index < 0.0 {
            return None;
        }
        match &self.heap.get(*id)?.class {
            Class::Array(elements) => match elements.get(*index as usize) {
                Some(JsValue::Hole) | None => None,
                Some(value) => Some(value.clone()),
            },
            Class::View(_) => crate::v9::builtins::typed_load(self, *id, *index as usize),
            _ => None,
        }
    }
    /** Array element write without key conversion when the slot exists. */
    fn fast_store(&mut self, object: &JsValue, key: &JsValue, value: &JsValue) -> bool {
        let (JsValue::Object(id), JsValue::Number(index)) = (object, key) else {
            return false;
        };
        if index.fract() != 0.0 || *index < 0.0 {
            return false;
        }
        let index = *index as usize;
        // Creating an element must consult inherited setters and readonly data properties.
        let fits = matches!(self.heap.get(*id).map(|o| (&o.class, o.extensible && o.props.is_empty())), Some((Class::Array(elements), true)) if !elements.is_repeated() && index < u32::MAX as usize && elements.get(index).is_some_and(|value| *value != JsValue::Hole));
        if fits {
            self.array_store(*id, index, value.clone());
        }
        fits
    }
    /** Retain a one-byte native string iterator as bounded canonical character storage. */
    #[cfg(feature = "conformance")]
    fn compact_character_spread(&mut self, array: &JsValue, iterator: u32) -> Res<bool> {
        use super::data::IterState;
        if !self.page.environment.conformance_d8 {
            return Ok(false);
        }
        let Some(id) = array.object() else {
            return Ok(false);
        };
        if !matches!(self.heap.get(id).map(|object| &object.class), Some(Class::Array(elements)) if elements.is_empty())
        {
            return Ok(false);
        }
        let text = match self.heap.get(iterator).map(|object| &object.class) {
            Some(Class::Iter(state)) => match state.as_ref() {
                IterState::String {
                    text,
                    index: 0,
                    done: false,
                } if text.is_latin1() && text.len() >= MIN_COMPACT_STRING_LENGTH => text.clone(),
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if text.len() > u32::MAX as usize
            || self.heap.counts.properties.saturating_add(258) > self.limits.properties
            || self
                .heap
                .counts
                .units
                .saturating_add(text.len())
                .saturating_add(256)
                > self.limits.units
        {
            return Ok(false);
        }
        self.charge(text.len())?;
        let mut roots = Vec::with_capacity(256);
        for unit in 0..=u8::MAX {
            roots.push(JsValue::String(self.character_root(unit)?));
        }
        let Some(elements) = super::array_elements::ArrayElements::character_copy(
            JsValue::String(text.clone()),
            roots,
        ) else {
            return Ok(false);
        };
        let slots = elements.resident_len();
        if slots > self.limits.array_length {
            return Ok(false);
        }
        let units = elements
            .resident_values()
            .map(JsValue::string_units)
            .sum::<usize>();
        for value in elements.resident_values() {
            self.heap.track_value(value);
        }
        let Some(Class::Array(target)) = self.heap.get_mut(id).map(|object| &mut object.class)
        else {
            return Ok(false);
        };
        *target = elements;
        self.heap.adjust(slots as isize, units as isize);
        if let Some(Class::Iter(state)) =
            self.heap.get_mut(iterator).map(|object| &mut object.class)
            && let IterState::String { index, done, .. } = state.as_mut()
        {
            *index = text.len() as u32;
            *done = true;
        }
        Ok(true)
    }
    /** Append to an array literal under construction. */
    pub fn array_append(&mut self, array: &JsValue, value: JsValue) -> Res<()> {
        let JsValue::Object(id) = array else {
            return Ok(());
        };
        let (length, resident) = match self.heap.get(*id).map(|object| &object.class) {
            Some(Class::Array(elements)) => (elements.len(), elements.resident_len()),
            _ => return Ok(()),
        };
        if length >= u32::MAX as usize {
            return Err(self.range_error("Invalid array length"));
        }
        if value != JsValue::Hole && resident >= self.limits.array_length {
            return Err(
                EngineError::limit("V9 resident array elements", self.limits.array_length).into(),
            );
        }
        self.array_store(*id, length, value);
        Ok(())
    }
    /** The elements of an array (holes as undefined), for spread calls. */
    pub fn array_values(&mut self, array: &JsValue) -> Res<Vec<JsValue>> {
        #[cfg(feature = "conformance")]
        if let Some(id) = array
            .object()
            .filter(|id| crate::v9::builtins::shared_container_active(self, *id))
        {
            let length = match self.heap.get(id).map(|object| &object.class) {
                Some(Class::Array(values)) => values.len(),
                _ => 0,
            };
            if length > self.limits.array_length {
                return Err(self.range_error("Maximum call stack size exceeded"));
            }
            return (0..length)
                .map(|index| self.get(array, &Key::from(index.to_string().as_str())))
                .collect();
        }
        match array
            .object()
            .and_then(|id| self.heap.get(id))
            .map(|object| &object.class)
        {
            Some(Class::Array(elements)) if elements.len() > self.limits.array_length => {
                Err(self.range_error("Maximum call stack size exceeded"))
            }
            Some(Class::Array(elements)) => Ok(elements
                .iter()
                .map(|value| {
                    if *value == JsValue::Hole {
                        JsValue::Undefined
                    } else {
                        value.clone()
                    }
                })
                .collect()),
            _ => Err(EngineError::invalid("Spread arguments were not an array.").into()),
        }
    }
    /** CreateDataProperty on an object literal or class field. */
    fn create_data(&mut self, object: &JsValue, key: Key, value: JsValue) -> Res<()> {
        let JsValue::Object(id) = object else {
            return Ok(());
        };
        if key.hidden() {
            return self.add_private_field(*id, key, value);
        }
        if !self.define(*id, key.clone(), Descriptor::data(value))? {
            return Err(self.refused_define(*id, &key));
        }
        Ok(())
    }
    /** The TypeError for destructuring a nullish value. The pattern's first key is named when it
     * has one, and the value as the source wrote it when the compiler kept that text, otherwise
     * by its type, as V8 does. */
    fn destructure_error(&mut self, source: &JsValue, name: u32) -> Thrown {
        let word = if matches!(source, JsValue::Null) {
            "null"
        } else {
            "undefined"
        };
        let text = match self.site_here() {
            Some((_, text)) => text,
            None => self.v8_callsite_text(source),
        };
        let key = self
            .frame()
            .code
            .strings
            .get(name as usize)
            .map(|key| key.lossy());
        let message = match key {
            Some(key) => {
                format!("Cannot destructure property '{key}' of '{text}' as it is {word}.")
            }
            None => format!("Cannot destructure '{text}' as it is {word}."),
        };
        self.type_error(&message)
    }
    /** The written form the compiler recorded for the instruction being executed. */
    fn site_here(&mut self) -> Option<(SiteKind, String)> {
        let pc = self.frame().pc.saturating_sub(1);
        self.frame()
            .code
            .callee_at(pc)
            .map(|(kind, text)| (kind, text.to_owned()))
    }
    /** `in` with a non-object on its right names both sides as V8 does. */
    fn in_operator_error(&mut self, key: &str, object: &JsValue) -> Thrown {
        let object = self.v8_value_text(object);
        self.type_error(&format!(
            "Cannot use 'in' operator to search for '{key}' in {object}"
        ))
    }
    /** Define a method, getter or setter and give the function its home object. */
    fn define_method(
        &mut self,
        object: &JsValue,
        key: Key,
        function: JsValue,
        kind: u8,
        enumerable: bool,
    ) -> Res<()> {
        let (JsValue::Object(id), JsValue::Object(function_id)) = (object, &function) else {
            return Ok(());
        };
        let name = builtins::inferred_function_name(self, &key)?;
        let prefix = match kind {
            1 => "get ",
            2 => "set ",
            _ => "",
        };
        let prefix = crate::v9::value::JsString::from_utf8(prefix)?;
        let name = self.concat_strings(&prefix, &name)?;
        self.set_name_snapshot(
            *function_id,
            super::data::FunctionName::flat(JsValue::String(name)),
        )?;
        if let Some(Class::Function(data)) = self.heap.get_mut(*function_id).map(|o| &mut o.class) {
            data.home = Some(*id);
        }
        let desc = match kind {
            0 => Descriptor {
                value: Some(function),
                writable: Some(true),
                enumerable: Some(enumerable),
                configurable: Some(true),
                ..Descriptor::default()
            },
            1 => Descriptor {
                get: Some(Some(*function_id)),
                enumerable: Some(enumerable),
                configurable: Some(true),
                ..Descriptor::default()
            },
            _ => Descriptor {
                set: Some(Some(*function_id)),
                enumerable: Some(enumerable),
                configurable: Some(true),
                ..Descriptor::default()
            },
        };
        if key.hidden() {
            self.define_private_method(*id, key, desc)?;
        } else if !self.define(*id, key.clone(), desc)? {
            return Err(self.type_error(&format!(
                "Cannot redefine property: {}",
                super::property::key_text(&key)
            )));
        }
        Ok(())
    }
    /** Auto-accessor functions share a captured private backing key and retain ordinary accessor descriptors. */
    fn define_auto_accessor(&mut self, home: &JsValue, public: Key, backing: Key) -> Res<()> {
        let roots = self.pins.len();
        let result = (|| {
            let (getter, setter) = builtins::create_auto_accessors(self, &public, backing)?;
            self.define_method(home, public.clone(), JsValue::Object(getter), 1, false)?;
            self.define_method(home, public, JsValue::Object(setter), 2, false)
        })();
        self.pins.truncate(roots);
        result
    }
    /** Copy enumerable own properties (object spread, rest). */
    pub fn copy_data(&mut self, target: &JsValue, source: &JsValue, excluded: &[Key]) -> Res<()> {
        let JsValue::Object(target) = target else {
            return Ok(());
        };
        let source_id = match source {
            JsValue::Object(id) => *id,
            JsValue::String(_) => self.to_object(source)?,
            _ => return Ok(()),
        };
        for key in self.own_keys(source_id)? {
            if excluded.contains(&key) {
                continue;
            }
            let Some(prop) = self.own(source_id, &key)? else {
                continue;
            };
            if prop.attrs & ENUMERABLE == 0 {
                continue;
            }
            let value = match prop.slot {
                Slot::Data(value) => value,
                Slot::Accessor(..) => self.get(&JsValue::Object(source_id), &key)?,
            };
            self.define(*target, key, Descriptor::data(value))?;
        }
        Ok(())
    }
    /** `delete` on any value. */
    fn delete_value(&mut self, object: &JsValue, key: &Key) -> Res<bool> {
        let id = self.to_object(object)?;
        let deleted = self.delete(id, key)?;
        if !deleted && self.reference_strict() {
            let holder = self.v8_value_text(&JsValue::Object(id));
            let name = self.key_display(key);
            return Err(self.type_error(&format!("Cannot delete property '{name}' of {holder}")));
        }
        Ok(deleted)
    }
    /** GetValue and PutValue reject null bases before attempting authored key conversion. */
    fn check_element_base(&mut self, base: &JsValue, key: &JsValue, setting: bool) -> Res<()> {
        if !matches!(base, JsValue::Null | JsValue::Undefined | JsValue::Hole) {
            return Ok(());
        }
        if !setting && matches!(key, JsValue::Symbol(id) if *id == super::realm::symbol::ITERATOR) {
            return Err(self.nullish_read_error(base, &Key::Sym(super::realm::symbol::ITERATOR)));
        }
        let what = if *base == JsValue::Null {
            "null"
        } else {
            "undefined"
        };
        let verb = if setting { "set" } else { "read" };
        // V8 names every key, a symbol as `Symbol(description)`; an object key would need
        // conversion that can run script, so only primitives are named here.
        let name = match key {
            JsValue::String(key) => Some(key.lossy()),
            JsValue::Symbol(id) => Some(self.key_display(&Key::Sym(*id))),
            JsValue::Object(_) => None,
            other => self.to_string(other).ok().map(|text| text.lossy()),
        };
        let detail = match name {
            Some(name) => {
                let operation = if setting { "setting" } else { "reading" };
                format!(" ({operation} '{name}')")
            }
            None => String::new(),
        };
        Err(self.type_error(&format!("Cannot {verb} properties of {what}{detail}")))
    }
    /** `super.key` reads from the home object's prototype with `this` as receiver. */
    fn super_get(&mut self, this: &JsValue, home: &JsValue, key: &Key) -> Res<JsValue> {
        let Some(proto) = home.object().and_then(|id| self.proto_of(id)) else {
            return Err(self.type_error("Cannot read properties of null"));
        };
        self.get_from(proto, key, this)
    }
    /** `super.key = value`. */
    fn super_set(&mut self, this: &JsValue, home: &JsValue, key: Key, value: JsValue) -> Res<()> {
        let Some(proto) = home.object().and_then(|id| self.proto_of(id)) else {
            return Err(self.type_error("Cannot set properties of null"));
        };
        if !self.set_from(proto, key.clone(), value, this)? && self.reference_strict() {
            return Err(self.set_failure(this, &key));
        }
        Ok(())
    }
    /** `super(...)`: construct the parent with the derived constructor's new.target. */
    fn super_call(&mut self, args: Vec<JsValue>) -> Res<JsValue> {
        let Some(index) = self
            .frames
            .iter()
            .rposition(|frame| frame.code.flags.derived && frame.construct)
        else {
            return Err(self.syntax_error("'super' keyword unexpected here"));
        };
        let callee = self.frames[index].callee.clone();
        let new_target = self.frames[index].new_target.clone();
        let parent = callee.object().and_then(|id| self.proto_of(id));
        let Some(parent) = parent.filter(|parent| self.is_constructor(&JsValue::Object(*parent)))
        else {
            return Err(self.super_constructor_error(&callee, parent));
        };
        let this = self.construct(JsValue::Object(parent), args, Some(new_target))?;
        // BindThisValue checks initialization after the parent constructor's observable work.
        if self.frames[index].this != JsValue::Hole {
            return Err(self.reference_error("Super constructor may only be called once"));
        }
        self.frames[index].this = this.clone();
        Ok(this)
    }
    /** A lexical arrow retains its constructor and new.target even when the constructor has returned. */
    fn construct_lexical_super(
        &mut self,
        callee: JsValue,
        target: JsValue,
        args: Vec<JsValue>,
    ) -> Res<JsValue> {
        let parent = callee.object().and_then(|id| self.proto_of(id));
        let Some(parent) = parent.filter(|parent| self.is_constructor(&JsValue::Object(*parent)))
        else {
            return Err(self.super_constructor_error(&callee, parent));
        };
        self.construct(JsValue::Object(parent), args, Some(target))
    }
    /** `Super constructor null of A is not a constructor`: the parent as V8 prints it and the
     * derived class by its name, or `anonymous class` when it has none. */
    fn super_constructor_error(&mut self, callee: &JsValue, parent: Option<ObjectId>) -> Thrown {
        let parent = parent.map_or(JsValue::Null, JsValue::Object);
        // A function parent is named, not printed as source; no name reads as `null`.
        let parent = match parent {
            JsValue::Object(id) if self.is_callable(&parent) => {
                match self
                    .own(id, &Key::from("name"))
                    .ok()
                    .flatten()
                    .map(|prop| prop.slot)
                {
                    Some(Slot::Data(JsValue::String(name))) if !name.is_empty() => name.lossy(),
                    _ => "null".to_string(),
                }
            }
            other => self.v8_value_text(&other),
        };
        let name = self.code_name_of(callee);
        let class = if name.is_empty() {
            "anonymous class".to_string()
        } else {
            name
        };
        self.type_error(&format!(
            "Super constructor {parent} of {class} is not a constructor"
        ))
    }
    /** The name a closure was compiled with, read without touching its properties. */
    fn code_name_of(&self, function: &JsValue) -> String {
        function
            .object()
            .and_then(|id| self.heap.get(id))
            .and_then(|object| object.function())
            .and_then(|function| match function.kind {
                FunctionKind::Closure { site, .. } => self.registry.get(site),
                _ => None,
            })
            .map(|code| code.name.lossy())
            .unwrap_or_default()
    }
    /** Build a class constructor and prototype from the stack. */
    fn class_op(&mut self, site: u32, has_super: bool) -> Res<()> {
        let fields = self.pop();
        let superclass = if has_super { Some(self.pop()) } else { None };
        let ids = &realm().ids;
        let (proto_parent, constructor_parent) = match superclass {
            None => (Some(ids.object_prototype), ids.function_prototype),
            Some(JsValue::Null) => (None, ids.function_prototype),
            Some(parent) => {
                if !self.is_constructor(&parent) {
                    let text = self.v8_value_text(&parent);
                    return Err(self.type_error(&format!(
                        "Class extends value {text} is not a constructor or null"
                    )));
                }
                let proto = self.get(&parent, &Key::prototype())?;
                let proto = match proto {
                    JsValue::Object(id) => Some(id),
                    JsValue::Null => None,
                    other => {
                        let text = self.v8_value_text(&other);
                        return Err(self.type_error(&format!(
                            "Class extends value does not have valid prototype property {text}"
                        )));
                    }
                };
                (proto, parent.object().expect("constructor object"))
            }
        };
        let prototype = self.alloc(Object::new(Class::Plain, proto_parent))?;
        self.push(JsValue::Object(prototype));
        let env = self.frame().env;
        let constructor = self.make_closure(site, env, Some(prototype))?;
        if let Some(object) = self.heap.get_mut(constructor) {
            object.proto = Some(constructor_parent);
            if let Class::Function(function) = &mut object.class {
                let Function { fields: slot, .. } = function.as_mut();
                *slot = fields.object();
            }
        }
        if let JsValue::Object(fields) = fields
            && let Some(Class::Function(function)) = self.heap.get_mut(fields).map(|o| &mut o.class)
        {
            function.home = Some(prototype);
            let _ = &function.kind as &FunctionKind;
        }
        self.heap.put(
            constructor,
            Key::prototype(),
            Prop {
                slot: Slot::Data(JsValue::Object(prototype)),
                attrs: 0,
            },
        );
        self.heap.put(
            prototype,
            Key::constructor(),
            Prop {
                slot: Slot::Data(JsValue::Object(constructor)),
                attrs: WRITABLE | CONFIGURABLE,
            },
        );
        self.pop();
        self.push(JsValue::Object(constructor));
        self.push(JsValue::Object(prototype));
        Ok(())
    }
    /** A delegate's noncompleted result remains the exact object returned by next, throw or return. */
    fn yield_star(&mut self) -> Res<Flow> {
        let mode = std::mem::take(&mut self.frame().resume);
        let received = self.pop();
        let iterator = self
            .peek(0)
            .object()
            .ok_or_else(|| EngineError::invalid("Invalid V9 delegation."))?;
        let value = match self.delegate_step(iterator, mode, received)? {
            Exit::YieldRaw(result) => {
                self.frame().pc -= 1;
                return Ok(self.suspend(Exit::YieldRaw(result)));
            }
            Exit::Return(value) => value,
            _ => return Err(EngineError::invalid("Invalid V9 delegate completion.").into()),
        };
        self.pop();
        if mode == Resume::Return {
            return self.do_return(value);
        }
        self.push(value);
        Ok(Flow::Next)
    }
}

/** ECMAScript `%`: the sign follows the dividend. */
pub(in crate::v9) fn js_remainder(a: f64, b: f64) -> f64 {
    if b.is_infinite() && a.is_finite() {
        return a;
    }
    a % b
}

/** ECMAScript `**`: 1 ** NaN is NaN, unlike powf. */
pub(in crate::v9) fn js_power(a: f64, b: f64) -> f64 {
    if b.is_nan() || (a.abs() == 1.0 && b.is_infinite()) {
        return f64::NAN;
    }
    a.powf(b)
}

impl Vm<'_> {
    /** Annex B eval creates a deletable global var only if that binding can actually be installed. */
    fn declare_annex_global(&mut self, name: crate::v9::value::JsString) -> Res<()> {
        let global = self.global_object();
        let key = Key::Str(name);
        if self.own(global, &key)?.is_some() {
            return Ok(());
        }
        let descriptor = Descriptor {
            value: Some(JsValue::Undefined),
            writable: Some(true),
            enumerable: Some(true),
            configurable: Some(true),
            ..Descriptor::default()
        };
        if !self.define(global, key, descriptor)? {
            return Err(
                self.type_error("Cannot declare a global variable on a non-extensible object")
            );
        }
        Ok(())
    }
    /// A var or function declaration of a name an earlier script declared with let, const or class
    /// is a SyntaxError, as for two declarations in one script. A top-level lexical name that
    /// collides with an existing global binding or a non-configurable global property (spec
    /// HasLexicalDeclaration and HasRestrictedGlobalProperty) is a SyntaxError too.
    fn check_global_lexical(&mut self, name: &crate::v9::value::JsString) -> Res<()> {
        let text = name.lossy();
        let shadowed = self
            .heap
            .get(self.global_object())
            .and_then(|object| object.props.get(&Key::Str(name.clone())))
            .is_some_and(|prop| prop.attrs & CONFIGURABLE == 0);
        // Non-configurable globals (undefined, NaN, Infinity) and the unforgeable
        // window, document and location cannot be shadowed by a global lexical either.
        let restricted = matches!(
            text.as_str(),
            "undefined" | "NaN" | "Infinity" | "window" | "document" | "location"
        );
        if self.globals.contains_key(&text) || shadowed || restricted {
            return Err(
                self.syntax_error(&format!("Identifier '{text}' has already been declared"))
            );
        }
        Ok(())
    }
    fn no_global_lexical(&mut self, name: &crate::v9::value::JsString) -> Res<()> {
        let text = name.lossy();
        if self.globals.contains_key(&text) {
            return Err(
                self.syntax_error(&format!("Identifier '{text}' has already been declared"))
            );
        }
        Ok(())
    }
}

/** Names at most this long convert to text on the stack. */
const SHORT_NAME: usize = 64;

/** Run `body` on a name's text. Short ASCII names, nearly every identifier, convert on the stack,
so a global read or write allocates nothing; any other name takes the lossy conversion the
global map's keys were made with. */
fn with_text<R>(name: &JsString, body: impl FnOnce(&str) -> R) -> R {
    let mut bytes = [0u8; SHORT_NAME];
    if name.len() <= SHORT_NAME && name.code_units().all(|unit| unit < 0x80) {
        for (byte, unit) in bytes.iter_mut().zip(name.code_units()) {
            *byte = unit as u8;
        }
        if let Ok(text) = std::str::from_utf8(&bytes[..name.len()]) {
            return body(text);
        }
    }
    body(&name.lossy())
}

/** Whether an object's named keys live only in its property list, so an inline cache may
answer for it: proxies, host objects and namespaces have their own lookups, and arrays, strings
and typed views answer index and `length` keys themselves. */
fn cacheable(class: &Class) -> bool {
    !matches!(
        class,
        Class::Proxy(_)
            | Class::Host(_)
            | Class::Namespace(_)
            | Class::Array(_)
            | Class::String(_)
            | Class::View(_)
            | Class::MappedArguments(_)
    )
}

/** Integer fast paths preserve actual tagged-small-integer provenance. */
#[cfg(feature = "conformance")]
fn tagged_smi_result(left: &JsValue, right: &JsValue, value: JsValue) -> JsValue {
    if matches!((left, right), (JsValue::Smi(_), JsValue::Smi(_)))
        && let JsValue::Number(number) = value
    {
        if number.is_finite()
            && number.fract() == 0.0
            && number >= -(1_i64 << 30) as f64
            && number < (1_i64 << 30) as f64
            && !(number == 0.0 && number.is_sign_negative())
        {
            return JsValue::Smi(number);
        }
        return JsValue::Number(number);
    }
    value
}
