/** Promise, its reaction jobs, and the microtask queue every request drains. */
use super::*;
mod forwarding;
use crate::protocol::EngineError;
use crate::v9::vm::Resume;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Class, Job, PromiseData, PromiseState, Reaction, ReactionHandler};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{Static, realm, string, symbol};
pub(in crate::v9) use forwarding::{forwarding_promise, resolving_promise};

/** Shared resolver state holds the pending target only until the first claim. */
const RESOLVER_TARGET: &str = "promise-resolver-target";

/** Native slots for the resolving functions and combinators. */
struct Natives {
    resolve: u32,
    reject: u32,
}

/** Native indexes captured by closures, set once at install. */
static NATIVES: std::sync::OnceLock<Natives> = std::sync::OnceLock::new();

/** The resolving-function natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("promise natives installed")
}

/** Promise and Promise.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.promise_prototype = proto;
    let ctor = b.constructor("Promise", 1, construct, proto, None);
    b.ids.promise = ctor;
    b.global("Promise", ctor);
    super::typed::species_getter(b, ctor);
    super::promise_combinators::install(b, ctor, proto);
    b.method(proto, "catch", 1, catch);
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Promise")),
        CONFIGURABLE,
    );
    let _ = NATIVES.set(Natives {
        resolve: b.native("", 1, resolve_function),
        reject: b.native("", 1, reject_function),
    });
}

impl Vm<'_> {
    /** A new pending promise has no parent unless a genuine reaction creates it. */
    pub fn new_promise(&mut self) -> Res<ObjectId> {
        self.new_promise_parent(None)
    }
    /** Hook parent metadata is attached before any author callback sees the actual pending Promise. */
    pub fn new_promise_parent(&mut self, parent: Option<ObjectId>) -> Res<ObjectId> {
        let promise = self.alloc(Object::new(
            Class::Promise(Box::new(PromiseData {
                state: PromiseState::Pending,
                reactions: Vec::new(),
                handled: false,
            })),
            Some(realm().ids.promise_prototype),
        ))?;
        #[cfg(feature = "conformance")]
        {
            super::debug_event(self, super::DebugEvent::PromiseCreated);
            let mark = self.pins.len();
            self.pins.push(JsValue::Object(promise));
            let result = super::async_hook_init(self, promise, parent);
            self.pins.truncate(mark);
            result?;
        }
        #[cfg(not(feature = "conformance"))]
        let _ = parent;
        Ok(promise)
    }
    /** Queue a microtask. */
    pub fn enqueue(&mut self, job: Job) -> Res<()> {
        #[cfg(feature = "conformance")]
        let job = match job {
            Job::Callback(callback) => Job::CallbackWithData {
                callback,
                continuation_data: self.page.state.continuation_data.clone(),
            },
            job => job,
        };
        #[cfg(feature = "conformance")]
        let custom_count = self.realms.custom_job_count();
        #[cfg(not(feature = "conformance"))]
        let custom_count = 0;
        if self.jobs.len().saturating_add(custom_count) >= self.limits.jobs {
            return Err(EngineError::limit("V9 pending jobs", self.limits.jobs).into());
        }
        #[cfg(feature = "conformance")]
        if let Some(bank) = self.custom_job_realm(&job) {
            let context = &mut self.realms.contexts[bank as usize];
            if !context.microtask_queue_closed {
                context.custom_jobs.push(job);
            }
            return Ok(());
        }
        self.jobs.push_back(job);
        Ok(())
    }
    /** V8 chooses the callback's creation realm, rather than the caller that settles the promise. */
    #[cfg(feature = "conformance")]
    fn custom_job_realm(&self, job: &Job) -> Option<u32> {
        let callback = match job {
            Job::Callback(callback) => callback.object(),
            Job::CallbackWithData { callback, .. } => callback.object(),
            Job::Thenable { then, .. } => then.object(),
            Job::Reaction { reaction, .. } => match &reaction.handler {
                ReactionHandler::Function(callback) => callback.object().or(reaction.target),
                ReactionHandler::Resume(holder) => Some(*holder),
            },
        };
        let bank = callback
            .and_then(|id| self.job_callback_realm(id))
            .unwrap_or(self.active_realm);
        self.realms
            .contexts
            .get(bank as usize)
            .filter(|context| context.own_microtask_queue && context.global.is_some())
            .map(|_| bank)
    }
    /** Bound functions and live proxies inherit the target's realm; revoked proxies use the current context. */
    #[cfg(feature = "conformance")]
    fn job_callback_realm(&self, mut id: ObjectId) -> Option<u32> {
        for _ in 0..self.limits.frames {
            match self.heap.get(id).map(|object| &object.class) {
                Some(Class::Proxy(proxy)) => id = proxy.target?,
                Some(Class::Function(function)) => match &function.kind {
                    crate::v9::vm::data::FunctionKind::Bound { target, .. } => id = *target,
                    _ => return Some(self.object_realm(id)),
                },
                _ => return Some(self.object_realm(id)),
            }
        }
        None
    }
    /** Settle a pending promise and schedule its reactions. */
    pub fn settle(&mut self, promise: ObjectId, state: PromiseState) -> Res<()> {
        let mark = self.pins.len();
        self.pins.push(JsValue::Object(promise));
        if let PromiseState::Fulfilled(value) | PromiseState::Rejected(value) = &state {
            self.pins.push(value.clone());
        }
        let result = (|| -> Res<()> {
            #[cfg(feature = "conformance")]
            if matches!(self.heap.get(promise).map(|object|&object.class),Some(Class::Promise(data))if matches!(data.state,PromiseState::Pending))
            {
                super::async_hook_resolve(self, promise)?;
            }
            self.settle_inner(promise, state)
        })();
        self.pins.truncate(mark);
        result
    }
    /** State transitions happen only after the real resolve hook, so hook-added reactions join the same settlement. */
    fn settle_inner(&mut self, promise: ObjectId, state: PromiseState) -> Res<()> {
        let (reactions, handled) = match self.heap.get_mut(promise).map(|o| &mut o.class) {
            Some(Class::Promise(data)) if matches!(data.state, PromiseState::Pending) => {
                data.state = state.clone();
                (std::mem::take(&mut data.reactions), data.handled)
            }
            _ => return Ok(()),
        };
        let (rejected, argument) = match state {
            PromiseState::Fulfilled(value) => (false, value),
            PromiseState::Rejected(value) => (true, value),
            PromiseState::Pending => return Ok(()),
        };
        #[cfg(feature = "conformance")]
        super::debug_event(self, super::DebugEvent::PromiseSettled);
        if rejected && !handled {
            self.page.unhandled.push(promise);
        }
        for reaction in reactions
            .into_iter()
            .filter(|reaction| reaction.reject == rejected)
        {
            self.enqueue(Job::Reaction {
                reaction,
                argument: argument.clone(),
            })?;
        }
        Ok(())
    }
    /** Resolve a promise with a value, adopting thenables. */
    pub fn resolve_promise(&mut self, promise: ObjectId, value: JsValue) -> Res<()> {
        let mark = self.pins.len();
        self.pins.push(JsValue::Object(promise));
        self.pins.push(value.clone());
        let result = (|| -> Res<()> {
            #[cfg(feature = "conformance")]
            super::async_hook_resolve(self, promise)?;
            self.resolve_promise_inner(promise, value)
        })();
        self.pins.truncate(mark);
        result
    }
    /** PromiseResolve keeps thenable access and adoption on the original language path. */
    fn resolve_promise_inner(&mut self, promise: ObjectId, value: JsValue) -> Res<()> {
        if value == JsValue::Object(promise) {
            let error = self.type_error("Chaining cycle detected for promise");
            let Thrown::Value(error) = error else {
                return Err(error);
            };
            return self.settle(promise, PromiseState::Rejected(error));
        }
        if !matches!(value, JsValue::Object(_)) {
            return self.settle(promise, PromiseState::Fulfilled(value));
        }
        let then = match self.get(&value, &key("then")) {
            Ok(then) => then,
            Err(Thrown::Value(error)) => {
                return self.settle(promise, PromiseState::Rejected(error));
            }
            Err(fatal) => return Err(fatal),
        };
        if !self.is_callable(&then) {
            return self.settle(promise, PromiseState::Fulfilled(value));
        }
        self.enqueue(Job::Thenable {
            #[cfg(feature = "conformance")]
            continuation_data: self.page.state.continuation_data.clone(),
            promise,
            thenable: value,
            then,
        })
    }
    /** PromiseResolve(%Promise%, value): a promise whose `constructor` is %Promise% passes
    through (the lookup is observable and may throw), anything else is wrapped. */
    pub fn promise_resolve(&mut self, value: JsValue) -> Res<ObjectId> {
        if let JsValue::Object(id) = value
            && matches!(self.heap.get(id).map(|o| &o.class), Some(Class::Promise(_)))
        {
            self.pin(value.clone());
            if self.get(&value, &key("constructor"))? == JsValue::Object(realm().ids.promise) {
                return Ok(id);
            }
        }
        let promise = self.new_promise()?;
        self.pin(JsValue::Object(promise));
        self.resolve_promise(promise, value)?;
        Ok(promise)
    }
    /** Add reactions to a promise (PerformPromiseThen). */
    pub fn perform_then(
        &mut self,
        promise: ObjectId,
        fulfilled: ReactionHandler,
        rejected: ReactionHandler,
        target: Option<ObjectId>,
    ) -> Res<()> {
        let reactions = [
            Reaction {
                #[cfg(feature = "conformance")]
                continuation_data: self.page.state.continuation_data.clone(),
                reject: false,
                handler: fulfilled,
                target,
            },
            Reaction {
                #[cfg(feature = "conformance")]
                continuation_data: self.page.state.continuation_data.clone(),
                reject: true,
                handler: rejected,
                target,
            },
        ];
        let state = match self.heap.get_mut(promise).map(|o| &mut o.class) {
            Some(Class::Promise(data)) => {
                data.handled = true;
                match &data.state {
                    PromiseState::Pending => {
                        data.reactions.extend(reactions);
                        return Ok(());
                    }
                    other => other.clone(),
                }
            }
            _ => return Err(EngineError::invalid("Invalid V9 promise.").into()),
        };
        self.page.unhandled.retain(|id| *id != promise);
        let (reject, argument) = match state {
            PromiseState::Fulfilled(value) => (false, value),
            PromiseState::Rejected(value) => (true, value),
            PromiseState::Pending => return Ok(()),
        };
        let reaction = reactions
            .into_iter()
            .find(|reaction| reaction.reject == reject)
            .expect("reaction");
        self.enqueue(Job::Reaction { reaction, argument })
    }
    /** Suspend an async function on a promise. */
    pub fn await_promise(&mut self, promise: ObjectId, holder: ObjectId) -> Res<()> {
        #[cfg(feature = "conformance")]
        let target = if self.page.environment.conformance_async_hooks {
            Some(self.new_promise_parent(Some(promise))?)
        } else {
            None
        };
        #[cfg(not(feature = "conformance"))]
        let target = None;
        self.perform_then(
            promise,
            ReactionHandler::Resume(holder),
            ReactionHandler::Resume(holder),
            target,
        )
    }
    /** Run queued microtasks until the queue is empty. */
    pub fn drain_jobs(&mut self) -> Res<()> {
        if self.draining_jobs {
            return Ok(());
        }
        self.draining_jobs = true;
        let result = self.drain_jobs_inner();
        self.draining_jobs = false;
        result?;
        // HTML's "cleanup Indexed Database transactions" closes the checkpoint at the end of a task.
        if self.indexed_db_cleanup && self.frames.is_empty() {
            crate::v9::platform::indexed_db::end_of_task(self)?;
        }
        Ok(())
    }
    /** One checkpoint owns the queue and always releases its reentrancy guard on failure. */
    fn drain_jobs_inner(&mut self) -> Res<()> {
        loop {
            self.charge(1)?;
            if self.expired() {
                return Err(super::super::platform::timeout().into());
            }
            // Collect while the next job is still in the queue, where the collector sees it.
            self.collect_if_needed()?;
            if self.collect_due {
                self.collect_if_due()?;
            }
            let Some(job) = self.jobs.pop_front() else {
                // A stable state: media elements whose attributes or position changed run
                // their load steps now, and whatever microtasks those queue drain next.
                match crate::v9::platform::media::stable_state(self) {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(Thrown::Value(error)) => {
                        crate::v9::platform::workers::report_exception(self, &error)?;
                        continue;
                    }
                    Err(fatal) => return Err(fatal),
                }
            };
            match self.run_rooted_job(job) {
                Ok(()) => {}
                Err(Thrown::Value(error)) => {
                    crate::v9::platform::workers::report_exception(self, &error)?
                }
                Err(fatal) => return Err(fatal),
            }
        }
        self.report_unhandled();
        if self.frames.is_empty() {
            self.clear_kept_objects();
        }
        Ok(())
    }
    /** Log rejections nothing handled by the end of a drain. */
    fn report_unhandled(&mut self) {
        let unhandled = std::mem::take(&mut self.page.unhandled);
        for promise in unhandled {
            let reason = match self.heap.get(promise).map(|o| &o.class) {
                Some(Class::Promise(data)) if !data.handled => match &data.state {
                    PromiseState::Rejected(reason) => reason.clone(),
                    _ => continue,
                },
                _ => continue,
            };
            #[cfg(feature = "conformance")]
            {
                self.note_conformance_native_error(&reason);
                if self.page.environment.conformance_ignore_unhandled_promises
                    && self.page.effects.conformance_native_error.is_none()
                {
                    continue;
                }
                self.page.effects.conformance_rejection = true;
            }
            let text = self.describe(&reason);
            #[cfg(feature = "conformance")]
            let text = self.conformance_description(&reason).unwrap_or(text);
            #[cfg(feature = "conformance")]
            if self.page.effects.conformance_error.is_none() {
                self.page.effects.conformance_error = Some(format!("Uncaught (in promise) {text}"));
            }
            self.page
                .console("error", format!("Uncaught (in promise) {text}"));
        }
    }
    /** A popped job's resource, callback and argument remain roots while hooks or handlers can collect. */
    fn run_rooted_job(&mut self, job: Job) -> Res<()> {
        let mark = self.pins.len();
        match &job {
            Job::Callback(function) => self.pins.push(function.clone()),
            #[cfg(feature = "conformance")]
            Job::CallbackWithData {
                callback,
                continuation_data,
            } => {
                self.pins.push(callback.clone());
                if let Some(value) = continuation_data {
                    self.pins.push(value.clone());
                }
            }
            Job::Thenable {
                promise,
                thenable,
                then,
                ..
            } => self
                .pins
                .extend([JsValue::Object(*promise), thenable.clone(), then.clone()]),
            Job::Reaction { reaction, argument } => {
                self.pins.push(argument.clone());
                if let Some(target) = reaction.target {
                    self.pins.push(JsValue::Object(target));
                }
                match &reaction.handler {
                    ReactionHandler::Function(handler) => self.pins.push(handler.clone()),
                    ReactionHandler::Resume(holder) => self.pins.push(JsValue::Object(*holder)),
                }
            }
        }
        #[cfg(feature = "conformance")]
        let resource = super::async_hook_resource(self, &job);
        #[cfg(feature = "conformance")]
        let previous_data = match &job {
            Job::Reaction { reaction, .. } => Some(reaction.continuation_data.clone()),
            Job::Thenable {
                continuation_data, ..
            } => Some(continuation_data.clone()),
            Job::Callback(_) => Some(None),
            Job::CallbackWithData {
                continuation_data, ..
            } => Some(continuation_data.clone()),
        }
        .map(|data| std::mem::replace(&mut self.page.state.continuation_data, data));
        #[cfg(feature = "conformance")]
        if let Some(Some(value)) = &previous_data {
            self.pins.push(value.clone());
        }
        let result = (|| -> Res<()> {
            #[cfg(feature = "conformance")]
            if let Some(resource) = resource
                && let Err(error) = super::async_hook_before(self, resource)
            {
                super::async_hook_unwind(self)?;
                return Err(error);
            }
            let result = self.run_job(job);
            #[cfg(feature = "conformance")]
            if let Some(resource) = resource {
                if matches!(result, Err(Thrown::Fatal(_))) {
                    super::async_hook_unwind(self)?;
                } else {
                    super::async_hook_after(self, resource)?;
                }
            }
            result
        })();
        #[cfg(feature = "conformance")]
        if let Some(previous) = previous_data {
            self.page.state.continuation_data = previous;
        }
        self.pins.truncate(mark);
        result
    }
    /** One microtask. */
    fn run_job(&mut self, job: Job) -> Res<()> {
        match job {
            Job::Callback(function) => {
                self.call(function, JsValue::Undefined, Vec::new())?;
                Ok(())
            }
            #[cfg(feature = "conformance")]
            Job::CallbackWithData { callback, .. } => {
                self.call(callback, JsValue::Undefined, Vec::new())?;
                Ok(())
            }
            Job::Thenable {
                promise,
                thenable,
                then,
                ..
            } => {
                let (resolve, reject) = self.resolving_functions(promise)?;
                match self.call(
                    then,
                    thenable,
                    vec![JsValue::Object(resolve), JsValue::Object(reject)],
                ) {
                    Ok(_) => {}
                    Err(Thrown::Value(error)) => {
                        self.call(JsValue::Object(reject), JsValue::Undefined, vec![error])?;
                    }
                    Err(fatal) => return Err(fatal),
                }
                Ok(())
            }
            Job::Reaction { reaction, argument } => match reaction.handler {
                ReactionHandler::Resume(holder) => {
                    let mode = if reaction.reject {
                        Resume::Throw
                    } else {
                        Resume::Next
                    };
                    self.drive_async(holder, Some((mode, argument)))?;
                    if let Some(target) = reaction.target {
                        self.resolve_promise(target, JsValue::Undefined)?;
                    }
                    Ok(())
                }
                ReactionHandler::Function(handler) => {
                    let outcome = if self.is_callable(&handler) {
                        self.call(handler, JsValue::Undefined, vec![argument])
                    } else if reaction.reject {
                        Err(Thrown::Value(argument))
                    } else {
                        Ok(argument)
                    };
                    let Some(target) = reaction.target else {
                        return outcome.map(|_| ());
                    };
                    match outcome {
                        Ok(value) => self.resolve_promise(target, value),
                        Err(Thrown::Value(error)) => {
                            self.settle(target, PromiseState::Rejected(error))
                        }
                        Err(fatal) => Err(fatal),
                    }
                }
            },
        }
    }
    /** A resolve/reject pair sharing one already-resolved flag. */
    pub fn resolving_functions(&mut self, promise: ObjectId) -> Res<(ObjectId, ObjectId)> {
        let flag = self.new_object()?;
        self.heap.put(
            flag,
            Key::private(RESOLVER_TARGET),
            crate::v9::vm::heap::Prop::data(JsValue::Object(promise)),
        );
        let slots = vec![JsValue::Object(flag)];
        let resolve = self.native_closure(natives().resolve, slots.clone())?;
        let reject = self.native_closure(natives().reject, slots)?;
        Ok((resolve, reject))
    }
}

/** Claim a resolving function's shared flag; false if already used. */
fn claim(vm: &mut Vm, call: &Call) -> Res<Option<ObjectId>> {
    let slots = vm.slots(call.callee);
    let (promise, flag, shared) = match slots.as_slice() {
        [JsValue::Object(flag)] => {
            let target = vm
                .heap
                .get(*flag)
                .and_then(|object| object.props.get(&Key::private(RESOLVER_TARGET)))
                .and_then(|prop| match &prop.slot {
                    crate::v9::vm::heap::Slot::Data(JsValue::Object(target)) => Some(*target),
                    _ => None,
                });
            let Some(target) = target else {
                return Ok(None);
            };
            (target, JsValue::Object(*flag), true)
        }
        [JsValue::Object(promise), flag] => (*promise, flag.clone(), false),
        _ => return Ok(None),
    };
    if vm.get(&flag, &key("done"))?.truthy() {
        return Ok(None);
    }
    if let JsValue::Object(flag) = flag {
        vm.put(flag, "done", JsValue::Boolean(true))?;
        if shared {
            vm.heap.put(
                flag,
                Key::private(RESOLVER_TARGET),
                crate::v9::vm::heap::Prop::data(JsValue::Undefined),
            );
        }
    }
    Ok(Some(promise))
}

/** A promise's resolve function. */
fn resolve_function(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(promise) = claim(vm, &call)? {
        vm.resolve_promise(promise, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** A promise's reject function. */
fn reject_function(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(promise) = claim(vm, &call)? {
        vm.settle(promise, PromiseState::Rejected(call.arg(0)))?;
    }
    Ok(JsValue::Undefined)
}

/** Script throws reject the promise; fatal engine failures must escape its executor. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Promise constructor cannot be invoked without 'new'"));
    }
    let executor = call.arg(0);
    if !vm.is_callable(&executor) {
        return Err(vm.type_error("Promise resolver is not a function"));
    }
    let promise = vm.new_promise()?;
    if let Some(target) = &call.new_target {
        let proto = vm.prototype_for(target, realm().ids.promise_prototype)?;
        vm.set_proto(promise, Some(proto))?;
    }
    let (resolve, reject) = vm.resolving_functions(promise)?;
    match vm.call(
        executor,
        JsValue::Undefined,
        vec![JsValue::Object(resolve), JsValue::Object(reject)],
    ) {
        Ok(_) => {}
        Err(Thrown::Value(error)) => {
            vm.call(JsValue::Object(reject), JsValue::Undefined, vec![error])?;
        }
        Err(error @ Thrown::Fatal(_)) => return Err(error),
    }
    Ok(JsValue::Object(promise))
}

/** Promise.prototype.catch. */
fn catch(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let method = vm.get(&call.this, &key("then"))?;
    vm.call(
        method,
        call.this.clone(),
        vec![JsValue::Undefined, call.arg(0)],
    )
}
