/** Frame lookup metadata carries identities only; the coordinator retains delivery authority. */
use super::{Page, messaging, nodes};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::heap::Key;
use crate::v9::vm::{Res, Vm};
use serde::{Deserialize, Serialize};
mod validation;
use validation::invalid;
pub(in crate::v9) use validation::{validate, validate_state};

/** Both positive identities and their negative wire encodings remain exact JavaScript numbers. */
pub const MAX_FRAME_WINDOW_ID: i64 = (1_i64 << 53) - 2;
/** Incarnations are private numeric slots, so they must also remain exactly representable. */
const MAX_INCARNATION: u64 = (1_u64 << 53) - 1;
/** A connected projection plus retained detached nodes bounds owner bookkeeping. */
const OWNER_RECORDS: usize = crate::constants::NODES + crate::constants::VM_DETACHED_NODES;

/** One immediate child context in its parent's current document order. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRoute {
    /** The embedding element, used only in the parent's node namespace. */
    pub owner: usize,
    /** A coordinator-issued context identity, never an array position. */
    pub window: i64,
    /** Only the parent's embedding attribute introduces a cross-origin name. */
    pub name: String,
}

/// Bounded native browsing-context metadata exposes relationships without granting document access.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameContext {
    /// Globally stable context identity, distinct from document generations and local node IDs.
    pub window: i64,
    /// Immediate parent; the actual root alone has no parent.
    pub parent: Option<i64>,
    /// Immediate child lookup metadata in this context's current DOM order.
    pub children: Vec<FrameRoute>,
}

/** Resolve a proxy into the native directory without interpreting local owner numbers globally. */
pub(super) fn proxy_window(vm: &Vm, target: i64) -> Option<i64> {
    let own = vm.page.environment.context_window?;
    let context = vm
        .page
        .environment
        .frame_contexts
        .iter()
        .find(|context| context.window == own)?;
    if target == messaging::PARENT {
        return context.parent.or(Some(own));
    }
    if let Ok(owner) = usize::try_from(target) {
        return context
            .children
            .iter()
            .find(|route| route.owner == owner)
            .map(|route| route.window);
    }
    sibling_window(target)
}

/** Canonical proxies preserve equality across contentWindow, parent, top and message sources. */
pub(super) fn canonical_target(vm: &Vm, window: i64) -> Option<i64> {
    let own = vm.page.environment.context_window?;
    let context = vm
        .page
        .environment
        .frame_contexts
        .iter()
        .find(|context| context.window == own)?;
    if context.parent == Some(window) {
        return Some(messaging::PARENT);
    }
    if let Some(route) = context.children.iter().find(|route| route.window == window)
        && vm
            .page
            .environment
            .scripted_frame_owners
            .contains(&route.owner)
    {
        return i64::try_from(route.owner).ok();
    }
    sibling_target(window)
}

/** Detachment retires the old context even when the same element is reinserted in one turn. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameDetach {
    /** The owner before adoption; adopted owners receive a separate updated record. */
    pub owner: usize,
    /** The new incarnation, which a retained old proxy cannot acquire. */
    pub incarnation: u64,
}

/** Sibling targets occupy a disjoint namespace from parent-local node identities. */
pub fn sibling_target(window: i64) -> Option<i64> {
    (1..=MAX_FRAME_WINDOW_ID)
        .contains(&window)
        .then(|| -window - 1)
}

/** Decode only canonical sibling identities; -1 remains the parent alias. */
pub fn sibling_window(target: i64) -> Option<i64> {
    let window = target.checked_neg()?.checked_sub(1)?;
    (1..=MAX_FRAME_WINDOW_ID)
        .contains(&window)
        .then_some(window)
}

/** The current owner incarnation is zero until a real connected detachment occurs. */
pub(in crate::v9) fn incarnation(page: &Page, owner: usize) -> u64 {
    page.state
        .frame_incarnations
        .get(&owner)
        .copied()
        .unwrap_or_default()
}

/** All connected iframes participate in indexed access, including blank and refused frames. */
pub(in crate::v9) fn children(page: &mut Page) -> Vec<usize> {
    page.descendants(nodes::HTML)
        .into_iter()
        .filter(|owner| page.tag(*owner) == "iframe")
        .collect()
}

/** Count current children without caching stale positions as context identities. */
pub(in crate::v9) fn child_count(page: &mut Page) -> usize {
    children(page).len()
}

/** A connected subtree removal invalidates every iframe incarnation it contains. */
pub(in crate::v9) fn detach(page: &mut Page, node: usize) -> EngineResult<()> {
    if !page.connected(node) {
        return Ok(());
    }
    let mut nodes = vec![node];
    nodes.extend(page.all_descendants(node));
    let owners: Vec<_> = nodes
        .into_iter()
        .filter(|id| page.tag(*id) == "iframe")
        .collect();
    for owner in owners {
        retire(page, owner)?;
    }
    Ok(())
}

/** Retire caches and pending loads together; retained objects keep their immutable old token. */
pub(super) fn retire(page: &mut Page, owner: usize) -> EngineResult<()> {
    if page.state.frame_incarnations.len() >= OWNER_RECORDS
        && !page.state.frame_incarnations.contains_key(&owner)
        || page.effects.frame_detaches.len() >= OWNER_RECORDS
    {
        return Err(EngineError::limit(
            "frame incarnation records",
            OWNER_RECORDS,
        ));
    }
    let incarnation = incarnation(page, owner)
        .checked_add(1)
        .filter(|id| *id <= MAX_INCARNATION)
        .ok_or_else(invalid)?;
    page.state.frame_incarnations.insert(owner, incarnation);
    page.effects
        .frame_detaches
        .push(FrameDetach { owner, incarnation });
    page.state.remote_windows.remove(&(owner as i64));
    page.state.frame_windows.remove(&owner);
    page.state.frame_loads.remove(&owner);
    if let Some(document) = page.state.frames.remove(&owner) {
        page.state.document_frames.remove(&document);
    }
    Ok(())
}

/** An adopted node retains its current incarnation while the old owner remains retired. */
pub(in crate::v9) fn adopt(page: &mut Page, old: usize, new: usize) -> EngineResult<()> {
    if old == new {
        return Ok(());
    }
    if let Some(incarnation) = page.state.frame_incarnations.get(&old).copied() {
        if page.state.frame_incarnations.len() >= OWNER_RECORDS
            || page.effects.frame_detaches.len() >= OWNER_RECORDS
        {
            return Err(EngineError::limit(
                "frame incarnation records",
                OWNER_RECORDS,
            ));
        }
        page.state.frame_incarnations.insert(new, incarnation);
        page.effects.frame_detaches.push(FrameDetach {
            owner: new,
            incarnation,
        });
    }
    Ok(())
}

/** Resolve virtual frame properties before the VM's ordinary-object fast path. */
pub(in crate::v9) fn lookup(vm: &mut Vm, object: ObjectId, key: &Key) -> Res<Option<JsValue>> {
    // Dedicated workers cannot contain WindowProxy objects or browsing contexts.
    if vm.page.environment.worker_name.is_some() {
        return Ok(None);
    }
    let Key::Str(name) = key else { return Ok(None) };
    // With no child contexts and no parent routes only the page's own window can name a frame
    // (its iframe elements). Every property load comes through here before its cache, and
    // finding a proxy target walks the heap: skipping it was ~5% of V9's execution on YouTube.
    let environment = &vm.page.environment;
    if environment.frame_contexts.is_empty()
        && environment.parent_frames.is_empty()
        && object != vm.global_object()
    {
        return Ok(None);
    }
    let proxy_target = messaging::target_of(vm, &JsValue::Object(object));
    if let Some(target) = proxy_target
        && let Some(window) = proxy_window(vm, target)
        && vm
            .page
            .environment
            .frame_contexts
            .iter()
            .any(|context| context.window == window)
    {
        if key.index().is_none() && (name.is_empty() || ordinary_property(vm, object, key)?) {
            return Ok(None);
        }
        let context = vm
            .page
            .environment
            .frame_contexts
            .iter()
            .find(|context| context.window == window)
            .expect("directory unchanged");
        let route = match key.index() {
            Some(index) => context.children.get(index as usize),
            None => context
                .children
                .iter()
                .find(|route| route.name == name.lossy()),
        };
        let target = route.and_then(|route| sibling_target(route.window));
        return target
            .map(|target| messaging::proxy(vm, target).map(Some))
            .unwrap_or(Ok(None));
    }
    let parent = proxy_target == Some(messaging::PARENT);
    let local = object == vm.global_object();
    if !parent && !local {
        return Ok(None);
    }
    if key.index().is_none() && (name.is_empty() || ordinary_property(vm, object, key)?) {
        return Ok(None);
    }
    if parent {
        return parent_lookup(vm, key, &name.lossy());
    }
    local_lookup(vm, key, &name.lossy())
}

/** Built-in and authored ordinary properties take precedence over named frame properties. */
fn ordinary_property(vm: &mut Vm, mut object: ObjectId, key: &Key) -> Res<bool> {
    for _ in 0..crate::v9::vm::property::CHAIN_STEPS {
        if vm.own(object, key)?.is_some() {
            return Ok(true);
        }
        let Some(parent) = vm.proto_of(object) else {
            return Ok(false);
        };
        object = parent;
    }
    Ok(true)
}

/** Parent snapshots never expose documents, only identity-preserving remote windows. */
fn parent_lookup(vm: &mut Vm, key: &Key, name: &str) -> Res<Option<JsValue>> {
    let routes = &vm.page.environment.parent_frames;
    let route = match key.index() {
        Some(index) => routes.get(index as usize),
        None => routes.iter().find(|route| route.name == name),
    };
    let Some(target) = route.and_then(|route| sibling_target(route.window)) else {
        return Ok(None);
    };
    messaging::proxy(vm, target).map(Some)
}

/** Parent-side lookup reads the current DOM, including insertions in the same script turn. */
fn local_lookup(vm: &mut Vm, key: &Key, name: &str) -> Res<Option<JsValue>> {
    let children = children(&mut vm.page);
    let owner = match key.index() {
        Some(index) => children.get(index as usize).copied(),
        None => children
            .into_iter()
            .find(|owner| vm.page.attribute_value(*owner, "name").as_deref() == Some(name)),
    };
    match owner {
        Some(owner) => super::frames::frame_get(vm, owner, "contentWindow").map(Some),
        None => Ok(None),
    }
}
