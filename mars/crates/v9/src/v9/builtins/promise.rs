/** Promise, its reaction jobs, and the microtask queue every request drains. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::Resume;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Class, Job, PromiseData, PromiseState, Reaction, ReactionHandler};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{Static, realm, string, symbol};

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
    /** A new pending promise. */
    pub fn new_promise(&mut self) -> Res<ObjectId> {
        self.alloc(Object::new(
            Class::Promise(Box::new(PromiseData {
                state: PromiseState::Pending,
                reactions: Vec::new(),
                handled: false,
            })),
            Some(realm().ids.promise_prototype),
        ))
    }
    /** Queue a microtask. */
    pub fn enqueue(&mut self, job: Job) -> Res<()> {
        if self.jobs.len() >= self.limits.jobs {
            return Err(EngineError::limit("V9 pending jobs", self.limits.jobs).into());
        }
        self.jobs.push_back(job);
        Ok(())
    }
    /** Settle a pending promise and schedule its reactions. */
    pub fn settle(&mut self, promise: ObjectId, state: PromiseState) -> Res<()> {
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
                reject: false,
                handler: fulfilled,
                target,
            },
            Reaction {
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
        self.perform_then(
            promise,
            ReactionHandler::Resume(holder),
            ReactionHandler::Resume(holder),
            None,
        )
    }
    /** Run queued microtasks until the queue is empty. */
    pub fn drain_jobs(&mut self) -> Res<()> {
        loop {
            self.charge(1)?;
            if self.expired() {
                return Err(super::super::platform::timeout().into());
            }
            // Collect while the next job is still in the queue, where the collector sees it.
            self.collect_if_needed()?;
            self.collect_if_due()?;
            let Some(job) = self.jobs.pop_front() else {
                break;
            };
            match self.run_job(job) {
                Ok(()) => {}
                Err(Thrown::Value(error)) => self.report_error(&error),
                Err(fatal) => return Err(fatal),
            }
        }
        self.report_unhandled();
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
            let text = self.describe(&reason);
            self.page
                .console("error", format!("Uncaught (in promise) {text}"));
        }
    }
    /** One microtask. */
    fn run_job(&mut self, job: Job) -> Res<()> {
        match job {
            Job::Callback(function) => {
                self.call(function, JsValue::Undefined, Vec::new())?;
                Ok(())
            }
            Job::Thenable {
                promise,
                thenable,
                then,
            } => {
                let (resolve, reject) = self.resolving_functions(promise)?;
                if let Err(Thrown::Value(error)) = self.call(
                    then,
                    thenable,
                    vec![JsValue::Object(resolve), JsValue::Object(reject)],
                ) {
                    self.call(JsValue::Object(reject), JsValue::Undefined, vec![error])?;
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
                    self.drive_async(holder, Some((mode, argument)))
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
        let slots = vec![JsValue::Object(promise), JsValue::Object(flag)];
        let resolve = self.native_closure(natives().resolve, slots.clone())?;
        let reject = self.native_closure(natives().reject, slots)?;
        Ok((resolve, reject))
    }
}

/** Claim a resolving function's shared flag; false if already used. */
fn claim(vm: &mut Vm, call: &Call) -> Res<Option<ObjectId>> {
    let slots = vm.slots(call.callee);
    let (Some(JsValue::Object(promise)), Some(flag)) = (slots.first(), slots.get(1)) else {
        return Ok(None);
    };
    if vm.get(flag, &key("done"))?.truthy() {
        return Ok(None);
    }
    if let JsValue::Object(flag) = flag {
        vm.put(*flag, "done", JsValue::Boolean(true))?;
    }
    Ok(Some(*promise))
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

/** `new Promise(executor)`. */
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
    if let Err(Thrown::Value(error)) = vm.call(
        executor,
        JsValue::Undefined,
        vec![JsValue::Object(resolve), JsValue::Object(reject)],
    ) {
        vm.call(JsValue::Object(reject), JsValue::Undefined, vec![error])?;
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
