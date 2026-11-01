/**
 * One node model over three stores: the app's projection (connected body content), nodes scripts
 * created or detached (kept in page state), and the virtual html, head and body the projection
 * does not carry. Mutations of connected nodes go through the projection and the change log, so
 * the app's document engine reproduces them exactly.
 */
use super::{Detached, DomLevel, Page};
use crate::constants::{
    FRAGMENT_BYTES, FRAGMENT_TOTAL_BYTES, SCRIPT_CHANGE_BYTES, STATEMENTS, VM_DETACHED_NODES,
};
use crate::document::{DomChange, InsertPosition, ScriptChange, ScriptNode};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::vm::heap::FastMap;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
mod leading_text;
mod namespace_store;

/** The source of tree generations. Every page draws from this one counter, so a generation a live
collection cached can never equal the current generation of another page, even after a page is replaced. */
static GENERATIONS: AtomicU64 = AtomicU64::new(1);

/** A fresh tree generation. Each change to the tree takes one, and a cached read is current only while
its generation is still the page's. */
pub(in crate::v9) fn next_generation() -> u64 {
    GENERATIONS.fetch_add(1, Ordering::Relaxed)
}

/** Text and attribute bytes one change adds to the request's change budget. */
fn change_bytes(change: &ScriptChange) -> usize {
    match change {
        ScriptChange::Dom {
            change: DomChange::Text { text, .. },
        }
        | ScriptChange::NodeText { text, .. } => text.len(),
        ScriptChange::Dom {
            change: DomChange::InsertHtml { html, .. },
        } => html.len(),
        ScriptChange::Attribute { value, .. } | ScriptChange::RootAttribute { value, .. } => {
            value.as_ref().map_or(0, String::len)
        }
        ScriptChange::Value { value, .. } => value.len(),
        ScriptChange::Selection { selected, .. } => selected.len() * std::mem::size_of::<usize>(),
        _ => 0,
    }
}

/** What a blank script-created text node crosses to the document parser as: a zero-width
space, which the parser keeps where it drops a blank run; the node's own text replaces it. */
const BLANK_TEXT_PLACEHOLDER: &str = "\u{200B}";
/** First identity outside the projection's range. */
pub(in crate::v9) const VIRTUAL_BASE: usize = 1 << 31;
/** The document element. */
pub(in crate::v9) const HTML: usize = VIRTUAL_BASE;
/** The head element. */
pub(in crate::v9) const HEAD: usize = VIRTUAL_BASE + 1;
/** The body element; projected roots are its children. */
pub(in crate::v9) const BODY: usize = VIRTUAL_BASE + 2;
/** First identity of script-created nodes. */
pub(in crate::v9) const DETACHED_BASE: usize = VIRTUAL_BASE + 16;

/** Children and parents of projected nodes, rebuilt after each mutation. */
#[derive(Default)]
pub(in crate::v9) struct Tree {
    children: FastMap<usize, Vec<usize>>,
    parents: FastMap<usize, usize>,
    index: FastMap<usize, usize>,
    /** Whether the projection holds any shadow root. */
    shadow: bool,
    /** Each parent's element children in order, built on the first element read and dropped when
    that parent's child list changes, so `children[i]` costs O(1) after the first read. */
    elements: FastMap<usize, Vec<usize>>,
}

/** The list a parent with no children reads as. */
const EMPTY: &[usize] = &[];

/** A read-only view of any node. */
#[derive(Clone, Debug)]
pub(in crate::v9) struct NodeView {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
    pub text: Option<String>,
    pub value: Option<String>,
}

/// Elements whose `value` IDL attribute reflects the `value` content attribute.
const REFLECTED_VALUE: [&str; 4] = ["option", "button", "data", "param"];

impl Page<'_> {
    /** The projected-node index, built on demand. */
    fn tree(&mut self) -> &Tree {
        if self.tree.is_none() {
            let mut tree = Tree::default();
            if let Some(document) = &self.document {
                for (position, node) in document.nodes.iter().enumerate() {
                    let parent = node.parent.unwrap_or(BODY);
                    tree.children.entry(parent).or_default().push(node.local_id);
                    tree.parents.insert(node.local_id, parent);
                    tree.index.insert(node.local_id, position);
                    tree.shadow |= node.tag == super::components::SHADOW_ROOT;
                }
            }
            self.tree = Some(tree);
        }
        self.tree.as_ref().expect("tree")
    }
    /** Forget the index after the projection changed. */
    pub fn invalidate(&mut self) {
        self.tree = None;
        self.touch();
    }
    /** Mark the tree changed. Every live collection rescans on its next read; the mark costs O(1), so
    a mutation pays nothing for the collections it may affect. */
    pub fn touch(&mut self) {
        self.generation = next_generation();
    }
    /** Bring the index up to date after `count` nodes were spliced in at `at`, instead of
    rebuilding it: a page that stamps thousands of elements (YouTube) would otherwise rebuild
    the whole index on each insert. Nodes after the splice shift by `count`; only the one
    parent whose child list gained the fragment's roots is reordered. */
    fn tree_inserted(&mut self, at: usize, count: usize) {
        self.touch();
        let (Some(document), Some(tree)) = (&self.document, &mut self.tree) else {
            return;
        };
        // ponytail: O(n - at) per insert; appends land at the end, so this stays small.
        for (position, node) in document.nodes.iter().enumerate().skip(at) {
            tree.index.insert(node.local_id, position);
        }
        let range = at..at + count;
        let mut roots = Vec::new();
        let mut roots_parent = None;
        for node in &document.nodes[range.clone()] {
            let parent = node.parent.unwrap_or(BODY);
            tree.elements.remove(&parent);
            tree.parents.insert(node.local_id, parent);
            tree.shadow |= node.tag == super::components::SHADOW_ROOT;
            if tree.index.get(&parent).is_some_and(|p| range.contains(p)) {
                tree.children.entry(parent).or_default().push(node.local_id);
            } else {
                roots_parent = Some(parent);
                roots.push(node.local_id);
            }
        }
        let Some(parent) = roots_parent else {
            return;
        };
        tree.elements.remove(&parent);
        let existing = tree.children.remove(&parent).unwrap_or_default();
        let split = existing
            .iter()
            .position(|child| tree.index.get(child).is_some_and(|p| *p >= range.end))
            .unwrap_or(existing.len());
        let mut children = existing[..split].to_vec();
        children.extend(roots);
        children.extend_from_slice(&existing[split..]);
        tree.children.insert(parent, children);
    }
    /** The projected node, if connected. */
    pub fn projected(&mut self, id: usize) -> Option<&ScriptNode> {
        let position = *self.tree().index.get(&id)?;
        self.document.as_ref()?.nodes.get(position)
    }
    /** Whether a node exists. */
    pub fn exists(&mut self, id: usize) -> bool {
        matches!(id, HTML | HEAD | BODY)
            || self.state.detached.contains_key(&id)
            || self.projected(id).is_some()
    }
    /** Whether the node is in the projected document (not head or detached). */
    pub fn is_projected(&mut self, id: usize) -> bool {
        self.projected(id).is_some()
    }
    /** Whether the node is in the document at all (including virtual nodes and head). */
    pub fn connected(&mut self, id: usize) -> bool {
        let mut cursor = Some(id);
        let mut steps = 0;
        while let Some(current) = cursor {
            if matches!(current, HTML | HEAD | BODY) || self.is_projected(current) {
                return true;
            }
            cursor = self.parent(current);
            steps += 1;
            if steps > VM_DETACHED_NODES {
                return false;
            }
        }
        false
    }
    /** A view of a node. */
    pub fn view(&mut self, id: usize) -> Option<NodeView> {
        // Every read a page makes goes through here, so the engine-only popover marker never
        // reaches getAttribute, attribute lists, serialization or clones.
        let mut view = self.view_with_marker(id)?;
        view.attributes
            .remove(crate::document::POPOVER_OPEN_ATTRIBUTE);
        Some(view)
    }
    /** The node as stored, engine-only attributes included. */
    fn view_with_marker(&mut self, id: usize) -> Option<NodeView> {
        let text = self.node_text(id);
        let value = self.state.values.get(&id).cloned();
        match id {
            HTML | HEAD | BODY => {
                let tag = match id {
                    HTML => "html",
                    HEAD => "head",
                    _ => "body",
                };
                return Some(NodeView {
                    tag: tag.into(),
                    attributes: self.root_attributes(id),
                    text: None,
                    value: None,
                });
            }
            _ => {}
        }
        if let Some(node) = self.state.detached.get(&id) {
            return Some(NodeView {
                tag: node.tag.clone(),
                attributes: node.attributes.clone(),
                text: node.text.clone(),
                value,
            });
        }
        let node = self.projected(id)?.clone();
        Some(NodeView {
            tag: node.tag,
            attributes: node.attributes,
            text,
            value: value.or(node.value),
        })
    }
    /** Text a projected text node (or text-set element) holds. */
    fn node_text(&self, id: usize) -> Option<String> {
        self.state
            .texts
            .get(&id)
            .or_else(|| self.environment.node_text.get(&id))
            .cloned()
    }
    /** The attributes of the virtual html, head or body element: the document's own until a
    script writes one, then the written set (seeded from the document's at the first write). */
    fn root_attributes(&self, id: usize) -> BTreeMap<String, String> {
        if let Some(written) = self.state.virtual_attributes.get(&id) {
            return written.clone();
        }
        match id {
            HTML => self.environment.html_attributes.clone(),
            BODY => self.environment.body_attributes.clone(),
            _ => BTreeMap::new(),
        }
    }
    /** The parent of a node. */
    pub fn parent(&mut self, id: usize) -> Option<usize> {
        match id {
            HTML => None,
            HEAD | BODY => Some(HTML),
            _ => {
                if let Some(node) = self.state.detached.get(&id) {
                    return node.parent;
                }
                self.tree().parents.get(&id).copied()
            }
        }
    }
    /** The children scripts see, in order: a host's shadow root is not one of them. */
    pub fn children(&mut self, id: usize) -> Vec<usize> {
        let children = self.all_children(id);
        if !self.has_shadows() {
            return children;
        }
        children
            .into_iter()
            .filter(|child| !self.is_shadow_root(*child))
            .collect()
    }
    /** The text of one attribute, read in place: a class write needs that one string, not a copy of
    every attribute the node has. */
    pub fn attribute_value(&mut self, id: usize, name: &str) -> Option<String> {
        self.with_element(id, |_, attributes| attributes.get(name).cloned())
            .flatten()
    }
    /** The child at `index` of a parent's live child list (element children only when `elements`).
    A direct parent answers from the tree's cached list, so a loop over `children[i]` does not
    rebuild the list per read; any other parent reads the children scripts see. */
    pub fn child_at(&mut self, parent: usize, elements: bool, index: usize) -> Option<usize> {
        if self.children_read_directly(parent) {
            return self.direct_children(parent, elements).get(index).copied();
        }
        self.child_list(parent, elements).get(index).copied()
    }
    /** How many children a parent's live child list holds, with the same reads as `child_at`. */
    pub fn child_count(&mut self, parent: usize, elements: bool) -> usize {
        if self.children_read_directly(parent) {
            return self.direct_children(parent, elements).len();
        }
        self.child_list(parent, elements).len()
    }
    /** Whether a parent's children are exactly its projected children: no head, detached, virtual
    or local children to merge in, and no shadow root to filter out. Only then is the tree's list
    the list scripts see. */
    fn children_read_directly(&mut self, parent: usize) -> bool {
        let local = self
            .state
            .local_children
            .get(&parent)
            .is_some_and(|locals| !locals.is_empty());
        !matches!(parent, HTML | HEAD)
            && !self.state.detached.contains_key(&parent)
            && !local
            && !self.has_shadows()
    }
    /** The projected children of a direct parent, or only its element children, from the tree. The
    element list is built once per tree state, O(children), and kept until the parent's children change. */
    fn direct_children(&mut self, parent: usize, elements: bool) -> &[usize] {
        if !elements {
            return self
                .tree()
                .children
                .get(&parent)
                .map_or(EMPTY, Vec::as_slice);
        }
        if !self.tree().elements.contains_key(&parent) {
            let all = self
                .tree()
                .children
                .get(&parent)
                .cloned()
                .unwrap_or_default();
            let kept: Vec<usize> = all
                .into_iter()
                .filter(|child| self.is_element(*child))
                .collect();
            if let Some(tree) = self.tree.as_mut() {
                tree.elements.insert(parent, kept);
            }
        }
        self.tree()
            .elements
            .get(&parent)
            .map_or(EMPTY, Vec::as_slice)
    }
    /** The children a parent's live list shows, by the general reads, for the parents the tree
    cannot answer alone. Costs O(children). */
    fn child_list(&mut self, parent: usize, elements: bool) -> Vec<usize> {
        let children = self.children(parent);
        if elements {
            children
                .into_iter()
                .filter(|child| self.is_element(*child))
                .collect()
        } else {
            children
        }
    }
    /** Whether any shadow root exists, so pages without components skip the shadow checks. */
    pub fn has_shadows(&mut self) -> bool {
        self.state.components.shadows || self.tree().shadow
    }
    /** Whether a node is a shadow root, without copying its attributes. */
    pub fn is_shadow_root(&mut self, id: usize) -> bool {
        if let Some(node) = self.state.detached.get(&id) {
            return node.tag == super::components::SHADOW_ROOT;
        }
        self.projected(id)
            .is_some_and(|node| node.tag == super::components::SHADOW_ROOT)
    }
    /** Every child of a node in order, shadow root included, for moving and serializing it. */
    pub fn all_children(&mut self, id: usize) -> Vec<usize> {
        match id {
            HTML => {
                let mut children = vec![HEAD, BODY];
                children.extend(self.state.html.iter().copied());
                children
            }
            _ => {
                if let Some(node) = self.state.detached.get(&id) {
                    return node.children.clone();
                }
                if id == HEAD {
                    return self.head_children();
                }
                let mut children = self.tree().children.get(&id).cloned().unwrap_or_default();
                for (child, before) in self.state.local_children.get(&id).into_iter().flatten() {
                    let at = before.and_then(|b| children.iter().position(|c| *c == b));
                    children.insert(at.unwrap_or(children.len()), *child);
                }
                children
            }
        }
    }
    /** Head children live as detached records parented to HEAD. */
    fn head_children(&self) -> Vec<usize> {
        self.state.head.clone()
    }
    /** Preorder descendants scripts see (excluding the node and any shadow tree). */
    pub fn descendants(&mut self, id: usize) -> Vec<usize> {
        self.walk(id, false)
    }
    /** Preorder descendants including shadow trees, for moving a subtree whole. */
    pub fn all_descendants(&mut self, id: usize) -> Vec<usize> {
        self.walk(id, true)
    }
    /** Preorder descendants, bounded by the node limits. */
    fn walk(&mut self, id: usize, shadow: bool) -> Vec<usize> {
        let children = |page: &mut Self, id| {
            if shadow {
                page.all_children(id)
            } else {
                page.children(id)
            }
        };
        let mut out = Vec::new();
        let mut stack: Vec<usize> = children(self, id).into_iter().rev().collect();
        while let Some(next) = stack.pop() {
            out.push(next);
            if out.len() > crate::constants::NODES + VM_DETACHED_NODES {
                break;
            }
            stack.extend(children(self, next).into_iter().rev());
        }
        out
    }
    /** Read a node's tag and attributes in place. `view` copies the node's attributes and
    text; a tag check over every node (a custom element definition upgrading the document,
    YouTube defines hundreds) made those copies 40% of the page's load. */
    pub fn with_element<R>(
        &mut self,
        id: usize,
        read: impl FnOnce(&str, &BTreeMap<String, String>) -> R,
    ) -> Option<R> {
        let root = match id {
            HTML => "html",
            HEAD => "head",
            BODY => "body",
            _ => "",
        };
        if !root.is_empty() {
            let attributes = match self.state.virtual_attributes.get(&id) {
                Some(written) => return Some(read(root, written)),
                None => self.root_attributes(id),
            };
            return Some(read(root, &attributes));
        }
        if let Some(node) = self.state.detached.get(&id) {
            return Some(read(&node.tag, &node.attributes));
        }
        let node = self.projected(id)?;
        Some(read(&node.tag, &node.attributes))
    }
    /** Tag of a node. */
    pub fn tag(&mut self, id: usize) -> String {
        self.with_element(id, |tag, _| tag.to_string())
            .unwrap_or_default()
    }
    /** Whether a node is an element (not text or comment). */
    pub fn is_element(&mut self, id: usize) -> bool {
        let tag = self.tag(id);
        !tag.is_empty() && !tag.starts_with('#')
    }
    /** A parsed page script's own source, while scripts have not given it children or text. */
    pub(in crate::v9) fn parsed_script_text(&mut self, id: usize) -> Option<String> {
        let index = self
            .state
            .parsed_scripts
            .as_ref()?
            .iter()
            .position(|node| *node == id)?;
        let text = self
            .inline_scripts
            .get(index)
            .filter(|text| !text.is_empty())?
            .clone();
        let untouched = self.all_children(id).is_empty()
            && self
                .view(id)
                .is_none_or(|view| view.text.is_none_or(|own| own.is_empty()));
        untouched.then_some(text)
    }
    /** All text inside a node, in document order. */
    pub fn text_content(&mut self, id: usize) -> String {
        if let Some(text) = self.parsed_script_text(id) {
            return text;
        }
        let view = self.view(id);
        if let Some(view) = &view
            && view.tag == "#text"
        {
            return view.text.clone().unwrap_or_default();
        }
        let mut out = String::new();
        if let Some(text) = view.and_then(|view| view.text) {
            out.push_str(&text);
        }
        for child in self.descendants(id) {
            if let Some(view) = self.view(child)
                && contributes_text(&view.tag)
            {
                out.push_str(view.text.as_deref().unwrap_or(""));
            }
        }
        out
    }
    /** A new detached node. */
    pub fn create(&mut self, tag: &str, text: Option<String>) -> EngineResult<usize> {
        if self.state.detached.len() >= VM_DETACHED_NODES {
            return Err(EngineError::limit("V9 detached nodes", VM_DETACHED_NODES));
        }
        let id = DETACHED_BASE + self.state.next_detached;
        self.state.next_detached += 1;
        self.state.detached.insert(
            id,
            Detached {
                tag: tag.into(),
                text,
                ..Detached::default()
            },
        );
        Ok(id)
    }
    /** Record a change in whichever log the app replays. */
    pub fn record(&mut self, change: ScriptChange) -> EngineResult<()> {
        self.touch();
        if self.environment.dom_level == DomLevel::Extended
            && let Some((node, name)) = attribute_key(&change)
            && let Some(slot) = self.attribute_slot(node, name)
        {
            return self.supersede(slot, change);
        }
        let bytes = change_bytes(&change);
        self.admit(bytes, 1)?;
        match (self.environment.dom_level, change) {
            (DomLevel::Legacy, ScriptChange::Dom { change }) => self.changes.push(change),
            (DomLevel::Extended, change) => {
                if let Some((node, name)) = attribute_key(&change) {
                    let position = self.script_changes.len();
                    self.attribute_slots
                        .insert((node, name.to_owned()), position);
                }
                self.script_changes.push(change);
            }
            (DomLevel::Legacy, _) => {
                return Err(EngineError::unsupported(
                    "This DOM change needs the extended change log, which the app has not enabled.",
                ));
            }
        }
        self.change_bytes += bytes;
        Ok(())
    }
    /**
     * Checks a change's cost against the limits before the document is touched. A write refused
     * after it changed the document leaves V9 and the app's engine disagreeing on structure.
     */
    fn admit(&self, bytes: usize, entries: usize) -> EngineResult<()> {
        if self.change_bytes + bytes > SCRIPT_CHANGE_BYTES {
            return Err(EngineError::limit(
                "V9 script change bytes",
                SCRIPT_CHANGE_BYTES,
            ));
        }
        if self.changes.len() + self.script_changes.len() + entries > STATEMENTS {
            return Err(EngineError::limit("V9 changes", STATEMENTS));
        }
        Ok(())
    }
    /**
     * Admits an attribute write before it lands. A write that would only replace the latest entry
     * for the same attribute adds no entry, and its bytes are counted as the new value's size.
     */
    fn admit_attribute(&self, node: usize, name: &str, value: Option<&str>) -> EngineResult<()> {
        let slot = self.attribute_slot(node, name);
        let previous = slot.map_or(0, |slot| change_bytes(&self.script_changes[slot]));
        self.admit(
            value.map_or(0, str::len).saturating_sub(previous),
            usize::from(slot.is_none()),
        )
    }
    /**
     * The log position of the latest write to this attribute. The hint is verified against the log,
     * so a log restored after a failed step can never send a write to the wrong entry.
     */
    fn attribute_slot(&self, node: usize, name: &str) -> Option<usize> {
        let slot = *self.attribute_slots.get(&(node, name.to_string()))?;
        self.script_changes
            .get(slot)
            .and_then(attribute_key)
            .filter(|(at, attribute)| *at == node && *attribute == name)
            .map(|_| slot)
    }
    /**
     * Replaces an earlier write to the same attribute in place. Attribute writes only set data, so
     * the final state is unchanged, and the log keeps one entry per attribute instead of one per call.
     */
    fn supersede(&mut self, slot: usize, change: ScriptChange) -> EngineResult<()> {
        let total = self
            .change_bytes
            .saturating_sub(change_bytes(&self.script_changes[slot]))
            + change_bytes(&change);
        if total > SCRIPT_CHANGE_BYTES {
            return Err(EngineError::limit(
                "V9 script change bytes",
                SCRIPT_CHANGE_BYTES,
            ));
        }
        self.change_bytes = total;
        self.script_changes[slot] = change;
        Ok(())
    }
    /** Whether only the original change log is available. */
    pub fn legacy_changes(&self) -> bool {
        self.environment.dom_level == DomLevel::Legacy
    }
    /**
     * In the legacy log a node this execution inserted and nothing else touched can leave again
     * by dropping its InsertHtml entry, so probes like Modernizr's work without a Remove change.
     */
    fn elide_insert(&mut self, id: usize) -> bool {
        let Some(position) = self.inserted.iter().position(|(_, root, _)| *root == id) else {
            return false;
        };
        let (index, _, subtree) = self.inserted[position].clone();
        let touched = self
            .changes
            .iter()
            .skip(index + 1)
            .any(|change| match change {
                DomChange::Text { html_id, .. } => self.document.as_ref().is_some_and(|document| {
                    document
                        .nodes
                        .iter()
                        .any(|node| subtree.contains(&node.local_id) && node.html_id == *html_id)
                }),
                DomChange::Move { node, target, .. } => {
                    subtree.contains(node) || target.is_some_and(|target| subtree.contains(&target))
                }
                DomChange::SetAttribute { node, .. } => subtree.contains(node),
                DomChange::InsertHtml { target, .. } => {
                    target.is_some_and(|target| subtree.contains(&target))
                }
            });
        if touched || index >= self.changes.len() {
            return false;
        }
        let records = self.capture(&subtree);
        self.changes.remove(index);
        for entry in &mut self.inserted {
            if entry.0 > index {
                entry.0 -= 1;
            }
        }
        self.inserted.remove(position);
        if let Some(document) = self.document.as_mut() {
            document
                .nodes
                .retain(|node| !subtree.contains(&node.local_id));
        }
        self.invalidate();
        self.store_detached(records);
        true
    }
    /** Detach a projected subtree into page state, keeping identities. */
    fn detach_projected(&mut self, id: usize) -> EngineResult<()> {
        let mut subtree = vec![id];
        subtree.extend(self.all_descendants(id));
        let records = self.capture(&subtree);
        let document = self.document.as_mut().ok_or_else(invalid)?;
        let change = document.remove(id)?;
        self.invalidate();
        self.record(change)?;
        self.store_detached(records);
        Ok(())
    }
    /** Captures each node's record as a detached node would hold it. Call this before the
    projection changes, since a node's view reads the projection. */
    fn capture(&mut self, nodes: &[usize]) -> Vec<(usize, Detached)> {
        nodes
            .iter()
            .map(|node| (*node, self.detached_record(*node)))
            .collect()
    }
    /** One node's record: its view, and its projected children for the parent links. */
    fn detached_record(&mut self, node: usize) -> Detached {
        if self.tag(node) == "option" {
            let selected = self.is_selected(node);
            self.state.selected_options.insert(node, selected);
        }
        let children = self.all_children(node);
        match self.view(node) {
            Some(view) => Detached {
                tag: view.tag,
                attributes: view.attributes,
                parent: None,
                children,
                text: view.text,
            },
            None => Detached {
                children,
                ..Detached::default()
            },
        }
    }
    /** Stores captured records as detached nodes, and points each captured child at its
    captured parent, so a removed subtree stays reachable from its root. */
    fn store_detached(&mut self, records: Vec<(usize, Detached)>) {
        let links: Vec<(usize, usize)> = records
            .iter()
            .flat_map(|(node, record)| record.children.iter().map(move |child| (*child, *node)))
            .collect();
        for (node, record) in records {
            self.state.detached.insert(node, record);
        }
        for (child, parent) in links {
            if let Some(record) = self.state.detached.get_mut(&child) {
                record.parent = Some(parent);
            }
        }
    }
    /** Remove a node from its parent. */
    pub fn remove(&mut self, id: usize) -> EngineResult<()> {
        super::media::removing(self, id);
        let owner = self.changed_option_owner(id);
        self.remove_node(id)?;
        if let Some(owner) = owner {
            self.normalize_selection(owner)?;
        }
        Ok(())
    }
    /** Detach storage and native projection before repairing the old SELECT's current state. */
    fn remove_node(&mut self, id: usize) -> EngineResult<()> {
        self.touch();
        if matches!(id, HTML | HEAD | BODY) {
            return Err(EngineError::unsupported(
                "Removing the html, head or body element is not supported.",
            ));
        }
        super::frame_directory::detach(self, id)?;
        if self.is_projected(id) {
            if self.legacy_changes() && self.elide_insert(id) {
                return Ok(());
            }
            return self.detach_projected(id);
        }
        let Some(parent) = self.state.detached.get(&id).and_then(|node| node.parent) else {
            self.state.head.retain(|child| *child != id);
            self.state.html.retain(|child| *child != id);
            return Ok(());
        };
        if let Some(record) = self.state.detached.get_mut(&parent) {
            record.children.retain(|child| *child != id);
        }
        if let Some(local) = self.state.local_children.get_mut(&parent) {
            local.retain(|(child, _)| *child != id);
        }
        if parent == HEAD {
            self.state.head.retain(|child| *child != id);
        }
        if parent == HTML {
            self.state.html.retain(|child| *child != id);
        }
        if let Some(record) = self.state.detached.get_mut(&id) {
            record.parent = None;
        }
        Ok(())
    }
    /** Insert `child` into `parent` before `reference` (or at the end). Returns new identities. */
    pub fn insert(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> EngineResult<Vec<(usize, usize)>> {
        let old_owner = self.changed_option_owner(child);
        let mapping = self.insert_node(parent, child, reference)?;
        let adopted = mapping
            .iter()
            .find(|(old, _)| *old == child)
            .map_or(child, |(_, new)| *new);
        super::media::inserted(self, adopted);
        let new_owner = self.changed_option_owner(adopted);
        if old_owner != new_owner
            && let Some(owner) = old_owner
        {
            self.normalize_selection(owner)?;
        }
        if let Some(owner) = new_owner {
            self.normalize_selection(owner)?;
        }
        Ok(mapping)
    }
    /** Whether inserting `child` into `parent` is a Text node joining a raw-text element. */
    fn appends_raw_text(&mut self, parent: usize, child: usize) -> bool {
        const RAW_TEXT: [&str; 5] = ["style", "script", "textarea", "title", "xmp"];
        self.tag(child) == "#text" && RAW_TEXT.contains(&self.tag(parent).as_str())
    }
    /** Structural insertion keeps its mapping so option state can follow adopted native identities. */
    fn insert_node(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> EngineResult<Vec<(usize, usize)>> {
        self.touch();
        if child == parent || self.ancestors(parent).contains(&child) {
            return Err(EngineError::invalid(
                "HierarchyRequestError: The new child element contains the parent.",
            ));
        }
        let reference = if reference == Some(child)
            && !(self.is_projected(child) && (parent == BODY || self.is_projected(parent)))
        {
            let siblings = self.children(parent);
            siblings
                .iter()
                .position(|node| *node == child)
                .and_then(|position| siblings.get(position + 1).copied())
        } else {
            reference
        };
        if self.parent(child).is_some()
            || self.state.head.contains(&child)
            || self.state.html.contains(&child)
        {
            if self.is_projected(child) && (parent == BODY || self.is_projected(parent)) {
                return self
                    .move_projected(parent, child, reference)
                    .map(|()| Vec::new());
            }
            self.remove(child)?;
        }
        if self.is_projected(parent) && self.appends_raw_text(parent, child) {
            // A style or script element's content is text, not markup: Modernizr appends
            // `createTextNode(css)` to a `<style>`, which the fragment parser cannot take.
            // ponytail: the text joins the parent's text; the Text node itself stays detached.
            let text = self.text_content(parent) + &self.text_content(child);
            return self.set_text(parent, &text).map(|()| Vec::new());
        }
        if parent == BODY || self.is_projected(parent) {
            return self.insert_projected(parent, child, reference);
        }
        if parent == HEAD {
            let position = reference
                .and_then(|r| self.state.head.iter().position(|c| *c == r))
                .unwrap_or(self.state.head.len());
            self.state.head.insert(position, child);
            if let Some(record) = self.state.detached.get_mut(&child) {
                record.parent = Some(HEAD);
            }
            return Ok(Vec::new());
        }
        if parent == HTML {
            // Extra html children (support probes, overlays) stay V9-local, like head content.
            let position = reference
                .and_then(|r| self.state.html.iter().position(|c| *c == r))
                .unwrap_or(self.state.html.len());
            self.state.html.insert(position, child);
            if let Some(record) = self.state.detached.get_mut(&child) {
                record.parent = Some(HTML);
            }
            return Ok(Vec::new());
        }
        let record = self.state.detached.get_mut(&parent).ok_or_else(invalid)?;
        let position = reference
            .and_then(|r| record.children.iter().position(|c| *c == r))
            .unwrap_or(record.children.len());
        record.children.insert(position, child);
        if let Some(record) = self.state.detached.get_mut(&child) {
            record.parent = Some(parent);
        }
        Ok(Vec::new())
    }
    /** Ancestors of a node, nearest first. */
    pub fn ancestors(&mut self, id: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut cursor = self.parent(id);
        while let Some(current) = cursor {
            out.push(current);
            if out.len() > crate::constants::DOCUMENT_DEPTH + VM_DETACHED_NODES {
                break;
            }
            cursor = self.parent(current);
        }
        out
    }
    /** The document anchor for inserting before `reference`: itself when the document holds it,
    else the next sibling the document holds (a V9-local child such as an undrawn SVG `<animate>`
    has no place in the projection), or None to append. */
    fn projected_anchor(&mut self, children: &[usize], reference: Option<usize>) -> Option<usize> {
        let start = children.iter().position(|node| Some(*node) == reference)?;
        children[start..]
            .iter()
            .copied()
            .find(|node| self.is_projected(*node))
    }
    /**
     * Where an insertion lands, as the change log expresses positions: before the first child is
     * "afterbegin" of the parent, and a node inserted before itself stays where it is.
     */
    fn placement(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> (Option<usize>, InsertPosition) {
        let children = self.children(parent);
        let first = children.iter().copied().find(|node| *node != child);
        let reference = if reference == Some(child) {
            let position = children.iter().position(|node| *node == child).unwrap_or(0);
            if position == 0 {
                return (
                    if parent == BODY { None } else { Some(parent) },
                    InsertPosition::AfterBegin,
                );
            }
            children.get(position + 1).copied()
        } else {
            reference
        };
        let reference = self.projected_anchor(&children, reference);
        match reference {
            Some(reference) if Some(reference) == first && children.first() == Some(&reference) => {
                (
                    if parent == BODY { None } else { Some(parent) },
                    InsertPosition::AfterBegin,
                )
            }
            Some(reference) => (Some(reference), InsertPosition::BeforeBegin),
            None if parent == BODY => (None, InsertPosition::BeforeEnd),
            None => (Some(parent), InsertPosition::BeforeEnd),
        }
    }
    /** Move a projected subtree. */
    fn move_projected(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> EngineResult<()> {
        let (target, position) = self.placement(parent, child, reference);
        let document = self.document.as_mut().ok_or_else(invalid)?;
        let change = document.relocate(child, target, position)?;
        self.invalidate();
        self.record(ScriptChange::Dom { change })
    }
    /** Keep `child` as a V9-local child of a projected `parent`, before `reference` when given. */
    fn insert_local(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> EngineResult<Vec<(usize, usize)>> {
        self.state
            .local_children
            .entry(parent)
            .or_default()
            .push((child, reference));
        if let Some(record) = self.state.detached.get_mut(&child) {
            record.parent = Some(parent);
        }
        self.touch();
        Ok(Vec::new())
    }
    /** Serialize a detached subtree, insert it through the fragment parser, and adopt the new ids. */
    fn insert_projected(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> EngineResult<Vec<(usize, usize)>> {
        self.materialize_leading_text(child)?;
        let mut old = vec![child];
        old.extend(self.all_descendants(child));
        // The document parser keeps no blank text run, but a script's blank text node can be
        // real: a binding placeholder it fills later (Polymer stamps `<span> </span>` and sets
        // the text node's data). Such a node crosses as a zero-width space and gets its own text
        // back once adopted: the inserted node itself, and any blank text that is its element's
        // only child. Blank runs between elements stay out, as in the parsed document.
        let blanks = self.blank_texts(child, &old);
        let html = self.serialize_with_placeholders(child, &blanks);
        if html.len() > FRAGMENT_BYTES && self.splittable(child) {
            return self.insert_in_parts(parent, child, reference);
        }
        let (target, position) = self.placement(parent, child, reference);
        let context = self.tag(parent);
        let nodes = self.parse_fragment(&context, &html)?;
        if nodes.is_empty() {
            // The document keeps none of it (SVG `<animate>` outside the drawing subset): it stays
            // a script-visible child that is never drawn, as a browser keeps undrawn elements.
            // Refusing stopped Apple's global header script.
            return self.insert_local(parent, child, reference);
        }
        let tags: Vec<String> = old.iter().map(|id| self.adoption_tag(*id)).collect();
        let Some(mapping) =
            adopted(&old, &tags, &nodes).filter(|m| m.first().map(|p| p.0) == Some(child))
        else {
            return Err(EngineError::unsupported(format!(
                "Inserting this element (<{}> into <{context}>) would change its structure when the document parses it.",
                tags.first().map_or("", String::as_str)
            )));
        };
        self.drop_unadopted(&old, &mapping);
        let first_id = nodes.first().map_or(self.next_id, |node| node.local_id);
        let texts: Vec<(usize, Option<String>)> = mapping
            .iter()
            .map(|(id, _)| id)
            .map(|id| {
                (
                    *id,
                    self.state.detached.get(id).and_then(|n| n.text.clone()),
                )
            })
            .collect();
        let count = nodes.len();
        let document = self.document.as_mut().ok_or_else(invalid)?;
        let at = document.insert(target, position, nodes)?;
        self.tree_inserted(at, count);
        self.record(ScriptChange::Dom {
            change: DomChange::InsertHtml {
                target,
                position,
                html,
                first_id,
            },
        })?;
        if self.legacy_changes() {
            let subtree: Vec<usize> = mapping.iter().map(|(_, new)| *new).collect();
            self.inserted
                .push((self.changes.len() - 1, first_id, subtree));
        }
        for ((old_id, new_id), (_, text)) in mapping.iter().zip(texts) {
            self.state.detached.remove(old_id);
            if let Some(text) = text {
                self.state.texts.insert(*new_id, text);
            }
            if self.state.dirty_options.remove(old_id) {
                self.state.dirty_options.insert(*new_id);
            }
            if let Some(selected) = self.state.selected_options.remove(old_id) {
                self.state.selected_options.insert(*new_id, selected);
            }
            if let Some(value) = self.state.values.remove(old_id) {
                self.state.values.insert(*new_id, value);
            }
        }
        for (old_id, text) in blanks {
            if let Some((_, new_id)) = mapping.iter().find(|(id, _)| *id == old_id) {
                self.set_text(*new_id, &text)?;
            }
        }
        let mut owners = std::collections::BTreeSet::new();
        for (_, id) in &mapping {
            if self.state.selected_options.contains_key(id)
                && let Some(owner) = self.option_owner(*id)
            {
                owners.insert(owner);
            }
        }
        for owner in owners {
            let selected: Vec<_> = self
                .select_options(owner)
                .into_iter()
                .filter(|id| self.is_selected(*id))
                .collect();
            if mapping.iter().any(|(_, adopted)| *adopted == owner) {
                self.select_exact(owner, &selected)?;
            } else {
                self.normalize_selection(owner)?;
            }
        }
        Ok(mapping)
    }
    /** Whether an inserted subtree too large for one fragment can go in as its element and then
    its children: an element whose children are its own (not a template's content, not raw text). */
    fn splittable(&mut self, id: usize) -> bool {
        let tag = self.tag(id);
        !tag.starts_with('#')
            && tag != "template"
            && !super::html::RAW.contains(&tag.as_str())
            && self
                .state
                .detached
                .get(&id)
                .is_some_and(|record| !record.children.is_empty())
    }
    /**
     * Insert a subtree whose markup passes FRAGMENT_BYTES in parts: the element alone, then each
     * child into it, recursing as needed. A browser has no fragment bound; this keeps each change
     * within the document engine's while the result is the same tree (a font-probing script
     * appends a div of hundreds of styled spans, LinkedIn's among them).
     */
    fn insert_in_parts(
        &mut self,
        parent: usize,
        child: usize,
        reference: Option<usize>,
    ) -> EngineResult<Vec<(usize, usize)>> {
        let children = self
            .state
            .detached
            .get_mut(&child)
            .map(|record| std::mem::take(&mut record.children))
            .unwrap_or_default();
        for id in &children {
            if let Some(record) = self.state.detached.get_mut(id) {
                record.parent = None;
            }
        }
        let mut mapping = self.insert_projected(parent, child, reference)?;
        let Some(&(_, shell)) = mapping.first() else {
            return Err(invalid());
        };
        for id in children {
            mapping.extend(self.insert(shell, id, None)?);
        }
        Ok(mapping)
    }
    /** The blank text nodes of an inserted subtree that cross as placeholders, with their text:
    the inserted node itself, and blank text that is its element's only child (outside raw-text
    elements, whose content is not markup). */
    fn blank_texts(&mut self, child: usize, subtree: &[usize]) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for id in subtree.iter().copied() {
            if self.tag(id) != "#text" {
                continue;
            }
            let text = self.text_content(id);
            if !text.trim().is_empty() {
                continue;
            }
            let sole = self.parent(id).is_some_and(|parent| {
                self.children(parent).len() == 1
                    && !super::html::RAW.contains(&self.tag(parent).as_str())
            });
            if id == child || sole {
                out.push((id, text));
            }
        }
        out
    }
    /** The subtree's markup with each of `blanks` written as BLANK_TEXT_PLACEHOLDER; the nodes'
    own text is left as it was. */
    fn serialize_with_placeholders(&mut self, child: usize, blanks: &[(usize, String)]) -> String {
        let swap = |nodes: &mut Self, text: &dyn Fn(&str) -> String| {
            for (id, original) in blanks {
                if let Some(record) = nodes.state.detached.get_mut(id) {
                    record.text = Some(text(original));
                }
            }
        };
        swap(self, &|_| BLANK_TEXT_PLACEHOLDER.into());
        let html = super::html::serialize_all(self, child);
        swap(self, &|original| original.into());
        html
    }
    /** Nodes the parser left out of an inserted subtree (a `<style>` the document keeps as a
    sheet, an ignorable text run) stay detached: each one whose parent was adopted becomes a
    detached root, so scripts holding it see a node outside the document. */
    fn drop_unadopted(&mut self, old: &[usize], mapping: &[(usize, usize)]) {
        let adopted: std::collections::BTreeSet<usize> =
            mapping.iter().map(|(id, _)| *id).collect();
        for id in old.iter().filter(|id| !adopted.contains(id)) {
            let parent = self.state.detached.get(id).and_then(|record| record.parent);
            if parent.is_some_and(|parent| adopted.contains(&parent)) {
                if let Some(record) = self.state.detached.get_mut(id) {
                    record.parent = None;
                }
                if let Some(record) = parent.and_then(|parent| self.state.detached.get_mut(&parent))
                {
                    record.children.retain(|child| child != id);
                }
            }
        }
    }
    /** Parse a fragment through the host parser with fresh identities. */
    pub fn parse_fragment(&mut self, context: &str, html: &str) -> EngineResult<Vec<ScriptNode>> {
        if html.len() > FRAGMENT_BYTES {
            return Err(EngineError::limit("fragment bytes", FRAGMENT_BYTES));
        }
        if self.fragment_bytes + html.len() > FRAGMENT_TOTAL_BYTES {
            return Err(EngineError::limit(
                "execution fragment bytes",
                FRAGMENT_TOTAL_BYTES,
            ));
        }
        self.fragment_bytes += html.len();
        let nodes = self.fragments.parse(context, html, self.next_id)?;
        let next = nodes
            .iter()
            .map(|node| node.local_id + 1)
            .max()
            .unwrap_or(self.next_id);
        if nodes.iter().any(|node| node.local_id < self.next_id) {
            return Err(EngineError::invalid(
                "The fragment parser reused node identities.",
            ));
        }
        self.next_id = next.max(self.next_id);
        Ok(nodes)
    }
    /** Replace a node's children with text. */
    pub fn set_text(&mut self, id: usize, text: &str) -> EngineResult<()> {
        self.touch();
        // A shadow host keeps its shadow root, so its light children go one by one.
        let host = self.has_shadows() && self.shadow_root(id).is_some();
        if (self.is_projected(id) || id == BODY) && !host {
            if id == BODY {
                return Err(EngineError::unsupported(
                    "Replacing the body's text is not supported.",
                ));
            }
            // Shadow trees inside the subtree go too; skipping them lost nodes scripts still held.
            let removed = self.all_descendants(id);
            let frames: Vec<_> = removed
                .iter()
                .copied()
                .filter(|owner| self.tag(*owner) == "iframe")
                .collect();
            let records = self.capture(&removed);
            let document = self.document.as_mut().ok_or_else(invalid)?;
            if self.environment.dom_level == DomLevel::Legacy {
                let change = document.set_text(id, text)?;
                self.invalidate();
                self.record(ScriptChange::Dom {
                    change: change.into(),
                })?;
            } else {
                let change = document.set_node_text(id, text)?;
                self.invalidate();
                self.record(change)?;
            }
            self.store_detached(records);
            for owner in frames {
                super::frame_directory::retire(self, owner)?;
            }
            for node in removed {
                self.state.texts.remove(&node);
            }
            self.state.texts.insert(id, text.into());
            return Ok(());
        }
        if matches!(id, HTML | HEAD) {
            return Ok(());
        }
        let children = self.children(id);
        for child in children {
            self.remove(child)?;
        }
        let tag = self.tag(id);
        // Character data (text, comment, processing instruction) holds its text on the node.
        if tag == "#text"
            || tag == "#comment"
            || tag == super::implementation::PROCESSING_INSTRUCTION
        {
            if let Some(record) = self.state.detached.get_mut(&id) {
                record.text = Some(text.into());
            }
            return Ok(());
        }
        if !text.is_empty() {
            let node = self.create("#text", Some(text.into()))?;
            self.insert(id, node, None)?;
        }
        Ok(())
    }
    /** Set or remove an attribute, queueing the change for reactions and observers. */
    pub fn set_attribute(
        &mut self,
        id: usize,
        name: &str,
        value: Option<&str>,
    ) -> EngineResult<()> {
        // Browsers refuse names with control characters (InvalidCharacterError); here that
        // also keeps every engine-only marker, like POPOVER_OPEN_ATTRIBUTE, out of a page's
        // reach: page writes and removals all come through this method.
        if name.chars().any(char::is_control) {
            // "<DOMException name>: message" becomes that catchable DOMException (dom_error).
            return Err(EngineError::invalid(
                "InvalidCharacterError: the attribute name contains a control character.",
            ));
        }
        let name = name.to_ascii_lowercase();
        self.set_named_attribute(id, &name, value)
    }
    /** Write one attribute under a name already in its final case, then tell images and the
    mutation log. Both setAttribute and the namespace-aware methods end here, so every write
    reaches the same observers. */
    pub fn set_named_attribute(
        &mut self,
        id: usize,
        name: &str,
        value: Option<&str>,
    ) -> EngineResult<()> {
        let old = self
            .view(id)
            .and_then(|view| view.attributes.get(name).cloned());
        let canvas_reset = super::canvas::prepare_resize(self, id, name, value)?;
        self.write_attribute(id, name, value)?;
        if name == "selected" && self.tag(id) == "option" {
            self.selected_attribute(id, value.is_some())?;
        }
        if value.is_none() {
            self.detach_attr(id, name, old.clone());
        }
        super::images::source_changed(self, id, name)?;
        super::media::attribute_changed(self, id, name);
        if let Some(bitmap) = canvas_reset {
            super::canvas::resized(self, id, bitmap);
        }
        if self.mutations.len() < crate::constants::STATEMENTS {
            self.mutations.push(super::components::Mutation::Attribute {
                node: id,
                name: name.to_string(),
                old,
            });
        }
        Ok(())
    }
    /** Tell layout a popover opened or closed: the engine-only marker, written without a
    mutation record, since the page set no attribute. */
    pub fn mark_popover_open(&mut self, id: usize, open: bool) -> EngineResult<()> {
        let marker = crate::document::POPOVER_OPEN_ATTRIBUTE;
        self.write_attribute(id, marker, open.then_some(""))
    }
    /** Set or remove an attribute in whichever store holds the node. */
    fn write_attribute(&mut self, id: usize, name: &str, value: Option<&str>) -> EngineResult<()> {
        if value.is_none() {
            // A removed attribute keeps no namespace record, so a later write starts null.
            self.forget_attribute_namespace(id, name);
        }
        self.touch();
        let name = name.to_string();
        if matches!(id, HTML | HEAD | BODY) {
            let seed = self.root_attributes(id);
            let unchanged = seed.get(&name).map(String::as_str) == value;
            if !unchanged
                && id != HEAD
                && !self.legacy_changes()
                && crate::document::script_attribute_allowed(&name)
            {
                crate::document::validate_text(value.unwrap_or_default())?;
                self.admit_attribute(id, &name, value)?;
            }
            let attributes = self.state.virtual_attributes.entry(id).or_insert(seed);
            match value {
                Some(value) => attributes.insert(name.clone(), value.into()),
                None => attributes.remove(&name),
            };
            // The document engine styles html and body from its own copy, so it hears of the
            // write; the head has no attributes that style anything.
            let tells = id != HEAD
                && !unchanged
                && !self.legacy_changes()
                && crate::document::script_attribute_allowed(&name);
            if tells {
                crate::document::validate_text(value.unwrap_or_default())?;
                self.record(ScriptChange::RootAttribute {
                    body: id == BODY,
                    name,
                    value: value.map(Into::into),
                })?;
            }
            return Ok(());
        }
        if let Some(record) = self.state.detached.get_mut(&id) {
            match value {
                Some(value) => record.attributes.insert(name, value.into()),
                None => record.attributes.remove(&name),
            };
            return Ok(());
        }
        let current = self
            .view_with_marker(id)
            .and_then(|view| view.attributes.get(&name).cloned());
        if current.as_deref() == value {
            return Ok(());
        }
        if self.environment.dom_level == DomLevel::Extended {
            self.admit_attribute(id, &name, value)?;
        }
        // An attribute write moves no node, so the tree index stays valid and is not rebuilt: a
        // class write on a page of thousands of elements used to rebuild the whole index.
        let at = self.tree().index.get(&id).copied();
        let document = self.document.as_mut().ok_or_else(invalid)?;
        if self.environment.dom_level == DomLevel::Legacy {
            let Some(value) = value else {
                return Err(EngineError::unsupported(
                    "Removing attributes needs the extended change log.",
                ));
            };
            let change = document.set_attribute(id, &name, value)?;
            return self.record(ScriptChange::Dom { change });
        }
        let change = match at {
            Some(at) => document.set_any_attribute_at(at, id, &name, value)?,
            None => document.set_any_attribute(id, &name, value)?,
        };
        self.record(change)
    }
    /** Set a field's value, or the reflected `value` attribute of elements that have one. */
    pub fn set_value(&mut self, id: usize, value: &str) -> EngineResult<()> {
        self.touch();
        crate::document::validate_text(value)?;
        if self.tag(id) == "select" {
            return self.select_value(id, value);
        }
        let tag = self.tag(id);
        // These elements' `value` reflects their content attribute (HTML's button, option,
        // data and param), so a write is an attribute change, never a field value.
        if REFLECTED_VALUE.contains(&tag.as_str()) {
            return self.set_attribute(id, "value", Some(value));
        }
        self.state.values.insert(id, value.into());
        // Only fields carry a native value the document engine replays (Apple's video
        // controls set a button's value, which Eye rightly refuses as a field write).
        let field = matches!(tag.as_str(), "input" | "textarea");
        if field && self.is_projected(id) && self.environment.dom_level == DomLevel::Extended {
            self.record(ScriptChange::Value {
                node: id,
                value: value.into(),
            })?;
        }
        Ok(())
    }
    /** Insert parsed HTML relative to a node, as insertAdjacentHTML does. */
    pub fn insert_html(
        &mut self,
        id: usize,
        position: InsertPosition,
        html: &str,
    ) -> EngineResult<()> {
        self.touch();
        if self.is_projected(id) || id == BODY {
            let target = (id != BODY).then_some(id);
            if id == BODY
                && matches!(
                    position,
                    InsertPosition::BeforeBegin | InsertPosition::AfterEnd
                )
            {
                return Err(EngineError::unsupported(
                    "The body accepts only afterbegin and beforeend insertion.",
                ));
            }
            let context = match position {
                InsertPosition::AfterBegin | InsertPosition::BeforeEnd => self.tag(id),
                _ => self
                    .parent(id)
                    .map(|parent| self.tag(parent))
                    .unwrap_or_else(|| "body".into()),
            };
            let nodes = self.parse_fragment(&context, html)?;
            let first_id = nodes.first().map_or(self.next_id, |node| node.local_id);
            let count = nodes.len();
            let document = self.document.as_mut().ok_or_else(invalid)?;
            let at = document.insert(target, position, nodes)?;
            self.tree_inserted(at, count);
            super::media::inserted_range(self, first_id, count);
            return self.record(ScriptChange::Dom {
                change: DomChange::InsertHtml {
                    target,
                    position,
                    html: html.into(),
                    first_id,
                },
            });
        }
        let parsed = super::dom::parse_detached(self, html)?;
        let (parent, reference) = match position {
            InsertPosition::AfterBegin => (Some(id), self.children(id).first().copied()),
            InsertPosition::BeforeEnd => (Some(id), None),
            InsertPosition::BeforeBegin => (self.parent(id), Some(id)),
            InsertPosition::AfterEnd => {
                let parent = self.parent(id);
                let next = parent.and_then(|p| {
                    let children = self.children(p);
                    children
                        .iter()
                        .position(|c| *c == id)
                        .and_then(|i| children.get(i + 1).copied())
                });
                (parent, next)
            }
        };
        let Some(parent) = parent else {
            return Err(EngineError::invalid(
                "NoModificationAllowedError: The element has no parent.",
            ));
        };
        for node in parsed {
            self.insert(parent, node, reference)?;
        }
        Ok(())
    }
}

/** Whether a node below the one textContent reads adds its own text: a text node does, and so
does an element, which holds its lone text child itself. Comments and processing instructions
never do, so the textContent of a parent leaves them out. */
fn contributes_text(tag: &str) -> bool {
    tag == "#text" || !tag.starts_with('#')
}

/** Projection inconsistencies are invalid input, never silently ignored. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid script document projection.")
}

/** Pair each parsed node with the next old node of the same tag, in preorder. The parser may
leave nodes out but never adds or reorders them; anything else is None. */
fn adopted(old: &[usize], tags: &[String], nodes: &[ScriptNode]) -> Option<Vec<(usize, usize)>> {
    let mut mapping = Vec::with_capacity(nodes.len());
    let mut next = 0;
    for node in nodes {
        let offset = tags[next..].iter().position(|tag| *tag == node.tag)?;
        next += offset;
        mapping.push((old[next], node.local_id));
        next += 1;
    }
    Some(mapping)
}

/** Virtual root attributes share bounded replacement accounting with ordinary attributes. */
fn attribute_key(change: &ScriptChange) -> Option<(usize, &str)> {
    match change {
        ScriptChange::Attribute { node, name, .. } => Some((*node, name)),
        ScriptChange::RootAttribute { body, name, .. } => {
            Some((if *body { BODY } else { HTML }, name))
        }
        _ => None,
    }
}
