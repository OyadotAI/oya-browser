/**
 * Shadow roots: attachShadow, the ShadowRoot interface, slot assignment and the composed event
 * path. A root is a `#shadow-root` child of its host carrying a `mode` attribute; a projected
 * host gets one through the same fragment insertion any markup takes, so Eye holds the identical
 * node and flattens it with the host's assigned light children for layout and agents.
 */
use super::SHADOW_ROOT;
use crate::constants::{DOCUMENT_DEPTH, VM_DETACHED_NODES};
use crate::document::InsertPosition;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::platform::dom::{self, dom_error, native_name, this_node, wrap};
use crate::v9::platform::{BODY, HEAD, HTML, Page};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::Key;
use crate::v9::vm::{Call, Res, Vm};

/** Elements the specification lets host a shadow root besides valid custom element names. */
const HOSTS: [&str; 18] = [
    "article",
    "aside",
    "blockquote",
    "body",
    "div",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "main",
    "nav",
    "p",
    "section",
    "span",
];

impl Page<'_> {
    /** The shadow root a host holds, open or closed. */
    pub fn shadow_root(&mut self, host: usize) -> Option<usize> {
        self.all_children(host)
            .into_iter()
            .find(|child| self.is_shadow_root(*child))
    }
}

/** The shadow root whose tree holds a node, if it is in one. */
pub(in crate::v9) fn containing_root(page: &mut Page, node: usize) -> Option<usize> {
    if !page.has_shadows() {
        return None;
    }
    let mut current = Some(node);
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        let id = current?;
        if page.is_shadow_root(id) {
            return Some(id);
        }
        current = page.parent(id);
    }
    None
}

/** Whether a root is open (closed roots stay hidden from `shadowRoot` and `assignedSlot`). */
fn open(page: &mut Page, root: usize) -> bool {
    page.view(root).is_none_or(|view| {
        view.attributes
            .get("mode")
            .is_none_or(|mode| mode != "closed")
    })
}

/** Whether an element may host a shadow root. */
fn may_host(tag: &str) -> bool {
    HOSTS.contains(&tag) || super::registry::valid_name(tag)
}

/** `Element.attachShadow({mode})`. */
pub(in crate::v9) fn attach_shadow(vm: &mut Vm, host: usize, init: &JsValue) -> Res<JsValue> {
    let mode = if matches!(init, JsValue::Object(_)) {
        let value = vm.get(init, &Key::from("mode"))?;
        vm.to_text(&value)?
    } else {
        String::new()
    };
    if mode != "open" && mode != "closed" {
        return Err(vm.type_error(&format!(
            "Failed to execute 'attachShadow' on 'Element': The provided value '{mode}' is not a valid enum value of type ShadowRootMode."
        )));
    }
    let tag = vm.page.tag(host);
    if matches!(host, HTML | HEAD | BODY) || !may_host(&tag) {
        return Err(vm.dom_exception(
            "NotSupportedError",
            &format!("Failed to execute 'attachShadow' on 'Element': This element does not support attachShadow ({tag})."),
        ));
    }
    if vm.page.shadow_root(host).is_some() {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Failed to execute 'attachShadow' on 'Element': Shadow root cannot be created on a host which already hosts a shadow tree.",
        ));
    }
    let root = create_root(&mut vm.page, host, &mode).map_err(|error| dom_error(vm, error))?;
    wrap(vm, root)
}

/** Make the root node in whichever store holds the host. */
fn create_root(page: &mut Page, host: usize, mode: &str) -> EngineResult<usize> {
    if page.is_projected(host) {
        let html = format!("<template shadowrootmode=\"{mode}\"></template>");
        page.insert_html(host, InsertPosition::AfterBegin, &html)?;
        return page.shadow_root(host).ok_or_else(|| {
            EngineError::unsupported("The document engine did not create the shadow root.")
        });
    }
    let root = page.create(SHADOW_ROOT, None)?;
    if let Some(record) = page.state.detached.get_mut(&root) {
        record.attributes.insert("mode".into(), mode.into());
    }
    page.state.components.shadows = true;
    let first = page.all_children(host).first().copied();
    page.insert(host, root, first)?;
    Ok(root)
}

/** ShadowRoot accessors: host, mode and the fixed init flags. */
pub(in crate::v9) fn shadow_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let root = this_node(vm, &call)?;
    if !vm.page.is_shadow_root(root) {
        return Err(vm.type_error("Illegal invocation"));
    }
    match native_name(&call) {
        "host" => {
            let host = vm.page.parent(root);
            dom::wrap_option(vm, host)
        }
        "mode" => {
            let open = open(&mut vm.page, root);
            Ok(JsValue::string(if open { "open" } else { "closed" })?)
        }
        "slotAssignment" => Ok(JsValue::string("named")?),
        "activeElement" => Ok(JsValue::Null),
        _ => Ok(JsValue::Boolean(false)),
    }
}

/** Element members for shadow DOM: shadowRoot, assignedSlot and the slot element's methods. */
pub(in crate::v9) fn element_shadow(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    match native_name(&call) {
        "shadowRoot" => {
            let root = vm
                .page
                .shadow_root(node)
                .filter(|root| open(&mut vm.page, *root));
            dom::wrap_option(vm, root)
        }
        "assignedSlot" => {
            let slot = assigned_slot(&mut vm.page, node).filter(|slot| {
                containing_root(&mut vm.page, *slot).is_some_and(|root| open(&mut vm.page, root))
            });
            dom::wrap_option(vm, slot)
        }
        name @ ("assignedNodes" | "assignedElements") => {
            if vm.page.tag(node) != "slot" {
                return Ok(JsValue::Undefined);
            }
            let flatten = super::option(vm, &call.arg(0), "flatten")?;
            let mut nodes = if flatten {
                flattened(&mut vm.page, node, 0)
            } else {
                assigned(&mut vm.page, node)
            };
            if name == "assignedElements" {
                nodes.retain(|id| vm.page.is_element(*id));
            }
            let values = nodes
                .into_iter()
                .map(|id| wrap(vm, id))
                .collect::<Res<Vec<_>>>()?;
            Ok(JsValue::Object(vm.new_array(values)?))
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** The slot name a light child asks for ("" is the default slot). */
fn slot_name(page: &mut Page, node: usize) -> String {
    page.view(node)
        .filter(|view| !view.tag.starts_with('#'))
        .and_then(|view| view.attributes.get("slot").cloned())
        .unwrap_or_default()
}

/** The name a slot element offers. */
fn offered(page: &mut Page, slot: usize) -> String {
    page.view(slot)
        .and_then(|view| view.attributes.get("name").cloned())
        .unwrap_or_default()
}

/** The first slot in a shadow tree with a name, in tree order. */
fn first_slot(page: &mut Page, root: usize, name: &str) -> Option<usize> {
    page.descendants(root)
        .into_iter()
        .find(|id| page.tag(*id) == "slot" && offered(page, *id) == name)
}

/** The slot a host's light child is assigned to. */
pub(in crate::v9) fn assigned_slot(page: &mut Page, node: usize) -> Option<usize> {
    if !page.has_shadows() || page.is_shadow_root(node) {
        return None;
    }
    let host = page.parent(node)?;
    let root = page.shadow_root(host)?;
    let tag = page.tag(node);
    if tag == "#comment" {
        return None;
    }
    let name = slot_name(page, node);
    first_slot(page, root, &name)
}

/** The light children assigned to a slot, in tree order. */
pub(in crate::v9) fn assigned(page: &mut Page, slot: usize) -> Vec<usize> {
    let Some(root) = containing_root(page, slot) else {
        return Vec::new();
    };
    let name = offered(page, slot);
    if first_slot(page, root, &name) != Some(slot) {
        return Vec::new();
    }
    let Some(host) = page.parent(root) else {
        return Vec::new();
    };
    page.children(host)
        .into_iter()
        .filter(|child| page.tag(*child) != "#comment" && slot_name(page, *child) == name)
        .collect()
}

/** `assignedNodes({flatten: true})`: nested slots give their own assignments or fallback. */
fn flattened(page: &mut Page, slot: usize, depth: usize) -> Vec<usize> {
    let mut nodes = assigned(page, slot);
    if nodes.is_empty() {
        nodes = page.children(slot);
    }
    if depth > DOCUMENT_DEPTH {
        return nodes;
    }
    let mut out = Vec::new();
    for node in nodes {
        if page.tag(node) == "slot" && containing_root(page, node).is_some() {
            out.extend(flattened(page, node, depth + 1));
        } else {
            out.push(node);
        }
    }
    out
}

/**
 * The next node up an event's path: a slotted node's slot, a shadow root's host when the event
 * is composed, otherwise the parent.
 */
pub(in crate::v9) fn composed_parent(
    page: &mut Page,
    node: usize,
    composed: bool,
) -> Option<usize> {
    if !page.has_shadows() {
        return page.parent(node);
    }
    if page.is_shadow_root(node) {
        return composed.then(|| page.parent(node)).flatten();
    }
    assigned_slot(page, node).or_else(|| page.parent(node))
}

/** Whether `ancestor` is `node` or contains it across shadow boundaries. */
fn shadow_including_ancestor(page: &mut Page, ancestor: usize, node: usize) -> bool {
    let mut current = Some(node);
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        let Some(id) = current else {
            return false;
        };
        if id == ancestor {
            return true;
        }
        current = page.parent(id);
    }
    false
}

/** The target listeners on `current` see: inside-shadow targets are retargeted to their host. */
pub(in crate::v9) fn retarget(page: &mut Page, target: usize, current: usize) -> usize {
    if !page.has_shadows() {
        return target;
    }
    let mut shown = target;
    for _ in 0..DOCUMENT_DEPTH {
        let Some(root) = containing_root(page, shown) else {
            return shown;
        };
        if shadow_including_ancestor(page, root, current) {
            return shown;
        }
        let Some(host) = page.parent(root) else {
            return shown;
        };
        shown = host;
    }
    shown
}

/** ShadowRoot and DocumentFragment members a fragment has but Node lacks. */
pub(in crate::v9) fn fragment_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    match native_name(&call) {
        "getElementById" => {
            let wanted = vm.to_text(&call.arg(0))?;
            let found = vm.page.descendants(node).into_iter().find(|id| {
                !wanted.is_empty()
                    && vm
                        .page
                        .view(*id)
                        .is_some_and(|view| view.attributes.get("id") == Some(&wanted))
            });
            dom::wrap_option(vm, found)
        }
        "getSelection" => super::super::range::selection(vm),
        "elementFromPoint" => Ok(JsValue::Null),
        "elementsFromPoint" | "getAnimations" => Ok(JsValue::Object(vm.new_array(Vec::new())?)),
        _ => Ok(JsValue::Undefined),
    }
}

/** Slots whose assignment a child list or attribute change of `node` may have changed. */
pub(in crate::v9) fn affected_slots(page: &mut Page, parent: usize, nodes: &[usize]) -> Vec<usize> {
    if !page.has_shadows() {
        return Vec::new();
    }
    let mut slots = Vec::new();
    if let Some(root) = page.shadow_root(parent) {
        for node in nodes {
            let name = slot_name(page, *node);
            slots.extend(first_slot(page, root, &name));
        }
    }
    for node in nodes {
        if page.tag(*node) == "slot" && containing_root(page, *node).is_some() {
            slots.push(*node);
        }
    }
    slots.sort_unstable();
    slots.dedup();
    slots
}
