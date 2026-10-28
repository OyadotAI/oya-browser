/** EventTarget, Event and CustomEvent, and dispatch through capture, target and bubble phases. */
use super::dom::{DOCUMENT, native_name, node_of, wrap};
use super::{Listener, Target};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, EventData, HostData};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, GLOBAL, Res, Thrown, Vm};

/// Event listeners one page may register on nodes, the document and the window together. A
/// component framework adds several per element: YouTube's Polymer app passes 512 as soon as
/// its guide and masthead render. Chromium has no such bound; this one only caps state size.
pub(in crate::v9) const LISTENERS: usize = 16 * 1024;

/** The listener target a value denotes. */
pub(in crate::v9) fn target_of(vm: &Vm, value: &JsValue) -> Option<Target> {
    // A bare `addEventListener(...)` call passes no receiver; operations of the global
    // interface then act on the window, as WebIDL specifies.
    if *value == JsValue::Object(GLOBAL) || value.nullish() {
        return Some(Target::Window);
    }
    match node_of(vm, value) {
        Some(DOCUMENT) => Some(Target::Document),
        Some(node) => Some(Target::Node(node)),
        None => value.object().map(Target::Object),
    }
}

/** The script value of a target. */
pub(in crate::v9) fn target_value(vm: &mut Vm, target: Target) -> Res<JsValue> {
    match target {
        Target::Window => Ok(JsValue::Object(GLOBAL)),
        Target::Document => Ok(JsValue::Object(realm().ids.document)),
        Target::Node(node) => wrap(vm, node),
        Target::Object(id) => Ok(JsValue::Object(id)),
    }
}

/** The options of one addEventListener or removeEventListener call. */
struct ListenerOptions {
    capture: bool,
    once: bool,
    passive: Option<bool>,
    signal: Option<JsValue>,
}

/**
 * Reads the options argument. Removal reads only `capture`; adding also reads `once`, `passive`
 * and `signal`, so a feature check through a getter sees exactly the members the DOM standard
 * reads. A signal must be an object, so `signal: null` throws.
 */
fn listener_options(vm: &mut Vm, value: &JsValue, adding: bool) -> Res<ListenerOptions> {
    let mut parsed = ListenerOptions {
        capture: value.truthy(),
        once: false,
        passive: None,
        signal: None,
    };
    if !matches!(value, JsValue::Object(_)) {
        return Ok(parsed);
    }
    parsed.capture = vm.get(value, &Key::from("capture"))?.truthy();
    if adding {
        parsed.once = vm.get(value, &Key::from("once"))?.truthy();
        let passive = vm.get(value, &Key::from("passive"))?;
        parsed.passive = (passive != JsValue::Undefined).then(|| passive.truthy());
        parsed.signal = signal_option(vm, value)?;
    }
    Ok(parsed)
}

/** The `signal` member: absent is None; anything that is not an object throws a TypeError. */
fn signal_option(vm: &mut Vm, options: &JsValue) -> Res<Option<JsValue>> {
    let signal = vm.get(options, &Key::from("signal"))?;
    if signal == JsValue::Undefined {
        return Ok(None);
    }
    if signal.object().is_some() {
        return Ok(Some(signal));
    }
    Err(vm.type_error(
        "Failed to execute 'addEventListener': member signal is not of type AbortSignal.",
    ))
}

/**
 * Whether a listener's signal has aborted. It is checked when the listener would run, so an
 * aborted listener never runs even though its registration is removed lazily.
 */
fn signal_aborted(vm: &mut Vm, signal: Option<&JsValue>) -> Res<bool> {
    match signal {
        Some(signal) => Ok(vm.get(signal, &Key::from("aborted"))?.truthy()),
        None => Ok(false),
    }
}

/** addEventListener, removeEventListener and dispatchEvent. */
pub(in crate::v9) fn event_target(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = target_of(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match native_name(&call) {
        "dispatchEvent" => dispatch_from_script(vm, target, &call.arg(0)),
        name => {
            let kind = vm.to_text(&call.arg(0))?;
            let adding = name == "addEventListener";
            // WebIDL converts the options before the null-callback early return.
            let options = listener_options(vm, &call.arg(2), adding)?;
            let callback = call.arg(1);
            if callback.nullish() {
                return Ok(JsValue::Undefined);
            }
            let Some(callback) = callback.object() else {
                return Err(vm.type_error("parameter 2 is not of type 'Object'."));
            };
            if adding {
                add_listener(vm, target, kind, callback, options)?;
            } else {
                remove_listener(vm, target, &kind, callback, options.capture);
            }
            Ok(JsValue::Undefined)
        }
    }
}

/** The `dispatchEvent(event)` method: the argument must be an Event that is not already in flight. */
fn dispatch_from_script(vm: &mut Vm, target: Target, event: &JsValue) -> Res<JsValue> {
    let Some(id) = event.object().filter(|id| is_event(vm, *id)) else {
        return Err(vm.type_error("parameter 1 is not of type 'Event'."));
    };
    if flag(vm, id, DISPATCHING) || flag(vm, id, UNINITIALIZED) {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "The event is already being dispatched or was not initialized.",
        ));
    }
    Ok(JsValue::Boolean(dispatch(vm, target, id)?))
}

/** Whether an object is an Event host object. */
fn is_event(vm: &Vm, id: ObjectId) -> bool {
    matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Host(data)) if matches!(**data, HostData::Event(_)))
}

/**
 * Touch and wheel listeners on the window, the document, the root element and the body default
 * to passive, so they cannot stall scrolling. Other targets keep the non-passive default.
 */
fn passive_by_default(target: Target, kind: &str) -> bool {
    const DEFAULT_PASSIVE_KINDS: [&str; 4] = ["touchstart", "touchmove", "wheel", "mousewheel"];
    let root = match target {
        Target::Window | Target::Document => true,
        Target::Node(node) => node == super::HTML || node == super::nodes::BODY,
        Target::Object(_) => false,
    };
    root && DEFAULT_PASSIVE_KINDS.contains(&kind)
}

/** Registers a listener unless the same one exists or its signal has already aborted. */
fn add_listener(
    vm: &mut Vm,
    target: Target,
    kind: String,
    callback: ObjectId,
    options: ListenerOptions,
) -> Res<()> {
    if signal_aborted(vm, options.signal.as_ref())? {
        return Ok(());
    }
    let listeners = &vm.page.state.listeners;
    if listeners.find(target, &kind, options.capture, callback).is_some() {
        return Ok(());
    }
    if listeners.len() >= LISTENERS {
        return Err(EngineError::limit("V9 event listeners", LISTENERS).into());
    }
    let passive = options
        .passive
        .unwrap_or_else(|| passive_by_default(target, &kind));
    let id = next_listener_id(vm);
    vm.page.state.listeners.insert(Listener {
        id,
        target,
        kind,
        callback: JsValue::Object(callback),
        capture: options.capture,
        once: options.once,
        passive,
        signal: options.signal,
    });
    Ok(())
}

/** Takes the next registration id; ids order listeners by registration. */
fn next_listener_id(vm: &mut Vm) -> u32 {
    let id = vm.page.state.next_listener;
    vm.page.state.next_listener += 1;
    id
}

/** Removes the matching registration, if any. */
fn remove_listener(vm: &mut Vm, target: Target, kind: &str, callback: ObjectId, capture: bool) {
    let listeners = &mut vm.page.state.listeners;
    if let Some(key) = listeners.find(target, kind, capture, callback) {
        listeners.remove(&key);
    }
}

/**
 * Resolution of event timestamps in virtual milliseconds: 5 microseconds, the floor browsers
 * clamp to. The page clock only moves with timers, so without a tick every event in a request
 * would share one timestamp.
 */
const EVENT_TICK_MS: f64 = 0.005;

/**
 * The timestamp for the next event: the page clock plus one tick per event this request made,
 * so timestamps strictly increase in creation order. Ceiling: a request creating N events
 * drifts N * 5 microseconds past the clock.
 */
fn next_event_timestamp(vm: &mut Vm) -> f64 {
    vm.page.event_ticks += 1;
    vm.page.state.clock + vm.page.event_ticks as f64 * EVENT_TICK_MS
}

/** A new event object. */
pub(in crate::v9) fn create_event(
    vm: &mut Vm,
    kind: &str,
    bubbles: bool,
    cancelable: bool,
) -> Res<JsValue> {
    let timestamp = next_event_timestamp(vm);
    let data = EventData {
        kind: kind.into(),
        target: JsValue::Null,
        current: JsValue::Null,
        bubbles,
        cancelable,
        default_prevented: false,
        stopped: false,
        immediate: false,
        phase: 0,
        detail: JsValue::Null,
        trusted: false,
        timestamp,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Event(Box::new(data)))),
        Some(realm().ids.event_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** Private event slots for the DOM standard's flags EventData does not carry. */
const DISPATCHING: &str = "dispatching";
/** Set on `document.createEvent` events until initEvent runs. */
pub(in crate::v9) const UNINITIALIZED: &str = "uninitialized";
/** Set while a passive listener runs for the event. */
const PASSIVE: &str = "passive";
/** The type of the event fired at the window when a listener throws. */
const ERROR_KIND: &str = "error";

/** Read one of an event's private flags. */
fn flag(vm: &Vm, event: ObjectId, name: &str) -> bool {
    matches!(
        vm.heap
            .get(event)
            .and_then(|object| object.props.get(&Key::private(name))),
        Some(crate::v9::vm::heap::Prop {
            slot: crate::v9::vm::heap::Slot::Data(JsValue::Boolean(true)),
            ..
        })
    )
}

/** Set or clear one of an event's private flags. */
pub(in crate::v9) fn set_flag(vm: &mut Vm, event: ObjectId, name: &str, on: bool) {
    vm.heap.put(
        event,
        Key::private(name),
        crate::v9::vm::heap::Prop {
            slot: crate::v9::vm::heap::Slot::Data(JsValue::Boolean(on)),
            attrs: crate::v9::vm::heap::HIDDEN_ATTRS,
        },
    );
}

/** `document.createEvent(interface)`: the DOM standard's table of legacy interface names,
matched ASCII case-insensitively; anything else is NotSupportedError. The event is created
uninitialized, so dispatching it before initEvent throws InvalidStateError. */
pub(in crate::v9) fn create_legacy_event(vm: &mut Vm, interface: &str) -> Res<JsValue> {
    const TABLE: &[(&str, &str)] = &[
        ("beforeunloadevent", "BeforeUnloadEvent"),
        ("compositionevent", "CompositionEvent"),
        ("customevent", "CustomEvent"),
        ("devicemotionevent", "DeviceMotionEvent"),
        ("deviceorientationevent", "DeviceOrientationEvent"),
        ("dragevent", "DragEvent"),
        ("event", "Event"),
        ("events", "Event"),
        ("focusevent", "FocusEvent"),
        ("hashchangeevent", "HashChangeEvent"),
        ("htmlevents", "Event"),
        ("keyboardevent", "KeyboardEvent"),
        ("messageevent", "MessageEvent"),
        ("mouseevent", "MouseEvent"),
        ("mouseevents", "MouseEvent"),
        ("storageevent", "StorageEvent"),
        ("svgevents", "Event"),
        ("textevent", "TextEvent"),
        ("touchevent", "TouchEvent"),
        ("uievent", "UIEvent"),
        ("uievents", "UIEvent"),
    ];
    let lower = interface.to_ascii_lowercase();
    let Some((_, name)) = TABLE.iter().find(|(key, _)| *key == lower) else {
        return Err(vm.dom_exception(
            "NotSupportedError",
            &format!("The provided event type ('{interface}') is invalid."),
        ));
    };
    let event = create_event(vm, "", false, false)?;
    let id = event.object().expect("event object");
    let ctor = vm.get(&JsValue::Object(GLOBAL), &Key::from(name))?;
    if let JsValue::Object(_) = ctor
        && let JsValue::Object(proto) = vm.get(&ctor, &Key::from("prototype"))?
    {
        vm.set_proto(id, Some(proto))?;
    }
    set_flag(vm, id, UNINITIALIZED, true);
    Ok(event)
}

/** Change an event's data. */
fn with_event<T>(vm: &mut Vm, id: ObjectId, change: impl FnOnce(&mut EventData) -> T) -> Option<T> {
    match vm.heap.get_mut(id).map(|object| &mut object.class) {
        Some(Class::Host(data)) => match data.as_mut() {
            HostData::Event(event) => Some(change(event)),
            _ => None,
        },
        _ => None,
    }
}

/** Read an event's data. */
fn event_data(vm: &Vm, value: &JsValue) -> Option<EventData> {
    match super::host_data(vm, value)? {
        HostData::Event(event) => Some(*event),
        _ => None,
    }
}

/** The propagation path from the window down to the target. Through shadow trees it runs from
a slotted node to its slot, and from a shadow root to its host only for composed events. */
fn path(vm: &mut Vm, target: Target, composed: bool) -> Vec<Target> {
    let mut path = vec![target];
    match target {
        Target::Node(node) => {
            let mut current = node;
            let mut reached_document = false;
            for _ in 0..crate::constants::DOCUMENT_DEPTH + crate::constants::VM_DETACHED_NODES {
                let Some(parent) =
                    super::components::composed_parent(&mut vm.page, current, composed)
                else {
                    break;
                };
                path.push(Target::Node(parent));
                current = parent;
            }
            if current == super::HTML {
                reached_document = true;
            }
            if reached_document {
                path.push(Target::Document);
                path.push(Target::Window);
            }
        }
        Target::Document => path.push(Target::Window),
        _ => {}
    }
    path.reverse();
    path
}

/** Dispatch an event object; returns false when a listener canceled it. */
pub(in crate::v9) fn dispatch(vm: &mut Vm, target: Target, event: ObjectId) -> Res<bool> {
    vm.pin(JsValue::Object(event));
    let target_value = target_value(vm, target)?;
    let data = with_event(vm, event, |data| {
        data.target = target_value.clone();
        data.clone()
    })
    .ok_or_else(|| EngineError::invalid("Invalid V9 event."))?;
    set_flag(vm, event, DISPATCHING, true);
    let outer = vm.page.current_event.replace(event);
    let result = propagate(vm, target, event, &data);
    vm.page.current_event = outer;
    // The DOM standard's dispatch ends by clearing the dispatch and both stop flags, so the
    // same event can be dispatched again and its listeners run again.
    set_flag(vm, event, DISPATCHING, false);
    with_event(vm, event, |data| {
        data.phase = 0;
        data.current = JsValue::Null;
        data.stopped = false;
        data.immediate = false;
    });
    result?;
    Ok(!prevented(vm, event))
}

/** Run the capture, target and bubble phases along the event path. */
fn propagate(vm: &mut Vm, target: Target, event: ObjectId, data: &EventData) -> Res<()> {
    if let Target::Node(node) = target {
        super::components::set_original(vm, event, node);
    }
    let composed =
        data.trusted && data.kind != "slotchange" || super::components::composed(vm, event);
    let route = path(vm, target, composed);
    let last = route.len().saturating_sub(1);
    // Each "inner invoke" returns at once when propagation already stopped, so a flag set
    // before dispatch, or by an earlier target, keeps every later listener from running.
    for (index, current) in route.iter().enumerate() {
        if index == last || stopped(vm, event) {
            break;
        }
        invoke(vm, *current, event, &data.kind, true, 1)?;
    }
    if !stopped(vm, event) {
        invoke(vm, target, event, &data.kind, true, 2)?;
    }
    if !stopped(vm, event) {
        invoke(vm, target, event, &data.kind, false, 2)?;
    }
    if data.bubbles {
        for current in route.iter().rev().skip(1) {
            if stopped(vm, event) {
                break;
            }
            invoke(vm, *current, event, &data.kind, false, 3)?;
        }
    }
    Ok(())
}

/** Whether propagation stopped. */
fn stopped(vm: &Vm, event: ObjectId) -> bool {
    event_data(vm, &JsValue::Object(event)).is_some_and(|data| data.stopped)
}

/** Whether immediate propagation stopped. */
fn immediate(vm: &Vm, event: ObjectId) -> bool {
    event_data(vm, &JsValue::Object(event)).is_some_and(|data| data.immediate)
}

/** Whether the default was prevented. */
fn prevented(vm: &Vm, event: ObjectId) -> bool {
    event_data(vm, &JsValue::Object(event)).is_some_and(|data| data.default_prevented)
}

/** The event target listeners on `current` see, when a shadow tree hides the original. */
fn retargeted(vm: &mut Vm, event: ObjectId, current: Target) -> Res<Option<JsValue>> {
    if !vm.page.has_shadows() {
        return Ok(None);
    }
    let original = super::components::original_target(vm, event);
    let (Some(original), Target::Node(current)) = (original, current) else {
        return Ok(None);
    };
    let shown = super::components::retarget(&mut vm.page, original, current);
    Ok(Some(super::dom::wrap(vm, shown)?))
}

/** Run one target's listeners for a phase, plus its `on<type>` handler during target and bubble. */
fn invoke(
    vm: &mut Vm,
    target: Target,
    event: ObjectId,
    kind: &str,
    capture: bool,
    phase: u8,
) -> Res<()> {
    let current = target_value(vm, target)?;
    let shown = retargeted(vm, event, target)?;
    with_event(vm, event, |data| {
        data.current = current.clone();
        data.phase = phase;
        if let Some(shown) = shown {
            data.target = shown;
        }
    });
    // The bucket of this target, type and capture is exactly what the phase fires: a phase reads
    // capture listeners in the capture passes and the others in the bubble passes.
    let firing = vm.page.state.listeners.firing(target, kind, capture);
    // A markup `on<type>` attribute was registered when the element was parsed, before any
    // script listener, so it runs first; listeners it adds wait for the next dispatch.
    let inline_first = !capture && inline_source(vm, target, kind).is_some();
    if inline_first {
        handler_property(vm, target, &current, event, kind)?;
    }
    for (key, listener) in firing {
        if immediate(vm, event) {
            return Ok(());
        }
        if !vm.page.state.listeners.contains(&key) {
            continue;
        }
        if listener.once || signal_aborted(vm, listener.signal.as_ref())? {
            vm.page.state.listeners.remove(&key);
        }
        if signal_aborted(vm, listener.signal.as_ref())? {
            continue;
        }
        call_listener(vm, &listener.callback, &current, event, listener.passive)?;
    }
    if !capture && !inline_first && !immediate(vm, event) {
        handler_property(vm, target, &current, event, kind)?;
    }
    Ok(())
}

/** The window handler properties `<body>` forwards (HTML's WindowEventHandlers, the body-forwarded
blur, error, focus, load, resize and scroll). Both `<body onload>` content attributes and the
body's `onload` script property set the window's own handler, so these names are the body's
accessors and the window's handler keys at once. */
pub(in crate::v9) const BODY_FORWARDED: [&str; 22] = [
    "onafterprint",
    "onbeforeprint",
    "onbeforeunload",
    "onblur",
    "onerror",
    "onfocus",
    "onhashchange",
    "onlanguagechange",
    "onload",
    "onmessage",
    "onmessageerror",
    "onoffline",
    "ononline",
    "onpagehide",
    "onpagereveal",
    "onpageshow",
    "onpopstate",
    "onrejectionhandled",
    "onresize",
    "onscroll",
    "onstorage",
    "onunload",
];

/** `document.body.onload` and its siblings read the window's handler, as HTML defines the body's
forwarded handlers: a body that answered with its own property would never be the one that fires. */
pub(in crate::v9) fn body_handler_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key = Key::from(native_name(&call));
    vm.get(&JsValue::Object(GLOBAL), &key)
}

/** Writing `document.body.onload` writes the window's handler, the one dispatch consults. Without
this the body kept a plain property that no event ever read, so `onload` set from script never ran. */
pub(in crate::v9) fn body_handler_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key = Key::from(native_name(&call));
    vm.put_key(GLOBAL, key, call.arg(0))?;
    Ok(JsValue::Undefined)
}

/** Installs the forwarded handler accessors on HTMLBodyElement.prototype. */
pub(in crate::v9) fn install_body_handlers(b: &mut Builder, proto: u32) {
    for name in BODY_FORWARDED {
        b.accessor(proto, name, Some(body_handler_get), Some(body_handler_set));
    }
}

/** The markup `on<kind>` attribute source of a node target, if it has one. */
fn inline_source(vm: &mut Vm, target: Target, kind: &str) -> Option<String> {
    let node = match target {
        Target::Node(node) => node,
        // HTML forwards these `<body on...>` attributes to the window's handlers, so
        // `<body onload="...">` runs on the window's load event.
        Target::Window if BODY_FORWARDED.contains(&format!("on{kind}").as_str()) => {
            super::nodes::BODY
        }
        _ => return None,
    };
    let name = format!("on{kind}");
    vm.page
        .view(node)
        .and_then(|view| view.attributes.get(&name).cloned())
        .or_else(|| {
            (kind == "click")
                .then(|| {
                    vm.page
                        .projected(node)
                        .and_then(|node| node.inline_handler.clone())
                })
                .flatten()
        })
}

/** The function to call and its `this` for a listener: a callable, or an object's handleEvent. */
fn listener_target(vm: &mut Vm, callback: &JsValue, current: &JsValue) -> Res<(JsValue, JsValue)> {
    if vm.is_callable(callback) {
        return Ok((callback.clone(), current.clone()));
    }
    Ok((
        vm.get(callback, &Key::from("handleEvent"))?,
        callback.clone(),
    ))
}

/**
 * Call a listener function or `handleEvent` object. While it runs, `passive` is set on the
 * event so its cancel requests are ignored. An exception is reported and then fired at the
 * window as an `error` event, as HTML's "report the exception" does.
 */
fn call_listener(
    vm: &mut Vm,
    callback: &JsValue,
    current: &JsValue,
    event: ObjectId,
    passive: bool,
) -> Res<()> {
    let (function, this) = listener_target(vm, callback, current)?;
    set_flag(vm, event, PASSIVE, passive);
    let outcome = vm.call(function, this, vec![JsValue::Object(event)]);
    set_flag(vm, event, PASSIVE, false);
    match outcome {
        Ok(result) => {
            if result == JsValue::Boolean(false) {
                cancel(vm, event);
            }
            Ok(())
        }
        Err(Thrown::Value(error)) => report_exception(vm, event, &error),
        Err(fatal) => Err(fatal),
    }
}

/** Reports an uncaught listener exception, then fires `error` at the window unless that event is itself `error`. */
fn report_exception(vm: &mut Vm, event: ObjectId, error: &JsValue) -> Res<()> {
    vm.report_error(error);
    let dispatching_error =
        event_data(vm, &JsValue::Object(event)).is_some_and(|data| data.kind == ERROR_KIND);
    if !dispatching_error {
        fire(vm, Target::Window, ERROR_KIND, false, true)?;
    }
    Ok(())
}

/** Cancels an event: only a cancelable event that no passive listener is running for. */
fn cancel(vm: &mut Vm, event: ObjectId) {
    if flag(vm, event, PASSIVE) {
        return;
    }
    with_event(vm, event, |data| {
        if data.cancelable {
            data.default_prevented = true;
        }
    });
}

/** `onclick = function` properties and inline `onclick="..."` attributes. */
fn handler_property(
    vm: &mut Vm,
    target: Target,
    current: &JsValue,
    event: ObjectId,
    kind: &str,
) -> Res<()> {
    let name = format!("on{kind}");
    let key = Key::Str(crate::v9::value::JsString::from_utf8(&name)?);
    let mut handler = if let JsValue::Object(id) = current {
        vm.own(*id, &key)?.and_then(|prop| match prop.slot {
            crate::v9::vm::heap::Slot::Data(value) => Some(value),
            _ => None,
        })
    } else {
        None
    };
    if handler.is_none()
        && let Some(source) = inline_source(vm, target, kind)
    {
        let source = format!("(function(event){{\n{source}\n}})");
        handler = Some(crate::v9::builtins::evaluate_source(vm, source)?);
    }
    if let Some(handler) = handler
        && vm.is_callable(&handler)
    {
        call_listener(vm, &handler, current, event, false)?;
    }
    Ok(())
}

/** Fire a trusted event of a type at a target; returns whether the default may proceed. */
pub(in crate::v9) fn fire(
    vm: &mut Vm,
    target: Target,
    kind: &str,
    bubbles: bool,
    cancelable: bool,
) -> Res<bool> {
    let event = create_event(vm, kind, bubbles, cancelable)?;
    let id = event.object().expect("event object");
    with_event(vm, id, |data| data.trusted = true);
    dispatch(vm, target, id)
}

/** Native keyboard events retain their intrinsic prototype across mutable global bindings and realm banks. */
static KEYBOARD_PROTOTYPE: std::sync::OnceLock<ObjectId> = std::sync::OnceLock::new();

/** Register the original intrinsic while the platform builds its deterministic native realm. */
pub(in crate::v9::platform) fn keyboard_prototype(id: ObjectId) {
    let _ = KEYBOARD_PROTOTYPE.set(id);
}

/** Dispatch a trusted cancelable keyboard event with native readonly key metadata. */
pub(in crate::v9::platform) fn fire_keyboard(
    vm: &mut Vm,
    target: Target,
    kind: &str,
    key: &str,
) -> Res<bool> {
    let event = create_event(vm, kind, true, true)?;
    let id = event.object().expect("native keyboard event");
    vm.pin(event);
    let proto = vm.intrinsic_id(
        *KEYBOARD_PROTOTYPE
            .get()
            .expect("keyboard prototype installed"),
    );
    vm.set_proto(id, Some(proto))?;
    for name in ["key", "code"] {
        vm.heap.put(
            id,
            Key::from(name),
            crate::v9::vm::heap::Prop {
                slot: crate::v9::vm::heap::Slot::Data(JsValue::string(key)?),
                attrs: crate::v9::vm::heap::ENUMERABLE,
            },
        );
    }
    with_event(vm, id, |data| data.trusted = true);
    dispatch(vm, target, id)
}

/** `new Event(type, init)` and `new CustomEvent(type, init)`. */
pub(in crate::v9) fn construct_event(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Failed to construct 'Event': Please use the 'new' operator, this DOM object constructor cannot be called as a function."));
    }
    if call.args.is_empty() {
        return Err(vm.type_error("Failed to construct 'Event': 1 argument required."));
    }
    let kind = vm.to_text(&call.arg(0))?;
    let init = call.arg(1);
    let flag = |vm: &mut Vm, name: &str| -> Res<bool> {
        if matches!(init, JsValue::Object(_)) {
            Ok(vm.get(&init, &Key::from(name))?.truthy())
        } else {
            Ok(false)
        }
    };
    let bubbles = flag(vm, "bubbles")?;
    let cancelable = flag(vm, "cancelable")?;
    let composed = flag(vm, "composed")?;
    let event = create_event(vm, &kind, bubbles, cancelable)?;
    let id = event.object().expect("event object");
    super::components::set_composed(vm, id, composed);
    if matches!(init, JsValue::Object(_)) {
        let detail = vm.get(&init, &Key::from("detail"))?;
        with_event(vm, id, |data| {
            data.detail = if detail == JsValue::Undefined {
                JsValue::Null
            } else {
                detail
            }
        });
    }
    let Some(target) = &call.new_target else {
        return Err(vm.type_error("Failed to construct 'Event': Please use the 'new' operator, this DOM object constructor cannot be called as a function."));
    };
    let proto = vm.prototype_for(target, realm().ids.event_prototype)?;
    vm.set_proto(id, Some(proto))?;
    Ok(event)
}

/** Event accessors. */
pub(in crate::v9) fn event_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(data) = event_data(vm, &call.this) else {
        return Ok(JsValue::Undefined);
    };
    Ok(match native_name(&call) {
        "type" => JsValue::string(&data.kind)?,
        "target" | "srcElement" => data.target,
        "currentTarget" => data.current,
        "eventPhase" => JsValue::Number(f64::from(data.phase)),
        "bubbles" => JsValue::Boolean(data.bubbles),
        "cancelable" => JsValue::Boolean(data.cancelable),
        "defaultPrevented" => JsValue::Boolean(data.default_prevented),
        "returnValue" => JsValue::Boolean(!data.default_prevented),
        "cancelBubble" => JsValue::Boolean(data.stopped),
        "isTrusted" => JsValue::Boolean(data.trusted),
        "composed" => JsValue::Boolean(
            data.trusted
                || call
                    .this
                    .object()
                    .is_some_and(|id| super::components::composed(vm, id)),
        ),
        "timeStamp" => JsValue::Number(data.timestamp),
        "detail" => data.detail,
        "button" | "buttons" | "clientX" | "clientY" | "pageX" | "pageY" | "screenX"
        | "screenY" | "offsetX" | "offsetY" | "keyCode" | "charCode" | "deltaX" | "deltaY" => {
            JsValue::Number(0.0)
        }
        "which" => JsValue::Number(if data.kind == "click" { 1.0 } else { 0.0 }),
        "altKey" | "ctrlKey" | "metaKey" | "shiftKey" | "repeat" | "isComposing" => {
            JsValue::Boolean(false)
        }
        "key" | "code" => JsValue::string("")?,
        "relatedTarget" | "view" => JsValue::Null,
        _ => JsValue::Undefined,
    })
}

/** Event methods. */
pub(in crate::v9) fn event_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(id) = call.this.object() else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match native_name(&call) {
        "preventDefault" => cancel(vm, id),
        "stopPropagation" => {
            with_event(vm, id, |data| data.stopped = true);
        }
        "stopImmediatePropagation" => {
            with_event(vm, id, |data| {
                data.stopped = true;
                data.immediate = true;
            });
        }
        "composedPath" => {
            let Some(data) = event_data(vm, &call.this) else {
                return Ok(JsValue::Undefined);
            };
            if !flag(vm, id, DISPATCHING) {
                return Ok(JsValue::Object(vm.new_array(Vec::new())?));
            }
            let target = match super::components::original_target(vm, id) {
                Some(node) => Some(Target::Node(node)),
                None => target_of(vm, &data.target),
            };
            let Some(target) = target else {
                return Ok(JsValue::Object(vm.new_array(Vec::new())?));
            };
            let composed = data.trusted || super::components::composed(vm, id);
            let mut values = Vec::new();
            for step in path(vm, target, composed).into_iter().rev() {
                values.push(target_value(vm, step)?);
            }
            return Ok(JsValue::Object(vm.new_array(values)?));
        }
        "initEvent" | "initCustomEvent" | "initUIEvent" | "initMouseEvent"
        | "initKeyboardEvent" => {
            let name = native_name(&call);
            if call.args.is_empty() {
                return Err(vm.type_error("1 argument required, but only 0 present."));
            }
            if flag(vm, id, DISPATCHING) {
                return Ok(JsValue::Undefined);
            }
            set_flag(vm, id, UNINITIALIZED, false);
            let kind = vm.to_text(&call.arg(0))?;
            let bubbles = call.arg(1).truthy();
            let cancelable = call.arg(2).truthy();
            let detail = detail_index(name).map_or(JsValue::Undefined, |index| call.arg(index));
            with_event(vm, id, |data| {
                data.kind = kind;
                data.bubbles = bubbles;
                data.cancelable = cancelable;
                data.stopped = false;
                data.immediate = false;
                data.default_prevented = false;
                data.target = JsValue::Null;
                if detail != JsValue::Undefined {
                    data.detail = detail;
                }
            });
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/**
 * Position of the `detail` argument in each legacy init method. Keyboard events have no
 * detail, and initEvent takes none.
 */
fn detail_index(name: &str) -> Option<usize> {
    match name {
        "initCustomEvent" => Some(3),
        "initUIEvent" | "initMouseEvent" => Some(4),
        _ => None,
    }
}

/** Event setters (returnValue, cancelBubble). */
pub(in crate::v9) fn event_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(id) = call.this.object() else {
        return Ok(JsValue::Undefined);
    };
    let value = call.arg(0).truthy();
    match native_name(&call) {
        "returnValue" if !value => cancel(vm, id),
        "cancelBubble" if value => {
            with_event(vm, id, |data| data.stopped = true);
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
}
