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

/** One keystroke a person types into a field, or a whole-text replacement of its value. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stroke {
    /** A printable character: keydown, keypress, beforeinput, input and keyup, then it is inserted. */
    Character(char),
    /** Backspace: keydown, beforeinput, input and keyup; no keypress, as in a browser. */
    Backspace,
    /** A fill: the whole text replaced in one edit, with beforeinput and input, then change, and no key events. */
    Replace(String),
    /** A secret field's stroke: the text is withheld, so the key is "Unidentified" and every edit reports no data. `press` says whether the stroke presses a key (a typed character) or only fills (a replacement). */
    Concealed { press: bool },
}

/** Typed strokes into one field under one execution, each as a person's keyboard delivers it. `value` is the field's text, or None for a secret field whose text never enters the engine. The result says per stroke whether its edit went through. */
pub fn typed_page(
    input: PageInput<'_>,
    state: VmState,
    target: usize,
    strokes: Vec<Stroke>,
    value: Option<String>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, super::vm::Limits::default())?;
        if !vm.page.exists(target) {
            return Err(EngineError::invalid("Invalid projected input target."));
        }
        let known = value.is_some();
        if let Some(text) = value {
            vm.page.state.values.insert(target, text);
        }
        vm.remaining = vm.limits.operations;
        platform::host_bridge::activate(&mut vm);
        platform::media::activate(&mut vm.page);
        vm.page.effects.typed =
            platform::type_strokes(&mut vm, target, &strokes, known).map_err(super::fatal)?;
        finish(vm)
    })
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
