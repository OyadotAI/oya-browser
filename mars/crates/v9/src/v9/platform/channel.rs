/**
 * MessageChannel: two entangled ports. A message posted on one port is delivered to the other
 * as a `message` event from a zero-delay task, which is how schedulers (React, YouTube's) yield
 * between chunks of work. Data is passed as is; V9 has one realm, so nothing needs cloning
 * across it.
 */
use super::{Target, events, timers};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The port prototype and the native task that delivers one message. */
struct Ids {
    port: u32,
    deliver: u32,
}

/** Set once while the realm is built. */
static IDS: OnceLock<Ids> = OnceLock::new();

/** The channel intrinsics. */
fn ids() -> &'static Ids {
    let _ = crate::v9::vm::realm::realm();
    IDS.get()
        .expect("channel intrinsics are installed with the realm")
}

/** Install MessageChannel and MessagePort. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let port = b.object(Some(b.ids.event_target_prototype));
    b.method(port, "postMessage", 1, post_message);
    for name in ["start", "close"] {
        b.method(port, name, 0, port_control);
    }
    let ctor = b.constructor(
        "MessagePort",
        0,
        super::dom::illegal_constructor,
        port,
        None,
    );
    b.global("MessagePort", ctor);
    let channel = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("MessageChannel", 0, construct, channel, None);
    b.global("MessageChannel", ctor);
    let deliver = b.function("", 2, deliver);
    let _ = IDS.set(Ids { port, deliver });
}

/** The private key holding a port's entangled peer. */
fn peer_key() -> Key {
    Key::private("messagePeer")
}

/** `new MessageChannel()`: port1 and port2, each the other's peer. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(
            vm.type_error("Failed to construct 'MessageChannel': Please use the 'new' operator.")
        );
    };
    let proto = vm.prototype_for(&target, crate::v9::vm::realm::realm().ids.object_prototype)?;
    let channel = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let first = new_port(vm)?;
    let second = new_port(vm)?;
    entangle(vm, first, second);
    entangle(vm, second, first);
    vm.put(channel, "port1", JsValue::Object(first))?;
    vm.put(channel, "port2", JsValue::Object(second))?;
    Ok(JsValue::Object(channel))
}

/** One port with no handler yet. */
fn new_port(vm: &mut Vm) -> Res<ObjectId> {
    let port = vm.alloc(Object::new(Class::Plain, Some(ids().port)))?;
    vm.put(port, "onmessage", JsValue::Null)?;
    Ok(port)
}

/** Point a port at its peer. */
fn entangle(vm: &mut Vm, port: ObjectId, peer: ObjectId) {
    vm.heap.put(
        port,
        peer_key(),
        Prop {
            slot: Slot::Data(JsValue::Object(peer)),
            attrs: HIDDEN_ATTRS,
        },
    );
}

/** A port's peer, unless the channel was closed. */
fn peer(vm: &Vm, port: &JsValue) -> Option<JsValue> {
    let id = port.object()?;
    match vm.heap.get(id)?.props.get(&peer_key()) {
        Some(Prop {
            slot: Slot::Data(value @ JsValue::Object(_)),
            ..
        }) => Some(value.clone()),
        _ => None,
    }
}

/** `port.postMessage(data)`: deliver to the peer from a task. */
fn post_message(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(peer) = peer(vm, &call.this) {
        timers::schedule(vm, JsValue::Object(ids().deliver), vec![peer, call.arg(0)])?;
    }
    Ok(JsValue::Undefined)
}

/** `start()` does nothing (ports deliver as soon as a handler is set); `close()` disentangles. */
fn port_control(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if super::dom::native_name(&call) == "close" {
        for port in [Some(call.this.clone()), peer(vm, &call.this)]
            .into_iter()
            .flatten()
        {
            if let Some(id) = port.object() {
                vm.heap.put(
                    id,
                    peer_key(),
                    Prop {
                        slot: Slot::Data(JsValue::Null),
                        attrs: HIDDEN_ATTRS,
                    },
                );
            }
        }
    }
    Ok(JsValue::Undefined)
}

/** The delivery task: a `message` event carrying the data, at the receiving port. */
fn deliver(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(port) = call.arg(0).object() else {
        return Ok(JsValue::Undefined);
    };
    let event = events::create_event(vm, "message", false, false)?;
    let Some(id) = event.object() else {
        return Ok(JsValue::Undefined);
    };
    vm.put(id, "data", call.arg(1))?;
    let ports = vm.new_array(Vec::new())?;
    vm.put(id, "ports", JsValue::Object(ports))?;
    vm.put(id, "origin", JsValue::string("")?)?;
    events::dispatch(vm, Target::Object(port), id)?;
    Ok(JsValue::Undefined)
}
