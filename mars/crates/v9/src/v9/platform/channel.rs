/**
 * MessageChannel: two entangled ports. A message posted on one port is delivered to the other
 * as a `message` event from a zero-delay task, which is how schedulers (React, YouTube's) yield
 * between chunks of work. Data is passed as is within the realm.
 *
 * A port posted to a window in another document (`transfer`) leaves this realm: its peer stays
 * here, linked to the port the other document receives by a channel link both share, and a
 * message on either end leaves as an effect (`HostEffects::port_messages`) for the app to carry
 * across, as the frame messages that carry the ports do (see `messaging`).
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
    b.host_method(port, "postMessage", 1, post_message);
    for name in ["start", "close"] {
        b.host_method(port, name, 0, port_control);
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

/** `port.postMessage(data)`: deliver to the peer from a task, or across its link. */
fn post_message(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(peer) = peer(vm, &call.this) {
        timers::schedule(vm, JsValue::Object(ids().deliver), vec![peer, call.arg(0)])?;
    } else if let Some(link) = link(vm, &call.this) {
        let data = super::messaging::encode(vm, &call)?;
        if vm.page.effects.port_messages.len() >= PORT_MESSAGES_PER_STEP {
            return Err(crate::protocol::EngineError::limit(
                "V9 port messages",
                PORT_MESSAGES_PER_STEP,
            )
            .into());
        }
        vm.page
            .effects
            .port_messages
            .push(PortMessage { link, data });
    }
    Ok(JsValue::Undefined)
}

/// Messages one step may post on ports linked to other documents.
const PORT_MESSAGES_PER_STEP: usize = 256;
/// Ports of one page linked to other documents.
const LINKED_PORTS: usize = 64;
/// Link identities stay exact in a JavaScript number.
const LINK_MASK: u64 = (1 << 53) - 1;

/** A message posted on a port whose peer lives in another document. */
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortMessage {
    /// The channel link both ends share.
    pub link: u64,
    /// The data, as window messages carry it.
    pub data: super::messaging::MessageData,
}

/** The private key holding a port's link to another document. */
fn link_key() -> Key {
    Key::private("messageLink")
}

/** The link a port has to another document, if any. */
fn link(vm: &Vm, port: &JsValue) -> Option<u64> {
    match vm.heap.get(port.object()?)?.props.get(&link_key()) {
        Some(Prop {
            slot: Slot::Data(JsValue::Number(link)),
            ..
        }) => Some(*link as u64),
        _ => None,
    }
}

/** Gives `port` the link `link` and registers it, so messages across the link reach it. */
fn bind(vm: &mut Vm, port: ObjectId, link: u64) -> Res<()> {
    if vm.page.state.linked_ports.len() >= LINKED_PORTS {
        return Err(crate::protocol::EngineError::limit("V9 linked ports", LINKED_PORTS).into());
    }
    vm.heap.put(
        port,
        link_key(),
        Prop {
            slot: Slot::Data(JsValue::Number(link as f64)),
            attrs: HIDDEN_ATTRS,
        },
    );
    vm.page.state.linked_ports.insert(link, port);
    Ok(())
}

/** A fresh link identity from the operating system's random source, so links two documents
make never collide. */
fn new_link() -> Res<u64> {
    use ring::rand::SecureRandom;
    let mut bytes = [0_u8; 8];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            crate::protocol::EngineError::unsupported(
                "The operating system random source is unavailable to V9.",
            )
        })?;
    Ok(u64::from_le_bytes(bytes) & LINK_MASK)
}

/** Whether `value` is a MessagePort. */
pub(in crate::v9) fn is_port(vm: &Vm, value: &JsValue) -> bool {
    value
        .object()
        .and_then(|id| vm.heap.get(id))
        .is_some_and(|object| object.proto == Some(ids().port))
}

/**
 * Sends `port` to another document: its peer stays here linked to the link this returns, and
 * `port` itself is detached here, as a transferred port is. A port already linked elsewhere
 * passes its own link on.
 */
pub(in crate::v9) fn transfer(vm: &mut Vm, port: &JsValue) -> Res<u64> {
    if let Some(link) = link(vm, port) {
        vm.page.state.linked_ports.remove(&link);
        return Ok(link);
    }
    let link = new_link()?;
    if let Some(JsValue::Object(peer)) = peer(vm, port) {
        bind(vm, peer, link)?;
        disentangle(vm, peer);
    }
    if let Some(id) = port.object() {
        disentangle(vm, id);
    }
    Ok(link)
}

/** A port received from another document over `link`. */
pub(in crate::v9) fn adopt(vm: &mut Vm, link: u64) -> Res<JsValue> {
    let port = new_port(vm)?;
    bind(vm, port, link)?;
    Ok(JsValue::Object(port))
}

/** Fires a message that came across `link` at the port bound to it here, if any. */
pub(in crate::v9) fn receive(vm: &mut Vm, link: u64, data: JsValue) -> Res<()> {
    let Some(port) = vm.page.state.linked_ports.get(&link).copied() else {
        return Ok(());
    };
    let event = events::create_event(vm, "message", false, false)?;
    let Some(id) = event.object() else {
        return Ok(());
    };
    vm.put(id, "data", data)?;
    let ports = vm.new_array(Vec::new())?;
    vm.put(id, "ports", JsValue::Object(ports))?;
    vm.put(id, "origin", JsValue::string("")?)?;
    events::dispatch(vm, Target::Object(port), id)?;
    Ok(())
}

/** Clears a port's local peer. */
fn disentangle(vm: &mut Vm, port: ObjectId) {
    vm.heap.put(
        port,
        peer_key(),
        Prop {
            slot: Slot::Data(JsValue::Null),
            attrs: HIDDEN_ATTRS,
        },
    );
}

/** `start()` does nothing (ports deliver as soon as a handler is set); `close()` disentangles. */
fn port_control(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if super::dom::native_name(&call) == "close" {
        if let Some(link) = link(vm, &call.this) {
            vm.page.state.linked_ports.remove(&link);
        }
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
