/** Iterator disposal forwards an optional return method and async disposal awaits it without consulting Promise species. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{PromiseState, ReactionHandler};
use std::sync::OnceLock;

/** A fulfilled async return is intentionally discarded by the specification's continuation. */
static IGNORE: OnceLock<u32> = OnceLock::new();

/** Sync and async iterator prototypes each expose their corresponding well-known disposal method. */
pub(super) fn install(builder: &mut Builder) {
    builder.symbol_method(
        builder.ids.iterator_prototype,
        symbol::DISPOSE,
        "[Symbol.dispose]",
        0,
        synchronous,
    );
    builder.symbol_method(
        builder.ids.async_iterator_prototype,
        symbol::ASYNC_DISPOSE,
        "[Symbol.asyncDispose]",
        0,
        asynchronous,
    );
    let _ = IGNORE.set(builder.native("", 1, ignore));
}

/** Sync disposal discards a return value without imposing IteratorClose's object-result requirement. */
fn synchronous(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let method = vm.get(&call.this, &Key::from("return"))?;
    if !method.nullish() {
        vm.call(method, call.this, Vec::new())?;
    }
    Ok(JsValue::Undefined)
}

/** Every script failure becomes an outer rejection; cancellation and other native failures remain fatal. */
fn asynchronous(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let outer = vm.new_promise()?;
    match start(vm, &call, outer) {
        Ok(()) => {}
        Err(Thrown::Value(error)) => vm.settle(outer, PromiseState::Rejected(error))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(outer))
}

/** Calling an intrinsic PerformPromiseThen preserves behavior even when Promise[Symbol.species] throws. */
fn start(vm: &mut Vm, call: &Call, outer: ObjectId) -> Res<()> {
    let method = vm.get(&call.this, &Key::from("return"))?;
    if method.nullish() {
        return vm.resolve_promise(outer, JsValue::Undefined);
    }
    let value = vm.call(method, call.this.clone(), Vec::new())?;
    let promise = vm.promise_resolve(value)?;
    let continuation = JsValue::Object(vm.native_closure(
        *IGNORE.get().expect("iterator disposal installed"),
        Vec::new(),
    )?);
    vm.perform_then(
        promise,
        ReactionHandler::Function(continuation),
        ReactionHandler::Function(JsValue::Undefined),
        Some(outer),
    )
}

/** Awaiting return never exposes its fulfillment value as the async disposal result. */
fn ignore(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Undefined)
}
