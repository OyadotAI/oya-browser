/** Opt-in upstream test host: executes host-supplied source as global Scripts, without IO.
The production browser has neither these globals nor an upstream fixture registry. */
use super::{arg_string, text};
use crate::v9::value::JsValue;
use crate::v9::vm::data::PromiseState;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
mod async_hooks;
mod cputrace;
mod fast_api;
mod files;
mod test_dom;
pub(in crate::v9) use test_dom::valid as validate_native_test_object;
mod memory;
mod output;
mod profiling;
pub(in crate::v9) use profiling::{
    Clock as ProfileClock, State as ProfileState, console as profile_console,
    tick as profile_tick, validate as validate_profile, validate_state as validate_profile_state,
};
mod realms;
mod serializer;
mod shadow;
mod strings;
mod workers;
pub(in crate::v9) use async_hooks::{
    async_hook_after, async_hook_before, async_hook_init, async_hook_resolve, async_hook_resource,
    async_hook_unwind, expose as expose_async_hooks,
};
pub(in crate::v9) use cputrace::validate as validate_environment;
pub(in crate::v9) use memory::measure as measure_memory;
pub(in crate::v9) use output::console as console_output;
use std::sync::OnceLock;
pub(in crate::v9) use workers::{WorkerGraph, activate as worker_activate, worker_decode};

/** Fresh globals expose only native test extensions requested by the owning environment's actual flags. */
pub(in crate::v9) fn expose_conformance_globals(
    vm: &mut Vm,
    global: crate::v9::value::ObjectId,
    bank: u32,
) {
    super::expose_async_hooks(vm, global, bank);
    fast_api::expose(vm, bank);
    cputrace::expose(vm, global, bank);
    expose_iterator_options(vm, bank);
}

/** Native embedder receiver state is not ordinary structured data and cannot acquire a fake clone brand. */
pub(in crate::v9) fn conformance_host_object(vm: &Vm, value: &JsValue) -> bool {
    value
        .object()
        .and_then(|id| vm.heap.get(id))
        .is_some_and(|object| {
            [
                "fast-api-brand",
                "conformance-worker-id",
                test_dom::BRAND_KEY,
            ]
            .iter()
            .any(|name| {
                object
                    .props
                    .get(&crate::v9::vm::heap::Key::private(name))
                    .is_some()
            })
        })
}

/** The host task performs collection before resolving its returned promise. */
static ASYNC_GC: OnceLock<u32> = OnceLock::new();

/** A host exit is uncatchable JavaScript control flow, never an assertion success shim. */
const EXIT_CODE: &str = "conformance_exit";

/** The loader must stop the whole source list when the test host requests process termination. */
pub(in crate::v9) fn is_test_exit(error: &crate::protocol::EngineError) -> bool {
    error.code == EXIT_CODE
}

/** Only real fixture operations are installed; absent d8 capabilities remain absent. */
pub(super) fn install(builder: &mut Builder) {
    realms::install(builder);
    shadow::install(builder);
    workers::install(builder);
    memory::install(builder);
    strings::install(builder);
    async_hooks::install(builder);
    output::install(builder);
    cputrace::install(builder);
    files::install(builder);
    let d8 = builder.object(Some(builder.ids.object_prototype));
    serializer::install(builder, d8);
    fast_api::install(builder, d8);
    test_dom::install(builder, d8);
    profiling::install(builder, d8);
    builder.method(d8, "terminate", 0, terminate);
    builder.method(d8, "terminateNow", 0, terminate);
    let file = builder.object(Some(builder.ids.object_prototype));
    builder.method(file, "execute", 1, execute);
    builder.method(file, "read", 1, read);
    builder.alias(d8, "file", file);
    super::wasm::install_conformance(builder, d8);
    builder.global("d8", d8);
    let collect = builder.function("gc", 0, collect);
    builder.global("gc", collect);
    let quit = builder.function("quit", 1, quit);
    builder.global("quit", quit);
    let _ = ASYNC_GC.set(builder.native("", 0, collect_task));
}

/** Return an uncatchable engine outcome so the owning host ends this test with the requested code. */
fn quit(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let status = crate::v9::value::to_int32(vm.to_number(&call.arg(0))?);
    let mut hint = String::from("The upstream test requested host termination.");
    if status != 0 && !vm.page.effects.console.is_empty() {
        hint.push_str("\nHost console before exit:");
        for entry in &vm.page.effects.console {
            hint.push_str(&format!("\n{}: {}", entry.level, entry.text));
        }
    }
    Err(crate::protocol::EngineError::new(EXIT_CODE, status.to_string(), &hint).into())
}

/** Host interruption is delivered at this native boundary and cannot be caught by script. */
fn terminate(_: &mut Vm, _: Call) -> Res<JsValue> {
    Err(crate::protocol::EngineError::new(
        EXIT_CODE,
        "0",
        "The upstream host interrupted this isolate.",
    )
    .into())
}

/** A fixture lookup cannot escape the registry: paths never reach an OS operation. */
fn source(vm: &mut Vm, call: &Call) -> Res<String> {
    let path = arg_string(vm, call, 0)?.lossy();
    vm.page
        .environment
        .conformance_sources
        .get(&path)
        .cloned()
        .ok_or_else(|| vm.type_error(&format!("Test host did not supply fixture {path}.")))
}

/** d8.file.execute runs ordered global Scripts and reports/wraps their ordinary exceptions like pinned d8. */
fn execute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    files::execute(vm, call)
}

/** d8.file.read returns the exact supplied source, without evaluating it. */
fn read(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let source = source(vm, &call)?;
    text(vm, &source)
}

/** Minor requests sweep the nursery; major requests trace and sweep every generation.
Asynchronous requests collect in a later task and resolve only after the real sweep finishes. */
fn collect(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let options = call.arg(0);
    if matches!(options, JsValue::Object(_)) {
        let kind = option(vm, &options, "type")?;
        let execution = option(vm, &options, "execution")?;
        option(vm, &options, "flavor")?;
        if kind.as_deref() == Some("major-snapshot") {
            return Err(vm.unsupported_error("heap snapshots from the test GC host"));
        }
        let young = kind.as_deref() == Some("minor");
        if execution.as_deref() == Some("async") {
            return collect_later(vm, young);
        }
        if young {
            vm.collect_young()?;
            return Ok(JsValue::Undefined);
        }
    }
    vm.collect_now()?;
    Ok(JsValue::Undefined)
}

/** V8's extension reads string-valued options without coercing arbitrary objects. */
fn option(vm: &mut Vm, object: &JsValue, name: &str) -> Res<Option<String>> {
    Ok(match vm.get(object, &super::key(name))? {
        JsValue::String(value) => Some(value.lossy()),
        _ => None,
    })
}

/** A rooted native closure keeps the promise pending until the scheduled collection runs. */
fn collect_later(vm: &mut Vm, young: bool) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    let callback = vm.native_closure(
        *ASYNC_GC.get().unwrap(),
        vec![JsValue::Object(promise), JsValue::Boolean(young)],
    )?;
    crate::v9::platform::schedule_task(vm, JsValue::Object(callback), Vec::new())?;
    Ok(JsValue::Object(promise))
}

/** Cleanup tasks queued by collection wait until after the promise's continuation microtasks. */
fn collect_task(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let Some(JsValue::Object(promise)) = slots.first().cloned() else {
        return Ok(JsValue::Undefined);
    };
    if slots.get(1) == Some(&JsValue::Boolean(true)) {
        vm.collect_young()?;
    } else {
        vm.collect_now()?;
    }
    vm.settle(promise, PromiseState::Fulfilled(JsValue::Undefined))?;
    Ok(JsValue::Undefined)
}

/** Staged helpers are absent unless their real source flags enable them in a fresh shell realm. */
fn expose_iterator_options(vm: &mut Vm, bank: u32) {
    if !vm.page.environment.conformance_d8 {
        return;
    }
    let prototype = crate::v9::vm::realm::banks::bank(bank)
        .ids
        .iterator_prototype;
    vm.materialize(prototype)
        .expect("installed iterator prototype");
    for (name, enabled) in [
        (
            "includes",
            vm.page.environment.conformance_iterator_includes,
        ),
        ("join", vm.page.environment.conformance_iterator_join),
    ] {
        if !enabled {
            vm.heap
                .remove(prototype, &crate::v9::vm::heap::Key::from(name));
        }
    }
}
