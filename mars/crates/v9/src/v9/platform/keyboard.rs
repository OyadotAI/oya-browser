/** Trusted key events and their native default actions share one focused-target, bounded execution path. */
use super::{BODY, Target, events, interfaces};
use crate::v9::keyboard::{KeyboardKey, Stroke};
use crate::v9::vm::{Res, Vm};

/** Dispatch Escape to the actual focus and honor cancellation before requesting the top popover close watcher. */
pub(in crate::v9) fn press_key(vm: &mut Vm, key: KeyboardKey, requested: Option<usize>) -> Res<()> {
    match key {
        KeyboardKey::Escape => escape(vm, requested),
        KeyboardKey::Enter => enter(vm, requested),
    }
}

/** Enter at a field: it takes focus as the keystroke's target, then keydown and keypress run,
and unless either is cancelled the app performs the native default. */
fn enter(vm: &mut Vm, requested: Option<usize>) -> Res<()> {
    let Some(target) = requested.filter(|node| vm.page.connected(*node)) else {
        return Err(crate::v9::keyboard::invalid_target().into());
    };
    super::focus(vm, target)?;
    let node = Target::Node(target);
    let proceed = events::fire_keyboard(vm, node, "keydown", "Enter")?
        && events::fire_keyboard(vm, node, "keypress", "Enter")?;
    let target = focused(vm);
    events::fire_keyboard(vm, Target::Node(target), "keyup", "Enter")?;
    vm.page.effects.key_default = proceed;
    Ok(())
}

/** Escape at the focused element: keydown, the top close watcher, keyup. */
fn escape(vm: &mut Vm, requested: Option<usize>) -> Res<()> {
    let target = focused(vm);
    if requested.is_some_and(|node| node != target || !vm.page.connected(node)) {
        return Err(crate::v9::keyboard::invalid_target().into());
    }
    if events::fire_keyboard(vm, Target::Node(target), "keydown", "Escape")? {
        interfaces::popover::close_top(vm)?;
    }
    let target = focused(vm);
    events::fire_keyboard(vm, Target::Node(target), "keyup", "Escape")?;
    Ok(())
}

/** A detached old focus falls back to body exactly as Document.activeElement does. */
fn focused(vm: &mut Vm) -> usize {
    vm.page
        .state
        .focused
        .filter(|node| vm.page.connected(*node))
        .unwrap_or(BODY)
}

/** Delivers each stroke to the field as a person's keyboard would, and returns whether its edit went through. `known` says the engine holds the field's text; a secret field's text never enters the engine, so its edits are left to the caller. */
pub(in crate::v9) fn type_strokes(
    vm: &mut Vm,
    target: usize,
    strokes: &[Stroke],
    known: bool,
) -> Res<Vec<bool>> {
    if !vm.page.connected(target) {
        return Err(crate::v9::keyboard::invalid_target().into());
    }
    super::focus(vm, target)?;
    let mut applied = Vec::with_capacity(strokes.len());
    for stroke in strokes {
        applied.push(type_stroke(vm, target, stroke, known)?);
    }
    Ok(applied)
}

/** One stroke's events in browser order: keydown (and keypress for a character), beforeinput,
the edit, input, then keyup. A cancelled keydown, keypress or beforeinput stops the edit, and
keyup still follows, as in a browser. */
fn type_stroke(vm: &mut Vm, target: usize, stroke: &Stroke, known: bool) -> Res<bool> {
    let node = Target::Node(target);
    let key = key_name(stroke);
    let proceed = match &key {
        Some(name) => key_down(vm, node, stroke, name)?,
        None => true,
    };
    let inserted = edit_data(stroke);
    let kind = input_type(stroke);
    let edit =
        proceed && events::fire_input(vm, node, "beforeinput", kind, inserted.as_deref(), true)?;
    if edit {
        apply_edit(vm, target, stroke, known)?;
        events::fire_input(vm, node, "input", kind, inserted.as_deref(), false)?;
        if matches!(stroke, Stroke::Replace(_)) {
            events::fire(vm, node, "change", true, false)?;
        }
    }
    if let Some(name) = &key {
        events::fire_keyboard(vm, node, "keyup", name)?;
    }
    vm.drain_jobs()?;
    Ok(edit)
}

/** keydown, then keypress for a printable character; false when a handler cancelled either. */
fn key_down(vm: &mut Vm, node: Target, stroke: &Stroke, name: &str) -> Res<bool> {
    if !events::fire_keyboard(vm, node, "keydown", name)? {
        return Ok(false);
    }
    match stroke {
        Stroke::Character(_) => events::fire_keyboard(vm, node, "keypress", name),
        Stroke::Backspace | Stroke::Replace(_) => Ok(true),
    }
}

/** The key a stroke presses; a whole-text replacement presses no key. */
fn key_name(stroke: &Stroke) -> Option<String> {
    match stroke {
        Stroke::Character(character) => Some(character.to_string()),
        Stroke::Backspace => Some("Backspace".into()),
        Stroke::Replace(_) => None,
    }
}

/** The `inputType` a stroke's edit reports, as browsers name it. */
fn input_type(stroke: &Stroke) -> &'static str {
    match stroke {
        Stroke::Backspace => "deleteContentBackward",
        Stroke::Character(_) | Stroke::Replace(_) => "insertText",
    }
}

/** The `data` an edit reports: the inserted text, or none when it inserts nothing. */
fn edit_data(stroke: &Stroke) -> Option<String> {
    match stroke {
        Stroke::Character(character) => Some(character.to_string()),
        Stroke::Backspace => None,
        Stroke::Replace(text) => Some(text.clone()),
    }
}

/** Writes an edit into the field's text and records it, so the document follows. A secret field's
text stays out of the engine, so nothing is written here. */
fn apply_edit(vm: &mut Vm, target: usize, stroke: &Stroke, known: bool) -> Res<()> {
    if !known {
        return Ok(());
    }
    let current = field_text(vm, target);
    let next = edited(&current, stroke);
    vm.page
        .set_value(target, &next)
        .map_err(|error| super::dom::dom_error(vm, error))
}

/** The field's text as the engine holds it: the value it was given, else its `value` attribute. */
fn field_text(vm: &mut Vm, target: usize) -> String {
    if let Some(text) = vm.page.state.values.get(&target) {
        return text.clone();
    }
    vm.page
        .view(target)
        .and_then(|view| {
            view.value
                .clone()
                .or_else(|| view.attributes.get("value").cloned())
        })
        .unwrap_or_default()
}

/** The text after one edit: a character is appended, Backspace drops the last character, and a
replacement sets the whole text. */
fn edited(text: &str, stroke: &Stroke) -> String {
    match stroke {
        Stroke::Character(character) => format!("{text}{character}"),
        Stroke::Backspace => {
            let mut next = text.to_owned();
            next.pop();
            next
        }
        Stroke::Replace(whole) => whole.clone(),
    }
}
