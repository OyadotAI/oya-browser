/** Trusted key events and their native default actions share one focused-target, bounded execution path. */
use super::{BODY, Target, events, interfaces};
use crate::v9::keyboard::KeyboardKey;
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
