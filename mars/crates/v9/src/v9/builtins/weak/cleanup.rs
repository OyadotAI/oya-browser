/** Deferred registry deliveries share a bounded task and retain registry identities only weakly. */
use super::*;
use crate::v9::vm::data::FunctionKind;

/** One native dispatcher serves every registry and context without owning their lifetimes. */
static TASK: OnceLock<u32> = OnceLock::new();

/** Disposal cancels pending deliveries in that creation context without disabling future ones. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn cancel_for_realm(vm: &mut Vm, bank: u32) {
    let cancelled: crate::v9::vm::heap::FastSet<_> = vm
        .heap
        .objects
        .iter()
        .filter_map(|(id, object)| {
            (vm.object_realm(*id) == bank
                && matches!(&object.class, Class::FinalizationRegistry(data) if data.scheduled))
            .then_some(*id)
        })
        .collect();
    for id in &cancelled {
        cancel_registry(vm, *id);
    }
    let mut empty = Vec::new();
    for (id, object) in &mut vm.heap.objects {
        if let Class::Function(function) = &mut object.class
            && let FunctionKind::Native { id: native, slots } = &mut function.kind
            && *native == *TASK.get().unwrap()
        {
            slots.retain(|value| !matches!(value, JsValue::Number(id) if cancelled.contains(&(*id as ObjectId))));
            if slots.is_empty() {
                empty.push(*id);
            }
        }
    }
    vm.page.state.timers.retain(|timer| {
        timer
            .callback
            .object()
            .is_none_or(|id| !empty.contains(&id))
    });
}

/** Cancelled cleared cells release their holdings; live registrations keep their weak targets. */
#[cfg(feature = "conformance")]
fn cancel_registry(vm: &mut Vm, id: ObjectId) {
    let mut removed = 0usize;
    let mut units = 0usize;
    if let Some(Class::FinalizationRegistry(data)) =
        vm.heap.get_mut(id).map(|object| &mut object.class)
    {
        data.cells.retain(|cell| {
            if cell.target.is_none() {
                removed += 1;
                units += cell.holdings.string_units();
                return false;
            }
            true
        });
        data.scheduled = false;
    }
    vm.heap.adjust(-(removed as isize), -(units as isize));
}

/** Capture the native index once; each queued task stores only weak numeric identities. */
pub(super) fn install(builder: &mut Builder) {
    let _ = TASK.set(builder.native("", 0, run));
}

/** A single task batches weak numeric registry identities, avoiding one timer per registration.
Every registry still finishes its own microtask checkpoint before the next cleanup starts. */
pub(in crate::v9) fn schedule(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let pending = vm.page.state.timers.iter().find_map(|timer| {
        let callback = timer.callback.object()?;
        match vm.heap.get(callback)?.function()?.kind {
            FunctionKind::Native { id: native, .. } if native == *TASK.get().unwrap() => {
                Some(callback)
            }
            _ => None,
        }
    });
    if let Some(callback) = pending {
        if let Class::Function(function) = &mut vm.heap.get_mut(callback).unwrap().class
            && let FunctionKind::Native { slots, .. } = &mut function.kind
        {
            slots.push(JsValue::Number(f64::from(id)));
        }
        return Ok(());
    }
    let callback = vm.native_closure(*TASK.get().unwrap(), vec![JsValue::Number(f64::from(id))])?;
    crate::v9::platform::schedule_task(vm, JsValue::Object(callback), Vec::new())
}

/** Deliver each cleared holding once; unregister calls from a cleanup affect the next visit. */
fn run(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let identities = vm.slots(call.callee);
    for (index, value) in identities.iter().enumerate() {
        let JsValue::Number(id) = value else {
            continue;
        };
        let mark = vm.pins.len();
        let result = cleanup_registry(vm, *id as ObjectId);
        vm.pins.truncate(mark);
        if let Err(error) = result {
            for value in &identities[index + 1..] {
                if let JsValue::Number(id) = value {
                    schedule(vm, *id as ObjectId)?;
                }
            }
            return Err(error);
        }
        vm.drain_jobs()?;
    }
    Ok(JsValue::Undefined)
}

/** Only the registry currently delivering its holdings is pinned during callback execution. */
fn cleanup_registry(vm: &mut Vm, id: ObjectId) -> Res<()> {
    vm.pin(JsValue::Object(id));
    while let Some((callback, holdings)) = take_holding(vm, id) {
        let mark = vm.pins.len();
        vm.pin(holdings.clone());
        let result = vm.call(
            JsValue::Object(callback),
            JsValue::Undefined,
            vec![holdings],
        );
        vm.pins.truncate(mark);
        if let Err(error) = result {
            finish_cleanup(vm, id)?;
            return Err(error);
        }
    }
    finish_cleanup(vm, id)?;
    Ok(())
}

/** Remove a cell before calling author code, so reentrant unregister cannot deliver it again. */
fn take_holding(vm: &mut Vm, id: ObjectId) -> Option<(ObjectId, JsValue)> {
    let Class::FinalizationRegistry(data) = &mut vm.heap.get_mut(id)?.class else {
        return None;
    };
    let index = data.cells.iter().position(|cell| cell.target.is_none())?;
    let holding = data.cells.remove(index).holdings;
    let callback = data.callback;
    vm.heap.adjust(-1, -(holding.string_units() as isize));
    Some((callback, holding))
}

/** A throwing cleanup may leave more holdings; they remain scheduled for a later task. */
fn finish_cleanup(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let pending = match vm.heap.get_mut(id).map(|object| &mut object.class) {
        Some(Class::FinalizationRegistry(data)) => {
            let pending = data.cells.iter().any(|cell| cell.target.is_none());
            data.scheduled = pending;
            pending
        }
        _ => false,
    };
    if pending {
        schedule(vm, id)?;
    }
    Ok(())
}
