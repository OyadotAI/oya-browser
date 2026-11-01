/**
 * Loading, linking and evaluating module graphs. Linking checks every named import, creates the
 * namespaces and instantiates each body (it runs its hoisting and suspends), so cycles see each
 * other's functions. Evaluation is depth first: a module runs once its dependencies have, except
 * dependencies on a cycle back to it; one whose dependency is still awaiting (top-level await)
 * waits for that promise and runs when it settles.
 */
use super::*;
use crate::v9::vm::data::{AsyncData, Class, PromiseState, ReactionHandler};
use crate::v9::vm::heap::Object;
use crate::v9::vm::{Exit, Frame, Resume, Thrown};

/** Whether a graph can run. */
enum Graph {
    /** Every module is here; keys in discovery order. */
    Ready(Vec<String>),
    /** Some modules were requested from the app. */
    Pending,
    /** A module failed to load or compile, or a specifier did not resolve. */
    Failed(EngineError),
}

/** What became of a module script. */
pub(in crate::v9) enum Outcome {
    /** It ran (or is still awaiting, and reports a later failure to the console). */
    Ran,
    /** Its graph waits for modules the app has been asked for. */
    Waiting,
    /** It failed and every effect of the attempt was rolled back. */
    Failed(EngineError),
}

/** Walk a graph from its root, requesting what is missing. */
fn graph(vm: &mut Vm, root: &str) -> Graph {
    let mut order = Vec::new();
    let mut stack = vec![root.to_string()];
    let mut pending = false;
    while let Some(key) = stack.pop() {
        if order.contains(&key) {
            continue;
        }
        if let Some(failed) = vm
            .modules
            .state
            .records
            .get(&key)
            .and_then(|r| r.failed.clone())
        {
            return Graph::Failed(module_error(&failed));
        }
        let requests = match vm.modules.compiled.get(&key).map(|module| &module.body) {
            None => {
                if !vm.modules.request(&key) {
                    return Graph::Failed(module_error(&format!(
                        "Module {key} was not loaded: a page loads at most {MODULES} modules."
                    )));
                }
                pending = true;
                continue;
            }
            Some(Err(error)) => return Graph::Failed(error.clone()),
            Some(Ok((_, info))) => info.requests.len(),
        };
        for request in 0..requests {
            match vm.modules.dependency(&key, request) {
                Ok(dependency) => stack.push(dependency),
                Err(message) => return Graph::Failed(module_error(&message)),
            }
        }
        order.push(key);
    }
    if pending {
        Graph::Pending
    } else {
        Graph::Ready(order)
    }
}

/** A module failure as the script error it causes. */
fn module_error(message: &str) -> EngineError {
    EngineError::new(
        "script_execution_error",
        message.to_string(),
        "Correct the module; that module script's changes were not applied.",
    )
}

/** Check every named import, then instantiate each module not yet instantiated. The inner
error is a link failure (a SyntaxError in browsers). */
fn link(vm: &mut Vm, keys: &[String]) -> Res<Result<(), String>> {
    for key in keys {
        let Some(Ok((_, info))) = vm.modules.compiled.get(key).map(|module| &module.body) else {
            continue;
        };
        let info = info.clone();
        for (request, imported) in &info.imports {
            let dependency = vm.modules.dependency(key, *request).unwrap_or_default();
            if resolve_export(&vm.modules, &dependency, imported, &mut Vec::new()).is_none() {
                return Ok(Err(format!(
                    "The requested module '{}' does not provide an export named '{imported}'",
                    info.requests[*request]
                )));
            }
        }
        // InitializeEnvironment: every indirect export must resolve, unambiguously.
        for (name, export) in &info.exports {
            if matches!(export, crate::v9::compile::Export::Indirect(_, Some(_)))
                && resolve_export(&vm.modules, key, name, &mut Vec::new()).is_none()
            {
                return Ok(Err(format!(
                    "The export '{name}' cannot be resolved: it is missing, circular or ambiguous"
                )));
            }
        }
    }
    for key in keys {
        instantiate(vm, key)?;
    }
    Ok(Ok(()))
}

/** A module's namespace object, created once: no prototype, not extensible, tagged "Module". */
pub(super) fn namespace(vm: &mut Vm, key: &str) -> Res<ObjectId> {
    if let Some(id) = vm.modules.state.records.get(key).and_then(|r| r.namespace) {
        return Ok(id);
    }
    let mut object = Object::new(Class::Namespace(key.to_string()), None);
    object.extensible = false;
    let id = vm.alloc(object)?;
    vm.heap.put(
        id,
        to_string_tag(),
        Prop {
            slot: HeapSlot::Data(JsValue::String(JsString::from_utf8("Module")?)),
            attrs: 0,
        },
    );
    vm.modules
        .state
        .records
        .entry(key.to_string())
        .or_default()
        .namespace = Some(id);
    Ok(id)
}

/** Run a module's hoisting with its dependencies' namespaces and its `import.meta`, keeping
the suspended body and its environment in the record. */
fn instantiate(vm: &mut Vm, key: &str) -> Res<()> {
    if vm
        .modules
        .state
        .records
        .get(key)
        .is_some_and(|r| r.holder.is_some())
    {
        return Ok(());
    }
    let Some(Ok((code, info))) = vm
        .modules
        .compiled
        .get(key)
        .map(|module| module.body.clone())
    else {
        return Err(EngineError::invalid("V9 linked a module it did not compile.").into());
    };
    let mut args = Vec::new();
    for request in 0..info.requests.len() {
        let dependency = vm.modules.dependency(key, request).unwrap_or_default();
        args.push(JsValue::Object(namespace(vm, &dependency)?));
    }
    let meta = import_meta(vm, key)?;
    args.push(JsValue::Object(meta));
    run_hoisting(vm, code, args)?;
    // The suspended body stays in `vm.suspended` (a root) until its holder exists, so a
    // collection these allocations trigger cannot free its environment.
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    if let Some(Class::Promise(data)) = vm.heap.get_mut(promise).map(|object| &mut object.class) {
        // Failures surface through their module script or import(), never as unhandled.
        data.handled = true;
    }
    let holder = vm.alloc(Object::new(
        Class::Async(Box::new(AsyncData {
            frame: None,
            promise,
        })),
        None,
    ))?;
    let saved = vm
        .suspended
        .take()
        .ok_or_else(|| EngineError::invalid("V9 lost a module body."))?;
    let env = saved.env;
    if let Some(Class::Async(data)) = vm.heap.get_mut(holder).map(|object| &mut object.class) {
        data.frame = Some(saved);
    }
    let record = vm.modules.state.records.entry(key.to_string()).or_default();
    record.holder = Some(holder);
    record.meta = Some(meta);
    record.env = env;
    Ok(())
}

/** `import.meta`: the module's URL (the document's for an inline module) and `resolve`. */
fn import_meta(vm: &mut Vm, key: &str) -> Res<ObjectId> {
    let url = vm
        .modules
        .compiled
        .get(key)
        .map(|module| {
            #[cfg(feature = "conformance")]
            if vm.page.environment.conformance_d8 && key.starts_with("data:text/javascript,") {
                return key.to_string();
            }
            module.base.to_string()
        })
        .unwrap_or_default();
    let meta = vm.new_object()?;
    vm.pin(JsValue::Object(meta));
    vm.put(meta, "url", JsValue::String(JsString::from_utf8(&url)?))?;
    let key = JsValue::String(JsString::from_utf8(key)?);
    let resolve = vm.native_closure(natives().resolve, vec![key])?;
    vm.put(meta, "resolve", JsValue::Object(resolve))?;
    Ok(meta)
}

/** Push a module body as its own boundary frame and run it to its post-hoisting suspension,
which leaves it in `vm.suspended`. */
fn run_hoisting(vm: &mut Vm, code: Arc<Code>, args: Vec<JsValue>) -> Res<()> {
    let env = match code.env {
        Some(slots) => Some(
            vm.heap
                .new_env(None, vec![JsValue::Undefined; slots as usize])?,
        ),
        None => None,
    };
    vm.frames.push(Frame {
        realm: vm.active_realm,
        regs: vec![JsValue::Undefined; code.registers as usize],
        code,
        pc: 0,
        stack: Vec::new(),
        env,
        this: JsValue::Undefined,
        callee: JsValue::Undefined,
        args,
        new_target: JsValue::Undefined,
        handlers: Vec::new(),
        envs: 0,
        boundary: true,
        caller_hidden: false,
        construct: false,
        resume: Resume::Next,
    });
    let pinning = std::mem::replace(&mut vm.pinning, false);
    vm.native_depth += 1;
    let result = vm.run();
    vm.native_depth -= 1;
    vm.pinning = pinning;
    match result? {
        Exit::Yield(_) if vm.suspended.is_some() => Ok(()),
        _ => Err(EngineError::invalid("V9 module body did not suspend after hoisting.").into()),
    }
}

/** The keys a module's requests resolve to. */
fn dependencies(vm: &Vm, key: &str) -> Vec<String> {
    let count = match vm.modules.compiled.get(key).map(|module| &module.body) {
        Some(Ok((_, info))) => info.requests.len(),
        _ => 0,
    };
    (0..count)
        .filter_map(|request| vm.modules.dependency(key, request).ok())
        .collect()
}

/** One evaluation walk: the modules being visited, and Tarjan's bookkeeping for finding each
strongly connected component (a cycle) and its root, the module the walk entered it by (the
spec's [[DFSAncestorIndex]] and [[CycleRoot]]). */
#[derive(Default)]
struct Walk {
    /// Modules being visited, outermost first.
    stack: Vec<String>,
    /// Visited modules whose component is not closed yet.
    open: Vec<String>,
    /// Each open module's lowest reachable visit index.
    low: HashMap<String, usize>,
    /// Visits so far, numbering each module.
    next: usize,
}

/** Visit a module depth first, then run it if its dependencies allow. When it closes a cycle,
every module of the cycle records it as their cycle root. */
fn start(vm: &mut Vm, key: &str, walk: &mut Walk) -> Res<()> {
    let record = vm.modules.state.records.entry(key.to_string()).or_default();
    if record.entered {
        return Ok(());
    }
    record.entered = true;
    let index = walk.next;
    walk.next += 1;
    walk.low.insert(key.to_string(), index);
    walk.open.push(key.to_string());
    walk.stack.push(key.to_string());
    for dependency in dependencies(vm, key) {
        start(vm, &dependency, walk)?;
        if let Some(&low) = walk.low.get(&dependency) {
            let mine = walk.low.get_mut(key).expect("open module");
            *mine = (*mine).min(low);
        }
    }
    walk.stack.pop();
    if walk.low.get(key) == Some(&index) {
        close_cycle(vm, key, walk);
    }
    try_run(vm, key, &walk.stack)
}

/** Pop a closed component off the walk: each member's cycle root is `root`. */
fn close_cycle(vm: &mut Vm, root: &str, walk: &mut Walk) {
    while let Some(member) = walk.open.pop() {
        walk.low.remove(&member);
        let done = member == root;
        vm.modules
            .state
            .records
            .entry(member)
            .or_default()
            .cycle_root = Some(root.to_string());
        if done {
            break;
        }
    }
}

/** The module whose completion stands for `key`'s: its cycle root, or itself. A module of a
cycle is done only when the whole cycle is (spec 16.2.1.5.3.1 InnerModuleEvaluation 11.c). */
fn cycle_root(vm: &Vm, key: &str) -> String {
    record(vm, key)
        .and_then(|record| record.cycle_root.clone())
        .unwrap_or_else(|| key.to_string())
}

/** Run a module whose dependencies finished; reject it if one failed; otherwise wait for the
first one still pending. A dependency on the visiting stack is a cycle back, never waited for. */
fn try_run(vm: &mut Vm, key: &str, stack: &[String]) -> Res<()> {
    if record(vm, key).is_none_or(|record| record.started) {
        return Ok(());
    }
    for dependency in dependencies(vm, key) {
        if dependency == key || stack.contains(&dependency) {
            let record = vm.modules.state.records.entry(key.to_string()).or_default();
            if !record.cycle.contains(&dependency) {
                record.cycle.push(dependency);
            }
            continue;
        }
        if record(vm, key).is_some_and(|record| record.cycle.contains(&dependency)) {
            continue;
        }
        // A dependency in another cycle counts as done once that cycle's root is.
        let root = cycle_root(vm, &dependency);
        let waited = if root == cycle_root(vm, key) {
            dependency
        } else {
            root
        };
        match completion(vm, &waited) {
            Some((_, PromiseState::Rejected(error))) => {
                return reject(vm, key, error);
            }
            Some((promise, PromiseState::Pending)) => {
                let slot = JsValue::String(JsString::from_utf8(key)?);
                let resume = vm.native_closure(natives().resume, vec![slot])?;
                let handler = ReactionHandler::Function(JsValue::Object(resume));
                return vm.perform_then(promise, handler.clone(), handler, None);
            }
            _ => {}
        }
    }
    run(vm, key)
}

/** A module's record. */
fn record<'a>(vm: &'a Vm, key: &str) -> Option<&'a Record> {
    vm.modules.state.records.get(key)
}

/** A module's completion promise and its state. */
fn completion(vm: &Vm, key: &str) -> Option<(ObjectId, PromiseState)> {
    let holder = record(vm, key)?.holder?;
    let Some(Class::Async(data)) = vm.heap.get(holder).map(|object| &object.class) else {
        return None;
    };
    match vm.heap.get(data.promise).map(|object| &object.class) {
        Some(Class::Promise(promise)) => Some((data.promise, promise.state.clone())),
        _ => None,
    }
}

/** Dedicated module workers accept messages only after their entry evaluation promise fulfills. */
pub(in crate::v9) fn worker_ready(vm: &Vm) -> bool {
    !vm.page.environment.worker_module
        || matches!(
            completion(vm, &vm.modules.entry_key(0)),
            Some((_, PromiseState::Fulfilled(_)))
        )
}

/** Fail a module without running it, because a dependency failed. */
fn reject(vm: &mut Vm, key: &str, error: JsValue) -> Res<()> {
    if let Some((promise, _)) = completion(vm, key) {
        vm.modules
            .state
            .records
            .entry(key.to_string())
            .or_default()
            .started = true;
        vm.settle(promise, PromiseState::Rejected(error))?;
    }
    Ok(())
}

/** Resume a module body after its hoisting; it runs like an async function's body. */
fn run(vm: &mut Vm, key: &str) -> Res<()> {
    let record = vm.modules.state.records.entry(key.to_string()).or_default();
    record.started = true;
    let Some(holder) = record.holder else {
        return Ok(());
    };
    let frame = match vm.heap.get_mut(holder).map(|object| &mut object.class) {
        Some(Class::Async(data)) => data.frame.take(),
        _ => return Err(EngineError::invalid("Invalid V9 module record.").into()),
    };
    let Some(frame) = frame else {
        return Ok(());
    };
    vm.restore_frame(frame, true)?;
    vm.drive_async(holder, None)
}

/** Resolve one export name of a module: its own binding, a re-export, or the one star export
that provides it (None when missing, circular or ambiguous). */
pub(super) fn resolve_export(
    modules: &Modules,
    key: &str,
    name: &str,
    seen: &mut Vec<(String, String)>,
) -> Option<Target> {
    if seen.len() > MODULES || seen.iter().any(|(k, n)| k == key && n == name) {
        return None;
    }
    seen.push((key.to_string(), name.to_string()));
    let Ok((_, info)) = &modules.compiled.get(key)?.body else {
        return None;
    };
    match info.exports.get(name) {
        Some(crate::v9::compile::Export::Slot(slot)) => {
            Some(Target::Binding(Name::from(key), *slot))
        }
        Some(crate::v9::compile::Export::Indirect(request, imported)) => {
            let dependency = modules.dependency(key, *request).ok()?;
            match imported {
                None => Some(Target::Namespace(Name::from(dependency.as_str()))),
                Some(imported) => resolve_export(modules, &dependency, imported, seen),
            }
        }
        None if name == "default" => None,
        None => {
            let mut found: Option<Target> = None;
            for request in &info.stars {
                let dependency = modules.dependency(key, *request).ok()?;
                let mut branch = seen.clone();
                if let Some(target) = resolve_export(modules, &dependency, name, &mut branch) {
                    if found.as_ref().is_some_and(|known| *known != target) {
                        return None;
                    }
                    found = Some(target);
                }
            }
            found
        }
    }
}

/** Every name a module exports, star exports included (`default` excepted). */
pub(super) fn export_names(
    modules: &Modules,
    key: &str,
    seen: &mut Vec<String>,
    names: &mut std::collections::BTreeSet<Name>,
) {
    if seen.len() > MODULES || seen.iter().any(|known| known == key) {
        return;
    }
    seen.push(key.to_string());
    let Some(Ok((_, info))) = modules.compiled.get(key).map(|module| &module.body) else {
        return;
    };
    names.extend(info.exports.keys().cloned());
    for request in &info.stars {
        if let Ok(dependency) = modules.dependency(key, *request) {
            let mut star = std::collections::BTreeSet::new();
            export_names(modules, &dependency, seen, &mut star);
            names.extend(star.into_iter().filter(|name| &**name != "default"));
        }
    }
}

/**
 * Run one module script during page load, isolated like a classic script: on any failure the
 * attempt is rolled back, and the modules that threw (or depend on one that did) are marked
 * failed, so a later import of them fails the same way instead of running them again.
 */
pub(in crate::v9) fn run_entry(vm: &mut Vm, index: usize) -> Outcome {
    let key = vm.modules.entry_key(index);
    let keys = match graph(vm, &key) {
        Graph::Pending => return Outcome::Waiting,
        Graph::Failed(error) => return Outcome::Failed(error),
        Graph::Ready(keys) => keys,
    };
    let saved = crate::v9::snapshot(vm);
    vm.remaining = vm.limits.operations;
    vm.page.throw_location = None;
    let result = evaluate_entry(vm, &key, &keys);
    let error = match result {
        Ok(None) => return Outcome::Ran,
        Ok(Some(error)) => error,
        Err(thrown) => {
            let location = vm.page.throw_location.unwrap_or_default();
            vm.uncaught(thrown, location)
        }
    };
    let rejected: Vec<String> = keys
        .iter()
        .filter(|key| matches!(completion(vm, key), Some((_, PromiseState::Rejected(_)))))
        .cloned()
        .collect();
    crate::v9::restore(vm, saved);
    // Modules that threw (or whose dependency did) stay failed; the rest may run in another graph.
    for key in rejected {
        vm.modules.state.records.entry(key).or_default().failed = Some(error.message.clone());
    }
    Outcome::Failed(error)
}

/** Request what the module scripts at these source indexes still miss, so their graphs load
together instead of one script at a time. */
pub(in crate::v9) fn discover(vm: &mut Vm, indexes: std::ops::Range<usize>) {
    for index in indexes {
        if matches!(
            vm.modules.entry(index),
            Some(EntryKind::Module | EntryKind::AsyncModule)
        ) {
            let key = vm.modules.entry_key(index);
            graph(vm, &key);
        }
    }
}

/** Link and evaluate an entry's graph and drain microtasks; Some is a failure to roll back. */
fn evaluate_entry(vm: &mut Vm, key: &str, keys: &[String]) -> Res<Option<EngineError>> {
    if let Err(message) = link(vm, keys)? {
        return Ok(Some(crate::v9::errors::syntax(
            crate::protocol::SourceLocation::default(),
            &message,
        )));
    }
    start(vm, key, &mut Walk::default())?;
    vm.drain_jobs()?;
    match completion(vm, key) {
        Some((_, PromiseState::Rejected(error))) => {
            let location = vm.page.throw_location.unwrap_or_default();
            Ok(Some(vm.uncaught(Thrown::Value(error), location)))
        }
        Some((promise, PromiseState::Pending)) => {
            let report = vm.native_closure(natives().report, Vec::new())?;
            let handler = ReactionHandler::Function(JsValue::Object(report));
            vm.perform_then(
                promise,
                ReactionHandler::Function(JsValue::Undefined),
                handler,
                None,
            )?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

/** Evaluate dynamic imports whose graphs are ready: each promise settles with the namespace
once its graph finishes, or with the error that stopped it. Waiting async module scripts are
the page loader's, not this. */
pub(in crate::v9) fn advance_dynamic(vm: &mut Vm) -> Res<()> {
    let waiting = std::mem::take(&mut vm.modules.state.waiting);
    let mut kept = Vec::new();
    for item in waiting {
        let Some(promise) = item.promise else {
            kept.push(item);
            continue;
        };
        let keys = match graph(vm, &item.key) {
            Graph::Pending => {
                kept.push(item);
                continue;
            }
            Graph::Failed(error) if error.code == "script_syntax_error" => {
                let error = vm.syntax_error(&error.message);
                settle_thrown(vm, promise, error)?;
                continue;
            }
            Graph::Failed(error) if error.code == "module_unavailable" => {
                let thrown = vm.throw(crate::v9::vm::error::ErrorKind::Error, &error.message);
                settle_thrown(vm, promise, thrown)?;
                continue;
            }
            Graph::Failed(error) => {
                reject_type_error(vm, promise, &error.message)?;
                continue;
            }
            Graph::Ready(keys) => keys,
        };
        if let Err(message) = link(vm, &keys)? {
            let error = vm.syntax_error(&message);
            settle_thrown(vm, promise, error)?;
            continue;
        }
        if let Err(thrown) = start(vm, &item.key, &mut Walk::default()) {
            settle_thrown(vm, promise, thrown)?;
            continue;
        }
        subscribe_import(vm, &item.key, promise)?;
    }
    kept.append(&mut vm.modules.state.waiting);
    vm.modules.state.waiting = kept;
    Ok(())
}

/** Reject every waiting dynamic import whose graph is complete with a TypeError, after an
engine failure rolled their evaluation back; graphs still loading keep waiting. */
pub(in crate::v9) fn refuse_dynamic(vm: &mut Vm, message: &str) {
    let waiting = std::mem::take(&mut vm.modules.state.waiting);
    let mut kept = Vec::new();
    for item in waiting {
        match (item.promise, graph(vm, &item.key)) {
            (Some(promise), Graph::Ready(_) | Graph::Failed(_)) => {
                let _ = reject_type_error(vm, promise, message);
            }
            _ => kept.push(item),
        }
    }
    vm.modules.state.waiting = kept;
}

/** Settle a dynamic import when its module's cycle root finishes (spec Evaluate step 3), so an
import of a member of a failed cycle rejects with the cycle's error. */
fn subscribe_import(vm: &mut Vm, key: &str, promise: ObjectId) -> Res<()> {
    let namespace = namespace(vm, key)?;
    let Some((root, _)) = completion(vm, &cycle_root(vm, key)) else {
        return vm.resolve_promise(promise, JsValue::Object(namespace));
    };
    let target = JsValue::Object(promise);
    let fulfilled = vm.native_closure(
        natives().imported,
        vec![target.clone(), JsValue::Object(namespace)],
    )?;
    vm.pin(JsValue::Object(fulfilled));
    let rejected = vm.native_closure(natives().refused, vec![target])?;
    vm.perform_then(
        root,
        ReactionHandler::Function(JsValue::Object(fulfilled)),
        ReactionHandler::Function(JsValue::Object(rejected)),
        None,
    )
}

/** Reject a promise with a thrown value; engine failures still stop the request. */
fn settle_thrown(vm: &mut Vm, promise: ObjectId, thrown: Thrown) -> Res<()> {
    match thrown {
        Thrown::Value(error) => vm.settle(promise, PromiseState::Rejected(error)),
        fatal => Err(fatal),
    }
}

/** Reject a promise with a TypeError carrying `message`. */
pub(super) fn reject_type_error(vm: &mut Vm, promise: ObjectId, message: &str) -> Res<JsValue> {
    let error = vm.type_error(message);
    settle_thrown(vm, promise, error)?;
    Ok(JsValue::Object(promise))
}

/** A waited-for dependency settled: try the module again. */
pub(super) fn resume_native(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(JsValue::String(key)) = vm.slots(call.callee).first() {
        try_run(vm, &key.lossy(), &[])?;
    }
    Ok(JsValue::Undefined)
}

/** The microtask `import()` queues. */
pub(super) fn advance_native(vm: &mut Vm, _: Call) -> Res<JsValue> {
    advance_dynamic(vm)?;
    Ok(JsValue::Undefined)
}

/** A dynamic import's graph finished: fulfill it with the namespace. */
pub(super) fn imported_native(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    if let (Some(JsValue::Object(promise)), Some(namespace)) = (slots.first(), slots.get(1)) {
        vm.resolve_promise(*promise, namespace.clone())?;
    }
    Ok(JsValue::Undefined)
}

/** A dynamic import's graph failed: reject it with the same error. */
pub(super) fn refused_native(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(JsValue::Object(promise)) = vm.slots(call.callee).first() {
        vm.settle(*promise, PromiseState::Rejected(call.arg(0)))?;
    }
    Ok(JsValue::Undefined)
}

/** An async module script failed after the load moved on: report it as uncaught. */
pub(super) fn report_native(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if vm.page.environment.worker_name.is_some() {
        crate::v9::platform::workers::report_exception(vm, &call.arg(0))?;
        if vm.page.environment.worker_module && !worker_ready(vm) {
            crate::v9::platform::workers::close(vm, call)?;
        }
    } else {
        vm.report_error(&call.arg(0));
    }
    Ok(JsValue::Undefined)
}
