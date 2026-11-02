/** The shell's debug delegate follows real isolate lifecycle and receives VM events without retaining values. */
use crate::v9::value::JsValue;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use serde::{Deserialize, Serialize};

/** Bounded delegate bookkeeping survives host state transport and remains isolate-local. */
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct State {
    enabled: bool,
    exceptions: u64,
    promises_created: u64,
    promises_settled: u64,
    last_realm: u32,
}
impl State {
    pub(in crate::v9) fn is_empty(&self) -> bool {
        !self.enabled
            && self.exceptions == 0
            && self.promises_created == 0
            && self.promises_settled == 0
    }
    fn event(&mut self, realm: u32, kind: Event) {
        if !self.enabled {
            return;
        }
        self.last_realm = realm;
        let counter = match kind {
            Event::Exception => &mut self.exceptions,
            Event::PromiseCreated => &mut self.promises_created,
            Event::PromiseSettled => &mut self.promises_settled,
        };
        *counter = counter.saturating_add(1);
    }
}
/** Event categories are emitted only by actual VM transitions. */
#[derive(Clone, Copy)]
pub(in crate::v9) enum Event {
    Exception,
    PromiseCreated,
    PromiseSettled,
}

/** Register the d8 namespace on each real shell global. */
pub(super) fn install(builder: &mut Builder, d8: crate::v9::value::ObjectId) {
    let namespace = builder.object(Some(builder.ids.object_prototype));
    builder.method(namespace, "enable", 0, enable);
    builder.method(namespace, "disable", 0, disable);
    builder.alias(d8, "debugger", namespace);
}
fn enable(vm: &mut Vm, _: Call) -> Res<JsValue> {
    vm.page.state.debug_delegate.enabled = true;
    Ok(JsValue::Undefined)
}
fn disable(vm: &mut Vm, _: Call) -> Res<JsValue> {
    vm.page.state.debug_delegate.enabled = false;
    Ok(JsValue::Undefined)
}
/** Deliver an event to the currently installed delegate in the current realm. */
pub(in crate::v9) fn record(vm: &mut Vm, kind: Event) {
    let realm = vm
        .frames
        .last()
        .map_or(vm.active_realm, |frame| frame.realm);
    vm.page.state.debug_delegate.event(realm, kind);
}
