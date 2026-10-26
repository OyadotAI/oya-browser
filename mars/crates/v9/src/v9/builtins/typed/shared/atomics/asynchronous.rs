/** Async waits register real futex futures and deliver their promises through ordinary VM tasks. */
use super::*;
use crate::v9::vm::data::{AsyncWait, FunctionKind, Job};
use std::sync::OnceLock;

/** Polling keeps foreign notifications observable without running another VM's callback IDs. */
const POLL_INTERVAL_MS: f64 = 1.0;
/** Each captured callback owns the actual promise and backing wrapper through normal GC edges. */
static POLL: OnceLock<u32> = OnceLock::new();

/** The synchronous API remains separate; pending results always contain a real caller-realm promise. */
pub(super) fn install(builder: &mut Builder, object: ObjectId) {
    builder.method(object, "waitAsync", 4, wait_async);
    let _ = POLL.set(builder.native("", 0, poll));
}

/** Coercion finishes before registration and never runs while the shared store mutex is held. */
fn wait_async(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, initial) = view(vm, &call.arg(0), true)?;
    let buffer = shared_buffer(vm, &JsValue::Object(initial.buffer))
        .ok_or_else(|| vm.type_error("Atomics.waitAsync requires shared storage"))?;
    let (_, target, index) = access(vm, &call.arg(0), &call.arg(1), true)?;
    let expected = converted(vm, target.kind, &call.arg(2))?;
    let expected = encode(vm, target.kind, &expected)?;
    let duration = timeout(vm, &call.arg(3))?;
    let deadline = duration.map_or(f64::INFINITY, |duration| {
        vm.page.state.clock + duration.as_secs_f64() * 1000.0
    });
    let promise = vm.new_promise()?;
    let callback = vm.native_closure(
        *POLL.get().expect("installed"),
        vec![
            JsValue::Object(target.buffer),
            JsValue::Number(0.0),
            JsValue::Object(promise),
            JsValue::Number(deadline),
        ],
    )?;
    match buffer.register_async(
        target.offset + index * element_size(target.kind),
        &expected,
        duration == Some(Duration::ZERO),
        callback,
    )? {
        AsyncWait::Immediate(value) => result(
            vm,
            false,
            JsValue::string(match value {
                WaitResult::NotEqual => "not-equal",
                _ => "timed-out",
            })?,
        ),
        AsyncWait::Pending(ticket) => {
            if let Some(Class::Function(function)) =
                vm.heap.get_mut(callback).map(|object| &mut object.class)
                && let FunctionKind::Native { slots, .. } = &mut function.kind
            {
                slots[1] = JsValue::Number(ticket as f64);
            }
            schedule(vm, callback, deadline)?;
            result(vm, true, JsValue::Object(promise))
        }
    }
}

/** Immediate results are ordinary objects; asynchronous values are actual native Promise instances. */
fn result(vm: &mut Vm, asynchronous: bool, value: JsValue) -> Res<JsValue> {
    let object = vm.new_object()?;
    vm.put(object, "async", JsValue::Boolean(asynchronous))?;
    vm.put(object, "value", value)?;
    Ok(JsValue::Object(object))
}

/** A task examines the real registered ticket instead of inferring success from changed memory. */
fn poll(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let buffer = shared_buffer(vm, &slots[0])
        .ok_or_else(|| vm.type_error("Invalid asynchronous atomic backing"))?;
    let JsValue::Number(ticket) = slots[1] else {
        unreachable!("native waiter ticket")
    };
    let JsValue::Number(deadline) = slots[3] else {
        unreachable!("native waiter deadline")
    };
    let (present, value) = buffer.poll_async(ticket as u64, vm.page.state.clock >= deadline);
    match value {
        Some(value) => {
            vm.page
                .state
                .timers
                .retain(|timer| timer.callback != JsValue::Object(call.callee));
            vm.resolve_promise(
                slots[2].object().expect("wait promise"),
                JsValue::string(if value == WaitResult::Ok {
                    "ok"
                } else {
                    "timed-out"
                })?,
            )?;
        }
        None if present => schedule(vm, call.callee, deadline)?,
        None => {}
    }
    Ok(JsValue::Undefined)
}

/** Positive bounded task spacing prevents a pending infinite wait from monopolizing one microtask drain. */
fn schedule(vm: &mut Vm, callback: ObjectId, deadline: f64) -> Res<()> {
    crate::v9::platform::schedule_task(vm, JsValue::Object(callback), Vec::new())?;
    let due = vm.page.state.clock
        + (deadline - vm.page.state.clock)
            .max(0.0)
            .min(POLL_INTERVAL_MS);
    if let Some(timer) = vm.page.state.timers.last_mut() {
        timer.due = due;
    }
    Ok(())
}

/** Native future captures are checked even when unreachable; forged state cannot enter a body with missing slots. */
pub(in crate::v9) fn valid_function(heap: &crate::v9::vm::heap::Heap, kind: &FunctionKind) -> bool {
    let _ = crate::v9::vm::realm::realm_for(crate::v9::vm::heap::INTRINSIC_BASE);
    let FunctionKind::Native { id, slots } = kind else {
        return true;
    };
    if Some(id) != POLL.get() {
        return true;
    }
    let [
        JsValue::Object(buffer),
        JsValue::Number(ticket),
        JsValue::Object(promise),
        JsValue::Number(deadline),
    ] = slots.as_slice()
    else {
        return false;
    };
    ticket.is_finite()
        && *ticket >= 1.0
        && *ticket <= crate::constants::VM_MAX_SAFE_LENGTH
        && ticket.fract() == 0.0
        && !deadline.is_nan()
        && *deadline >= 0.0
        && matches!(
            heap.get(*buffer).map(|object| &object.class),
            Some(Class::SharedArrayBuffer(_))
        )
        && matches!(
            heap.get(*promise).map(|object| &object.class),
            Some(Class::Promise(_))
        )
}
/** Queue entries cannot redirect another backing's native completion or disagree about the exact ticket. */
pub(in crate::v9) fn valid_ticket(
    heap: &crate::v9::vm::heap::Heap,
    callback: ObjectId,
    ticket: u64,
    identity: u64,
) -> bool {
    let Some(function) = heap.get(callback).and_then(|object| object.function()) else {
        return false;
    };
    if !valid_function(heap, &function.kind) {
        return false;
    }
    let FunctionKind::Native { id, slots } = &function.kind else {
        return false;
    };
    Some(id)==POLL.get() && slots[1]==JsValue::Number(ticket as f64) && slots[0].object().and_then(|id|heap.get(id)).is_some_and(|object|matches!(&object.class,Class::SharedArrayBuffer(buffer) if buffer.identity()==identity))
}

/** Only callbacks belonging to this agent become VM jobs; foreign waiters receive a native signal only. */
pub(in crate::v9) fn notify_shared(
    vm: &mut Vm,
    buffer: &SharedBuffer,
    offset: usize,
    count: usize,
) -> Res<usize> {
    let (notified, callbacks) = buffer.notify_callbacks(offset, count);
    for callback in callbacks {
        vm.enqueue(Job::Callback(JsValue::Object(callback)))?;
    }
    Ok(notified)
}
