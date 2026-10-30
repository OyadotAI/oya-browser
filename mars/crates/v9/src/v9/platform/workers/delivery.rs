/** Worker delivery uses only the current isolate and its branded handles. */
use super::*;

/** Worker-local cancellation prevents owner propagation; recursive handler failures stay bounded. */
pub(in crate::v9) fn report_exception(vm: &mut Vm, error: &JsValue) -> Res<()> {
    if vm.page.environment.worker_name.is_none() {
        vm.report_error(error);
        return Ok(());
    }
    vm.pin(error.clone());
    let message = vm.describe(error);
    if !vm.page.reporting_worker_error {
        vm.page.reporting_worker_error = true;
        let result = local_error(vm, error, &message);
        vm.page.reporting_worker_error = false;
        if !result? {
            return Ok(());
        }
    }
    let errors = &mut vm.page.effects.worker_errors;
    if errors.len() >= WORKER_TASKS
        || errors
            .iter()
            .map(String::len)
            .sum::<usize>()
            .saturating_add(message.len())
            > crate::v9::WORKER_MESSAGE_BYTES
    {
        return Err(EngineError::limit("Worker errors", WORKER_TASKS).into());
    }
    errors.push(message);
    vm.report_error(error);
    Ok(())
}

/** ErrorEvent details are available in the worker before deciding whether the owner is notified. */
fn local_error(vm: &mut Vm, error: &JsValue, message: &str) -> Res<bool> {
    let location = vm.page.throw_location.unwrap_or_default();
    let filename = vm
        .page
        .state
        .worker_imports
        .range(..=location.byte_offset)
        .next_back()
        .filter(|(base, _)| {
            vm.dynamic.iter().any(|source| {
                source.base == Some(**base) && location.byte_offset < **base + source.source.len()
            })
        })
        .map(|(_, url)| url.clone())
        .unwrap_or_else(|| vm.page.environment.url.clone());
    let event = events::create_event(vm, "error", false, true)?
        .object()
        .expect("event object");
    vm.put(event, "message", JsValue::string(message)?)?;
    vm.put(event, "error", error.clone())?;
    vm.put(event, "filename", JsValue::string(&filename)?)?;
    vm.put(event, "lineno", JsValue::Number(location.line as f64))?;
    vm.put(event, "colno", JsValue::Number(location.column as f64))?;
    events::dispatch(vm, Target::Window, event)
}

/** Deliver to the exact active Worker object, or to this dedicated worker's global. */
pub(in crate::v9) fn receive(vm: &mut Vm, id: u32, data: &WorkerData, ports: &[u64]) -> Res<()> {
    let target = if id == 0 && vm.page.environment.worker_name.is_some() {
        Target::Window
    } else if let Some(handle) = vm
        .page
        .state
        .workers
        .get(&id)
        .filter(|handle| handle.active)
    {
        Target::Object(handle.object)
    } else {
        return Ok(());
    };
    if ports.len() > WORKER_TASKS {
        return Err(EngineError::invalid("Invalid Worker ports").into());
    }
    let data = worker_clone::decode(vm, data)?;
    let ports = ports
        .iter()
        .map(|link| channel::adopt(vm, *link, channel::Route::Worker(id)))
        .collect::<Res<Vec<_>>>()?;
    let ports = vm.new_array(ports)?;
    let event = events::create_event(vm, "message", false, false)?
        .object()
        .expect("event object");
    vm.put(event, "data", data)?;
    vm.put(event, "ports", JsValue::Object(ports))?;
    vm.put(event, "origin", JsValue::string("")?)?;
    events::dispatch(vm, target, event)?;
    Ok(())
}

/** Loading errors are asynchronous events, with ordinary cancellation and console reporting. */
pub(in crate::v9) fn error(vm: &mut Vm, id: u32, message: &str) -> Res<()> {
    let Some(handle) = vm
        .page
        .state
        .workers
        .get(&id)
        .filter(|handle| handle.active)
    else {
        return Ok(());
    };
    let target = Target::Object(handle.object);
    let event = events::create_event(vm, "error", false, true)?
        .object()
        .expect("event object");
    vm.put(event, "message", JsValue::string(message)?)?;
    if events::dispatch(vm, target, event)? {
        vm.report_error(&JsValue::string(message)?);
    }
    Ok(())
}
