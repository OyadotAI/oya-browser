/** Calling and constructing: closures push frames, natives run inline, bound functions forward. */
use super::data::{
    AsyncData, Class, Function, FunctionKind, GeneratorData, GeneratorState, PromiseState,
    SavedFrame,
};
use super::heap::{HIDDEN_ATTRS, INTRINSIC_BASE, Key, NATIVE_BASE, Object, Prop, Slot};
use super::realm::realm;
use super::{Call, Exit, Frame, GLOBAL, Res, Resume, Thrown, Vm};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};

/** The outcome of starting a call from the interpreter loop. */
pub(in crate::v9) enum Invoked {
    /** The call finished and produced a value. */
    Value(JsValue),
    /** A frame was pushed; the loop continues in it. */
    Frame,
}

/** What a function object does when called. */
enum Behavior {
    Closure(u32, Option<u32>),
    Native(u32),
    Bound(ObjectId, JsValue, Vec<JsValue>),
    /** A proxy whose target is callable. */
    Proxy(ObjectId),
}

impl Vm<'_> {
    /** What calling this value would do, if it is callable. */
    fn behavior(&self, value: &JsValue) -> Option<Behavior> {
        let JsValue::Object(id) = value else {
            return None;
        };
        if let Some(object) = self.heap.get(*id) {
            if let Class::Proxy(proxy) = &object.class {
                // A proxy is callable when its target is; once revoked it no longer is.
                let target = proxy.target?;
                return (target != *id && self.unproxied_callable(target))
                    .then_some(Behavior::Proxy(*id));
            }
            return match &object.function()?.kind {
                FunctionKind::Closure { site, env } => Some(Behavior::Closure(*site, *env)),
                FunctionKind::Native { id, .. } => Some(Behavior::Native(*id)),
                FunctionKind::Bound { target, this, args } => {
                    Some(Behavior::Bound(*target, this.clone(), args.clone()))
                }
            };
        }
        if *id >= NATIVE_BASE {
            return realm()
                .native(*id)
                .map(|_| Behavior::Native(*id - NATIVE_BASE));
        }
        if *id >= INTRINSIC_BASE {
            return realm().object(*id)?.call.map(Behavior::Native);
        }
        None
    }
    /** IsCallable. */
    pub fn is_callable(&self, value: &JsValue) -> bool {
        self.behavior(value).is_some()
    }
    /** IsConstructor. */
    pub fn is_constructor(&self, value: &JsValue) -> bool {
        match self.behavior(value) {
            Some(Behavior::Closure(site, _)) => self.registry.get(site).is_some_and(|code| {
                let flags = code.flags;
                flags.constructor
                    || !flags.arrow
                        && !flags.method
                        && !flags.asynchronous
                        && !flags.generator
                        && !flags.fields
            }),
            Some(Behavior::Native(index)) => realm().natives[index as usize].constructor,
            Some(Behavior::Proxy(id)) => self
                .proxy_of(id)
                .and_then(|proxy| proxy.target)
                .is_some_and(|target| {
                    target != id && self.is_constructor(&JsValue::Object(target))
                }),
            Some(Behavior::Bound(..)) => self
                .unbound(value)
                .is_some_and(|target| self.is_constructor(&target)),
            None => false,
        }
    }
    /** The function a bound chain ends at, or None past VM_CALL_DEPTH hops (only forged state
    holds a cycle; bind itself can only target an existing function). */
    fn unbound(&self, value: &JsValue) -> Option<JsValue> {
        let mut current = value.clone();
        for _ in 0..=self.limits.frames {
            match self.behavior(&current) {
                Some(Behavior::Bound(target, ..)) => current = JsValue::Object(target),
                _ => return Some(current),
            }
        }
        None
    }
    /** The target of a bound function. */
    pub fn bound_target(&self, value: &JsValue) -> Option<ObjectId> {
        match self.behavior(value)? {
            Behavior::Bound(target, ..) => Some(target),
            _ => None,
        }
    }
    /** Call a function from native code and wait for its result. */
    pub fn call(&mut self, function: JsValue, this: JsValue, args: Vec<JsValue>) -> Res<JsValue> {
        match self.invoke(function, this, args, None, true)? {
            Invoked::Value(value) => Ok(value),
            Invoked::Frame => self.run_nested(),
        }
    }
    /** `new` from native code. */
    pub fn construct(
        &mut self,
        function: JsValue,
        args: Vec<JsValue>,
        new_target: Option<JsValue>,
    ) -> Res<JsValue> {
        let target = new_target.unwrap_or_else(|| function.clone());
        match self.invoke(function, JsValue::Undefined, args, Some(target), true)? {
            Invoked::Value(value) => Ok(value),
            Invoked::Frame => self.run_nested(),
        }
    }
    /** Run the frame just pushed as a boundary until it returns. */
    fn run_nested(&mut self) -> Res<JsValue> {
        let pinning = std::mem::replace(&mut self.pinning, false);
        self.native_depth += 1;
        let result = self.run();
        self.native_depth -= 1;
        self.pinning = pinning;
        match result? {
            Exit::Return(value) => Ok(value),
            Exit::Yield(_) | Exit::Await(_) => Err(EngineError::invalid(
                "V9 suspended outside a generator or async function.",
            )
            .into()),
        }
    }
    /** Start a call or construction; `construct` carries new.target. */
    pub fn invoke(
        &mut self,
        function: JsValue,
        this: JsValue,
        args: Vec<JsValue>,
        construct: Option<JsValue>,
        boundary: bool,
    ) -> Res<Invoked> {
        if self.frames.len() + self.native_depth >= self.limits.frames {
            return Err(self.range_error("Maximum call stack size exceeded"));
        }
        let Some(behavior) = self.behavior(&function) else {
            let text = self.callee_text(&function);
            return Err(self.type_error(&format!("{text} is not a function")));
        };
        if construct.is_some() && !self.is_constructor(&function) {
            let text = self.callee_text(&function);
            return Err(self.type_error(&format!("{text} is not a constructor")));
        }
        match behavior {
            Behavior::Native(index) => {
                let callee = function.object().expect("callable object");
                self.call_native(index, callee, this, &args, construct)
                    .map(Invoked::Value)
            }
            Behavior::Bound(target, bound_this, bound_args) => {
                self.charge(1)?;
                // Each hop is one level of native recursion, so a chain (or a forged cycle) meets
                // the same depth bound as any recursion instead of overflowing the native stack.
                self.native_depth += 1;
                let result = self.invoke_bound(
                    function, target, bound_this, bound_args, args, construct, boundary,
                );
                self.native_depth -= 1;
                result
            }
            Behavior::Closure(site, env) => {
                self.call_closure(function, site, env, this, args, construct, boundary)
            }
            Behavior::Proxy(id) => {
                let proxy = self.proxy_of(id).expect("proxy behavior");
                self.native_depth += 1;
                let result = self.proxy_invoke(&proxy, this, args, construct);
                self.native_depth -= 1;
                result.map(Invoked::Value)
            }
        }
    }
    /** Whether a proxy target (followed through nested proxies, bounded) is callable. */
    fn unproxied_callable(&self, target: ObjectId) -> bool {
        let mut current = target;
        for _ in 0..self.limits.frames {
            match self.heap.get(current).map(|object| &object.class) {
                Some(Class::Proxy(proxy)) => match proxy.target {
                    Some(next) => current = next,
                    None => return false,
                },
                _ => return self.is_callable(&JsValue::Object(current)),
            }
        }
        false
    }
    /** Forward a bound call to its target with the bound receiver and leading arguments. */
    #[allow(clippy::too_many_arguments)]
    fn invoke_bound(
        &mut self,
        function: JsValue,
        target: ObjectId,
        bound_this: JsValue,
        mut bound_args: Vec<JsValue>,
        args: Vec<JsValue>,
        construct: Option<JsValue>,
        boundary: bool,
    ) -> Res<Invoked> {
        bound_args.extend(args);
        let construct = construct.map(|new_target| {
            if new_target == function {
                JsValue::Object(target)
            } else {
                new_target
            }
        });
        self.invoke(
            JsValue::Object(target),
            bound_this,
            bound_args,
            construct,
            boundary,
        )
    }
    /** Run a native with its allocations pinned until it returns. */
    pub fn call_native(
        &mut self,
        index: u32,
        callee: ObjectId,
        this: JsValue,
        args: &[JsValue],
        new_target: Option<JsValue>,
    ) -> Res<JsValue> {
        self.charge(1)?;
        if self.frames.len() + self.native_depth >= self.limits.frames {
            return Err(self.range_error("Maximum call stack size exceeded"));
        }
        let def = &realm().natives[index as usize];
        let mark = self.pins.len();
        // The receiver and arguments live only in this native's Rust locals once the caller's
        // stack drops them, so they are pinned beside what the native allocates.
        self.pins.push(this.clone());
        self.pins.extend(args.iter().cloned());
        let pinning = std::mem::replace(&mut self.pinning, true);
        // A native that calls a native (apply.apply, bind chains through call) recurses in Rust
        // without pushing a frame, so it counts against the call depth here.
        self.native_depth += 1;
        let result = (def.function)(
            self,
            Call {
                this,
                args,
                new_target,
                callee,
            },
        );
        self.native_depth -= 1;
        self.pinning = pinning;
        self.pins.truncate(mark);
        result
    }
    /** Push a closure frame, or run it to completion for async functions. */
    #[allow(clippy::too_many_arguments)]
    fn call_closure(
        &mut self,
        function: JsValue,
        site: u32,
        env: Option<u32>,
        this: JsValue,
        args: Vec<JsValue>,
        construct: Option<JsValue>,
        boundary: bool,
    ) -> Res<Invoked> {
        let code = self
            .registry
            .get(site)
            .cloned()
            .ok_or_else(|| EngineError::invalid("Invalid V9 function code site."))?;
        let flags = code.flags;
        if flags.constructor && construct.is_none() {
            return Err(self.type_error(&format!(
                "Class constructor {} cannot be invoked without 'new'",
                code.name.lossy()
            )));
        }
        let this = if construct.is_some() {
            if flags.derived {
                JsValue::Hole
            } else {
                let target = construct.clone().expect("construct");
                let proto = self.prototype_for(&target, realm().ids.object_prototype)?;
                JsValue::Object(self.alloc(Object::new(Class::Plain, Some(proto)))?)
            }
        } else if !code.strict && this.nullish() {
            JsValue::Object(GLOBAL)
        } else if !code.strict && !matches!(this, JsValue::Object(_)) {
            // OrdinaryCallBindThis: sloppy functions see a primitive receiver boxed.
            JsValue::Object(self.to_object(&this)?)
        } else {
            this
        };
        let env = match code.env {
            Some(slots) => Some(
                self.heap
                    .new_env(env, vec![JsValue::Undefined; slots as usize])?,
            ),
            None => env,
        };
        let frame = Frame {
            pc: 0,
            stack: Vec::new(),
            regs: vec![JsValue::Undefined; code.registers as usize],
            env,
            this: this.clone(),
            callee: function.clone(),
            args,
            new_target: construct.clone().unwrap_or(JsValue::Undefined),
            handlers: Vec::new(),
            envs: 0,
            boundary: boundary || flags.asynchronous,
            construct: construct.is_some(),
            resume: Resume::Next,
            code,
        };
        if construct.is_some() && !flags.derived {
            self.run_fields(&function, &this)?;
        }
        self.frames.push(frame);
        if flags.asynchronous {
            return self.start_async().map(Invoked::Value);
        }
        Ok(Invoked::Frame)
    }
    /** Install a class's instance fields on a new object. */
    pub fn run_fields(&mut self, function: &JsValue, this: &JsValue) -> Res<()> {
        let fields = function
            .object()
            .and_then(|id| self.heap.get(id))
            .and_then(|object| object.function())
            .and_then(|function| function.fields);
        if let Some(fields) = fields {
            self.call(JsValue::Object(fields), this.clone(), Vec::new())?;
        }
        Ok(())
    }
    /** The prototype for a new object: new.target's `prototype` if it is an object. */
    pub fn prototype_for(&mut self, new_target: &JsValue, fallback: ObjectId) -> Res<ObjectId> {
        match self.get(new_target, &Key::from("prototype"))? {
            JsValue::Object(proto) => Ok(proto),
            _ => Ok(fallback),
        }
    }
    /** Create a closure over an environment. */
    pub fn make_closure(
        &mut self,
        site: u32,
        env: Option<u32>,
        home: Option<ObjectId>,
    ) -> Res<ObjectId> {
        let code = self
            .registry
            .get(site)
            .ok_or_else(|| EngineError::invalid("Invalid V9 function code site."))?;
        let proto = if code.flags.generator {
            realm().ids.generator_function_prototype
        } else if code.flags.asynchronous {
            realm().ids.async_function_prototype
        } else {
            realm().ids.function_prototype
        };
        self.alloc(Object::new(
            Class::Function(Box::new(Function {
                kind: FunctionKind::Closure { site, env },
                home,
                fields: None,
            })),
            Some(proto),
        ))
    }
    /** A native function object with captured values. */
    pub fn native_closure(&mut self, index: u32, slots: Vec<JsValue>) -> Res<ObjectId> {
        self.alloc(Object::new(
            Class::Function(Box::new(Function {
                kind: FunctionKind::Native { id: index, slots },
                home: None,
                fields: None,
            })),
            Some(realm().ids.function_prototype),
        ))
    }
    /** The captured values of the native function being called. */
    pub fn slots(&self, callee: ObjectId) -> Vec<JsValue> {
        match self
            .heap
            .get(callee)
            .and_then(|object| object.function())
            .map(|f| &f.kind)
        {
            Some(FunctionKind::Native { slots, .. }) => slots.clone(),
            _ => Vec::new(),
        }
    }
    /** An arguments object over a frame's arguments. */
    pub fn arguments_object(&mut self, args: &[JsValue]) -> Res<ObjectId> {
        let id = self.arguments_list(args)?;
        // CreateUnmappedArgumentsObject (strict code) guards `callee` with %ThrowTypeError%;
        // CreateMappedArgumentsObject (sloppy code) exposes the running function.
        let frame = self.frames.last();
        let strict = frame.is_some_and(|frame| frame.code.strict);
        let callee = frame.map_or(JsValue::Undefined, |frame| frame.callee.clone());
        let thrower = realm().ids.throw_type_error;
        let prop = if strict {
            Prop {
                slot: Slot::Accessor(Some(thrower), Some(thrower)),
                attrs: 0,
            }
        } else {
            Prop {
                slot: Slot::Data(callee),
                attrs: HIDDEN_ATTRS,
            }
        };
        self.heap.put(id, Key::from("callee"), prop);
        Ok(id)
    }
    /** The indexed elements, `length` and iterator every arguments object carries. */
    fn arguments_list(&mut self, args: &[JsValue]) -> Res<ObjectId> {
        let id = self.alloc(Object::new(
            Class::Arguments,
            Some(realm().ids.object_prototype),
        ))?;
        for (index, value) in args.iter().enumerate() {
            self.heap.put(
                id,
                Key::Str(crate::v9::value::JsString::from_utf8(&index.to_string())?),
                Prop::data(value.clone()),
            );
        }
        self.heap.put(
            id,
            Key::from("length"),
            Prop {
                slot: Slot::Data(JsValue::Number(args.len() as f64)),
                attrs: HIDDEN_ATTRS,
            },
        );
        self.heap.put(
            id,
            Key::Sym(super::realm::symbol::ITERATOR),
            Prop {
                slot: Slot::Data(JsValue::Object(realm().ids.array_values)),
                attrs: HIDDEN_ATTRS,
            },
        );
        Ok(id)
    }
    /** A short name for error messages, without running script code. */
    pub fn callee_text(&mut self, value: &JsValue) -> String {
        match value {
            JsValue::Undefined | JsValue::Hole => "undefined".into(),
            JsValue::Null => "null".into(),
            JsValue::String(_) => "a string".into(),
            JsValue::Number(_) => "a number".into(),
            JsValue::Boolean(_) => "a boolean".into(),
            JsValue::Symbol(_) => "a symbol".into(),
            JsValue::BigInt(_) => "a bigint".into(),
            JsValue::Object(_) => "an object".into(),
        }
    }
    /** Run a just-pushed async function frame until it first awaits or finishes. */
    fn start_async(&mut self) -> Res<JsValue> {
        let promise = self.new_promise()?;
        self.pin(JsValue::Object(promise));
        let holder = self.alloc(Object::new(
            Class::Async(Box::new(AsyncData {
                frame: None,
                promise,
            })),
            None,
        ))?;
        self.pin(JsValue::Object(holder));
        self.drive_async(holder, None)?;
        Ok(JsValue::Object(promise))
    }
    /** Run (or resume) an async frame; on await, park it in `holder` and subscribe to the value. */
    pub fn drive_async(&mut self, holder: ObjectId, resume: Option<(Resume, JsValue)>) -> Res<()> {
        let promise = match self.heap.get(holder).map(|object| &object.class) {
            Some(Class::Async(data)) => data.promise,
            _ => return Err(EngineError::invalid("Invalid V9 async state.").into()),
        };
        if let Some((mode, value)) = resume {
            let frame = match self.heap.get_mut(holder).map(|object| &mut object.class) {
                Some(Class::Async(data)) => data.frame.take(),
                _ => None,
            };
            let Some(frame) = frame else {
                return Ok(());
            };
            self.restore_frame(frame, true)?;
            self.resume_with(mode, value)?;
        }
        let pinning = std::mem::replace(&mut self.pinning, false);
        self.native_depth += 1;
        let result = self.run();
        self.native_depth -= 1;
        self.pinning = pinning;
        match result {
            Ok(Exit::Return(value)) => self.resolve_promise(promise, value),
            Ok(Exit::Await(value)) => {
                let frame = self.suspended.take();
                if let Some(Class::Async(data)) =
                    self.heap.get_mut(holder).map(|object| &mut object.class)
                {
                    data.frame = frame;
                }
                let awaited = self.promise_resolve(value)?;
                self.await_promise(awaited, holder)
            }
            Ok(Exit::Yield(_)) => {
                Err(EngineError::invalid("V9 yield in an async function.").into())
            }
            Err(Thrown::Value(error)) => self.settle(promise, PromiseState::Rejected(error)),
            Err(fatal) => Err(fatal),
        }
    }
    /** Save the top frame (just exited by Yield or Await). */
    pub fn save_frame(&mut self, frame: Frame) -> SavedFrame {
        SavedFrame {
            site: frame.code.site,
            pc: frame.pc as u32,
            stack: frame.stack,
            regs: frame.regs,
            env: frame.env,
            this: frame.this,
            callee: frame.callee,
            args: frame.args,
            new_target: frame.new_target,
            handlers: frame.handlers,
            envs: frame.envs,
        }
    }
    /** Push a saved frame back, as a boundary frame. */
    pub fn restore_frame(&mut self, saved: SavedFrame, boundary: bool) -> Res<()> {
        let code = self
            .registry
            .get(saved.site)
            .cloned()
            .ok_or_else(|| EngineError::invalid("Invalid V9 suspended frame code."))?;
        if saved.pc as usize > code.ops.len() || saved.regs.len() != code.registers as usize {
            return Err(EngineError::invalid("Invalid V9 suspended frame.").into());
        }
        if self.frames.len() + self.native_depth >= self.limits.frames {
            return Err(self.range_error("Maximum call stack size exceeded"));
        }
        self.frames.push(Frame {
            code,
            pc: saved.pc as usize,
            stack: saved.stack,
            regs: saved.regs,
            env: saved.env,
            this: saved.this,
            callee: saved.callee,
            args: saved.args,
            new_target: saved.new_target,
            handlers: saved.handlers,
            envs: saved.envs,
            boundary,
            construct: false,
            resume: Resume::Next,
        });
        Ok(())
    }
    /** Make a generator object for a frame suspended at its start. */
    pub fn new_generator(&mut self, frame: SavedFrame, callee: &JsValue) -> Res<ObjectId> {
        let proto = self.prototype_for(callee, realm().ids.generator_prototype)?;
        self.alloc(Object::new(
            Class::Generator(Box::new(GeneratorData {
                state: GeneratorState::Start,
                frame: Some(frame),
            })),
            Some(proto),
        ))
    }
    /** Resume a generator: next, throw or return. Returns the iterator result value and done. */
    pub fn resume_generator(
        &mut self,
        generator: ObjectId,
        mode: Resume,
        value: JsValue,
    ) -> Res<(JsValue, bool)> {
        let (state, frame) = match self.heap.get_mut(generator).map(|object| &mut object.class) {
            Some(Class::Generator(data)) => (data.state, data.frame.take()),
            _ => return Err(self.type_error("next method called on incompatible receiver")),
        };
        match state {
            GeneratorState::Running => {
                self.set_generator(generator, GeneratorState::Running, frame);
                return Err(self.type_error("Generator is already running"));
            }
            GeneratorState::Done => {
                return match mode {
                    Resume::Next => Ok((JsValue::Undefined, true)),
                    Resume::Return => Ok((value, true)),
                    Resume::Throw => Err(Thrown::Value(value)),
                };
            }
            GeneratorState::Start if mode != Resume::Next => {
                self.set_generator(generator, GeneratorState::Done, None);
                return match mode {
                    Resume::Return => Ok((value, true)),
                    _ => Err(Thrown::Value(value)),
                };
            }
            _ => {}
        }
        let Some(frame) = frame else {
            return Ok((JsValue::Undefined, true));
        };
        self.set_generator(generator, GeneratorState::Running, None);
        self.restore_frame(frame, true)?;
        if state == GeneratorState::Yielded {
            self.resume_with(mode, value)?;
        }
        let pinning = std::mem::replace(&mut self.pinning, false);
        self.native_depth += 1;
        let result = self.run();
        self.native_depth -= 1;
        self.pinning = pinning;
        match result {
            Ok(Exit::Yield(value)) => {
                let frame = self.suspended.take();
                self.set_generator(generator, GeneratorState::Yielded, frame);
                Ok((value, false))
            }
            Ok(Exit::Return(value)) => {
                self.set_generator(generator, GeneratorState::Done, None);
                Ok((value, true))
            }
            Ok(Exit::Await(_)) => Err(EngineError::invalid("V9 await in a generator.").into()),
            Err(error) => {
                self.set_generator(generator, GeneratorState::Done, None);
                Err(error)
            }
        }
    }
    /** Update a generator's state and frame. */
    fn set_generator(
        &mut self,
        generator: ObjectId,
        state: GeneratorState,
        frame: Option<SavedFrame>,
    ) {
        if let Some(Class::Generator(data)) =
            self.heap.get_mut(generator).map(|object| &mut object.class)
        {
            data.state = state;
            data.frame = frame;
        }
    }
}
