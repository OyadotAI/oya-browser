/** Worker execution opens an isolated global directly, without constructing a hidden document. */
use super::{
    Compiled, Execution, Limits, PageInput, VmState, finish, modules, on_v9_stack, session,
};
use crate::protocol::{EngineError, EngineResult};

/** The owner supplies sources synchronously under its policy; None is an ordinary load failure. */
pub trait ImportScriptsHost: Sync {
    /// The URL is a source request within the caller's bound worker policy, never authority.
    fn load(&self, url: &str) -> EngineResult<Option<String>>;
}

/** Evaluate the selected grammar in a dedicated worker; later tasks retain the isolated state. */
pub fn load_worker(input: PageInput<'_>) -> EngineResult<Execution> {
    on_v9_stack(|| load(input, None))
}

/** Resume fetched static or dynamic modules without creating document or script-element state. */
pub fn resume_worker_modules(input: PageInput<'_>, state: VmState) -> EngineResult<Execution> {
    on_v9_stack(|| load(input, Some(state)))
}

/** Reject document capabilities before opening a worker's interpreter state. */
fn validate(input: &PageInput<'_>) -> EngineResult<()> {
    if input.environment.worker_name.is_none()
        || !input.document.nodes.is_empty()
        || input.sources.len() != 1
        || input.storage != super::StorageState::default()
    {
        return Err(EngineError::invalid("Invalid dedicated worker input."));
    }
    let entries = &input.modules.entries;
    let valid = if input.environment.worker_module {
        entries.len() == 1
            && entries[0].index == 0
            && entries[0].kind == super::EntryKind::Module
            && entries[0].url.as_deref() == Some(input.environment.url.as_str())
    } else {
        entries.is_empty()
    };
    if !valid {
        return Err(EngineError::invalid(
            "Invalid dedicated worker module entry.",
        ));
    }
    Ok(())
}

/** Run a true Script in the worker realm, retaining declarations and imported closures. */
fn load(input: PageInput<'_>, state: Option<VmState>) -> EngineResult<Execution> {
    validate(&input)?;
    let resumed = state.is_some();
    let (mut vm, compiled) = session(
        Limits::default(),
        input.sources,
        input.source_keys,
        true,
        Some(input.document),
        input.storage,
        input.host,
        input.environment,
        state,
        input.modules,
        input.module_keys,
    )?;
    let Compiled::Each(mut scripts) = compiled else {
        return Err(EngineError::invalid("Invalid worker compilation mode."));
    };
    if resumed || input.environment.worker_module {
        return module_turn(vm, resumed);
    }
    let script = scripts.remove(0)?;
    let result = vm.run_script(script).and_then(|_| vm.drain_jobs());
    let failure = match result {
        Ok(()) => None,
        Err(super::Thrown::Fatal(error)) => return Err(error),
        Err(super::Thrown::Value(value)) => {
            let error = vm.uncaught(super::Thrown::Value(value.clone()), Default::default());
            super::platform::workers::report_exception(&mut vm, &value)
                .map_err(|thrown| vm.uncaught(thrown, Default::default()))?;
            Some(super::ScriptFailure { index: 0, error })
        }
    };
    let mut execution = finish(vm)?;
    execution.failed.extend(failure);
    Ok(execution)
}

/** Only the first module graph can pause worker initialization; dynamic imports keep their promises. */
fn module_turn(mut vm: super::Vm<'_>, resumed: bool) -> EngineResult<Execution> {
    let initial = !resumed || vm.modules.state.cursor.take().is_some();
    if initial && vm.page.environment.worker_module {
        match modules::run_entry(&mut vm, 0) {
            modules::Outcome::Ran => {}
            modules::Outcome::Waiting => vm.modules.state.cursor = Some(0),
            modules::Outcome::Failed(error) => return Err(error),
        }
    }
    modules::advance_dynamic(&mut vm)
        .and_then(|()| vm.drain_jobs())
        .map_err(|thrown| vm.uncaught(thrown, Default::default()))?;
    finish(vm)
}
