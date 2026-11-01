/** BroadcastChannel owns branded endpoints and inert ordered effects; native publication owns routing. */
mod bindings;
mod delivery;
mod state;
mod wire;
pub(in crate::v9) use bindings::{handler, install, message_prototype};
pub(in crate::v9) use delivery::receive;
pub(in crate::v9) use state::{State, publish, roots, sweep, validate};
pub use wire::{
    BROADCAST_CHANNELS, BROADCAST_COMMANDS, BROADCAST_EFFECT_BYTES, BROADCAST_NAME_UNITS,
    BroadcastCommand, BroadcastEndpoint,
};

use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::heap::{Key, Prop, Slot};
use crate::v9::vm::{Res, Vm};

/** Private branding makes the endpoint non-cloneable without relying on a writable prototype. */
fn brand() -> Key {
    Key::private("broadcastChannel")
}

/** Native table identity survives subclassing, close and prototype changes. */
fn identity(vm: &mut Vm, value: &JsValue) -> Res<u64> {
    vm.page
        .state
        .broadcast
        .channels
        .iter()
        .find(|(_, record)| Some(record.object) == value.object())
        .map(|(id, _)| *id)
        .ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** Handler slots are private native data, never author getters inspected by GC. */
fn handler_key(name: &str) -> Key {
    Key::private(&format!("broadcast:{name}"))
}

/** One complete command is admitted before any endpoint or host queue mutation. */
fn admit(vm: &mut Vm, command: &BroadcastCommand) -> Res<()> {
    let commands = &vm.page.effects.broadcast_commands;
    let used = commands.iter().try_fold(0usize, |bytes, item| {
        item.bytes().map(|size| bytes.saturating_add(size))
    })?;
    let added = command.bytes()?;
    if commands.len() >= BROADCAST_COMMANDS || used.saturating_add(added) > BROADCAST_EFFECT_BYTES {
        return Err(
            EngineError::limit("BroadcastChannel effect bytes", BROADCAST_EFFECT_BYTES).into(),
        );
    }
    vm.charge(used.saturating_add(added).saturating_add(1))?;
    Ok(())
}

/** Closed wrappers remain branded and keep their name while script still references them. */
fn closed(vm: &Vm, id: u64) -> bool {
    vm.page
        .state
        .broadcast
        .channels
        .get(&id)
        .is_none_or(|record| record.closed)
}
