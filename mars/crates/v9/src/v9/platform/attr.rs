/**
 * The model behind Attr objects: one record per attribute object, the identity map that gives each
 * (element, name) one Attr, and the rules for attaching, detaching and reading one. An attached
 * Attr reads its element's value; removing the attribute detaches it and keeps the last value, as
 * the DOM standard does. Every lookup is a nested BTreeMap read, O(log n), and nothing here touches
 * the JS heap: attr_dom.rs wraps these records as objects.
 */
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::namespaces::AttrNs;
use super::Page;

/** One Attr object's record: where it is attached, its name and namespace, and its detached value. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::v9) struct AttrNode {
    /** The element the attribute belongs to, or None once the Attr is detached. */
    pub owner: Option<usize>,
    /** The name the element stores the attribute under, which the Attr's `name` reports. */
    pub name: String,
    /** The attribute's namespace, or None for the null namespace. */
    pub namespace: Option<String>,
    /** The attribute's prefix, when it has one. */
    pub prefix: Option<String>,
    /** The attribute's local name, which `localName` reports. */
    pub local: String,
    /** The value kept while detached; an attached Attr reads its element's value instead. */
    pub value: String,
}

impl Page<'_> {
    /** The Attr for an attribute an element holds under `name`, made on first use and reused after,
    so `attributes[i] === getAttributeNode(name)` holds. */
    pub fn attr_identity(&mut self, node: usize, name: &str) -> u64 {
        if let Some(id) = self.attached_attr(node, name) {
            return id;
        }
        let record = self.attached_record(node, name);
        let id = self.new_attr(record);
        self.attach_attr(id, node, name);
        id
    }

    /** The id of the Attr currently attached under `name` on `node`, if one was made. */
    fn attached_attr(&self, node: usize, name: &str) -> Option<u64> {
        self.state.attr_of.get(&node)?.get(name).copied()
    }

    /** A record for an attribute the element holds: its namespace comes from the namespace records,
    and a null-namespace attribute is its own local name with no prefix. */
    fn attached_record(&self, node: usize, name: &str) -> AttrNode {
        let namespaced: Option<AttrNs> = self
            .state
            .attr_ns
            .get(&node)
            .and_then(|by_name| by_name.get(name))
            .cloned();
        let (namespace, prefix, local) = match namespaced {
            Some(ns) => (Some(ns.namespace), ns.prefix, ns.local),
            None => (None, None, name.to_string()),
        };
        AttrNode {
            owner: Some(node),
            name: name.to_string(),
            namespace,
            prefix,
            local,
            value: String::new(),
        }
    }

    /** Store a new record and return its id. Ids come from a counter that only grows, so an id is
    never handed to two Attr objects. */
    pub fn new_attr(&mut self, record: AttrNode) -> u64 {
        let id = self.state.next_attr;
        self.state.next_attr += 1;
        self.state.attr_nodes.insert(id, record);
        id
    }

    /** The record of an Attr, or None for an id nothing made. */
    pub fn attr_record(&self, id: u64) -> Option<&AttrNode> {
        self.state.attr_nodes.get(&id)
    }

    /** The value an Attr holds now: its element's while attached, its kept value once detached. */
    pub fn attr_value_of(&mut self, id: u64) -> String {
        let Some(record) = self.state.attr_nodes.get(&id).cloned() else {
            return String::new();
        };
        match record.owner {
            Some(node) => self.attribute_value(node, &record.name).unwrap_or(record.value),
            None => record.value,
        }
    }

    /** Make an Attr the attached one for `name` on `node`. The caller has already written the
    element's attribute, so the Attr now reads through to it. */
    pub fn attach_attr(&mut self, id: u64, node: usize, name: &str) {
        if let Some(record) = self.state.attr_nodes.get_mut(&id) {
            record.owner = Some(node);
        }
        self.state
            .attr_of
            .entry(node)
            .or_default()
            .insert(name.to_string(), id);
    }

    /** Detach the Attr attached under `name` on `node`, keeping `last` as its value. A removal calls
    this, so a held Attr keeps the value it had. A name with no attached Attr is a no-op. */
    pub fn detach_attr(&mut self, node: usize, name: &str, last: Option<String>) {
        let Some(id) = self
            .state
            .attr_of
            .get_mut(&node)
            .and_then(|by_name| by_name.remove(name))
        else {
            return;
        };
        if self.state.attr_of.get(&node).is_some_and(BTreeMap::is_empty) {
            self.state.attr_of.remove(&node);
        }
        if let Some(record) = self.state.attr_nodes.get_mut(&id) {
            record.owner = None;
            record.value = last.unwrap_or_default();
        }
    }
}
