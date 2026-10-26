/** Trusted key events and their native default actions share one focused-target, bounded execution path. */
use super::{BODY, Target, events, interfaces};
use crate::v9::keyboard::KeyboardKey;
use crate::v9::vm::{Res, Vm};

/** Dispatch Escape to the actual focus and honor cancellation before requesting the top popover close watcher. */
pub(in crate::v9) fn press_key(vm: &mut Vm, key: KeyboardKey, requested: Option<usize>) -> Res<()> {
    let target = focused(vm);
    if requested.is_some_and(|node| node != target || !vm.page.connected(node)) {
        return Err(crate::v9::keyboard::invalid_target().into());
    }
    let KeyboardKey::Escape = key;
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
