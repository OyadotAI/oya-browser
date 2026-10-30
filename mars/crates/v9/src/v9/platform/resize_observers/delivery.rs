/** Bounded broadcasts drain their current records before callbacks and leave author timers untouched. */
use super::*;

/** One broadcast pass per native turn prevents callback-driven resize loops from monopolizing the engine. */
pub(in crate::v9) fn deliver_all(vm: &mut Vm) -> Res<()> {
    let ids: Vec<_> = vm
        .page
        .state
        .resize_observers
        .iter()
        .filter(|(_, state)| !state.queued.is_empty())
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        let result = deliver_id(vm, id).and_then(|_| vm.drain_jobs());
        match result {
            Err(crate::v9::vm::Thrown::Value(error)) => {
                super::super::workers::report_exception(vm, &error)?
            }
            result => result?,
        }
    }
    Ok(())
}

/** A callback may disconnect another observer before its turn, so check its live queue again. */
fn deliver_id(vm: &mut Vm, id: ObjectId) -> Res<()> {
    if vm
        .page
        .state
        .resize_observers
        .get(&id)
        .is_none_or(|state| state.queued.is_empty())
    {
        return Ok(());
    }
    let callback = vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&callback_key()))
        .and_then(|prop| match &prop.slot {
            Slot::Data(value) => Some(value.clone()),
            _ => None,
        })
        .ok_or_else(|| EngineError::invalid("Invalid ResizeObserver callback."))?;
    vm.pin(JsValue::Object(id));
    vm.pin(callback.clone());
    let records = take_records(vm, id)?;
    vm.call(
        callback,
        JsValue::Object(id),
        vec![records, JsValue::Object(id)],
    )?;
    Ok(())
}

/** Preserve the earlier compatibility drain API without inventing extra callbacks or retaining stale entries. */
pub(super) fn take_records(vm: &mut Vm, id: ObjectId) -> Res<JsValue> {
    let records = std::mem::take(
        &mut vm
            .page
            .state
            .resize_observers
            .get_mut(&id)
            .expect("brand")
            .queued,
    );
    for record in &records {
        vm.pin(JsValue::Object(record.target));
    }
    let state = vm.page.state.resize_observers.get_mut(&id).expect("brand");
    for record in &records {
        if let Some(target) = state
            .targets
            .iter_mut()
            .find(|target| target.node == record.node)
        {
            target.previous = record.observed;
        }
    }
    let mut values = Vec::with_capacity(records.len());
    for record in records {
        values.push(entry(vm, record)?);
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Box size arrays preserve physical content rectangles and logical inline/block dimensions separately. */
fn entry(vm: &mut Vm, record: Entry) -> Res<JsValue> {
    let id = vm.new_object()?;
    vm.pin(JsValue::Object(id));
    vm.put(id, "target", JsValue::Object(record.target))?;
    let rect = rect(vm, record.content)?;
    vm.put(id, "contentRect", rect)?;
    for (name, dimensions) in [
        ("contentBoxSize", record.content_size),
        ("borderBoxSize", record.border_size),
        ("devicePixelContentBoxSize", record.device_size),
    ] {
        let size = vm.new_object()?;
        vm.pin(JsValue::Object(size));
        vm.put(size, "inlineSize", JsValue::Number(dimensions[0]))?;
        vm.put(size, "blockSize", JsValue::Number(dimensions[1]))?;
        crate::v9::builtins::freeze_object(vm, size)?;
        let list = vm.new_array(vec![JsValue::Object(size)])?;
        crate::v9::builtins::freeze_object(vm, list)?;
        vm.put(id, name, JsValue::Object(list))?;
    }
    crate::v9::builtins::freeze_object(vm, id)?;
    Ok(JsValue::Object(id))
}

/** contentRect uses the measured content offset and dimensions, not the visual document rectangle. */
fn rect(vm: &mut Vm, [x, y, width, height]: [f64; 4]) -> Res<JsValue> {
    let id = vm.new_object()?;
    vm.pin(JsValue::Object(id));
    for (name, value) in [
        ("x", x),
        ("y", y),
        ("top", y),
        ("left", x),
        ("width", width),
        ("height", height),
        ("right", x + width),
        ("bottom", y + height),
    ] {
        vm.put(id, name, JsValue::Number(value))?;
    }
    crate::v9::builtins::freeze_object(vm, id)?;
    Ok(JsValue::Object(id))
}
