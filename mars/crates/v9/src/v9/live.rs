/**
 * A live page realm: one `Vm` the in-process service keeps between tasks instead of rebuilding
 * one from committed state for every timer run, response or event (stage 3 of the resident
 * realm). A task whose base is the live token runs in place: the page is re-based on the step's
 * projection, environment and storage, the task body runs, and `finish_live` packages a copy of
 * the state so tokens, `keep` and rollback work exactly as for a resident step; any other base
 * restores a realm from that kept state through the same `session` a resident step took. Loads
 * and module continuations keep the consuming path, and the task after them restores once.
 * A realm holds thread-local values (`Rc`), so it lives and runs only on the executor thread.
 */
use super::platform::{self, PageEnvironment};
use super::vm::{Limits, Vm};
use super::{
    DocumentHost, Execution, HostResponse, ImageResponse, KeyboardKey, ModuleInput, PageEvent,
    SourceKey, StorageState, Stroke, VmState, compile_sources, execution_thread, finish_live,
    inline_sources, modules, offsets, report_media_changes, report_resize, script_deadline,
    session, validate_languages,
};
use crate::document::ScriptDocument;
use crate::protocol::{EngineError, EngineResult};
use std::panic::{AssertUnwindSafe, catch_unwind};

/** What a task runs against the live realm; each maps to one page entry point's body. */
pub enum LiveAction {
    Event(PageEvent),
    Click {
        target: usize,
    },
    Input {
        target: usize,
        value: Option<String>,
    },
    Typed {
        target: usize,
        strokes: Vec<Stroke>,
        value: Option<String>,
    },
    Keyboard {
        key: KeyboardKey,
        target: Option<usize>,
    },
    Timers {
        now_ms: f64,
        budget: usize,
    },
    Observe,
    Media,
    Resize {
        complete: bool,
    },
    Resume(Vec<HostResponse>),
    Images(Vec<ImageResponse>),
}

/** A task's inputs: as `PageInput`, with a host that outlives the realm. */
pub struct LiveInput<'a> {
    pub sources: &'a [String],
    pub source_keys: &'a [SourceKey],
    pub module_keys: &'a [SourceKey],
    pub document: ScriptDocument,
    pub storage: StorageState,
    pub host: &'a DocumentHost<'static>,
    pub environment: &'a PageEnvironment,
    pub modules: &'a ModuleInput,
}

/** The realm. Opaque to the service: it only runs tasks and restores. */
pub struct LiveVm {
    vm: Vm<'static>,
}

impl LiveVm {
    /** A realm over committed `state`, built as a resident step builds its VM. */
    pub fn restore(input: &LiveInput<'_>, state: VmState) -> EngineResult<Self> {
        executor()?;
        let vm = guarded(|| {
            session(
                Limits::default(),
                input.sources,
                input.source_keys,
                true,
                Some(input.document.clone()),
                input.storage.clone(),
                input.host,
                input.environment,
                Some(state),
                input.modules,
                input.module_keys,
            )
            .map(|(vm, _)| vm)
        })?;
        Ok(Self { vm })
    }

    /** Run one task in place and package its outcome; the realm stays current for the next. */
    pub fn run(&mut self, input: &LiveInput<'_>, action: LiveAction) -> EngineResult<Execution> {
        executor()?;
        let vm = &mut self.vm;
        guarded(|| {
            rebase(vm, input)?;
            body(vm, action)?;
            finish_live(vm)
        })
    }
}

/** A realm runs only on an executor thread: its stack and its thread-local values live there. */
fn executor() -> EngineResult<()> {
    if execution_thread::has_stack() {
        Ok(())
    } else {
        Err(EngineError::unsupported(
            "A live V9 realm runs only on its executor thread.",
        ))
    }
}

/** The task's panic boundary: a panic is an engine error, and the caller drops the realm. */
fn guarded<T>(work: impl FnOnce() -> EngineResult<T>) -> EngineResult<T> {
    catch_unwind(AssertUnwindSafe(work))
        .unwrap_or_else(|_| Err(EngineError::invalid("V9 stopped on an internal error.")))
}

/**
 * Put the step's projection, environment and storage under the realm, as `session` does for a
 * restored state, without rebuilding the heap, globals, registry or bootstraps: validate the
 * inputs, open a fresh per-task page over the live page state, re-open the module map over
 * the delivered sources, adopt the page scripts from the cache, and report a viewport change.
 */
fn rebase(vm: &mut Vm<'static>, input: &LiveInput<'_>) -> EngineResult<()> {
    input.document.validate()?;
    input.storage.validate()?;
    let environment = input.environment;
    environment.handler_policy.validate()?;
    mars_core::dom::validate_metadata(&environment.metadata, &environment.script_attributes)?;
    platform::frames::validate_routes(environment, Some(&input.document))?;
    platform::frame_directory::validate(environment)?;
    if vm.page.state.identity != input.document.identity {
        return Err(EngineError::invalid(
            "V9 state belongs to another document.",
        ));
    }
    let languages = validate_languages(input.host.languages)?;
    let state = std::mem::take(&mut vm.page.state);
    let mut page = platform::Page::new(
        environment.clone(),
        languages,
        input.host.fragments,
        Some(input.document.clone()),
        state,
        input.storage.clone(),
    );
    page.canvas_host = input.host.canvas;
    page.imports = input.host.imports;
    page.inline_scripts = inline_sources(input.sources, &environment.scripts);
    vm.page = page;
    vm.registry.begin_step();
    // The map is re-opened only when the app delivered a source since (it only appends): on
    // YouTube re-adopting every module source each task was a thirtieth of the realm's time.
    if vm.modules.opened_sources() != input.modules.sources.len() {
        let module_state = std::mem::take(&mut vm.modules.state);
        modules::Modules::open(
            vm,
            input.modules,
            input.module_keys,
            input.sources,
            &offsets(input.sources),
            module_state,
        )?;
    }
    compile_sources(vm, input.sources, input.source_keys, true)?;
    platform::images::validate_targets(&vm.page)?;
    platform::media::validate(vm)?;
    vm.page.start_execution_clock();
    vm.deadline = Some(std::time::Instant::now() + script_deadline());
    vm.remaining = vm.limits.operations;
    report_resize(vm)?;
    report_media_changes(vm)
}

/** The task body an action names. */
fn body(vm: &mut Vm, action: LiveAction) -> EngineResult<()> {
    match action {
        LiveAction::Event(event) => super::event_body(vm, event),
        LiveAction::Click { target } => super::click_body(vm, target),
        LiveAction::Input { target, value } => super::input_body(vm, target, value),
        LiveAction::Typed {
            target,
            strokes,
            value,
        } => super::keyboard::typed_body(vm, target, strokes, value),
        LiveAction::Keyboard { key, target } => super::keyboard::keyboard_body(vm, key, target),
        LiveAction::Timers { now_ms, budget } => super::timers_body(vm, now_ms, budget),
        LiveAction::Observe => super::observe_body(vm),
        LiveAction::Media => Ok(()),
        LiveAction::Resize { complete } => super::resize_body(vm, complete),
        LiveAction::Resume(responses) => super::resume_body(vm, responses),
        LiveAction::Images(responses) => super::images_body(vm, responses),
    }
}
