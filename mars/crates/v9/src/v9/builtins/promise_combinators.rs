/** Promise statics and `then` over PromiseCapability records, ported from the spec's
NewPromiseCapability, PerformPromiseAll/AllSettled/Any/Race and V8's promise-all.tq: every one
works on `this` as a constructor (so subclasses and thenable-shaped constructors are honoured),
looks `resolve` up once, steps the iterator lazily and closes it when a step after the first
fails. The earlier shortcut collected the whole iterable first and always built a %Promise%,
which looped forever on infinite iterators and ignored subclass constructors. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Class, ReactionHandler};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{realm, symbol};

/** Native slots this file's closures run under, set once at install. */
struct Natives {
    executor: u32,
    all_element: u32,
    settled_element: u32,
    any_element: u32,
    reaction: u32,
    then_finally: u32,
    catch_finally: u32,
    value_thunk: u32,
    thrower: u32,
}

/** The closure natives. */
static NATIVES: std::sync::OnceLock<Natives> = std::sync::OnceLock::new();

/** The installed closure natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("promise combinator natives installed")
}

/** Which aggregate a combinator builds. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Combinator {
    All,
    AllSettled,
    Any,
    Race,
}

/** Promise's statics and `then`, `finally` on its prototype. */
pub(super) fn install(b: &mut Builder, ctor: ObjectId, proto: ObjectId) {
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
    b.method(ctor, "race", 1, |vm, call| {
        combinator(vm, call, Combinator::Race)
    });
    b.method(ctor, "withResolvers", 0, with_resolvers);
    b.method(proto, "then", 2, then);
    b.method(proto, "finally", 1, finally);
    let _ = NATIVES.set(Natives {
        executor: b.native("", 2, executor),
        all_element: b.native("", 1, all_element),
        settled_element: b.native("", 1, settled_element),
        any_element: b.native("", 1, any_element),
        reaction: b.native("", 1, reaction),
        then_finally: b.native("", 1, then_finally),
        catch_finally: b.native("", 1, catch_finally),
        value_thunk: b.native("", 0, value_thunk),
        thrower: b.native("", 0, thrower),
    });
}

/** A PromiseCapability record. */
struct Capability {
    promise: JsValue,
    resolve: JsValue,
    reject: JsValue,
}

/** NewPromiseCapability(C): construct C with an executor that captures resolve and reject,
and refuse a C that is not a constructor or whose executor got non-callables. */
fn new_capability(vm: &mut Vm, c: &JsValue) -> Res<Capability> {
    if !vm.is_constructor(c) {
        return Err(vm.type_error("Promise capability constructor is not a constructor"));
    }
    let record = vm.new_object()?;
    vm.pin(JsValue::Object(record));
    let executor = vm.native_closure(natives().executor, vec![JsValue::Object(record)])?;
    let promise = vm.construct(c.clone(), vec![JsValue::Object(executor)], None)?;
    let resolve = vm.get(&JsValue::Object(record), &key("resolve"))?;
    let reject = vm.get(&JsValue::Object(record), &key("reject"))?;
    if !vm.is_callable(&resolve) || !vm.is_callable(&reject) {
        return Err(vm.type_error("Promise resolve or reject function is not callable"));
    }
    Ok(Capability {
        promise,
        resolve,
        reject,
    })
}

/** GetCapabilitiesExecutor: store resolve and reject once each. */
fn executor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let record = vm
        .slots(call.callee)
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined);
    let JsValue::Object(id) = record else {
        return Ok(JsValue::Undefined);
    };
    for name in ["resolve", "reject"] {
        if vm.get(&record, &key(name))? != JsValue::Undefined {
            return Err(vm.type_error("Promise executor has already been invoked"));
        }
    }
    vm.put(id, "resolve", call.arg(0))?;
    vm.put(id, "reject", call.arg(1))?;
    Ok(JsValue::Undefined)
}

/** IsPromise. */
fn is_promise(vm: &Vm, value: &JsValue) -> bool {
    matches!(value, JsValue::Object(id)
        if matches!(vm.heap.get(*id).map(|o| &o.class), Some(Class::Promise(_))))
}

/** The %Promise% intrinsic. */
fn intrinsic() -> JsValue {
    JsValue::Object(realm().ids.promise)
}

/** SpeciesConstructor(O, %Promise%). */
fn species_constructor(vm: &mut Vm, object: &JsValue) -> Res<JsValue> {
    let ctor = vm.get(object, &key("constructor"))?;
    if ctor == JsValue::Undefined {
        return Ok(intrinsic());
    }
    if !matches!(ctor, JsValue::Object(_)) {
        return Err(vm.type_error("The promise's constructor is not an object"));
    }
    let species = vm.get(&ctor, &Key::Sym(symbol::SPECIES))?;
    if species.nullish() {
        return Ok(intrinsic());
    }
    if vm.is_constructor(&species) {
        return Ok(species);
    }
    Err(vm.type_error("The promise's [Symbol.species] is not a constructor"))
}

/** PromiseResolve(C, x): a promise built by C passes through, anything else is wrapped. */
fn promise_resolve(vm: &mut Vm, c: &JsValue, value: JsValue) -> Res<JsValue> {
    if is_promise(vm, &value) {
        let ctor = vm.get(&value, &key("constructor"))?;
        if ctor == *c {
            return Ok(value);
        }
    }
    let cap = new_capability(vm, c)?;
    vm.call(cap.resolve, JsValue::Undefined, vec![value])?;
    Ok(cap.promise)
}

/** `this` as the constructor a static runs on. */
fn this_constructor(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    if matches!(call.this, JsValue::Object(_)) {
        return Ok(call.this.clone());
    }
    Err(vm.type_error("Promise static method called on a non-object"))
}

/** Promise.resolve. */
fn resolve_static(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let c = this_constructor(vm, &call)?;
    promise_resolve(vm, &c, call.arg(0))
}

/** Promise.reject. */
fn reject_static(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let c = this_constructor(vm, &call)?;
    let cap = new_capability(vm, &c)?;
    vm.call(cap.reject, JsValue::Undefined, vec![call.arg(0)])?;
    Ok(cap.promise)
}

/** Promise.withResolvers. */
fn with_resolvers(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let c = this_constructor(vm, &call)?;
    let cap = new_capability(vm, &c)?;
    let result = vm.new_object()?;
    vm.put(result, "promise", cap.promise)?;
    vm.put(result, "resolve", cap.resolve)?;
    vm.put(result, "reject", cap.reject)?;
    Ok(JsValue::Object(result))
}

/** Invoke(value, "then", args). */
fn invoke_then(vm: &mut Vm, value: &JsValue, args: Vec<JsValue>) -> Res<JsValue> {
    let method = vm.get(value, &key("then"))?;
    vm.call(method, value.clone(), args)
}

/** IfAbruptRejectPromise: a thrown value rejects the capability instead of escaping. */
fn reject_abrupt(vm: &mut Vm, cap: &Capability, result: Res<JsValue>) -> Res<JsValue> {
    match result {
        Err(Thrown::Value(error)) => {
            vm.call(cap.reject.clone(), JsValue::Undefined, vec![error])?;
            Ok(cap.promise.clone())
        }
        other => other,
    }
}

/** Promise.all, allSettled, any and race: the shared prologue and the iterator-close rule. */
fn combinator(vm: &mut Vm, call: Call, kind: Combinator) -> Res<JsValue> {
    let c = call.this.clone();
    if !matches!(c, JsValue::Object(_)) {
        let name = match kind {
            Combinator::All => "all",
            Combinator::AllSettled => "allSettled",
            Combinator::Any => "any",
            Combinator::Race => "race",
        };
        return Err(vm.type_error(&format!("Promise.{name} called on non-object")));
    }
    let cap = new_capability(vm, &c)?;
    vm.pin(cap.promise.clone());
    let resolve = vm.get(&c, &key("resolve"));
    let resolve = match resolve {
        Ok(f) if vm.is_callable(&f) => Ok(f),
        Ok(_) => Err(vm.type_error("Promise resolve is not a function")),
        Err(e) => Err(e),
    };
    let resolve = match resolve {
        Ok(f) => f,
        Err(e) => return reject_abrupt(vm, &cap, Err(e)),
    };
    let iterator = match vm.get_iterator(&call.arg(0)) {
        Ok(id) => id,
        Err(e) => return reject_abrupt(vm, &cap, Err(e)),
    };
    vm.pin(JsValue::Object(iterator));
    let mut done = false;
    let result = perform(vm, &c, &cap, &resolve, iterator, &mut done, kind);
    if result.is_err() && !done {
        match vm.iter_close(iterator) {
            Err(Thrown::Value(_)) | Ok(()) => {}
            Err(fatal) => return Err(fatal),
        }
    }
    reject_abrupt(vm, &cap, result)
}

/** The shared state the element functions of one combinator call write to. */
struct Shared {
    values: JsValue,
    remaining: JsValue,
}

/** Add to the remaining-elements count and return the new count. */
fn count(vm: &mut Vm, remaining: &JsValue, delta: f64) -> Res<f64> {
    let current = vm.get(remaining, &key("n"))?;
    let next = vm.to_number(&current)? + delta;
    if let JsValue::Object(id) = remaining {
        vm.put(*id, "n", JsValue::Number(next))?;
    }
    Ok(next)
}

/** PerformPromiseAll and its siblings: step, resolve each value through C.resolve, then. */
fn perform(
    vm: &mut Vm,
    c: &JsValue,
    cap: &Capability,
    resolve: &JsValue,
    iterator: ObjectId,
    done: &mut bool,
    kind: Combinator,
) -> Res<JsValue> {
    let values = JsValue::Object(vm.new_array(Vec::new())?);
    vm.pin(values.clone());
    let remaining = vm.new_object()?;
    vm.put(remaining, "n", JsValue::Number(1.0))?;
    let shared = Shared {
        values,
        remaining: JsValue::Object(remaining),
    };
    let mut index = 0usize;
    loop {
        *done = true;
        let Some(next) = vm.iter_step(iterator)? else {
            return finish(vm, cap, &shared, kind);
        };
        *done = false;
        if kind != Combinator::Race {
            vm.array_append(&shared.values, JsValue::Undefined)?;
        }
        let next_promise = vm.call(resolve.clone(), c.clone(), vec![next])?;
        let handlers = element_handlers(vm, cap, &shared, index, kind)?;
        if kind != Combinator::Race {
            count(vm, &shared.remaining, 1.0)?;
        }
        invoke_then(vm, &next_promise, handlers)?;
        index += 1;
    }
}

/** When the iterator ends: settle the aggregate if every element already arrived. */
fn finish(vm: &mut Vm, cap: &Capability, shared: &Shared, kind: Combinator) -> Res<JsValue> {
    if kind == Combinator::Race || count(vm, &shared.remaining, -1.0)? != 0.0 {
        return Ok(cap.promise.clone());
    }
    let values = shared.values.clone();
    if kind == Combinator::Any {
        let error = aggregate_error(vm, values)?;
        vm.call(cap.reject.clone(), JsValue::Undefined, vec![error])?;
    } else {
        vm.call(cap.resolve.clone(), JsValue::Undefined, vec![values])?;
    }
    Ok(cap.promise.clone())
}

/** The [onFulfilled, onRejected] pair one element gets. */
fn element_handlers(
    vm: &mut Vm,
    cap: &Capability,
    shared: &Shared,
    index: usize,
    kind: Combinator,
) -> Res<Vec<JsValue>> {
    let called = JsValue::Object(vm.new_object()?);
    let slots = |settle: &JsValue| {
        vec![
            JsValue::Number(index as f64),
            shared.values.clone(),
            settle.clone(),
            shared.remaining.clone(),
            called.clone(),
        ]
    };
    let n = natives();
    Ok(match kind {
        Combinator::Race => vec![cap.resolve.clone(), cap.reject.clone()],
        Combinator::All => vec![
            JsValue::Object(vm.native_closure(n.all_element, slots(&cap.resolve))?),
            cap.reject.clone(),
        ],
        Combinator::AllSettled => {
            let mut rejected = slots(&cap.resolve);
            rejected.push(JsValue::Boolean(true));
            vec![
                JsValue::Object(vm.native_closure(n.settled_element, slots(&cap.resolve))?),
                JsValue::Object(vm.native_closure(n.settled_element, rejected)?),
            ]
        }
        Combinator::Any => vec![
            cap.resolve.clone(),
            JsValue::Object(vm.native_closure(n.any_element, slots(&cap.reject))?),
        ],
    })
}

/** An element function's work: once only, store the value, settle when the count hits zero. */
fn store(
    vm: &mut Vm,
    call: &Call,
    value: impl FnOnce(&mut Vm) -> Res<JsValue>,
    any: bool,
) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let [
        JsValue::Number(index),
        values,
        settle,
        remaining,
        called,
        ..,
    ] = slots.as_slice()
    else {
        return Ok(JsValue::Undefined);
    };
    if vm.get(called, &key("done"))?.truthy() {
        return Ok(JsValue::Undefined);
    }
    if let JsValue::Object(flag) = called {
        vm.put(*flag, "done", JsValue::Boolean(true))?;
    }
    let value = value(vm)?;
    if let JsValue::Object(id) = values {
        vm.array_store(*id, *index as usize, value);
    }
    if count(vm, remaining, -1.0)? != 0.0 {
        return Ok(JsValue::Undefined);
    }
    let result = if any {
        aggregate_error(vm, values.clone())?
    } else {
        values.clone()
    };
    vm.call(settle.clone(), JsValue::Undefined, vec![result])
}

/** Promise.all resolve element function. */
fn all_element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    store(vm, &call, |_| Ok(value), false)
}

/** Promise.allSettled element function (slot 5 marks the rejection branch). */
fn settled_element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let rejected = vm.slots(call.callee).get(5).is_some_and(JsValue::truthy);
    let value = call.arg(0);
    store(
        vm,
        &call,
        |vm| {
            let record = vm.new_object()?;
            let status = text(vm, if rejected { "rejected" } else { "fulfilled" })?;
            vm.put(record, "status", status)?;
            vm.put(record, if rejected { "reason" } else { "value" }, value)?;
            Ok(JsValue::Object(record))
        },
        false,
    )
}

/** Promise.any reject element function. */
fn any_element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    store(vm, &call, |_| Ok(value), true)
}

/** A new AggregateError whose own `errors` is the list. */
fn aggregate_error(vm: &mut Vm, errors: JsValue) -> Res<JsValue> {
    let error = vm.make_error(
        crate::v9::vm::error::ErrorKind::Error,
        "All promises were rejected",
    )?;
    vm.set_proto(error, Some(realm().ids.aggregate_error_prototype))?;
    vm.define(
        error,
        key("errors"),
        Descriptor {
            value: Some(errors),
            writable: Some(true),
            enumerable: Some(false),
            configurable: Some(true),
            ..Descriptor::default()
        },
    )?;
    Ok(JsValue::Object(error))
}

/** The promise a receiver is. */
fn this_promise(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    match call.this {
        JsValue::Object(id) if is_promise(vm, &call.this) => Ok(id),
        _ => Err(vm.type_error("Method Promise.prototype.then called on incompatible receiver")),
    }
}

/** A handler slot: the function, or undefined for the pass-through. */
fn handler(vm: &Vm, value: JsValue) -> JsValue {
    if vm.is_callable(&value) {
        value
    } else {
        JsValue::Undefined
    }
}

/** Promise.prototype.then: PerformPromiseThen into a capability of the species constructor.
%Promise% keeps the direct reaction target; any other constructor gets reaction closures that
call its capability's resolve and reject. */
fn then(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = this_promise(vm, &call)?;
    let c = species_constructor(vm, &call.this)?;
    let (fulfilled, rejected) = (handler(vm, call.arg(0)), handler(vm, call.arg(1)));
    if c == intrinsic() {
        let target = vm.new_promise_parent(Some(promise))?;
        vm.perform_then(
            promise,
            ReactionHandler::Function(fulfilled),
            ReactionHandler::Function(rejected),
            Some(target),
        )?;
        return Ok(JsValue::Object(target));
    }
    let cap = new_capability(vm, &c)?;
    let wrap = |vm: &mut Vm, f: JsValue, reject: bool| -> Res<ReactionHandler> {
        let slots = vec![
            f,
            cap.resolve.clone(),
            cap.reject.clone(),
            JsValue::Boolean(reject),
        ];
        let closure = vm.native_closure(natives().reaction, slots)?;
        #[cfg(feature = "conformance")]
        if vm.page.environment.conformance_async_hooks && is_promise(vm, &cap.promise) {
            vm.heap.put(
                closure,
                Key::private("async-hook-resource"),
                crate::v9::vm::heap::Prop::data(cap.promise.clone()),
            );
        }
        Ok(ReactionHandler::Function(JsValue::Object(closure)))
    };
    let on_fulfilled = wrap(vm, fulfilled, false)?;
    let on_rejected = wrap(vm, rejected, true)?;
    vm.perform_then(promise, on_fulfilled, on_rejected, None)?;
    Ok(cap.promise)
}

/** NewPromiseReactionJob for a capability that is not a %Promise%: run the handler, then hand
its outcome to the capability. */
fn reaction(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let [f, resolve, reject, JsValue::Boolean(is_reject)] = slots.as_slice() else {
        return Ok(JsValue::Undefined);
    };
    let argument = call.arg(0);
    let outcome = if vm.is_callable(f) {
        vm.call(f.clone(), JsValue::Undefined, vec![argument])
    } else if *is_reject {
        Err(Thrown::Value(argument))
    } else {
        Ok(argument)
    };
    match outcome {
        Ok(value) => vm.call(resolve.clone(), JsValue::Undefined, vec![value]),
        Err(Thrown::Value(error)) => vm.call(reject.clone(), JsValue::Undefined, vec![error]),
        Err(fatal) => Err(fatal),
    }
}

/** Promise.prototype.finally: thenFinally and catchFinally over the species constructor. */
fn finally(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Err(vm.type_error("Promise.prototype.finally called on a non-object"));
    }
    let c = species_constructor(vm, &call.this)?;
    let on_finally = call.arg(0);
    if !vm.is_callable(&on_finally) {
        return invoke_then(vm, &call.this, vec![on_finally.clone(), on_finally]);
    }
    let slots = vec![on_finally, c];
    let then_f = vm.native_closure(natives().then_finally, slots.clone())?;
    let catch_f = vm.native_closure(natives().catch_finally, slots)?;
    invoke_then(
        vm,
        &call.this,
        vec![JsValue::Object(then_f), JsValue::Object(catch_f)],
    )
}

/** Shared body of thenFinally and catchFinally: call onFinally, wait for its result, then
return a thunk that yields (or throws) the original settlement. */
fn after_finally(vm: &mut Vm, call: &Call, thunk: u32) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let [on_finally, c] = slots.as_slice() else {
        return Ok(JsValue::Undefined);
    };
    let result = vm.call(on_finally.clone(), JsValue::Undefined, Vec::new())?;
    let promise = promise_resolve(vm, c, result)?;
    let thunk = vm.native_closure(thunk, vec![call.arg(0)])?;
    invoke_then(vm, &promise, vec![JsValue::Object(thunk)])
}

/** thenFinally. */
fn then_finally(vm: &mut Vm, call: Call) -> Res<JsValue> {
    after_finally(vm, &call, natives().value_thunk)
}

/** catchFinally. */
fn catch_finally(vm: &mut Vm, call: Call) -> Res<JsValue> {
    after_finally(vm, &call, natives().thrower)
}

/** Returns the captured value. */
fn value_thunk(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(vm
        .slots(call.callee)
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined))
}

/** Throws the captured reason. */
fn thrower(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let reason = vm
        .slots(call.callee)
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined);
    Err(Thrown::Value(reason))
}
