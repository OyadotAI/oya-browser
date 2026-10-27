/**
 * DOM Traversal: NodeFilter, TreeWalker and NodeIterator over the tree scripts see (shadow roots
 * are not children). Each object keeps its root, filter and position in private slots; filters
 * are page functions or `acceptNode` objects, and their exceptions propagate as in browsers.
 */
use super::{FRAGMENT, SHADOW_ROOT, get_slot, set_slot};
use crate::constants::{DOCUMENT_DEPTH, NODES, VM_DETACHED_NODES};
use crate::v9::platform::HTML;
use crate::v9::platform::dom::{DOCUMENT, native_name, node_of, wrap};
use crate::v9::platform::frames::{DOCTYPE, DOCUMENT_NODE};
use crate::v9::platform::implementation::PROCESSING_INSTRUCTION;
use crate::v9::value::JsValue;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::{Call, Res, Vm};

/** NodeFilter results and `whatToShow` bits, as the specification numbers them. */
pub(super) const FILTERS: [(&str, f64); 16] = [
    ("FILTER_ACCEPT", 1.0),
    ("FILTER_REJECT", 2.0),
    ("FILTER_SKIP", 3.0),
    ("SHOW_ALL", 4_294_967_295.0),
    ("SHOW_ELEMENT", 1.0),
    ("SHOW_ATTRIBUTE", 2.0),
    ("SHOW_TEXT", 4.0),
    ("SHOW_CDATA_SECTION", 8.0),
    ("SHOW_ENTITY_REFERENCE", 16.0),
    ("SHOW_ENTITY", 32.0),
    ("SHOW_PROCESSING_INSTRUCTION", 64.0),
    ("SHOW_COMMENT", 128.0),
    ("SHOW_DOCUMENT", 256.0),
    ("SHOW_DOCUMENT_TYPE", 512.0),
    ("SHOW_DOCUMENT_FRAGMENT", 1024.0),
    ("SHOW_NOTATION", 2048.0),
];
/** Every node type is shown. */
const SHOW_ALL: u32 = u32::MAX;
/** The private slot that is true while the traversal's filter is running. */
const ACTIVE_FLAG: &str = "active";
/** The DOM error a filter that calls back into its own traversal raises. */
const INVALID_STATE: &str = "InvalidStateError";
/** The message for that error. */
const RECURSIVE_FILTER: &str = "The filter is already running for this traversal.";
/** The node is accepted. */
const ACCEPT: u8 = 1;
/** The node and its subtree are rejected (TreeWalker); just the node (NodeIterator). */
const REJECT: u8 = 2;
/** The node is skipped but its children are considered. */
const SKIP: u8 = 3;
/** Node types that set `whatToShow` bits. */
const ELEMENT_TYPE: u32 = 1;
const PROCESSING_INSTRUCTION_TYPE: u32 = 7;
const TEXT_TYPE: u32 = 3;
const COMMENT_TYPE: u32 = 8;
const DOCUMENT_TYPE: u32 = 9;
const DOCTYPE_TYPE: u32 = 10;
const FRAGMENT_TYPE: u32 = 11;
/** The node type of each engine tag that is not an element, by tag. */
const TAG_TYPES: [(&str, u32); 7] = [
    ("#text", TEXT_TYPE),
    ("#comment", COMMENT_TYPE),
    (PROCESSING_INSTRUCTION, PROCESSING_INSTRUCTION_TYPE),
    (DOCTYPE, DOCTYPE_TYPE),
    (DOCUMENT_NODE, DOCUMENT_TYPE),
    (FRAGMENT, FRAGMENT_TYPE),
    (SHADOW_ROOT, FRAGMENT_TYPE),
];

/** `document.createTreeWalker` and `document.createNodeIterator`. */
pub(in crate::v9) fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let root = node_of(vm, &call.arg(0)).ok_or_else(|| {
        vm.type_error("Failed to execute 'createTreeWalker': parameter 1 is not of type 'Node'.")
    })?;
    let show = match call.arg(1) {
        JsValue::Undefined => SHOW_ALL,
        value => vm.to_number(&value)? as i64 as u32,
    };
    let walker = native_name(&call) == "createTreeWalker";
    let proto = if walker {
        super::ids().tree_walker
    } else {
        super::ids().node_iterator
    };
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let root_value = wrap(vm, root)?;
    set_slot(vm, id, "root", root_value.clone());
    set_slot(vm, id, "current", root_value);
    set_slot(vm, id, "show", JsValue::Number(f64::from(show)));
    let filter = match call.arg(2) {
        JsValue::Undefined => JsValue::Null,
        value => value,
    };
    set_slot(vm, id, "filter", filter);
    set_slot(vm, id, "before", JsValue::Boolean(true));
    Ok(JsValue::Object(id))
}

/** One traversal object's root, current node and filter. */
struct Walk {
    this: JsValue,
    root: usize,
    show: u32,
    filter: JsValue,
}

/** Read a traversal object's state, refusing other receivers. */
fn walk(vm: &mut Vm, this: &JsValue) -> Res<Walk> {
    let root = get_slot(vm, this, "root");
    let Some(root) = node_of(vm, &root) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let show = match get_slot(vm, this, "show") {
        JsValue::Number(number) => number as u32,
        _ => SHOW_ALL,
    };
    Ok(Walk {
        this: this.clone(),
        root,
        show,
        filter: get_slot(vm, this, "filter"),
    })
}

/** The traversal's current node (TreeWalker) or reference node (NodeIterator). */
fn current(vm: &mut Vm, walk: &Walk) -> usize {
    let value = get_slot(vm, &walk.this, "current");
    node_of(vm, &value).unwrap_or(walk.root)
}

/** Move the current node and return its wrapper. */
fn settle(vm: &mut Vm, walk: &Walk, node: usize) -> Res<JsValue> {
    let value = wrap(vm, node)?;
    if let Some(id) = walk.this.object() {
        set_slot(vm, id, "current", value.clone());
    }
    Ok(value)
}

/** The node type number behind `whatToShow`: a tag from TAG_TYPES, else an element. */
fn node_type(vm: &mut Vm, node: usize) -> u32 {
    if node == DOCUMENT {
        return DOCUMENT_TYPE;
    }
    let tag = vm.page.tag(node);
    TAG_TYPES
        .iter()
        .find(|(known, _)| *known == tag)
        .map_or(ELEMENT_TYPE, |(_, kind)| *kind)
}

/** Set or clear one of the traversal's private flags on its object. */
fn set_flag(vm: &mut Vm, walk: &Walk, key: &str, value: bool) {
    if let Some(id) = walk.this.object() {
        set_slot(vm, id, key, JsValue::Boolean(value));
    }
}

/** Mark the traversal as running its filter, failing if it already is: a filter may not call
back into the traversal that is calling it (the spec's InvalidStateError). */
fn enter_filter(vm: &mut Vm, walk: &Walk) -> Res<()> {
    if get_slot(vm, &walk.this, ACTIVE_FLAG).truthy() {
        return Err(vm.dom_exception(INVALID_STATE, RECURSIVE_FILTER));
    }
    set_flag(vm, walk, ACTIVE_FLAG, true);
    Ok(())
}

/** Call the page's filter (a function or an object with acceptNode) for one node. */
fn call_filter(vm: &mut Vm, walk: &Walk, node: usize) -> Res<JsValue> {
    let value = wrap(vm, node)?;
    if vm.is_callable(&walk.filter) {
        return vm.call(walk.filter.clone(), JsValue::Undefined, vec![value]);
    }
    let method = vm.get(&walk.filter, &Key::from("acceptNode"))?;
    if !vm.is_callable(&method) {
        return Err(vm.type_error("NodeFilter has no acceptNode method"));
    }
    vm.call(method, walk.filter.clone(), vec![value])
}

/** Filter a node: whatToShow first, then the page's filter. The active flag is cleared even when
the filter throws, so a later call is not refused as recursive. */
fn filter(vm: &mut Vm, walk: &Walk, node: usize) -> Res<u8> {
    let bit = 1u32 << (node_type(vm, node) - 1);
    if walk.show & bit == 0 {
        return Ok(SKIP);
    }
    if walk.filter.nullish() {
        return Ok(ACCEPT);
    }
    enter_filter(vm, walk)?;
    let result = call_filter(vm, walk, node);
    set_flag(vm, walk, ACTIVE_FLAG, false);
    Ok(vm.to_number(&result?)? as u8)
}

/** Children for traversal, including the document's html element. */
fn children(vm: &mut Vm, node: usize) -> Vec<usize> {
    match node {
        DOCUMENT => vec![HTML],
        _ => vm.page.children(node),
    }
}

/** Parent for traversal; shadow roots have none. */
fn parent(vm: &mut Vm, node: usize) -> Option<usize> {
    match node {
        DOCUMENT => None,
        HTML => Some(DOCUMENT),
        _ if vm.page.has_shadows() && vm.page.is_shadow_root(node) => None,
        _ => vm.page.parent(node),
    }
}

/** A sibling at `offset` (1 or -1). */
fn sibling(vm: &mut Vm, node: usize, next: bool) -> Option<usize> {
    let parent = parent(vm, node)?;
    let siblings = children(vm, parent);
    let position = siblings.iter().position(|child| *child == node)?;
    if next {
        siblings.get(position + 1).copied()
    } else {
        position
            .checked_sub(1)
            .and_then(|index| siblings.get(index).copied())
    }
}

/** First or last child. */
fn edge_child(vm: &mut Vm, node: usize, first: bool) -> Option<usize> {
    let children = children(vm, node);
    if first {
        children.first().copied()
    } else {
        children.last().copied()
    }
}

/** Steps one traversal may take, so a cycle-free but huge tree still ends. */
const STEPS: usize = NODES + VM_DETACHED_NODES;

/** TreeWalker accessors. */
pub(in crate::v9) fn walker_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let walk = walk(vm, &call.this)?;
    match native_name(&call) {
        "root" => wrap(vm, walk.root),
        "whatToShow" => Ok(JsValue::Number(f64::from(walk.show))),
        "filter" => Ok(walk.filter),
        "referenceNode" => {
            let node = current(vm, &walk);
            wrap(vm, node)
        }
        "pointerBeforeReferenceNode" => Ok(get_slot(vm, &call.this, "before")),
        _ => {
            let node = current(vm, &walk);
            wrap(vm, node)
        }
    }
}

/** `currentNode = node`. */
pub(in crate::v9) fn walker_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let walk = walk(vm, &call.this)?;
    let Some(node) = node_of(vm, &call.arg(0)) else {
        return Err(
            vm.type_error("Failed to set 'currentNode': The provided value is not of type 'Node'.")
        );
    };
    settle(vm, &walk, node)?;
    Ok(JsValue::Undefined)
}

/** TreeWalker navigation methods. */
pub(in crate::v9) fn walker_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let walk = walk(vm, &call.this)?;
    let found = match native_name(&call) {
        "parentNode" => parent_node(vm, &walk)?,
        "firstChild" => traverse_children(vm, &walk, true)?,
        "lastChild" => traverse_children(vm, &walk, false)?,
        "nextSibling" => traverse_siblings(vm, &walk, true)?,
        "previousSibling" => traverse_siblings(vm, &walk, false)?,
        "nextNode" => next_node(vm, &walk)?,
        "previousNode" => previous_node(vm, &walk)?,
        _ => None,
    };
    match found {
        Some(node) => settle(vm, &walk, node),
        None => Ok(JsValue::Null),
    }
}

/** TreeWalker.parentNode. */
fn parent_node(vm: &mut Vm, walk: &Walk) -> Res<Option<usize>> {
    let mut node = Some(current(vm, walk));
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        let Some(id) = node.filter(|id| *id != walk.root) else {
            return Ok(None);
        };
        node = parent(vm, id);
        if let Some(candidate) = node
            && filter(vm, walk, candidate)? == ACCEPT
        {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/** TreeWalker.firstChild and lastChild. */
fn traverse_children(vm: &mut Vm, walk: &Walk, first: bool) -> Res<Option<usize>> {
    let start = current(vm, walk);
    let mut node = edge_child(vm, start, first);
    for _ in 0..STEPS {
        let Some(id) = node else {
            return Ok(None);
        };
        let result = filter(vm, walk, id)?;
        if result == ACCEPT {
            return Ok(Some(id));
        }
        if result == SKIP
            && let Some(child) = edge_child(vm, id, first)
        {
            node = Some(child);
            continue;
        }
        node = climb(vm, walk, id, start, first);
    }
    Ok(None)
}

/** The next candidate after a node's subtree, stopping at the root or the start node. */
fn climb(vm: &mut Vm, walk: &Walk, mut node: usize, start: usize, next: bool) -> Option<usize> {
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        if let Some(sibling) = sibling(vm, node, next) {
            return Some(sibling);
        }
        let parent = parent(vm, node)?;
        if parent == walk.root || parent == start {
            return None;
        }
        node = parent;
    }
    None
}

/** TreeWalker.nextSibling and previousSibling. */
fn traverse_siblings(vm: &mut Vm, walk: &Walk, next: bool) -> Res<Option<usize>> {
    let mut node = current(vm, walk);
    if node == walk.root {
        return Ok(None);
    }
    for _ in 0..STEPS {
        let mut candidate = sibling(vm, node, next);
        while let Some(id) = candidate {
            node = id;
            let result = filter(vm, walk, id)?;
            if result == ACCEPT {
                return Ok(Some(id));
            }
            candidate = edge_child(vm, id, next).filter(|_| result != REJECT);
            if candidate.is_none() {
                candidate = sibling(vm, id, next);
            }
        }
        let Some(up) = parent(vm, node).filter(|up| *up != walk.root) else {
            return Ok(None);
        };
        node = up;
        if filter(vm, walk, up)? == ACCEPT {
            return Ok(None);
        }
    }
    Ok(None)
}

/** TreeWalker.nextNode: preorder, skipping rejected subtrees. */
fn next_node(vm: &mut Vm, walk: &Walk) -> Res<Option<usize>> {
    let mut node = current(vm, walk);
    let mut result = ACCEPT;
    for _ in 0..STEPS {
        while result != REJECT
            && let Some(child) = edge_child(vm, node, true)
        {
            node = child;
            result = filter(vm, walk, node)?;
            if result == ACCEPT {
                return Ok(Some(node));
            }
        }
        let Some(following) = following_outside(vm, walk.root, node) else {
            return Ok(None);
        };
        node = following;
        result = filter(vm, walk, node)?;
        if result == ACCEPT {
            return Ok(Some(node));
        }
    }
    Ok(None)
}

/** The first node after `node`'s subtree within `root`. */
fn following_outside(vm: &mut Vm, root: usize, mut node: usize) -> Option<usize> {
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        if node == root {
            return None;
        }
        if let Some(sibling) = sibling(vm, node, true) {
            return Some(sibling);
        }
        node = parent(vm, node)?;
    }
    None
}

/** TreeWalker.previousNode. */
fn previous_node(vm: &mut Vm, walk: &Walk) -> Res<Option<usize>> {
    let mut node = current(vm, walk);
    for _ in 0..STEPS {
        if node == walk.root {
            return Ok(None);
        }
        let mut candidate = sibling(vm, node, false);
        while let Some(id) = candidate {
            node = id;
            let mut result = filter(vm, walk, node)?;
            while result != REJECT
                && let Some(last) = edge_child(vm, node, false)
            {
                node = last;
                result = filter(vm, walk, node)?;
            }
            if result == ACCEPT {
                return Ok(Some(node));
            }
            candidate = sibling(vm, node, false);
        }
        let Some(up) = parent(vm, node).filter(|_| node != walk.root) else {
            return Ok(None);
        };
        node = up;
        if filter(vm, walk, node)? == ACCEPT {
            return Ok(Some(node));
        }
    }
    Ok(None)
}

/** NodeIterator.nextNode and previousNode: a flat preorder over the root's subtree. */
pub(in crate::v9) fn iterator_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let walk = walk(vm, &call.this)?;
    let next = match native_name(&call) {
        "nextNode" => true,
        "previousNode" => false,
        _ => return Ok(JsValue::Undefined),
    };
    let mut node = current(vm, &walk);
    let mut before = get_slot(vm, &walk.this, "before").truthy();
    for _ in 0..STEPS {
        if next == before {
            before = !before;
        } else {
            let step = if next {
                preorder_next(vm, walk.root, node)
            } else {
                preorder_previous(vm, walk.root, node)
            };
            let Some(step) = step else {
                return Ok(JsValue::Null);
            };
            node = step;
        }
        if filter(vm, &walk, node)? == ACCEPT {
            if let Some(id) = walk.this.object() {
                set_slot(vm, id, "before", JsValue::Boolean(before));
            }
            return settle(vm, &walk, node);
        }
    }
    Ok(JsValue::Null)
}

/** The next node in preorder within `root`. */
fn preorder_next(vm: &mut Vm, root: usize, node: usize) -> Option<usize> {
    edge_child(vm, node, true).or_else(|| following_outside(vm, root, node))
}

/** The previous node in preorder within `root`. */
fn preorder_previous(vm: &mut Vm, root: usize, node: usize) -> Option<usize> {
    if node == root {
        return None;
    }
    let Some(mut previous) = sibling(vm, node, false) else {
        return parent(vm, node);
    };
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        match edge_child(vm, previous, false) {
            Some(last) => previous = last,
            None => break,
        }
    }
    Some(previous)
}

/** NodeIterator.detach does nothing, as in current browsers. */
pub(in crate::v9) fn detach(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Undefined)
}
