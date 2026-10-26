/** Lifecycle entrypoints tag actual promises and bracket actual microtasks, retaining no synthetic callback resources. */
use super::*;
use crate::v9::vm::data::{Job, ReactionHandler};

/** Init callbacks observe the real pending Promise before its executor or resolving functions run. */
pub(in crate::v9) fn init(vm: &mut Vm, promise: ObjectId, parent: Option<ObjectId>) -> Res<()> {
    if !vm.page.environment.conformance_async_hooks {
        return Ok(());
    }
    let id = state::next(vm)?;
    let trigger = match parent {
        Some(parent) => state::ids(vm, parent)?.0,
        None => 0,
    };
    state::put(vm, promise, "async-hook-id", JsValue::Number(id as f64));
    state::put(
        vm,
        promise,
        "async-hook-trigger",
        JsValue::Number(trigger as f64),
    );
    state::put(vm, promise, "async-hook-resolved", JsValue::Boolean(false));
    dispatch(
        vm,
        "init",
        &[
            JsValue::Number(id as f64),
            JsValue::string("PROMISE")?,
            JsValue::Number(trigger as f64),
            JsValue::Object(promise),
        ],
    )
}
/** Resolve hooks run once at the first real resolution attempt, including adoption of a pending thenable. */
pub(in crate::v9) fn resolve(vm: &mut Vm, promise: ObjectId) -> Res<()> {
    if !vm.page.environment.conformance_async_hooks {
        return Ok(());
    }
    let (id, _) = state::ids(vm, promise)?;
    match state::get(vm, promise, "async-hook-resolved") {
        Some(JsValue::Boolean(true)) => return Ok(()),
        Some(JsValue::Boolean(false)) => {}
        _ => return Err(state::invalid()),
    }
    state::put(vm, promise, "async-hook-resolved", JsValue::Boolean(true));
    dispatch(vm, "promiseResolve", &[JsValue::Number(id as f64)])
}
/** Bracket the actual reaction resource, not the promise merely supplying the fulfilled value. */
pub(in crate::v9) fn before(vm: &mut Vm, promise: ObjectId) -> Res<()> {
    if !vm.page.environment.conformance_async_hooks {
        return Ok(());
    }
    let (id, trigger) = state::ids(vm, promise)?;
    state::push(vm, id, trigger)?;
    dispatch(vm, "before", &[JsValue::Number(id as f64)])
}
/** d8 pops the async context before notifying after hooks. */
pub(in crate::v9) fn after(vm: &mut Vm, promise: ObjectId) -> Res<()> {
    if !vm.page.environment.conformance_async_hooks {
        return Ok(());
    }
    let (id, _) = state::ids(vm, promise)?;
    state::pop(vm)?;
    dispatch(vm, "after", &[JsValue::Number(id as f64)])
}
/** Fatal unwinding restores the context stack without executing another author hook after termination. */
pub(in crate::v9) fn unwind(vm: &mut Vm) -> Res<()> {
    state::pop(vm)
}
/** Custom Promise capabilities annotate their genuine callback with its actual Promise resource. */
pub(in crate::v9) fn resource(vm: &Vm, job: &Job) -> Option<ObjectId> {
    if !vm.page.environment.conformance_async_hooks {
        return None;
    }
    match job {
        Job::Thenable { promise, .. } => Some(*promise),
        Job::Reaction { reaction, .. } => reaction.target.or_else(|| match &reaction.handler {
            ReactionHandler::Function(callback) => callback
                .object()
                .and_then(|id| state::get(vm, id, "async-hook-resource"))
                .and_then(|value| value.object()),
            _ => None,
        }),
        _ => None,
    }
}
