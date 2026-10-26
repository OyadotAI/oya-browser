/** Typed trusted keyboard entry points run over committed state without accepting page-authored event authority. */
use super::{Execution, PageInput, VmState, finish, on_v9_stack, open, platform};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

/** Only implemented native key actions can cross the service boundary. */
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardKey {
    /** Dispatch keydown, run the top close watcher's default action, then dispatch keyup. */
    Escape,
}

/** Dispatch a real native key to the current focused node; an optional target must match that focus. */
pub fn keyboard_page(
    input: PageInput<'_>,
    state: VmState,
    key: KeyboardKey,
    target: Option<usize>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, super::vm::Limits::default())?;
        platform::press_key(&mut vm, key, target).map_err(super::fatal)?;
        vm.drain_jobs().map_err(super::fatal)?;
        finish(vm)
    })
}

/** A host cannot redirect a focused key to a stale or unrelated node. */
pub(super) fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The keyboard target is not the connected focused element.",
        "Use the current focused element or omit node_id.",
    )
}
