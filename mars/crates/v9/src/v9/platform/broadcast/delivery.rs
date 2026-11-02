/** A native task delivers one immutable packet to an exact still-open BroadcastChannel endpoint. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::{WorkerData, worker_clone};

/** Recipients are selected by the native broker at post time; close/GC still suppress stale tasks. */
pub(in crate::v9) fn receive(vm: &mut Vm, id: u64, data: &WorkerData, origin: &str) -> Res<()> {
    if origin.len() > crate::constants::URL_BYTES {
        return Err(EngineError::invalid("Invalid BroadcastChannel origin.").into());
    }
    data.bytes()?;
    if closed(vm, id) || vm.page.state.worker_closed {
        return Ok(());
    }
    let object = vm.page.state.broadcast.channels[&id].object;
    vm.pin(JsValue::Object(object));
    let (kind, value) = match worker_clone::decode(vm, data) {
        Ok(value) => ("message", value),
        Err(Thrown::Value(_)) => ("messageerror", JsValue::Null),
        Err(error) => return Err(error),
    };
    vm.pin(value.clone());
    let event = super::super::events::create_event(vm, kind, false, false)?
        .object()
        .expect("event");
    vm.set_proto(
        event,
        Some(
            *bindings::MESSAGE_PROTOTYPE
                .get()
                .expect("installed MessageEvent"),
        ),
    )?;
    for (name, value) in [
        ("data", value),
        ("origin", JsValue::string(origin)?),
        ("source", JsValue::Null),
        ("lastEventId", JsValue::string("")?),
    ] {
        vm.put(event, name, value)?;
    }
    let ports = vm.new_array(Vec::new())?;
    vm.put(event, "ports", JsValue::Object(ports))?;
    if let Some(crate::v9::vm::data::Class::Host(data)) =
        vm.heap.get_mut(event).map(|object| &mut object.class)
        && let crate::v9::vm::data::HostData::Event(event) = data.as_mut()
    {
        event.trusted = true;
    }
    super::super::events::dispatch(vm, super::super::Target::Object(object), event)?;
    Ok(())
}
