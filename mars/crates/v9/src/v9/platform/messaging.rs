/**
 * Window messaging (HTML's postMessage) between a page and the frames it embeds. Each document
 * runs in its own realm, so a window in another realm is a proxy: `parent` and `top` in a frame,
 * and an iframe's `contentWindow` when its document is not one scripts here may reach. A proxy
 * offers what a cross-origin window does that a page relies on: `postMessage`, `closed` and
 * itself as `window` and `self`.
 *
 * A message posted to a proxy leaves the realm as an effect (`HostEffects::messages`) for the app
 * to carry to the other document, which receives it as a `message` event from a task. Data crosses
 * as text when it is a string and as JSON otherwise: the structured clone of ordinary data minus
 * Map, Set, Date and cycles, which frame protocols (gapi, OAuth relays) do not send.
 * A message to the page's own window is delivered from a zero-delay task here, never
 * synchronously, as setImmediate polyfills that post to themselves expect.
 */
use super::{Target, events, timers};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The proxy target that names the embedding page, from inside a frame.
pub const PARENT: i64 = -1;
/// Messages one step may post to other documents.
const MESSAGES_PER_STEP: usize = 256;

/** A message's data as it crosses between realms. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MessageData {
    /// A string, delivered as the same string.
    Text(String),
    /// Any other value, as JSON text; delivered parsed.
    Json(String),
}

/** A message a script posted to a window in another document. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameMessage {
    /// PARENT, or the iframe element (by node identity) whose document receives it.
    pub target: i64,
    /// The data.
    pub data: MessageData,
    /// The origin the receiver must have: "*" for any, else a serialized origin.
    pub target_origin: String,
    /// Links of the MessagePorts transferred with it (`channel::transfer`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<u64>,
}

/** The proxy prototype and the native task that delivers a message to this window. */
struct Ids {
    proxy: u32,
    deliver: u32,
}

/** Set once while the realm is built. */
static IDS: OnceLock<Ids> = OnceLock::new();

/** The messaging intrinsics. */
fn ids() -> &'static Ids {
    let _ = crate::v9::vm::realm::realm();
    IDS.get()
        .expect("messaging intrinsics are installed with the realm")
}

/** Install the window proxy prototype and the delivery task. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let proxy = b.object(Some(b.ids.object_prototype));
    b.host_method(proxy, "postMessage", 1, proxy_post);
    for name in ["focus", "blur", "close"] {
        b.host_method(proxy, name, 0, proxy_nothing);
    }
    for name in ["closed", "window", "self", "frames", "length"] {
        b.getter(proxy, name, proxy_get);
    }
    let deliver = b.function("", 4, deliver);
    let _ = IDS.set(Ids { proxy, deliver });
}

/** The private key holding a proxy's target. */
fn target_key() -> Key {
    Key::private("remoteWindow")
}

/** The proxy for `target`, made once and kept while referenced, so `event.source === parent`. */
pub(in crate::v9) fn proxy(vm: &mut Vm, target: i64) -> Res<JsValue> {
    if let Some(id) = vm.page.state.remote_windows.get(&target)
        && vm.heap.get(*id).is_some()
    {
        return Ok(JsValue::Object(*id));
    }
    let id = vm.alloc(Object::new(Class::Plain, Some(ids().proxy)))?;
    vm.heap.put(
        id,
        target_key(),
        Prop {
            slot: Slot::Data(JsValue::Number(target as f64)),
            attrs: HIDDEN_ATTRS,
        },
    );
    vm.page.state.remote_windows.insert(target, id);
    Ok(JsValue::Object(id))
}

/** The target a proxy stands for. */
fn target_of(vm: &Vm, proxy: &JsValue) -> Option<i64> {
    let id = proxy.object()?;
    match vm.heap.get(id)?.props.get(&target_key()) {
        Some(Prop {
            slot: Slot::Data(JsValue::Number(target)),
            ..
        }) => Some(*target as i64),
        _ => None,
    }
}

/** `parent` and `top`: the embedding page's proxy in a frame, this window otherwise. */
pub(in crate::v9) fn parent(vm: &mut Vm) -> Res<JsValue> {
    if vm.page.environment.embedder_origin.is_some() {
        return proxy(vm, PARENT);
    }
    Ok(JsValue::Object(vm.global_object()))
}

/** `postMessage(data, targetOrigin)` on a proxy: the message leaves as an effect. */
fn proxy_post(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = target_of(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let data = encode(vm, &call)?;
    let target_origin = target_origin(vm, &call)?;
    if vm.page.effects.messages.len() >= MESSAGES_PER_STEP {
        return Err(EngineError::limit("V9 frame messages", MESSAGES_PER_STEP).into());
    }
    let mut ports = Vec::new();
    for port in transferred(vm, &call)? {
        ports.push(super::channel::transfer(vm, &port)?);
    }
    vm.page.effects.messages.push(FrameMessage {
        target,
        data,
        target_origin,
        ports,
    });
    Ok(JsValue::Undefined)
}

/** `focus`, `blur` and `close` on a window in another document do nothing here. */
fn proxy_nothing(_vm: &mut Vm, _call: Call) -> Res<JsValue> {
    Ok(JsValue::Undefined)
}

/** `closed` is false; `window`, `self` and `frames` are the proxy; `length` is 0. */
fn proxy_get(_vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(match super::dom::native_name(&call) {
        "closed" => JsValue::Boolean(false),
        "length" => JsValue::Number(0.0),
        _ => call.this.clone(),
    })
}

/** The MessagePorts in the transfer list (the third argument, or the `transfer` option);
anything else in it is ignored, as V9 transfers no buffers. */
fn transferred(vm: &mut Vm, call: &Call) -> Res<Vec<JsValue>> {
    let options = call.arg(1);
    let list = match options.object() {
        Some(_) => vm.get(&options, &Key::from("transfer"))?,
        None => call.arg(2),
    };
    if list.nullish() {
        return Ok(Vec::new());
    }
    let items = crate::v9::builtins::list_from_array_like(vm, &list)?;
    Ok(items
        .into_iter()
        .filter(|item| super::channel::is_port(vm, item))
        .collect())
}

/** The data as it crosses: a string as text, anything else as JSON (undefined as null). */
pub(in crate::v9) fn encode(vm: &mut Vm, call: &Call) -> Res<MessageData> {
    let value = call.arg(0);
    if let JsValue::String(text) = &value {
        return Ok(MessageData::Text(text.lossy()));
    }
    let arguments = [value];
    let encoded = crate::v9::builtins::json_stringify(
        vm,
        Call {
            this: JsValue::Undefined,
            args: &arguments,
            new_target: None,
            callee: call.callee,
        },
    )?;
    Ok(MessageData::Json(match encoded {
        JsValue::String(text) => text.lossy(),
        _ => "null".into(),
    }))
}

/** The targetOrigin argument (or the `targetOrigin` option), "/" meaning this page's origin. */
fn target_origin(vm: &mut Vm, call: &Call) -> Res<String> {
    let argument = call.arg(1);
    let argument = match argument.object() {
        Some(_) => vm.get(&argument, &Key::from("targetOrigin"))?,
        None => argument,
    };
    if argument.nullish() {
        return Ok("/".into());
    }
    let text = vm.to_text(&argument)?;
    Ok(match text.as_str() {
        "*" => text,
        "/" => own_origin(vm),
        _ => match super::net::parse_url(&text) {
            Some(url) => url.origin().ascii_serialization(),
            None => {
                return Err(vm.dom_exception(
                    "SyntaxError",
                    &format!("Failed to execute 'postMessage' on 'Window': Invalid target origin '{text}' in a call to 'postMessage'."),
                ));
            }
        },
    })
}

/** This page's serialized origin. */
fn own_origin(vm: &Vm) -> String {
    super::net::parse_url(&vm.page.state.href)
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "null".into())
}

/** `window.postMessage` on this page's own window: delivered from a task, as in a browser. */
pub(in crate::v9) fn post_to_self(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let origin = own_origin(vm);
    let wanted = target_origin(vm, call)?;
    if wanted != "*" && wanted != origin {
        return Ok(JsValue::Undefined);
    }
    let source = JsValue::Object(vm.global_object());
    let origin = JsValue::string(&origin)?;
    let ports = transferred(vm, call)?;
    let ports = JsValue::Object(vm.new_array(ports)?);
    timers::schedule(
        vm,
        JsValue::Object(ids().deliver),
        vec![call.arg(0), origin, source, ports],
    )?;
    Ok(JsValue::Undefined)
}

/** A message from another document, delivered now as the app hands it over: the data decoded,
the sender's origin, and `source` the sender's proxy (None for no source). */
pub(in crate::v9) fn receive(
    vm: &mut Vm,
    data: &MessageData,
    origin: &str,
    source: Option<i64>,
    links: &[u64],
) -> Res<()> {
    let value = decode(vm, data)?;
    let source = match source {
        Some(target) => proxy(vm, target)?,
        None => JsValue::Null,
    };
    let origin = JsValue::string(origin)?;
    let mut ports = Vec::new();
    for link in links {
        ports.push(super::channel::adopt(vm, *link)?);
    }
    let ports = JsValue::Object(vm.new_array(ports)?);
    dispatch_message(vm, value, origin, source, ports)
}

/** A message that came across a port link, fired at the port bound to it here. */
pub(in crate::v9) fn receive_on_port(vm: &mut Vm, link: u64, data: &MessageData) -> Res<()> {
    let value = decode(vm, data)?;
    super::channel::receive(vm, link, value)
}

/** The value a message's data stands for. */
fn decode(vm: &mut Vm, data: &MessageData) -> Res<JsValue> {
    Ok(match data {
        MessageData::Text(text) => JsValue::string(text)?,
        MessageData::Json(json) => super::json_value(vm, json).unwrap_or(JsValue::Null),
    })
}

/** The self-delivery task. */
fn deliver(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let ports = match call.arg(3) {
        ports @ JsValue::Object(_) => ports,
        _ => JsValue::Object(vm.new_array(Vec::new())?),
    };
    dispatch_message(vm, call.arg(0), call.arg(1), call.arg(2), ports)?;
    Ok(JsValue::Undefined)
}

/** A `message` event at the window carrying `data`, `origin`, `source` and `ports`. */
fn dispatch_message(
    vm: &mut Vm,
    data: JsValue,
    origin: JsValue,
    source: JsValue,
    ports: JsValue,
) -> Res<()> {
    let event = events::create_event(vm, "message", false, false)?;
    let Some(id) = event.object() else {
        return Ok(());
    };
    vm.put(id, "data", data)?;
    vm.put(id, "origin", origin)?;
    vm.put(id, "lastEventId", JsValue::string("")?)?;
    vm.put(id, "source", source)?;
    vm.put(id, "ports", ports)?;
    events::dispatch(vm, Target::Window, id)?;
    Ok(())
}

/** Forget proxies the heap collected; a later lookup makes a new one. */
pub(in crate::v9) fn sweep(
    windows: &mut std::collections::BTreeMap<i64, ObjectId>,
    live: &crate::v9::vm::heap::FastSet<ObjectId>,
) {
    windows.retain(|_, id| live.contains(id));
}
