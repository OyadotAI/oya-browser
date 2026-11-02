/** Each Rust worker thread executes and retains only its own V9 isolate and committed state. */
use super::{Endpoint, PARK_INTERVAL, WorkerGraph};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::JsValue;
use crate::v9::vm::{Res, Vm};
use crate::v9::{
    self, Compiled, DocumentHost, Limits, ModuleInput, ModuleSource, PageEnvironment, PageInput,
    StorageState,
};
use std::sync::atomic::Ordering;
use std::sync::mpsc;

/** A worker error terminates that isolate; its queued messages remain retrievable as in d8. */
pub(super) fn run(
    environment: PageEnvironment,
    source: String,
    arguments: Option<WorkerGraph>,
    endpoint: Endpoint,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execute(&environment, source, arguments, &endpoint)
    }));
    let error = match result {
        Ok(result) => result.err(),
        Err(_) => Some(EngineError::invalid(
            "Worker V9 isolate stopped on an internal error.",
        )),
    };
    if let Some(error) = error {
        let timeout = v9::platform::timeout();
        let cancelled = endpoint.worker.cancel.load(Ordering::Acquire)
            && (error.code == timeout.code && error.message == timeout.message
                || error.code == "conformance_worker_cancelled");
        let normal_exit = error.code == "conformance_exit" && error.message == "0";
        if !cancelled && !normal_exit {
            *endpoint.worker.error.lock().unwrap() = Some(error.message.clone());
            if !matches!(
                error.code.as_str(),
                "script_execution_error" | "script_syntax_error"
            ) {
                *endpoint.worker.failure.lock().unwrap() = Some(error);
            }
        }
    }
    endpoint.worker.done.store(true, Ordering::Release);
    if let Some(host) = environment.conformance_workers {
        host.finished(environment.conformance_worker_id);
    }
}
/** A child receives only inert source fixtures supplied by its parent environment. */
fn fixtures(environment: &PageEnvironment) -> ModuleInput {
    ModuleInput {
        entries: Vec::new(),
        sources: environment
            .conformance_sources
            .iter()
            .map(|(url, source)| ModuleSource {
                url: url.clone(),
                source: Some(source.clone()),
                classic: true,
            })
            .collect(),
    }
}
/** Host projections are empty; d8 workers have no document, DOM nodes or browser authority. */
fn input<'a>(
    sources: &'a [String],
    environment: &'a PageEnvironment,
    host: &'a DocumentHost<'a>,
    modules: &'a ModuleInput,
) -> PageInput<'a> {
    PageInput {
        sources,
        source_keys: &[],
        module_keys: &[],
        environment,
        host,
        modules,
        storage: StorageState::default(),
        document: crate::document::ScriptDocument {
            identity: "conformance-worker".into(),
            charset: "UTF-8".into(),
            nodes: Vec::new(),
        },
    }
}
/** Compile the true Script once, invoke function workers with cloned arguments, then deliver actual events. */
fn execute(
    environment: &PageEnvironment,
    source: String,
    arguments: Option<WorkerGraph>,
    endpoint: &Endpoint,
) -> EngineResult<()> {
    let sources = vec![source];
    let host = DocumentHost::default();
    let modules = fixtures(environment);
    let input = input(&sources, environment, &host, &modules);
    let (mut vm, compiled) = v9::session(
        Limits::default(),
        &sources,
        &[],
        true,
        Some(input.document.clone()),
        StorageState::default(),
        &host,
        environment,
        None,
        &modules,
        &[],
    )?;
    let Compiled::Each(codes) = compiled else {
        return Err(EngineError::invalid("Worker compilation mode mismatch."));
    };
    let code = codes
        .into_iter()
        .next()
        .ok_or_else(|| EngineError::invalid("Worker source is missing."))??;
    crate::v9::builtins::worker_activate(&mut vm).map_err(|error| uncaught(&mut vm, error))?;
    let started = vm.run_script(code).and_then(|function| {
        if let Some(arguments) = arguments {
            let arguments = crate::v9::builtins::worker_decode(&mut vm, arguments)?;
            let length = vm.length_of(&arguments)?;
            let mut values = Vec::new();
            for index in 0..length {
                values.push(vm.get(
                    &arguments,
                    &crate::v9::vm::heap::Key::from(&index.to_string()),
                )?);
            }
            let global = JsValue::Object(vm.global_object());
            vm.call(function, global, values)?;
        }
        vm.drain_jobs()
    });
    terminal(&mut vm, started)?;
    vm.pins.clear();
    checkpoint(&mut vm)?;
    loop {
        if endpoint.worker.cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        if let (Some(host), Some(id)) = (
            &environment.conformance_workers,
            environment.conformance_worker_id,
        ) {
            host.collection_safepoint(Some(id), &mut vm)
                .map_err(|error| uncaught(&mut vm, error))?;
        }
        let global = JsValue::Object(vm.global_object());
        let handler = vm
            .get(&global, &crate::v9::vm::heap::Key::from("onmessage"))
            .map_err(|thrown| uncaught(&mut vm, thrown))?;
        let has_handler = vm.is_callable(&handler);
        match endpoint.input.try_recv() {
            Ok(message) => deliver(&mut vm, message)?,
            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Err(mpsc::TryRecvError::Empty) => {
                if let Some(due) = vm.next_timer() {
                    begin(&mut vm);
                    let result = vm.run_due_timers(due, 1);
                    terminal(&mut vm, result)?;
                    checkpoint(&mut vm)?;
                } else if !has_handler {
                    return Ok(());
                } else {
                    match endpoint.input.recv_timeout(PARK_INTERVAL) {
                        Ok(message) => deliver(&mut vm, message)?,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
            }
        }
    }
}
/** A real task gets the same operation and wall-clock budgets as one public V9 event delivery. */
fn begin(vm: &mut Vm) {
    vm.remaining = vm.limits.operations;
    vm.deadline = Some(std::time::Instant::now() + crate::constants::script_deadline());
    vm.page.throw_location = None;
    vm.page.throw_trace.clear();
}
/** Message shells stay pinned through the callback and its complete microtask checkpoint, then release. */
fn deliver(vm: &mut Vm, message: WorkerGraph) -> EngineResult<()> {
    begin(vm);
    let mark = vm.pins.len();
    let result = (|| -> Res<()> {
        let data = crate::v9::builtins::worker_decode(vm, message)?;
        let event = vm.new_object()?;
        vm.pin(JsValue::Object(event));
        vm.put(event, "data", data)?;
        let global = JsValue::Object(vm.global_object());
        let callback = vm.get(&global, &crate::v9::vm::heap::Key::from("onmessage"))?;
        if vm.is_callable(&callback) {
            vm.call(callback, global, vec![JsValue::Object(event)])?;
        }
        vm.drain_jobs()
    })();
    terminal(vm, result)?;
    vm.pins.truncate(mark);
    checkpoint(vm)
}
/** Native output is checked before clearing per-task effects; bounded console diagnostics remain resident. */
fn checkpoint(vm: &mut Vm) -> EngineResult<()> {
    if let Some((code, message)) = vm.page.effects.conformance_native_error.take() {
        return Err(EngineError::new(
            &code,
            message,
            "The child reached an unsupported native capability.",
        ));
    }
    if let Some(message) = vm.page.effects.conformance_error.take() {
        let code = if vm.page.effects.conformance_rejection {
            "worker_unhandled_rejection"
        } else {
            "script_execution_error"
        };
        return Err(EngineError::new(
            code,
            message,
            "The worker callback failed.",
        ));
    }
    if !vm.page.effects.requests.is_empty() || !vm.modules.requests.is_empty() {
        return Err(EngineError::unsupported(
            "Worker external requests require an explicitly supplied host delivery.",
        ));
    }
    let mut effects = crate::v9::platform::HostEffects::default();
    effects.console = std::mem::take(&mut vm.page.effects.console);
    effects.console_dropped = vm.page.effects.console_dropped;
    vm.page.effects = effects;
    vm.collect_if_due().map_err(|error| uncaught(vm, error))
}
/** Preserve the actual terminal error, including deferred assertion messages, for diagnostics. */
fn uncaught(vm: &mut Vm, error: crate::v9::vm::Thrown) -> EngineError {
    vm.uncaught(error, vm.page.throw_location.unwrap_or_default())
}
/** Script failures terminate the worker instead of falling through into a falsely healthy event loop. */
fn terminal<T>(vm: &mut Vm, result: Res<T>) -> EngineResult<()> {
    result.map(|_| ()).map_err(|error| uncaught(vm, error))
}
