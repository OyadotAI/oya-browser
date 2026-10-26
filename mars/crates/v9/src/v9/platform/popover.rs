/**
 * The HTML popover API as Blink runs it (html_element.cc ShowPopoverInternal and
 * HidePopoverInternal, popover_data.h): the `popover` attribute, showPopover, hidePopover and
 * togglePopover, `popovertarget` activation on buttons, and the `beforetoggle` and `toggle`
 * events. A popover's visibility lives in a private slot on its wrapper, never in an attribute,
 * so a page reading attributes sees what it would in Chromium. Auto and hint popovers share one
 * stack ordered by when they opened; showing one hides every open one that is not its ancestor.
 */
use crate::v9::platform::{Page, Target, components, dom, events, timers};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::heap::{ENUMERABLE, HIDDEN_ATTRS, Heap, Key, Prop, Slot};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The wrapper slot holding the open order (a number) while the popover is showing. */
const SHOWING: &str = "popoverShowing";
/** The effective type used by an open popover until a type mutation finishes hiding it. */
const OPEN_TYPE: &str = "popoverOpenType";
/** Recursive hiding suppresses another beforetoggle dispatch, as Blink's ScopedPopoverHiding does. */
const HIDING: &str = "popoverHiding";
/** The wrapper slot holding the queued, not yet fired `toggle` event, for coalescing. */
const PENDING: &str = "popoverToggle";
/** The document slot counting opens, so the auto stack keeps its order. */
const SEQUENCE: &str = "popoverSequence";
/** The native index of the queued task that fires a coalesced `toggle`. */
static TOGGLE_TASK: OnceLock<u32> = OnceLock::new();

/** The popover members of HTMLElement.prototype, and the invoker members of buttons and inputs. */
pub(in crate::v9::platform) fn install(b: &mut Builder, html: u32, invokers: &[u32]) {
    b.accessor(html, "popover", Some(attribute_get), Some(attribute_set));
    for name in ["showPopover", "hidePopover", "togglePopover"] {
        b.method(html, name, 0, method);
    }
    for proto in invokers {
        b.accessor(
            *proto,
            "popoverTargetAction",
            Some(attribute_get),
            Some(attribute_set),
        );
        b.accessor(
            *proto,
            "popoverTargetElement",
            Some(target_get),
            Some(target_set),
        );
    }
    let _ = TOGGLE_TASK.set(b.native("toggle", 0, toggle_task));
}

/** The popover type: auto for an empty or `auto` value, hint, else manual; None without one. */
fn kind(vm: &mut Vm, node: usize) -> Option<&'static str> {
    let value = vm.page.view(node)?.attributes.get("popover")?.clone();
    attribute_kind(Some(&value))
}

/** Content values normalize independently of the internal type used during a hide callback. */
fn attribute_kind(value: Option<&str>) -> Option<&'static str> {
    Some(match value?.to_ascii_lowercase().as_str() {
        "" | "auto" => "auto",
        "hint" => "hint",
        _ => "manual",
    })
}

/** Showing popovers retain their original internal type while a content attribute mutation fires events. */
fn internal_kind(vm: &mut Vm, node: usize) -> Option<&'static str> {
    let id = vm.page.state.wrappers.get(&node).copied();
    if let Some(id) = id.filter(|id| slot(vm, *id, SHOWING).is_some()) {
        if let Some(JsValue::String(value)) = slot(vm, id, OPEN_TYPE) {
            return attribute_kind(Some(&value.lossy()));
        }
    }
    kind(vm, node)
}

/** Every attribute mutation uses the same hide steps, including IDL, set/remove/toggleAttribute and reactions. */
pub(in crate::v9::platform) fn attribute_changed(vm: &mut Vm, node: usize) -> Res<()> {
    if is_showing(vm, node) && internal_kind(vm, node) != kind(vm, node) {
        hide(vm, node, true, false)?;
    }
    Ok(())
}

/** Removal closes the entire disconnected subtree without firing another beforetoggle event. */
pub(in crate::v9::platform) fn removed(vm: &mut Vm, root: usize) {
    let mut nodes = vec![root];
    nodes.extend(vm.page.all_descendants(root));
    for node in nodes {
        if !vm.page.connected(node) {
            if let Some(id) = vm.page.state.wrappers.get(&node).copied() {
                set_slot(vm, id, SHOWING, JsValue::Undefined);
                set_slot(vm, id, OPEN_TYPE, JsValue::Undefined);
            }
        }
    }
}

/** `popover` and `popoverTargetAction`: enumerated reflections with Blink's defaults. */
fn attribute_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    if dom::native_name(&call) == "popover" {
        return match kind(vm, node) {
            Some(kind) => Ok(JsValue::string(kind)?),
            None => Ok(JsValue::Null),
        };
    }
    let action = target_action(vm, node);
    Ok(JsValue::string(action)?)
}

/** Writes either reflected attribute; a null `popover` removes it, as its nullable IDL type. */
fn attribute_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| attribute_set_steps(vm, call))
}

/** Reflection writes the content value before the shared mutation reaction runs the hide algorithm. */
fn attribute_set_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let name = match dom::native_name(&call) {
        "popover" => "popover",
        _ => "popovertargetaction",
    };
    let value = match call.arg(0) {
        value if value.nullish() && name == "popover" => None,
        value => Some(vm.to_text(&value)?),
    };
    vm.page
        .set_attribute(node, name, value.as_deref())
        .map_err(|error| dom::dom_error(vm, error))?;
    Ok(JsValue::Undefined)
}

/** `popovertargetaction`: show, hide, or toggle for a missing or unknown value. */
fn target_action(vm: &mut Vm, node: usize) -> &'static str {
    let value = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get("popovertargetaction").cloned())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match value.as_str() {
        "show" => "show",
        "hide" => "hide",
        _ => "toggle",
    }
}

/** The element `popovertarget` names in the invoker's document, if any. */
fn target_of(vm: &mut Vm, node: usize) -> Option<usize> {
    let wanted = vm.page.view(node)?.attributes.get("popovertarget")?.clone();
    if wanted.is_empty() {
        return None;
    }
    let found = dom::search(vm, dom::DOCUMENT, |page, id| {
        page.view(id)
            .is_some_and(|view| view.attributes.get("id") == Some(&wanted))
    });
    found.first().copied()
}

/** `popoverTargetElement`: the named element. */
fn target_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let target = target_of(vm, node);
    dom::wrap_option(vm, target)
}

/** Setting `popoverTargetElement` points the attribute at the element's id, or clears it. */
// ponytail: Blink keeps an explicitly set element without an id; this reflects through ids only.
fn target_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let target = dom::node_of(vm, &call.arg(0));
    let id = target
        .and_then(|target| vm.page.view(target))
        .and_then(|view| view.attributes.get("id").cloned());
    vm.page
        .set_attribute(node, "popovertarget", id.as_deref())
        .map_err(|error| dom::dom_error(vm, error))?;
    Ok(JsValue::Undefined)
}

/** showPopover, hidePopover and togglePopover. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    match dom::native_name(&call) {
        "showPopover" => show(vm, node, true).map(|_| JsValue::Undefined),
        "hidePopover" => hide(vm, node, true, true).map(|_| JsValue::Undefined),
        _ => toggle(vm, node, &call.arg(0)),
    }
}

/** togglePopover(force | {force}): hide when showing, show when not, as `force` allows; the
result is whether the popover is showing afterwards. */
fn toggle(vm: &mut Vm, node: usize, options: &JsValue) -> Res<JsValue> {
    let force = match options {
        JsValue::Boolean(force) => Some(*force),
        JsValue::Object(_) => {
            let force = vm.get(options, &Key::from("force"))?;
            (force != JsValue::Undefined).then(|| force.truthy())
        }
        _ => None,
    };
    let showing = is_showing(vm, node);
    if showing && force != Some(true) {
        hide(vm, node, true, true)?;
    } else if !showing && force != Some(false) {
        show(vm, node, true)?;
    } else {
        valid(vm, node, showing, true)?;
    }
    Ok(JsValue::Boolean(is_showing(vm, node)))
}

/** The wrapper object that carries a node's popover slots. */
fn wrapper(vm: &mut Vm, node: usize) -> Res<Option<ObjectId>> {
    Ok(dom::wrap(vm, node)?.object())
}

/** A private slot's value on an object. */
fn slot(vm: &Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    match vm.heap.get(id)?.props.get(&Key::private(name))?.slot {
        Slot::Data(ref value) if *value != JsValue::Undefined => Some(value.clone()),
        _ => None,
    }
}

/** Write a private slot; undefined clears it. */
fn set_slot(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    let prop = Prop {
        slot: Slot::Data(value),
        attrs: HIDDEN_ATTRS,
    };
    vm.heap.put(id, Key::private(name), prop);
}

/** Whether a popover is in the showing state. */
pub(in crate::v9::platform) fn is_showing(vm: &mut Vm, node: usize) -> bool {
    matches_open(&vm.page, &vm.heap, node)
}

/** Selectors inspect the actual private visibility slot without exposing or synthesizing an attribute. */
pub(in crate::v9::platform) fn matches_open(page: &Page, heap: &Heap, node: usize) -> bool {
    page.state
        .wrappers
        .get(&node)
        .and_then(|id| heap.get(*id))
        .and_then(|object| object.props.get(&Key::private(SHOWING)))
        .is_some_and(
            |property| matches!(&property.slot, Slot::Data(value) if *value != JsValue::Undefined),
        )
}

/** "Check popover validity": a popover that is already in the expected state answers false;
one without a valid `popover` attribute, disconnected, or an open modal-less dialog cannot
change, which throws when asked to. */
fn valid(vm: &mut Vm, node: usize, showing: bool, throw: bool) -> Res<bool> {
    let error = if internal_kind(vm, node).is_none() {
        Some((
            "NotSupportedError",
            "Not supported on elements that do not have a valid value for the 'popover' attribute.",
        ))
    } else if is_showing(vm, node) != showing {
        return Ok(false);
    } else if !vm.page.connected(node) {
        Some((
            "InvalidStateError",
            "Invalid on disconnected popover elements.",
        ))
    } else if !showing && open_dialog(vm, node) {
        Some((
            "InvalidStateError",
            "The dialog is already open as a dialog, and therefore cannot be opened as a popover.",
        ))
    } else {
        None
    };
    match error {
        Some((name, message)) if throw => Err(vm.dom_exception(name, message)),
        Some(_) => Ok(false),
        None => Ok(true),
    }
}

/** A dialog already open through its `open` attribute. */
fn open_dialog(vm: &mut Vm, node: usize) -> bool {
    vm.page
        .view(node)
        .is_some_and(|view| view.tag == "dialog" && view.attributes.contains_key("open"))
}

/** Fire `beforetoggle` with its states; false when a listener canceled it. */
fn before_toggle(vm: &mut Vm, node: usize, opening: bool) -> Res<bool> {
    let event = toggle_event(vm, "beforetoggle", opening, opening)?;
    events::dispatch(vm, Target::Node(node), event)
}

/** A ToggleEvent-shaped event: `oldState` and `newState` as read-only own members. */
fn toggle_event(vm: &mut Vm, kind: &str, opening: bool, cancelable: bool) -> Res<ObjectId> {
    let event = events::create_event(vm, kind, false, cancelable)?;
    let id = event.object().expect("event object");
    let (old, new) = if opening {
        ("closed", "open")
    } else {
        ("open", "closed")
    };
    for (name, value) in [("oldState", old), ("newState", new)] {
        let prop = Prop {
            slot: Slot::Data(JsValue::string(value)?),
            attrs: ENUMERABLE,
        };
        vm.heap.put(id, Key::from(name), prop);
    }
    Ok(id)
}

/** Queue `toggle` as a task; a toggle still queued keeps its oldState and takes the new state,
as Blink coalesces them (an open then close in one task fires nothing visible as open). */
fn queue_toggle(vm: &mut Vm, node: usize, opening: bool) -> Res<()> {
    let Some(id) = wrapper(vm, node)? else {
        return Ok(());
    };
    if let Some(JsValue::Object(pending)) = slot(vm, id, PENDING) {
        let state = JsValue::string(if opening { "open" } else { "closed" })?;
        let prop = Prop {
            slot: Slot::Data(state),
            attrs: ENUMERABLE,
        };
        vm.heap.put(pending, Key::from("newState"), prop);
        return Ok(());
    }
    let event = toggle_event(vm, "toggle", opening, false)?;
    set_slot(vm, id, PENDING, JsValue::Object(event));
    let task = *TOGGLE_TASK.get().expect("popover installed");
    let callback = vm.native_closure(task, vec![JsValue::Object(id)])?;
    timers::schedule(vm, JsValue::Object(callback), Vec::new())
}

/** The queued task: take the coalesced event off the popover and dispatch it there. */
fn toggle_task(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(JsValue::Object(id)) = vm.slots(call.callee).first().cloned() else {
        return Ok(JsValue::Undefined);
    };
    let Some(JsValue::Object(event)) = slot(vm, id, PENDING) else {
        return Ok(JsValue::Undefined);
    };
    set_slot(vm, id, PENDING, JsValue::Undefined);
    if let Some(node) = dom::node_of(vm, &JsValue::Object(id)) {
        events::dispatch(vm, Target::Node(node), event)?;
    }
    Ok(JsValue::Undefined)
}

/** The open auto and hint popovers, oldest first. */
fn stack(vm: &mut Vm) -> Vec<usize> {
    let wrappers: Vec<(usize, ObjectId)> = vm
        .page
        .state
        .wrappers
        .iter()
        .map(|(n, id)| (*n, *id))
        .collect();
    let mut open: Vec<(f64, usize)> = Vec::new();
    for (node, id) in wrappers {
        if let Some(JsValue::Number(order)) = slot(vm, id, SHOWING)
            && internal_kind(vm, node) != Some("manual")
        {
            open.push((order, node));
        }
    }
    open.sort_by(|a, b| a.0.total_cmp(&b.0));
    open.into_iter().map(|(_, node)| node).collect()
}

/** Escape requests closure only from the newest auto or hint close watcher; manual popovers do not register one. */
pub(in crate::v9::platform) fn close_top(vm: &mut Vm) -> Res<()> {
    if let Some(node) = stack(vm).last().copied() {
        hide(vm, node, true, false)?;
    }
    Ok(())
}

/** Hide, newest first, every stacked popover above `keep` (all of them when None). */
fn hide_above(vm: &mut Vm, keep: Option<usize>, fire: bool) -> Res<()> {
    loop {
        let open = stack(vm);
        let Some(top) = open.last().copied() else {
            return Ok(());
        };
        if Some(top) == keep {
            return Ok(());
        }
        hide(vm, top, fire, false)?;
        if is_showing(vm, top) {
            return Ok(());
        }
    }
}

/** The nearest open stacked popover among a node's ancestors, which stays open beneath it. */
// ponytail: Blink also walks the invoker chain (popovertarget sources); ancestry is DOM only.
fn ancestor(vm: &mut Vm, node: usize) -> Option<usize> {
    let open = stack(vm);
    vm.page
        .ancestors(node)
        .into_iter()
        .find(|id| open.contains(id))
}

/** ShowPopoverInternal: validity, a cancelable `beforetoggle`, closing unrelated auto popovers,
then the showing state and a queued `toggle`. */
fn show(vm: &mut Vm, node: usize, throw: bool) -> Res<()> {
    if !valid(vm, node, false, throw)? || !before_toggle(vm, node, true)? {
        return Ok(());
    }
    if !valid(vm, node, false, false)? {
        return Ok(());
    }
    let original_type = kind(vm, node);
    if original_type != Some("manual") {
        let keep = ancestor(vm, node);
        hide_above(vm, keep, true)?;
        if kind(vm, node) != original_type {
            return if throw {
                Err(vm.dom_exception(
                    "InvalidStateError",
                    "The value of the popover attribute was changed while hiding popovers.",
                ))
            } else {
                Ok(())
            };
        }
        if !valid(vm, node, false, false)? {
            return Ok(());
        }
    }
    let document = vm.intrinsic_id(realm().ids.document);
    vm.materialize(document)?;
    let order = match slot(vm, document, SEQUENCE) {
        Some(JsValue::Number(order)) => order + 1.0,
        _ => 1.0,
    };
    set_slot(vm, document, SEQUENCE, JsValue::Number(order));
    if let Some(id) = wrapper(vm, node)? {
        set_slot(
            vm,
            id,
            OPEN_TYPE,
            JsValue::string(original_type.unwrap_or("manual"))?,
        );
        set_slot(vm, id, SHOWING, JsValue::Number(order));
    }
    queue_toggle(vm, node, true)
}

/** HidePopoverInternal: popovers stacked above this one close first, then (with events) a
non-cancelable `beforetoggle`, the closed state and a queued `toggle`. */
fn hide(vm: &mut Vm, node: usize, fire: bool, throw: bool) -> Res<()> {
    if !valid(vm, node, true, throw)? {
        return Ok(());
    }
    let Some(id) = wrapper(vm, node)? else {
        return Ok(());
    };
    let previous = slot(vm, id, HIDING).unwrap_or(JsValue::Undefined);
    let nested = previous.truthy();
    set_slot(vm, id, HIDING, JsValue::Boolean(true));
    let result = hide_steps(vm, node, id, fire && !nested);
    set_slot(vm, id, HIDING, previous);
    result
}

/** Reentrant callbacks may close and reopen this node; an earlier hide never closes that newer opening. */
fn hide_steps(vm: &mut Vm, node: usize, id: ObjectId, fire: bool) -> Res<()> {
    let opening = slot(vm, id, SHOWING);
    if internal_kind(vm, node) != Some("manual") && stack(vm).contains(&node) {
        hide_above(vm, Some(node), fire)?;
        if slot(vm, id, SHOWING) != opening || !valid(vm, node, true, false)? {
            return Ok(());
        }
    }
    if fire {
        before_toggle(vm, node, false)?;
        if slot(vm, id, SHOWING) != opening || !valid(vm, node, true, false)? {
            return Ok(());
        }
    }
    set_slot(vm, id, SHOWING, JsValue::Undefined);
    set_slot(vm, id, OPEN_TYPE, JsValue::Undefined);
    if fire {
        queue_toggle(vm, node, false)?;
    }
    Ok(())
}

/** A button's or input button's `popovertarget` activation after an uncanceled click: show,
hide or toggle the named popover. Submit buttons with a form owner submit instead. */
pub(in crate::v9::platform) fn activate(vm: &mut Vm, node: usize) -> Res<()> {
    let Some(view) = vm.page.view(node) else {
        return Ok(());
    };
    let button_type = view.attributes.get("type").cloned().unwrap_or_default();
    let button_type = button_type.to_ascii_lowercase();
    let invoker = match view.tag.as_str() {
        "button" => button_type == "button" || button_type == "reset" || !in_form(vm, node),
        "input" => ["button", "reset", "submit", "image"].contains(&button_type.as_str()),
        _ => false,
    };
    if !invoker || view.attributes.contains_key("disabled") {
        return Ok(());
    }
    let Some(target) = target_of(vm, node) else {
        return Ok(());
    };
    if kind(vm, target).is_none() {
        return Ok(());
    }
    let action = target_action(vm, node);
    let showing = is_showing(vm, target);
    if showing && action != "show" {
        hide(vm, target, true, false)
    } else if !showing && action != "hide" {
        show(vm, target, false)
    } else {
        Ok(())
    }
}

/** Whether a node sits inside a form, which makes a default-type button submit it. */
fn in_form(vm: &mut Vm, node: usize) -> bool {
    let ancestors = vm.page.ancestors(node);
    ancestors.into_iter().any(|id| vm.page.tag(id) == "form")
}
