/** Resource disposal uses private captured records shared by using declarations and public stack objects. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Class, PromiseState, ReactionHandler};
use crate::v9::vm::heap::{Object, Prop, Slot};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{Static, string, symbol};
use std::sync::OnceLock;
mod drain;
mod public;
mod state;
mod suppressed;

/** Call mode records distinguish receiver disposal, adopted value arguments and deferred no-argument calls. */
const ASYNC_FLAG: u32 = 1;
/** Adoption passes the resource as an argument instead of as this. */
const ARGUMENT_FLAG: u32 = 2;
/** Deferred callbacks receive neither a resource receiver nor an argument. */
const DEFER_FLAG: u32 = 4;

/** Continuation identities are registered once and translated into each callback's creation realm. */
struct Natives {
    fulfilled: u32,
    rejected: u32,
    fallback: u32,
}
/** Native closures retain the private state through GC and state transport. */
static NATIVES: OnceLock<Natives> = OnceLock::new();

/** Public disposal objects and compiler-only operations use the same native implementation. */
pub(super) fn install(builder: &mut Builder) {
    suppressed::install(builder);
    public::install(builder);
    let _ = NATIVES.set(Natives {
        fulfilled: builder.native("", 1, drain::fulfilled),
        rejected: builder.native("", 1, drain::rejected),
        fallback: builder.native("", 0, sync_fallback),
    });
}

/** Compiler-created stacks have no public surface or author-accessible constructor dependency. */
pub(in crate::v9) fn create(vm: &mut Vm) -> Res<JsValue> {
    state::create(vm).map(JsValue::Object)
}

/** Initializers capture GetMethod eagerly and return the original value for lexical binding initialization. */
pub(in crate::v9) fn add(
    vm: &mut Vm,
    stack: JsValue,
    value: JsValue,
    is_async: bool,
) -> Res<JsValue> {
    let id = state::identity(vm, &stack)?;
    state::active(vm, id)?;
    if value.nullish() {
        if is_async {
            state::append(vm, id, JsValue::Undefined, JsValue::Undefined, ASYNC_FLAG)?;
        }
        return Ok(value);
    }
    if !matches!(value, JsValue::Object(_)) {
        return Err(vm.type_error("An object is expected with using declarations"));
    }
    let method = dispose_method(vm, &value, is_async)?;
    state::append(
        vm,
        id,
        value.clone(),
        method,
        if is_async { ASYNC_FLAG } else { 0 },
    )?;
    Ok(value)
}

/** A body throw is remembered only to suppress it if a later disposer itself fails. */
pub(in crate::v9) fn error(vm: &mut Vm, stack: JsValue, error: JsValue) -> Res<()> {
    let id = state::identity(vm, &stack)?;
    state::put(vm, id, "error", error);
    state::put(vm, id, "has-error", JsValue::Boolean(true));
    Ok(())
}

/** Synchronous finalizers throw immediately; asynchronous finalizers return a real promise over cleanup jobs. */
pub(in crate::v9) fn dispose(vm: &mut Vm, stack: JsValue, is_async: bool) -> Res<JsValue> {
    let id = state::identity(vm, &stack)?;
    if is_async {
        let promise = vm.new_promise()?;
        if state::boolean(vm, id, "disposed")? {
            vm.resolve_promise(promise, JsValue::Undefined)?;
        } else {
            state::put(vm, id, "disposed", JsValue::Boolean(true));
            drain::asynchronous(vm, id, promise)?;
        }
        return Ok(JsValue::Object(promise));
    }
    if state::boolean(vm, id, "disposed")? {
        return Ok(JsValue::Undefined);
    }
    state::put(vm, id, "disposed", JsValue::Boolean(true));
    drain::synchronous(vm, id)?;
    Ok(JsValue::Undefined)
}

/** Async disposal falls back only when the async method is nullish and wraps the synchronous method. */
fn dispose_method(vm: &mut Vm, value: &JsValue, is_async: bool) -> Res<JsValue> {
    let mut method = vm.get(
        value,
        &Key::Sym(if is_async {
            symbol::ASYNC_DISPOSE
        } else {
            symbol::DISPOSE
        }),
    )?;
    if is_async && method.nullish() {
        method = vm.get(value, &Key::Sym(symbol::DISPOSE))?;
        if !vm.is_callable(&method) {
            return Err(vm.type_error("Symbol.dispose is not callable"));
        }
        method = JsValue::Object(vm.native_closure(
            NATIVES.get().expect("disposal installed").fallback,
            vec![method],
        )?);
    }
    if !vm.is_callable(&method) {
        return Err(vm.type_error("Dispose method is not callable"));
    }
    Ok(method)
}

/** The async-from-sync adapter ignores a returned thenable and turns synchronous throws into promise rejection. */
fn sync_fallback(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let method = vm
        .slots(call.callee)
        .first()
        .cloned()
        .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid disposal adapter."))?;
    let promise = vm.new_promise()?;
    match vm.call(method, call.this, Vec::new()) {
        Ok(_) => vm.resolve_promise(promise, JsValue::Undefined)?,
        Err(Thrown::Value(error)) => vm.settle(promise, PromiseState::Rejected(error))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(promise))
}

/** Compiler cleanup starts once, before any resource callback can reenter its capability. */
pub(in crate::v9) fn begin(vm: &mut Vm, stack: JsValue) -> Res<()> {
    let id = state::identity(vm, &stack)?;
    state::put(vm, id, "disposed", JsValue::Boolean(true));
    Ok(())
}
/** Compiled async finally awaits individual disposer promises without introducing an outer result promise. */
pub(in crate::v9) fn step(vm: &mut Vm, stack: JsValue) -> Res<Option<JsValue>> {
    let id = state::identity(vm, &stack)?;
    drain::step(vm, id)
}
/** A rejected awaited disposer combines with pending body and earlier disposal exceptions. */
pub(in crate::v9) fn rejected(vm: &mut Vm, stack: JsValue, error: JsValue) -> Res<()> {
    let id = state::identity(vm, &stack)?;
    state::failure(vm, id, error)
}
