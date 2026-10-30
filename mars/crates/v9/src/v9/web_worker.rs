/** Worker execution opens an isolated global directly, without constructing a hidden document. */
use super::{Compiled, Execution, Limits, PageInput, finish, on_v9_stack, session};
use crate::protocol::{EngineError, EngineResult};

/** The owner supplies sources synchronously under its policy; None is an ordinary load failure. */
pub trait ImportScriptsHost: Sync {
    /// The URL is a source request within the caller's bound worker policy, never authority.
    fn load(&self, url: &str) -> EngineResult<Option<String>>;
}

/** Evaluate a classic dedicated worker's initial script; later tasks retain its returned state. */
pub fn load_worker(input: PageInput<'_>) -> EngineResult<Execution> {
    on_v9_stack(|| load(input))
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
    Ok(())
}

/** Run a true Script in the worker realm, retaining declarations and imported closures. */
fn load(input: PageInput<'_>) -> EngineResult<Execution> {
    validate(&input)?;
    let (mut vm, compiled) = session(
        Limits::default(),
        input.sources,
        input.source_keys,
        true,
        Some(input.document),
        input.storage,
        input.host,
        input.environment,
        None,
        input.modules,
    )?;
    let Compiled::Each(mut scripts) = compiled else {
        return Err(EngineError::invalid("Invalid worker compilation mode."));
    };
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
