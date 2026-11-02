/**
 * Web Components for page scripts: custom elements (define, upgrade and lifecycle callbacks),
 * shadow roots with slots, template content fragments, MutationObserver records, and the
 * TreeWalker/NodeIterator traversals component libraries build on.
 *
 * A shadow root is a `#shadow-root` child node of its host in whichever store holds the host
 * (projection or detached nodes), so it moves, serializes and re-identifies with the host. Script
 * traversal never sees it as a child; Eye flattens it for layout, paint and agent inspection.
 * DOM natives queue what changed during one call, and `flush` runs custom element reactions and
 * records mutations before the call returns, as the [CEReactions] binding steps do.
 */
mod candidates;
mod install;
mod observer;
mod reactions;
mod registry;
mod shadow;
mod template;
mod traversal;

pub(in crate::v9) use install::{ids, install};
pub(in crate::v9) use reactions::reacting;
pub(in crate::v9) use reactions::{adopt, flush};
pub(in crate::v9) use registry::create_element;
pub(in crate::v9) use shadow::{attach_shadow, containing_root};
pub(in crate::v9) use shadow::{composed_parent, retarget};
pub(in crate::v9) use template::clone_content;

use crate::v9::value::{JsValue, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/** The tag of a shadow root node; no HTML tag can be spelled with `#`, so markup cannot forge one. */
pub(in crate::v9) const SHADOW_ROOT: &str = "#shadow-root";
/** The tag of a template content or script-created fragment. */
pub(in crate::v9) const FRAGMENT: &str = "#document-fragment";
/** Custom element definitions one page may register. */
const DEFINITIONS: usize = 4096;
/** Mutation observers that may watch nodes at once. */
const OBSERVATIONS: usize = 4096;
/** Records one observer holds before older ones are dropped. */
const OBSERVER_RECORDS: usize = 4096;
/** Reactions one flush runs before the rest wait for the next DOM call. */
const REACTIONS: usize = 65_536;

/** One `customElements.define` registration. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Definition {
    /** The custom element name. */
    pub name: String,
    /** The built-in tag a customized built-in extends, if any. */
    pub extends: Option<String>,
    /** The class or function registered. */
    pub constructor: JsValue,
    /** Lifecycle callbacks read once from the prototype at definition time. */
    pub connected: JsValue,
    pub disconnected: JsValue,
    pub attribute_changed: JsValue,
    /** Lowercase names from `observedAttributes`. */
    pub observed: Vec<String>,
}

/** One `MutationObserver.observe` registration. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Observation {
    pub observer: ObjectId,
    pub target: usize,
    pub child_list: bool,
    pub attributes: bool,
    pub character_data: bool,
    pub subtree: bool,
    pub attribute_old: bool,
    pub character_old: bool,
    /** Attribute names the observer filters on; None observes every attribute. */
    pub filter: Option<Vec<String>>,
}

/** Component state that survives between requests, inside PageState. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct State {
    pub definitions: Vec<Definition>,
    /** whenDefined promises still waiting, by name. */
    pub waiting: BTreeMap<String, ObjectId>,
    /** Upgraded custom elements and the definition each runs. */
    pub custom: BTreeMap<usize, usize>,
    /** Connected elements not yet upgraded, by the name a definition would match; see candidates.rs. */
    pub candidates: BTreeMap<String, BTreeSet<usize>>,
    /** Whether `candidates` has been built from the document (by the first definition). */
    pub candidates_built: bool,
    /** Custom elements whose connectedCallback ran and whose disconnectedCallback has not. */
    pub connected: BTreeSet<usize>,
    /** Elements whose constructor threw; they stay failed, as in browsers. */
    pub failed: BTreeSet<usize>,
    /** Template elements and their content fragments. */
    pub contents: BTreeMap<usize, usize>,
    /** Whether any detached shadow root exists (projected ones are seen in the projection). */
    pub shadows: bool,
    pub observations: Vec<Observation>,
    /** Records waiting for each observer's next delivery. */
    pub records: BTreeMap<ObjectId, Vec<JsValue>>,
    /** Whether a delivery microtask is queued. */
    pub delivery: bool,
    /** Elements being upgraded, innermost last; their constructors' super() returns them. */
    pub constructing: Vec<usize>,
    /** Slots whose assigned nodes changed and that owe a slotchange event. */
    pub slot_changes: Vec<usize>,
}

impl State {
    /** Values this state keeps alive between requests. */
    pub fn roots(&self, out: &mut Vec<JsValue>, wrappers: &BTreeMap<usize, ObjectId>) {
        for definition in &self.definitions {
            out.extend([
                definition.constructor.clone(),
                definition.connected.clone(),
                definition.disconnected.clone(),
                definition.attribute_changed.clone(),
            ]);
        }
        out.extend(self.waiting.values().map(|id| JsValue::Object(*id)));
        for (observer, records) in &self.records {
            out.push(JsValue::Object(*observer));
            out.extend(records.iter().cloned());
        }
        // Construction and queued slot changes own their targets until those operations finish.
        // Upgrade metadata alone cannot keep a discarded element and its closures alive.
        out.extend(
            self.constructing
                .iter()
                .chain(&self.slot_changes)
                .filter_map(|node| wrappers.get(node))
                .map(|id| JsValue::Object(*id)),
        );
    }
}

/** One change a DOM call made, queued for reactions and observers. */
#[derive(Clone, Debug)]
pub(in crate::v9) enum Mutation {
    /** Children were added to or removed from `parent`. */
    Children {
        parent: usize,
        added: Vec<usize>,
        removed: Vec<usize>,
        previous: Option<usize>,
        next: Option<usize>,
    },
    /** An attribute was set or removed; `old` is its previous value. */
    Attribute {
        node: usize,
        name: String,
        old: Option<String>,
    },
    /** A text or comment node's data changed. */
    Text { node: usize, old: String },
    /** Nodes were created by cloning or parsing; defined custom elements among them upgrade. */
    Created { root: usize },
}

/** A private key: scripts can neither name nor enumerate it, so internal state stays internal. */
fn slot_key(name: &str) -> crate::v9::vm::heap::Key {
    crate::v9::vm::heap::Key::private(name)
}

/** Store an internal value on an object. */
fn set_slot(vm: &mut crate::v9::vm::Vm, object: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(
        object,
        slot_key(name),
        crate::v9::vm::heap::Prop {
            slot: crate::v9::vm::heap::Slot::Data(value),
            attrs: crate::v9::vm::heap::HIDDEN_ATTRS,
        },
    );
}

/** Read an internal value from an object (undefined when absent). */
fn get_slot(vm: &crate::v9::vm::Vm, object: &JsValue, name: &str) -> JsValue {
    let Some(id) = object.object() else {
        return JsValue::Undefined;
    };
    match vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&slot_key(name)))
    {
        Some(crate::v9::vm::heap::Prop {
            slot: crate::v9::vm::heap::Slot::Data(value),
            ..
        }) => value.clone(),
        _ => JsValue::Undefined,
    }
}

/** A boolean member of an options dictionary (false when the argument is not an object). */
pub(in crate::v9) fn option(
    vm: &mut crate::v9::vm::Vm,
    options: &JsValue,
    name: &str,
) -> crate::v9::vm::Res<bool> {
    if !matches!(options, JsValue::Object(_)) {
        return Ok(false);
    }
    Ok(vm
        .get(options, &crate::v9::vm::heap::Key::from(name))?
        .truthy())
}

/** Remember whether a script-made event was constructed with `composed: true`. */
pub(in crate::v9) fn set_composed(vm: &mut crate::v9::vm::Vm, event: ObjectId, composed: bool) {
    if composed {
        set_slot(vm, event, "composed", JsValue::Boolean(true));
    }
}

/** Whether an event crosses shadow boundaries. */
pub(in crate::v9) fn composed(vm: &crate::v9::vm::Vm, event: ObjectId) -> bool {
    get_slot(vm, &JsValue::Object(event), "composed").truthy()
}

/** Remember the node an event was dispatched at, before any retargeting. */
pub(in crate::v9) fn set_original(vm: &mut crate::v9::vm::Vm, event: ObjectId, node: usize) {
    set_slot(vm, event, "origin", JsValue::Number(node as f64));
}

/** The node an event was dispatched at. */
pub(in crate::v9) fn original_target(vm: &crate::v9::vm::Vm, event: ObjectId) -> Option<usize> {
    match get_slot(vm, &JsValue::Object(event), "origin") {
        #[cfg(feature = "conformance")]
        JsValue::Smi(node) => Some(node as usize),
        JsValue::Number(node) => Some(node as usize),
        _ => None,
    }
}
