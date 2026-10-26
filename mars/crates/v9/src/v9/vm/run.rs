/** The instruction loop: one bytecode op at a time, with explicit unwinding and suspension. */
/** Activations a throw trace keeps; the innermost ones say where an error came from. */
const THROW_TRACE_FRAMES: usize = 8;
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
use crate::v9::compile::Op;
use crate::v9::value::{JsString, JsValue, ObjectId, to_int32, to_uint32};

mod asyncgen;
mod eval;
mod with;

/** Completion kinds a finally block's registers hold. */
const NORMAL: f64 = 0.0;
const THROW: f64 = 1.0;
const RETURN: f64 = 2.0;
const JUMP: f64 = 3.0;
/** Packs handler and environment depths into one completion register. */
const DEPTH_SHIFT: f64 = 65_536.0;

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
    /** Execute instructions until an exit or an exception. */
    fn execute(&mut self) -> Res<Exit> {
        loop {
            self.select_frame_realm()?;
            self.remaining = match self.remaining.checked_sub(1) {
                Some(remaining) => remaining,
                None => return Err(self.charge(1).expect_err("budget exhausted")),
            };
            if self.remaining.is_multiple_of(SCRIPT_CLOCK_STRIDE) {
                if self.expired() {
                    return Err(Thrown::Fatal(super::super::platform::timeout()));
                }
                self.collect_if_needed()?;
            }
            self.collect_if_due()?;
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
    fn frame(&mut self) -> &mut super::Frame {
        self.frames.last_mut().expect("running frame")
    }
    /** Push onto the operand stack. */
    pub(in crate::v9) fn push(&mut self, value: JsValue) {
        self.frame().stack.push(value);
    }
    /** Pop the operand stack. */
    pub(in crate::v9) fn pop(&mut self) -> JsValue {
        self.frame().stack.pop().unwrap_or(JsValue::Undefined)
    }
    /** Read the value `depth` below the top. */
    fn peek(&mut self, depth: usize) -> JsValue {
        let stack = &self.frame().stack;
        stack
            .get(stack.len().wrapping_sub(depth + 1))
            .cloned()
            .unwrap_or(JsValue::Undefined)
    }
    /** Drop `count` values. */
    fn drop_n(&mut self, count: usize) {
        let stack = &mut self.frame().stack;
        let length = stack.len().saturating_sub(count);
        stack.truncate(length);
    }
    /** A string constant of the running code. */
    fn constant(&mut self, index: u32) -> JsString {
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
            Op::Number(value) => self.push(JsValue::Number(value)),
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
            Op::DeclareVar(name, deletable) => {
                let name = self.constant(name);
                self.no_global_lexical(&name)?;
                let key = Key::Str(name);
                // Window's intrinsic properties are represented on its immediate prototype.
                // Ordinary inherited properties do not satisfy CreateGlobalVarBinding.
                let window = realm().ids.window_prototype;
                let platform = self.proto_of(self.global_object()) == Some(window)
                    && realm()
                        .object(window)
                        .is_some_and(|definition| definition.get(&key).is_some())
                    && self.own(window, &key)?.is_some();
                let inherited = self.own(self.global_object(), &key)?.is_some() || platform;
                if !inherited {
                    self.define(
                        self.global_object(),
                        key,
                        Descriptor {
                            value: Some(JsValue::Undefined),
                            writable: Some(true),
                            enumerable: Some(true),
                            configurable: Some(deletable),
                            ..Descriptor::default()
                        },
                    )?;
                }
            }
            Op::DeclareFunction(name, deletable) => {
                let name = self.constant(name);
                self.no_global_lexical(&name)?;
                if matches!(name.lossy().as_str(), "undefined" | "NaN" | "Infinity") {
                    let message = "Identifier has already been declared";
                    return Err(if deletable {
                        self.type_error(message)
                    } else {
                        self.syntax_error(message)
                    });
                }
                let value = self.pop();
                let key = Key::Str(name);
                if !self.define(
                    self.global_object(),
                    key.clone(),
                    Descriptor {
                        value: Some(value.clone()),
                        writable: Some(true),
                        enumerable: Some(true),
                        configurable: Some(deletable),
                        ..Descriptor::default()
                    },
                )? {
                    self.write_own(self.global_object(), key, value)?;
                }
            }
            Op::DirectEval(count, context) => return self.direct_eval(count, context),
            Op::GetExt(name, hops, slot) => self.get_ext(name, hops, slot)?,
            Op::SetExt(name, hops, slot) => self.set_ext(name, hops, slot)?,
            Op::DeclareExt(name, hops, slot) => self.declare_ext(name, hops, slot)?,
            Op::DeleteExt(name, hops, slot) => self.delete_ext(name, hops, slot)?,
            Op::GetWith(name, hops, slot, call) => self.get_with(name, hops, slot, call)?,
            Op::SetWith(name, hops, slot) => self.set_with(name, hops, slot)?,
            Op::DeleteWith(name, hops, slot) => self.delete_with(name, hops, slot)?,
            Op::ToObject => {
                let value = self.pop();
                let id = self.to_object(&value)?;
                self.push(JsValue::Object(id));
            }
            Op::ThrowSyntax(message) => {
                let message = self.constant(message).lossy();
                return Err(self.syntax_error(&message));
            }
            Op::DeclareLexical(name, constant) => {
                let name = self.constant(name);
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
                    return Err(self
                        .syntax_error(&format!("Identifier '{text}' has already been declared")));
                }
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
                let iterator = self.get_iterator(&iterable)?;
                self.push(JsValue::Object(iterator));
                while let Some(value) = self.iter_step(iterator)? {
                    self.array_append(&array, value)?;
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
            Op::CopyRest(count) => {
                let stack = &mut self.frame().stack;
                let start = stack.len().saturating_sub(count as usize);
                let keys: Vec<JsValue> = stack.drain(start..).collect();
                let source = self.peek(0);
                if source.nullish() {
                    return Err(self.type_error("Cannot destructure 'undefined' or 'null'."));
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
                if !self.fast_store(&object, &key, &value) {
                    let key = self.to_key(&key)?;
                    let strict = self.frame().code.strict;
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
                let key = self.to_key(&key)?;
                let result = self.delete_value(&object, &key)?;
                self.drop_n(2);
                self.push(JsValue::Boolean(result));
            }
            Op::In => {
                let object = self.peek(0);
                let key = self.peek(1);
                let JsValue::Object(id) = object else {
                    return Err(self.type_error(
                        "Cannot use 'in' operator to search for a key in a non-object",
                    ));
                };
                let key = self.to_key(&key)?;
                let result = self.has_property(id, &key)?;
                self.drop_n(2);
                self.push(JsValue::Boolean(result));
            }
            Op::PrivateIn(key) => {
                let key = self.constant_key(key);
                let JsValue::Object(id) = self.peek(0) else {
                    return Err(self.type_error(
                        "Cannot use 'in' operator to search for a key in a non-object",
                    ));
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
                let key = self.to_key(&key)?;
                let value = self.super_get(&this, &home, &key)?;
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
            Op::Return => {
                let value = self.pop();
                return self.do_return(value);
            }
            Op::ReturnDerived => {
                let this = self.pop();
                let value = self.pop();
                let result = match value {
                    JsValue::Object(_) => value,
                    JsValue::Undefined => {
                        if this == JsValue::Hole {
                            return Err(self.reference_error(
                                "Must call super constructor in derived class before accessing 'this' or returning from derived constructor",
                            ));
                        }
                        this
                    }
                    _ => {
                        return Err(self.type_error(
                            "Derived constructors may only return object or undefined",
                        ));
                    }
                };
                self.frame().construct = false;
                return self.do_return(result);
            }
            Op::Add => {
                let right = self.peek(0);
                let left = self.peek(1);
                let value = self.add(&left, &right)?;
                self.drop_n(2);
                self.push(value);
            }
            Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Exp => {
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
                self.push(JsValue::Number(value));
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
                self.push(JsValue::Number(value));
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
                if !matches!(self.peek(0), JsValue::Number(_)) =>
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
                self.push(JsValue::Number(-number));
            }
            Op::ToNumber | Op::ToNumeric => {
                let value = self.peek(0);
                if !matches!(value, JsValue::Number(_)) {
                    let number = self.to_number(&value)?;
                    self.pop();
                    self.push(JsValue::Number(number));
                }
            }
            Op::BitNot => {
                let value = self.peek(0);
                let number = self.to_number(&value)?;
                self.pop();
                self.push(JsValue::Number(f64::from(!to_int32(number))));
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
                self.push(JsValue::Number(if op == Op::Inc {
                    number + 1.0
                } else {
                    number - 1.0
                }));
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
                let iterator = self.get_iterator(&value)?;
                self.pop();
                self.push(JsValue::Object(iterator));
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
            Op::IterRest => {
                let iterator = self.peek(0);
                let JsValue::Object(id) = iterator else {
                    return Err(EngineError::invalid("Invalid V9 iterator.").into());
                };
                let array = self.new_array(Vec::new())?;
                self.push(JsValue::Object(array));
                while let Some(value) = self.iter_step(id)? {
                    self.array_append(&JsValue::Object(array), value)?;
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
                let chunks = self
                    .frame()
                    .code
                    .templates
                    .get(index as usize)
                    .cloned()
                    .unwrap_or_default();
                let id = self.template_object(&chunks)?;
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
    /** The dead-zone ReferenceError for a name. */
    fn dead_zone(&mut self, name: u32) -> Thrown {
        let name = self.constant(name).lossy();
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
        let text = name.lossy();
        let key = Key::Str(name.clone());
        let strict = self.frames.last().is_some_and(|frame| frame.code.strict);
        if strict && !self.has_property(self.global_object(), &key)? {
            return Err(self.reference_error(&format!("{text} is not defined")));
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
        let invoked = self.invoke(function, this, args, construct, false);
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
        let value = if frame.construct && !matches!(value, JsValue::Object(_)) {
            if frame.this == JsValue::Hole {
                return Err(self.reference_error(
                    "Must call super constructor in derived class before accessing 'this' or returning from derived constructor",
                ));
            }
            frame.this
        } else {
            value
        };
        Ok(self.finish_frame(value, frame.boundary))
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
        let strict = self.frame().code.strict;
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
        let JsValue::Object(id) = base else {
            return false;
        };
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
        let fits = matches!(self.heap.get(*id).map(|o| (&o.class, o.extensible && o.props.is_empty())), Some((Class::Array(elements), true)) if index < u32::MAX as usize && (elements.resident_len() < self.limits.array_length || elements.get(index).is_some_and(|value| *value != JsValue::Hole)));
        if fits {
            self.array_store(*id, index, value.clone());
        }
        fits
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
        match array
            .object()
            .and_then(|id| self.heap.get(id))
            .map(|object| &object.class)
        {
            Some(Class::Array(elements)) if elements.len() > self.limits.array_length => {
                Err(EngineError::limit("V9 call arguments", self.limits.array_length).into())
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
            let name = super::property::key_text(&key);
            let mut target = *id;
            for _ in 0..self.limits.frames {
                match self.proxy_of(target).and_then(|proxy| proxy.target) {
                    Some(next) => target = next,
                    None => break,
                }
            }
            let message = if self
                .heap
                .get(target)
                .is_some_and(|object| !object.extensible)
            {
                format!("Cannot define property {name}, object is not extensible")
            } else {
                format!("Cannot redefine property: {name}")
            };
            return Err(self.type_error(&message));
        }
        Ok(())
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
        } else {
            self.define(*id, key, desc)?;
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
        if !deleted && self.frame().code.strict {
            return Err(self.type_error(&format!(
                "Cannot delete property '{}'",
                super::property::key_text(key)
            )));
        }
        Ok(deleted)
    }
    /** `super.key` reads from the home object's prototype with `this` as receiver. */
    fn super_get(&mut self, this: &JsValue, home: &JsValue, key: &Key) -> Res<JsValue> {
        let Some(proto) = home.object().and_then(|id| self.proto_of(id)) else {
            return Ok(JsValue::Undefined);
        };
        self.get_from(proto, key, this)
    }
    /** `super.key = value`. */
    fn super_set(&mut self, this: &JsValue, home: &JsValue, key: Key, value: JsValue) -> Res<()> {
        let Some(proto) = home.object().and_then(|id| self.proto_of(id)) else {
            return Ok(());
        };
        self.set_from(proto, key, value, this)?;
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
        if self.frames[index].this != JsValue::Hole {
            return Err(self.reference_error("Super constructor may only be called once"));
        }
        let parent = callee.object().and_then(|id| self.proto_of(id));
        let Some(parent) = parent.filter(|parent| self.is_constructor(&JsValue::Object(*parent)))
        else {
            return Err(self.type_error("Super constructor is not a constructor"));
        };
        let this = self.construct(JsValue::Object(parent), args, Some(new_target))?;
        self.frames[index].this = this.clone();
        Ok(this)
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
                    return Err(self.type_error("Class extends value is not a constructor or null"));
                }
                let proto = self.get(&parent, &Key::from("prototype"))?;
                let proto = match proto {
                    JsValue::Object(id) => Some(id),
                    JsValue::Null => None,
                    _ => {
                        return Err(self.type_error(
                            "Class extends value does not have valid prototype property",
                        ));
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
            Key::from("prototype"),
            Prop {
                slot: Slot::Data(JsValue::Object(prototype)),
                attrs: 0,
            },
        );
        self.heap.put(
            prototype,
            Key::from("constructor"),
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
    /** One step of `yield*`: the stack holds the inner iterator and the value received. */
    fn yield_star(&mut self) -> Res<Flow> {
        let mode = std::mem::take(&mut self.frame().resume);
        let received = self.pop();
        let iterator = self
            .peek(0)
            .object()
            .ok_or_else(|| EngineError::invalid("Invalid V9 delegation."))?;
        let (value, done) = self.delegate_step(iterator, mode, received)?;
        if !done {
            self.frame().pc -= 1;
            return Ok(self.suspend(Exit::Yield(value)));
        }
        self.pop();
        if mode == Resume::Return {
            return self.do_return(value);
        }
        self.push(value);
        Ok(Flow::Next)
    }
    /** A template strings array with its raw strings, frozen. */
    fn template_object(&mut self, chunks: &[(Option<JsString>, JsString)]) -> Res<ObjectId> {
        let cooked: Vec<JsValue> = chunks
            .iter()
            .map(|(chunk, _)| chunk.clone().map_or(JsValue::Undefined, JsValue::String))
            .collect();
        let raw_values: Vec<JsValue> = chunks
            .iter()
            .map(|(_, raw)| JsValue::String(raw.clone()))
            .collect();
        let raw = self.new_array(raw_values)?;
        let strings = self.new_array(cooked)?;
        self.heap.put(
            strings,
            Key::from("raw"),
            Prop {
                slot: Slot::Data(JsValue::Object(raw)),
                attrs: 0,
            },
        );
        for id in [raw, strings] {
            if let Some(object) = self.heap.get_mut(id) {
                object.extensible = false;
            }
        }
        Ok(strings)
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
    /** A var or function declaration of a name an earlier script declared with let, const or
    class is a SyntaxError, as for two declarations in one script. */
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
    let units = name.as_units();
    let mut bytes = [0u8; SHORT_NAME];
    if units.len() <= SHORT_NAME && units.iter().all(|unit| *unit < 0x80) {
        for (byte, unit) in bytes.iter_mut().zip(units) {
            *byte = *unit as u8;
        }
        if let Ok(text) = std::str::from_utf8(&bytes[..units.len()]) {
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
