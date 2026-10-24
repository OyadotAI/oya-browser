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
    all_element: u32,
    settled_element: u32,
    any_element: u32,
    finally_fulfilled: u32,
    finally_rejected: u32,
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
    b.method(ctor, "resolve", 1, resolve_static);
    b.method(ctor, "reject", 1, reject_static);
    b.method(ctor, "all", 1, |vm, call| {
        combinator(vm, call, Combinator::All)
    });
    b.method(ctor, "allSettled", 1, |vm, call| {
        combinator(vm, call, Combinator::AllSettled)
    });
    b.method(ctor, "any", 1, |vm, call| {
        combinator(vm, call, Combinator::Any)
    });
    b.method(ctor, "race", 1, race);
    b.method(ctor, "withResolvers", 0, with_resolvers);
    b.method(proto, "then", 2, then);
    b.method(proto, "catch", 1, catch);
    b.method(proto, "finally", 1, finally);
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Promise")),
        CONFIGURABLE,
    );
    let _ = NATIVES.set(Natives {
        resolve: b.native("", 1, resolve_function),
        reject: b.native("", 1, reject_function),
        all_element: b.native("", 1, all_element),
        settled_element: b.native("", 1, settled_element),
        any_element: b.native("", 1, any_element),
        finally_fulfilled: b.native("", 1, finally_fulfilled),
        finally_rejected: b.native("", 1, finally_rejected),
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
    /** PromiseResolve: promises pass through, other values are wrapped. */
    pub fn promise_resolve(&mut self, value: JsValue) -> Res<ObjectId> {
        if let JsValue::Object(id) = value
            && matches!(self.heap.get(id).map(|o| &o.class), Some(Class::Promise(_)))
        {
            return Ok(id);
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

/** Promise.resolve. */
fn resolve_static(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.promise_resolve(call.arg(0)).map(JsValue::Object)
}

/** Promise.reject. */
fn reject_static(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.settle(promise, PromiseState::Rejected(call.arg(0)))?;
    Ok(JsValue::Object(promise))
}

/** Promise.withResolvers. */
fn with_resolvers(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    let (resolve, reject) = vm.resolving_functions(promise)?;
    let result = vm.new_object()?;
    vm.put(result, "promise", JsValue::Object(promise))?;
    vm.put(result, "resolve", JsValue::Object(resolve))?;
    vm.put(result, "reject", JsValue::Object(reject))?;
    Ok(JsValue::Object(result))
}

/** The promise a receiver is. */
fn this_promise(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    if let JsValue::Object(id) = call.this
        && matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Promise(_)))
    {
        return Ok(id);
    }
    Err(vm.type_error("Method Promise.prototype.then called on incompatible receiver"))
}

/** Promise.prototype.then. */
fn then(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = this_promise(vm, &call)?;
    let target = vm.new_promise()?;
    let handler = |vm: &Vm, value: JsValue| {
        ReactionHandler::Function(if vm.is_callable(&value) {
            value
        } else {
            JsValue::Undefined
        })
    };
    let fulfilled = handler(vm, call.arg(0));
    let rejected = handler(vm, call.arg(1));
    vm.perform_then(promise, fulfilled, rejected, Some(target))?;
    Ok(JsValue::Object(target))
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

/** Promise.prototype.finally. */
fn finally(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let on_finally = call.arg(0);
    let method = vm.get(&call.this, &key("then"))?;
    if !vm.is_callable(&on_finally) {
        return vm.call(
            method,
            call.this.clone(),
            vec![on_finally.clone(), on_finally],
        );
    }
    let fulfilled = vm.native_closure(natives().finally_fulfilled, vec![on_finally.clone()])?;
    let rejected = vm.native_closure(natives().finally_rejected, vec![on_finally])?;
    vm.call(
        method,
        call.this.clone(),
        vec![JsValue::Object(fulfilled), JsValue::Object(rejected)],
    )
}

/** finally's fulfillment pass-through. */
fn finally_fulfilled(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let on_finally = vm
        .slots(call.callee)
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined);
    vm.call(on_finally, JsValue::Undefined, Vec::new())?;
    Ok(call.arg(0))
}

/** finally's rejection pass-through. */
fn finally_rejected(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let on_finally = vm
        .slots(call.callee)
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined);
    vm.call(on_finally, JsValue::Undefined, Vec::new())?;
    Err(Thrown::Value(call.arg(0)))
}

/** Which aggregate. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Combinator {
    All,
    AllSettled,
    Any,
}

/** Promise.all, allSettled and any share one shape: element functions fill a values array. */
fn combinator(vm: &mut Vm, call: Call, kind: Combinator) -> Res<JsValue> {
    let result = vm.new_promise()?;
    let items = match vm.iterate_to_vec(&call.arg(0)) {
        Ok(items) => items,
        Err(Thrown::Value(error)) => {
            vm.settle(result, PromiseState::Rejected(error))?;
            return Ok(JsValue::Object(result));
        }
        Err(fatal) => return Err(fatal),
    };
    let values = vm.new_array(vec![JsValue::Undefined; items.len()])?;
    let counter = vm.new_object()?;
    vm.put(counter, "remaining", JsValue::Number(items.len() as f64))?;
    if items.is_empty() {
        if kind == Combinator::Any {
            let error = vm.make_error(
                crate::v9::vm::error::ErrorKind::Error,
                "All promises were rejected",
            )?;
            vm.settle(result, PromiseState::Rejected(JsValue::Object(error)))?;
        } else {
            vm.resolve_promise(result, JsValue::Object(values))?;
        }
        return Ok(JsValue::Object(result));
    }
    let (_, reject) = vm.resolving_functions(result)?;
    for (index, item) in items.into_iter().enumerate() {
        let promise = vm.promise_resolve(item)?;
        let slots = vec![
            JsValue::Number(index as f64),
            JsValue::Object(values),
            JsValue::Object(counter),
            JsValue::Object(result),
        ];
        let (fulfilled, rejected) = match kind {
            Combinator::All => (vm.native_closure(natives().all_element, slots)?, reject),
            Combinator::AllSettled => {
                let mut rejected_slots = slots.clone();
                rejected_slots.push(JsValue::Boolean(true));
                (
                    vm.native_closure(natives().settled_element, slots)?,
                    vm.native_closure(natives().settled_element, rejected_slots)?,
                )
            }
            Combinator::Any => {
                let (resolve, _) = vm.resolving_functions(result)?;
                (resolve, vm.native_closure(natives().any_element, slots)?)
            }
        };
        vm.perform_then(
            promise,
            ReactionHandler::Function(JsValue::Object(fulfilled)),
            ReactionHandler::Function(JsValue::Object(rejected)),
            None,
        )?;
    }
    Ok(JsValue::Object(result))
}

/** Store one element; settle the aggregate when the last arrives. */
fn store_element(
    vm: &mut Vm,
    call: &Call,
    value: JsValue,
    finish: impl FnOnce(&mut Vm, ObjectId, JsValue) -> Res<()>,
) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let index = match slots.first() {
        Some(JsValue::Number(index)) => *index as usize,
        _ => return Ok(JsValue::Undefined),
    };
    let (Some(values), Some(counter), Some(JsValue::Object(result))) =
        (slots.get(1), slots.get(2), slots.get(3))
    else {
        return Ok(JsValue::Undefined);
    };
    if let JsValue::Object(id) = values {
        vm.array_store(*id, index, value);
    }
    let remaining = vm.get(counter, &key("remaining"))?;
    let remaining = vm.to_number(&remaining)? - 1.0;
    if let JsValue::Object(counter) = counter {
        vm.put(*counter, "remaining", JsValue::Number(remaining))?;
    }
    if remaining == 0.0 {
        finish(vm, *result, values.clone())?;
    }
    Ok(JsValue::Undefined)
}

/** Promise.all element function. */
fn all_element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    store_element(vm, &call, call.arg(0), |vm, result, values| {
        vm.resolve_promise(result, values)
    })
}

/** Promise.allSettled element function (slot 4 marks the rejection branch). */
fn settled_element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rejected = vm.slots(call.callee).get(4).is_some_and(JsValue::truthy);
    let record = vm.new_object()?;
    let status = super::text(vm, if rejected { "rejected" } else { "fulfilled" })?;
    vm.put(record, "status", status)?;
    vm.put(
        record,
        if rejected { "reason" } else { "value" },
        call.arg(0),
    )?;
    store_element(vm, &call, JsValue::Object(record), |vm, result, values| {
        vm.resolve_promise(result, values)
    })
}

/** Promise.any rejection element function. */
fn any_element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    store_element(vm, &call, call.arg(0), |vm, result, values| {
        let error = vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            "All promises were rejected",
        )?;
        vm.put(error, "errors", values)?;
        vm.settle(result, PromiseState::Rejected(JsValue::Object(error)))
    })
}

/** Promise.race. */
fn race(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = vm.new_promise()?;
    let (resolve, reject) = vm.resolving_functions(result)?;
    for item in vm.iterate_to_vec(&call.arg(0))? {
        let promise = vm.promise_resolve(item)?;
        vm.perform_then(
            promise,
            ReactionHandler::Function(JsValue::Object(resolve)),
            ReactionHandler::Function(JsValue::Object(reject)),
            None,
        )?;
    }
    Ok(JsValue::Object(result))
}
