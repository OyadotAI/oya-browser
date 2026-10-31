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
    /** Focus the target, dispatch keydown, keypress and keyup, and report through
    `effects.key_default` whether the app should do Enter's native action (submit, newline).
    Google submits its search from its own keydown handler and cancels the newline. */
    Enter,
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
        platform::host_bridge::activate(&mut vm);
        super::platform::media::activate(&mut vm.page);
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
