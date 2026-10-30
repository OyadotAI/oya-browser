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
    bind_prepared(vm, port, link);
    Ok(())
}

/** The whole transfer batch reserves its links and capacity before any endpoint is detached. */
fn bind_prepared(vm: &mut Vm, port: ObjectId, link: u64) {
    vm.heap.put(
        port,
        link_key(),
        Prop {
            slot: Slot::Data(JsValue::Number(link as f64)),
            attrs: HIDDEN_ATTRS,
        },
    );
    vm.page.state.linked_ports.insert(link, port);
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

/** A transferred endpoint cannot be transferred again or regain its old communication link. */
pub(in crate::v9) fn transferable(vm: &Vm, value: &JsValue) -> bool {
    is_port(vm, value)
        && value
            .object()
            .and_then(|id| vm.heap.get(id))
            .is_some_and(|object| object.props.get(&Key::private("portTransferred")).is_none())
}

/** Separate native route registries cannot safely move an endpoint between their domains. */
#[derive(Clone, Copy)]
pub(in crate::v9) enum Route {
    Frame,
    Worker(u32),
}

impl Route {
    /** Zero identifies frame routing; positive values bind a worker to this realm's handle. */
    fn tag(self) -> f64 {
        match self {
            Self::Frame => 0.0,
            Self::Worker(id) => f64::from(id) + 1.0,
        }
    }
}

/** Retain routing provenance in private heap state across serialized VM turns. */
fn set_route(vm: &mut Vm, port: ObjectId, route: Route) {
    vm.heap.put(
        port,
        Key::private("messageRoute"),
        Prop::data(JsValue::Number(route.tag())),
    );
}

/** Refuse unsupported ownership changes before any endpoint in the batch is detached. */
fn validate_route(vm: &mut Vm, ports: &[JsValue], route: Route) -> Res<()> {
    for port in ports {
        if matches!(route, Route::Worker(_)) && link(vm, port).is_some() {
            return Err(vm.dom_exception(
                "NotSupportedError",
                "Retransferring a linked Worker MessagePort is not supported",
            ));
        }
        let tag = port
            .object()
            .and_then(|id| vm.heap.get(id))
            .and_then(|object| object.props.get(&Key::private("messageRoute")));
        if matches!(tag, Some(Prop { slot: Slot::Data(JsValue::Number(tag)), .. }) if *tag != route.tag())
        {
            return Err(vm.dom_exception(
                "NotSupportedError",
                "Moving a MessagePort between frame and worker routes is not supported",
            ));
        }
    }
    Ok(())
}

/** Capture existing-link provenance before transfer clears the source endpoint. */
pub(super) fn moved_links(vm: &Vm, ports: &[JsValue]) -> Vec<u64> {
    ports.iter().filter_map(|port| link(vm, port)).collect()
}

/** One endpoint's prepared move, with all fallible work completed before commit. */
struct Transfer {
    port: ObjectId,
    link: u64,
    peer: Option<ObjectId>,
    existing: bool,
}

/** Validate an entire list and allocate its wire links before detaching its first endpoint. */
pub(in crate::v9) fn transfer_many(vm: &mut Vm, ports: &[JsValue], route: Route) -> Res<Vec<u64>> {
    validate_route(vm, ports, route)?;
    let plans = prepare_transfers(vm, ports)?;
    let inert = plans
        .iter()
        .filter(|plan| !plan.existing && plan.peer.is_none())
        .count();
    if vm.page.effects.port_closes.len() + inert > PORT_MESSAGES_PER_STEP {
        return Err(crate::protocol::EngineError::limit(
            "V9 port closures",
            PORT_MESSAGES_PER_STEP,
        )
        .into());
    }
    for plan in &plans {
        if plan.existing {
            vm.page.state.linked_ports.remove(&plan.link);
        }
    }
    for plan in &plans {
        commit_transfer(vm, plan, route);
    }
    Ok(plans.iter().map(|plan| plan.link).collect())
}

/** Both ends of one local channel cannot be represented by this external-endpoint protocol. */
pub(super) fn validate_transfer_list(vm: &mut Vm, ports: &[JsValue]) -> Res<()> {
    if ports.iter().any(|port| {
        vm.page.state.timers.iter().any(|task| {
            task.callback.object() == Some(ids().deliver) && task.args.first() == Some(port)
        })
    }) {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Transferring a MessagePort with queued local messages is not supported",
        ));
    }
    let mut identities = std::collections::BTreeSet::new();
    for port in ports {
        if !transferable(vm, port) || !identities.insert(port.object()) {
            return Err(vm.dom_exception(
                "DataCloneError",
                "Transfer entries must be distinct live MessagePorts",
            ));
        }
    }
    for port in ports {
        if let Some(link) = link(vm, port) {
            let anchor = vm
                .heap
                .get(port.object().expect("validated port"))
                .is_some_and(|object| object.props.get(&Key::private("messageAnchor")).is_some());
            if anchor
                || vm
                    .page
                    .effects
                    .port_messages
                    .iter()
                    .any(|message| message.link == link)
            {
                return Err(vm.dom_exception("NotSupportedError", if anchor {
                    "Moving the original peer of an external MessagePort is not supported"
                } else { "Forwarding a MessagePort after posting on it in the same turn is not supported" }));
            }
        }
    }
    if ports
        .iter()
        .any(|port| peer(vm, port).is_some_and(|peer| ports.contains(&peer)))
    {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Transferring both channel endpoints together is not supported",
        ));
    }
    Ok(())
}

/** Capacity and collision checks account for endpoints moved out before new peers are installed. */
fn prepare_transfers(vm: &mut Vm, ports: &[JsValue]) -> Res<Vec<Transfer>> {
    validate_transfer_list(vm, ports)?;
    let mut links = std::collections::BTreeSet::new();
    let mut plans = Vec::new();
    for port in ports {
        let existing = link(vm, port);
        let link = match existing {
            Some(link) => link,
            None => new_link()?,
        };
        if !links.insert(link)
            || existing.is_none() && vm.page.state.linked_ports.contains_key(&link)
        {
            return Err(vm.dom_exception("DataCloneError", "MessagePort link collision"));
        }
        plans.push(Transfer {
            port: port.object().expect("validated port"),
            link,
            peer: peer(vm, port).and_then(|peer| peer.object()),
            existing: existing.is_some(),
        });
    }
    let removed = plans.iter().filter(|plan| plan.existing).count();
    let added = plans
        .iter()
        .filter(|plan| !plan.existing && plan.peer.is_some())
        .count();
    if vm
        .page
        .state
        .linked_ports
        .len()
        .saturating_sub(removed)
        .saturating_add(added)
        > LINKED_PORTS
    {
        return Err(crate::protocol::EngineError::limit("V9 linked ports", LINKED_PORTS).into());
    }
    Ok(plans)
}

/** Prepared commits cannot fail halfway through detaching the list. */
fn commit_transfer(vm: &mut Vm, plan: &Transfer, route: Route) {
    vm.heap.put(
        plan.port,
        Key::private("portTransferred"),
        Prop::data(JsValue::Boolean(true)),
    );
    if plan.existing {
        vm.heap
            .put(plan.port, link_key(), Prop::data(JsValue::Null));
    } else if let Some(peer) = plan.peer {
        bind_prepared(vm, peer, plan.link);
        set_route(vm, peer, route);
        vm.heap.put(
            peer,
            Key::private("messageAnchor"),
            Prop::data(JsValue::Boolean(true)),
        );
        disentangle(vm, peer);
    }
    if !plan.existing && plan.peer.is_none() {
        vm.page.effects.port_closes.push(plan.link);
    }
    disentangle(vm, plan.port);
}

/** A port received from another document over `link`. */
pub(in crate::v9) fn adopt(vm: &mut Vm, link: u64, route: Route) -> Res<JsValue> {
    if let Some(peer) = vm.page.state.linked_ports.get(&link).copied() {
        validate_route(vm, &[JsValue::Object(peer)], route)?;
    }
    let port = new_port(vm)?;
    if let Some(peer) = vm.page.state.linked_ports.remove(&link) {
        vm.heap.remove(peer, &link_key());
        vm.heap.remove(peer, &Key::private("messageRoute"));
        vm.heap.remove(peer, &Key::private("messageAnchor"));
        entangle(vm, port, peer);
        entangle(vm, peer, port);
    } else {
        bind(vm, port, link)?;
        set_route(vm, port, route);
    }
    Ok(JsValue::Object(port))
}

/** Preserve transfer-time queue direction after both endpoints become ordinary local ports. */
pub(super) fn queue_returned(
    vm: &mut Vm,
    returned: &JsValue,
    data: JsValue,
    to_transferred: bool,
) -> Res<()> {
    let target = if to_transferred {
        returned.clone()
    } else {
        peer(vm, returned).ok_or_else(|| {
            crate::protocol::EngineError::invalid("Invalid reunited MessagePort queue.")
        })?
    };
    timers::schedule(vm, JsValue::Object(ids().deliver), vec![target, data])
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

/** Native retirement disentangles the surviving endpoint without producing an echo effect. */
pub(in crate::v9) fn remote_close(vm: &mut Vm, link: u64) {
    if let Some(port) = vm.page.state.linked_ports.remove(&link) {
        vm.heap.put(port, link_key(), Prop::data(JsValue::Null));
        vm.heap.remove(port, &Key::private("messageAnchor"));
    }
}

/** Closed or transferred endpoints cannot receive already queued local tasks. */
fn inactive(vm: &Vm, port: ObjectId) -> bool {
    vm.heap.get(port).is_none_or(|object| {
        object.props.get(&Key::private("portClosed")).is_some()
            || object.props.get(&Key::private("portTransferred")).is_some()
    })
}

/** Retire a linked endpoint once and clear both directions before exposing the native effect. */
fn close_port(vm: &mut Vm, port: ObjectId) -> Res<()> {
    if inactive(vm, port) {
        return Ok(());
    }
    let value = JsValue::Object(port);
    if let Some(link) = link(vm, &value) {
        if vm.page.effects.port_closes.len() >= PORT_MESSAGES_PER_STEP {
            return Err(crate::protocol::EngineError::limit(
                "V9 port closures",
                PORT_MESSAGES_PER_STEP,
            )
            .into());
        }
        vm.page.state.linked_ports.remove(&link);
        vm.heap.put(port, link_key(), Prop::data(JsValue::Null));
        vm.page.effects.port_closes.push(link);
        vm.heap.remove(port, &Key::private("messageAnchor"));
    }
    if let Some(peer) = peer(vm, &value).and_then(|value| value.object()) {
        disentangle(vm, peer);
    }
    disentangle(vm, port);
    vm.heap.put(
        port,
        Key::private("portClosed"),
        Prop::data(JsValue::Boolean(true)),
    );
    Ok(())
}

/** Start retains existing eager delivery; close irreversibly disables this endpoint. */
fn port_control(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !is_port(vm, &call.this) {
        return Err(vm.type_error("Illegal invocation"));
    }
    if super::dom::native_name(&call) == "close" {
        close_port(vm, call.this.object().expect("branded port"))?;
    }
    Ok(JsValue::Undefined)
}

/** The delivery task: a `message` event carrying the data, at the receiving port. */
fn deliver(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(port) = call.arg(0).object() else {
        return Ok(JsValue::Undefined);
    };
    if inactive(vm, port) {
        return Ok(JsValue::Undefined);
    }
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

/** Validate serialized route provenance before private fields can influence native routing effects. */
pub(in crate::v9) fn validate_state(
    state: &super::PageState,
    heap: &crate::v9::vm::heap::Heap,
) -> crate::protocol::EngineResult<()> {
    let invalid = || crate::protocol::EngineError::invalid("Invalid MessagePort route state.");
    if state.linked_ports.len() > LINKED_PORTS {
        return Err(invalid());
    }
    for object in heap.objects.values() {
        if let Some(anchor) = object.props.get(&Key::private("messageAnchor"))
            && (!matches!(anchor.slot, Slot::Data(JsValue::Boolean(true)))
                || object.proto != Some(ids().port))
        {
            return Err(invalid());
        }
        for (key, maximum) in [
            ("messageRoute", f64::from(u32::MAX) + 1.0),
            ("messageLink", LINK_MASK as f64),
        ] {
            if let Some(prop) = object.props.get(&Key::private(key)) {
                let valid = matches!(&prop.slot, Slot::Data(JsValue::Number(value)) if value.is_finite() && *value >= 0.0 && *value <= maximum && value.fract() == 0.0)
                    || key == "messageLink" && matches!(prop.slot, Slot::Data(JsValue::Null));
                if !valid || object.proto != Some(ids().port) {
                    return Err(invalid());
                }
            }
        }
    }
    for (link, port) in &state.linked_ports {
        let object = heap.get(*port).ok_or_else(invalid)?;
        if *link > LINK_MASK
            || object.proto != Some(ids().port)
            || !matches!(object.props.get(&link_key()), Some(Prop {slot:Slot::Data(JsValue::Number(value)),..}) if *value == *link as f64)
            || object.props.get(&Key::private("messageRoute")).is_none()
            || object.props.get(&Key::private("portClosed")).is_some()
            || object.props.get(&Key::private("portTransferred")).is_some()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/** Host summaries retain route counts and identities but deliberately omit their VM heap. */
pub(in crate::v9) fn validate_bounds(
    state: &super::PageState,
) -> crate::protocol::EngineResult<()> {
    if state.linked_ports.len() > LINKED_PORTS
        || state.linked_ports.keys().any(|link| *link > LINK_MASK)
    {
        return Err(crate::protocol::EngineError::invalid(
            "Invalid MessagePort route state.",
        ));
    }
    Ok(())
}
