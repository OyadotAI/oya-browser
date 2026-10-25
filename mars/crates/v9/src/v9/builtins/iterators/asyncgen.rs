/** %AsyncIteratorPrototype%, async generator objects and functions, the GeneratorFunction-family
constructors, and %AsyncFromSyncIteratorPrototype% (spec 27.1.4, 27.3, 27.4, 27.6, 27.7). */
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, IterState, PromiseState, ReactionHandler};
use crate::v9::vm::heap::{CONFIGURABLE, Key};
use crate::v9::vm::realm::{Builder, NativeFn, Static, string, symbol};
use crate::v9::vm::{Call, Res, Resume, Thrown, Vm};

/** Native slots captured by AsyncFromSyncIteratorContinuation's closures. */
struct Natives {
    unwrap: u32,
    close: u32,
}

/** Native indexes captured by closures, set once at install. */
static NATIVES: std::sync::OnceLock<Natives> = std::sync::OnceLock::new();

/** The continuation natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("async iteration natives installed")
}

/** Install the async iteration intrinsics once Function and the generator prototypes exist. */
pub(super) fn install(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let async_iterator = b.object(Some(object_prototype));
    b.ids.async_iterator_prototype = async_iterator;
    b.symbol_method(
        async_iterator,
        symbol::ASYNC_ITERATOR,
        "[Symbol.asyncIterator]",
        0,
        super::identity,
    );
    let generator = b.object(Some(async_iterator));
    b.ids.async_generator_prototype = generator;
    b.method(generator, "next", 1, next);
    b.method(generator, "return", 1, return_method);
    b.method(generator, "throw", 1, throw);
    tag(b, generator, "AsyncGenerator");
    let from_sync = b.object(Some(async_iterator));
    b.ids.async_from_sync_iterator_prototype = from_sync;
    b.method(from_sync, "next", 1, from_sync_next);
    b.method(from_sync, "return", 1, from_sync_return);
    b.method(from_sync, "throw", 1, from_sync_throw);
    let _ = NATIVES.set(Natives {
        unwrap: b.native("", 1, unwrap_fulfilled),
        close: b.native("", 1, close_rejected),
    });
    constructors(b, generator);
}

/** A configurable Symbol.toStringTag. */
fn tag(b: &mut Builder, target: ObjectId, name: &str) {
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        CONFIGURABLE,
    );
}

/** GeneratorFunction, AsyncGeneratorFunction and AsyncFunction: hidden constructors that inherit
from Function and compile their kind of function. */
fn constructors(b: &mut Builder, async_generator: ObjectId) {
    let function = b
        .globals
        .iter()
        .find(|(name, _)| *name == "Function")
        .and_then(|(_, value)| value.object());
    let async_function = b.object(Some(b.ids.function_prototype));
    b.ids.async_generator_function_prototype = async_function;
    b.value(
        async_function,
        "prototype",
        JsValue::Object(async_generator),
        CONFIGURABLE,
    );
    b.value(
        async_generator,
        "constructor",
        JsValue::Object(async_function),
        CONFIGURABLE,
    );
    tag(b, async_function, "AsyncGeneratorFunction");
    let kinds: [(&'static str, ObjectId, NativeFn); 3] = [
        (
            "AsyncGeneratorFunction",
            async_function,
            async_generator_function,
        ),
        (
            "GeneratorFunction",
            b.ids.generator_function_prototype,
            generator_function,
        ),
        (
            "AsyncFunction",
            b.ids.async_function_prototype,
            async_function_constructor,
        ),
    ];
    for (name, prototype, construct) in kinds {
        let ctor = b.constructor(name, 1, construct, prototype, function);
        b.value(
            prototype,
            "constructor",
            JsValue::Object(ctor),
            CONFIGURABLE,
        );
    }
    tag(b, b.ids.generator_function_prototype, "GeneratorFunction");
}

/** AsyncGeneratorFunction(params..., body). */
fn async_generator_function(vm: &mut Vm, call: Call) -> Res<JsValue> {
    crate::v9::builtins::function::create_dynamic(vm, call, "async function*")
}

/** GeneratorFunction(params..., body). */
fn generator_function(vm: &mut Vm, call: Call) -> Res<JsValue> {
    crate::v9::builtins::function::create_dynamic(vm, call, "function*")
}

/** AsyncFunction(params..., body). */
fn async_function_constructor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    crate::v9::builtins::function::create_dynamic(vm, call, "async function")
}

/** AsyncGenerator.prototype.next. */
fn next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.async_generator_enqueue(call.this.clone(), Resume::Next, call.arg(0))
}

/** AsyncGenerator.prototype.return. */
fn return_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.async_generator_enqueue(call.this.clone(), Resume::Return, call.arg(0))
}

/** AsyncGenerator.prototype.throw. */
fn throw(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.async_generator_enqueue(call.this.clone(), Resume::Throw, call.arg(0))
}

/** The sync iterator an AsyncFromSyncIterator wraps, and its next method. */
fn sync_record(vm: &Vm, this: &JsValue) -> Option<(JsValue, JsValue)> {
    match vm.heap.get(this.object()?).map(|object| &object.class) {
        Some(Class::Iter(state)) => match state.as_ref() {
            IterState::Record { iterator, next, .. } => Some((iterator.clone(), next.clone())),
            _ => None,
        },
        _ => None,
    }
}

/** A fresh promise for an AsyncFromSyncIterator method, pinned while the method runs. */
fn capability(vm: &mut Vm) -> Res<ObjectId> {
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    Ok(promise)
}

/** IfAbruptRejectPromise: a thrown error rejects the promise, a fatal one propagates. */
fn reject(vm: &mut Vm, promise: ObjectId, error: Thrown) -> Res<JsValue> {
    match error {
        Thrown::Value(reason) => {
            vm.settle(promise, PromiseState::Rejected(reason))?;
            Ok(JsValue::Object(promise))
        }
        fatal => Err(fatal),
    }
}

/** The argument list when the method was given a value. */
fn passed(call: &Call) -> Vec<JsValue> {
    call.args.first().cloned().into_iter().collect()
}

/** Call a sync iterator method and require an object result. */
fn step(vm: &mut Vm, method: JsValue, iterator: JsValue, args: Vec<JsValue>) -> Res<JsValue> {
    let result = vm.call(method, iterator, args)?;
    if !matches!(result, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator result is not an object"));
    }
    Ok(result)
}

/** %AsyncFromSyncIteratorPrototype%.next. */
fn from_sync_next(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = capability(vm)?;
    let Some((iterator, next)) = sync_record(vm, &call.this) else {
        let error = vm.type_error("not an AsyncFromSyncIterator");
        return reject(vm, promise, error);
    };
    match step(vm, next, iterator.clone(), passed(&call)) {
        Ok(result) => continuation(vm, result, promise, iterator, true),
        Err(error) => reject(vm, promise, error),
    }
}

/** %AsyncFromSyncIteratorPrototype%.return. */
fn from_sync_return(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = capability(vm)?;
    let Some((iterator, _)) = sync_record(vm, &call.this) else {
        let error = vm.type_error("not an AsyncFromSyncIterator");
        return reject(vm, promise, error);
    };
    let method = match vm.get_method(&iterator, &Key::from("return")) {
        Ok(method) => method,
        Err(error) => return reject(vm, promise, error),
    };
    let Some(method) = method else {
        let result = vm.iter_result(call.arg(0), true)?;
        vm.resolve_promise(promise, result)?;
        return Ok(JsValue::Object(promise));
    };
    match step(vm, method, iterator.clone(), passed(&call)) {
        Ok(result) => continuation(vm, result, promise, iterator, false),
        Err(error) => reject(vm, promise, error),
    }
}

/** %AsyncFromSyncIteratorPrototype%.throw; without a sync `throw` the iterator is closed and the
promise rejects with a TypeError. */
fn from_sync_throw(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = capability(vm)?;
    let Some((iterator, _)) = sync_record(vm, &call.this) else {
        let error = vm.type_error("not an AsyncFromSyncIterator");
        return reject(vm, promise, error);
    };
    let method = match vm.get_method(&iterator, &Key::from("throw")) {
        Ok(method) => method,
        Err(error) => return reject(vm, promise, error),
    };
    let Some(method) = method else {
        if let Err(error) = close_normally(vm, &iterator) {
            return reject(vm, promise, error);
        }
        let error = vm.type_error("The iterator does not provide a 'throw' method");
        return reject(vm, promise, error);
    };
    match step(vm, method, iterator.clone(), passed(&call)) {
        Ok(result) => continuation(vm, result, promise, iterator, true),
        Err(error) => reject(vm, promise, error),
    }
}

/** IteratorClose with a normal completion: its errors and a non-object result count. */
fn close_normally(vm: &mut Vm, iterator: &JsValue) -> Res<()> {
    if let Some(method) = vm.get_method(iterator, &Key::from("return"))? {
        step(vm, method, iterator.clone(), Vec::new())?;
    }
    Ok(())
}

/** IteratorClose with a throw completion: the close is attempted and the error stays. */
fn close_quietly(vm: &mut Vm, iterator: &JsValue) -> Res<()> {
    let method = match vm.get_method(iterator, &Key::from("return")) {
        Ok(Some(method)) => method,
        Ok(None) | Err(Thrown::Value(_)) => return Ok(()),
        Err(fatal) => return Err(fatal),
    };
    match vm.call(method, iterator.clone(), Vec::new()) {
        Ok(_) | Err(Thrown::Value(_)) => Ok(()),
        Err(fatal) => Err(fatal),
    }
}

/** AsyncFromSyncIteratorContinuation: await the result's value, then settle with an iterator
result; a rejected value closes the sync iterator unless it was done. */
fn continuation(
    vm: &mut Vm,
    result: JsValue,
    promise: ObjectId,
    iterator: JsValue,
    close_on_rejection: bool,
) -> Res<JsValue> {
    let done = match vm.get(&result, &Key::from("done")) {
        Ok(done) => done.truthy(),
        Err(error) => return reject(vm, promise, error),
    };
    let value = match vm.get(&result, &Key::from("value")) {
        Ok(value) => value,
        Err(error) => return reject(vm, promise, error),
    };
    let close = !done && close_on_rejection;
    let wrapper = match vm.promise_resolve(value) {
        Ok(wrapper) => wrapper,
        Err(error) => {
            if close && matches!(error, Thrown::Value(_)) {
                close_quietly(vm, &iterator)?;
            }
            return reject(vm, promise, error);
        }
    };
    let fulfilled = vm.native_closure(natives().unwrap, vec![JsValue::Boolean(done)])?;
    vm.pin(JsValue::Object(fulfilled));
    let rejected = if close {
        let closer = vm.native_closure(natives().close, vec![iterator])?;
        vm.pin(JsValue::Object(closer));
        JsValue::Object(closer)
    } else {
        JsValue::Undefined
    };
    vm.perform_then(
        wrapper,
        ReactionHandler::Function(JsValue::Object(fulfilled)),
        ReactionHandler::Function(rejected),
        Some(promise),
    )?;
    Ok(JsValue::Object(promise))
}

/** The fulfilled step of a continuation: an iterator result with the captured done. */
fn unwrap_fulfilled(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let done = vm.slots(call.callee).first().is_some_and(JsValue::truthy);
    vm.iter_result(call.arg(0), done)
}

/** The rejected step of a continuation: close the sync iterator and keep the rejection. */
fn close_rejected(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let iterator = vm
        .slots(call.callee)
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined);
    close_quietly(vm, &iterator)?;
    Err(Thrown::Value(call.arg(0)))
}
