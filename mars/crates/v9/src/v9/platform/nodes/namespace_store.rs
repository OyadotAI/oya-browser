use super::super::Page;
/**
 * Namespace records of the node model: the namespace, prefix and local name of script-created
 * elements, the namespace of attributes set through the namespace-aware methods, and the
 * "locate a namespace" walk that lookupNamespaceURI and isDefaultNamespace run. Every map is
 * keyed by node first, then by name, so each read or write costs O(log n) in the records
 * rather than a scan of the document. The maps are nested rather than tuple-keyed because page
 * state is written as JSON, which only takes string map keys.
 */
use super::super::namespaces::{AttrNs, ElementNs, QName, implied_namespace, non_empty};
use crate::constants::DOCUMENT_DEPTH;
use crate::protocol::EngineResult;

/** The prefix a declaration attribute carries in front of the bound name. */
const XMLNS_ATTRIBUTE_PREFIX: &str = "xmlns:";
/** The declaration attribute that binds the default namespace. */
const XMLNS_ATTRIBUTE: &str = "xmlns";

impl Page<'_> {
    /** The namespace an element is in: a script-created element's own, otherwise the one its tag
    implies. Non-elements have none. */
    pub fn element_namespace(&mut self, node: usize) -> Option<String> {
        if let Some(record) = self.state.element_ns.get(&node) {
            return record.namespace.clone();
        }
        let tag = self.tag(node);
        if tag.is_empty() || tag.starts_with('#') {
            return None;
        }
        Some(implied_namespace(&tag).to_string())
    }

    /** The prefix an element carries. Only script-created elements can have one. */
    pub fn element_prefix(&self, node: usize) -> Option<String> {
        self.state
            .element_ns
            .get(&node)
            .and_then(|record| record.prefix.clone())
    }

    /** The local name `localName` reports: the record's when there is one, else the tag. */
    pub fn element_local_name(&mut self, node: usize) -> String {
        match self.state.element_ns.get(&node) {
            Some(record) => record.local.clone(),
            None => self.tag(node),
        }
    }

    /** Remember the namespace a script-created element was made with, keyed by its node. */
    pub fn set_element_namespace(&mut self, node: usize, record: ElementNs) {
        self.state.element_ns.insert(node, record);
    }

    /** Give a copy the namespace records of the node it was cloned from. */
    pub fn copy_namespaces(&mut self, from: usize, to: usize) {
        if let Some(record) = self.state.element_ns.get(&from).cloned() {
            self.state.element_ns.insert(to, record);
        }
        if let Some(attributes) = self.state.attr_ns.get(&from).cloned() {
            for (name, record) in attributes {
                self.remember_attribute_namespace(to, &name, record);
            }
        }
    }

    /** Store an attribute set through a namespace-aware method. The write keeps the name's case
    and goes through the same change log as setAttribute, so the app engine sees it. */
    pub fn set_namespaced_attribute(
        &mut self,
        node: usize,
        namespace: Option<&str>,
        qname: &QName,
        qualified: &str,
        value: &str,
    ) -> EngineResult<()> {
        let target = namespace
            .and_then(|ns| self.namespaced_attribute_name(node, Some(ns), &qname.local))
            .unwrap_or_else(|| qualified.to_string());
        self.forget_attribute_namespace(node, &target);
        self.set_named_attribute(node, &target, Some(value))?;
        if let Some(namespace) = namespace {
            let record = AttrNs {
                namespace: namespace.to_string(),
                prefix: qname.prefix.clone(),
                local: qname.local.clone(),
            };
            self.remember_attribute_namespace(node, &target, record);
        }
        Ok(())
    }

    /** The stored name a namespace-aware read names: the indexed attribute for a non-null
    namespace, or the attribute of that exact name when it is in the null namespace. */
    pub fn namespaced_attribute_name(
        &self,
        node: usize,
        namespace: Option<&str>,
        local: &str,
    ) -> Option<String> {
        match namespace {
            Some(namespace) => self
                .state
                .attr_index
                .get(&node)
                .and_then(|by_namespace| by_namespace.get(namespace))
                .and_then(|by_local| by_local.get(local))
                .cloned(),
            None if self.attribute_has_namespace(node, local) => None,
            None => Some(local.to_string()),
        }
    }

    /** Whether an attribute was stored under a non-null namespace. */
    fn attribute_has_namespace(&self, node: usize, name: &str) -> bool {
        self.state
            .attr_ns
            .get(&node)
            .is_some_and(|attributes| attributes.contains_key(name))
    }

    /** Drop the namespace record of an attribute that was removed or replaced. */
    pub fn forget_attribute_namespace(&mut self, node: usize, name: &str) {
        let Some(record) = self
            .state
            .attr_ns
            .get_mut(&node)
            .and_then(|attributes| attributes.remove(name))
        else {
            return;
        };
        if let Some(by_namespace) = self.state.attr_index.get_mut(&node)
            && let Some(by_local) = by_namespace.get_mut(&record.namespace)
        {
            by_local.remove(&record.local);
        }
    }

    /** Record an attribute's namespace and index it under that namespace and local name. */
    fn remember_attribute_namespace(&mut self, node: usize, name: &str, record: AttrNs) {
        self.state
            .attr_index
            .entry(node)
            .or_default()
            .entry(record.namespace.clone())
            .or_default()
            .insert(record.local.clone(), name.to_string());
        self.state
            .attr_ns
            .entry(node)
            .or_default()
            .insert(name.to_string(), record);
    }

    /** The namespace the DOM's "locate a namespace" finds for a prefix (None for the default),
    walking from an element up through its ancestors. The nearest declaration wins. */
    pub fn lookup_namespace(&mut self, node: usize, prefix: Option<&str>) -> Option<String> {
        let mut current = Some(node);
        for _ in 0..=DOCUMENT_DEPTH {
            let element = current?;
            if !self.is_element(element) {
                return None;
            }
            if let Some(answer) = self.declared_namespace(element, prefix) {
                return answer;
            }
            current = self.parent(element);
        }
        None
    }

    /** What one element answers for a prefix: its own namespace when it carries that prefix, or
    its declaration attribute. None means it says nothing and the walk goes on. */
    fn declared_namespace(
        &mut self,
        element: usize,
        prefix: Option<&str>,
    ) -> Option<Option<String>> {
        let namespace = self.element_namespace(element);
        if namespace.is_some() && self.element_prefix(element).as_deref() == prefix {
            return Some(namespace);
        }
        let attribute = match prefix {
            Some(prefix) => format!("{XMLNS_ATTRIBUTE_PREFIX}{prefix}"),
            None => XMLNS_ATTRIBUTE.to_string(),
        };
        let declared = self.attribute_value(element, &attribute)?;
        Some(non_empty(&declared))
    }
}
