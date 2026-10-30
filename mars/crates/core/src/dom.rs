/** Inert bounded document projections and text proposals share one identity across engines. */
use crate::{
    constants::{DOCUMENT_DEPTH, ID_BYTES, NODES, SCRIPT_BYTES, TEXT_BYTES},
    protocol::{EngineError, EngineResult},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Script output is data; it grants no agent or networking capabilities.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextChange {
    /// Source HTML identifier, resolved against this candidate only.
    pub html_id: String,
    /// New text, validated before any change is applied.
    pub text: String,
}

/// Script-visible state carries no native handles, credentials or protected field values.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptDocument {
    /// Coordinator-selected page identity prevents references surviving navigation or tab changes.
    pub identity: String,
    /// Preorder nodes retain their original identities after descendant replacement.
    pub nodes: Vec<ScriptNode>,
    /// The encoding name `document.characterSet` reports: the one the parser settled, after a
    /// meta charset. Empty (old saved state) reads as UTF-8.
    #[serde(default)]
    pub charset: String,
}

/// A metadata element Eye keeps out of layout: `meta`, `link`, `style`, `title`, `base` and
/// non-JavaScript `script` data blocks, wherever they appear. V9 gives each one a head element
/// at load so `getElementById`, `querySelector` and `textContent` find them as a browser's DOM
/// does; they never become projected nodes, so the divergence digest and the fresh-identity
/// rules are unchanged, and changes scripts make to them stay inside V9 like other head content.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MetadataElement {
    /// Lowercase element name.
    pub tag: String,
    /// Author attributes as parsed.
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    /// Raw text content (style sheet, title, data block); empty when it exceeded TEXT_BYTES.
    #[serde(default)]
    pub text: String,
    /// Whether it sits in the body rather than the head. A data block inside an element is that
    /// element's child in the DOM (`<react-partial>` reads its props from its own child
    /// `<script type=application/json>`), so a head copy is not found by `querySelector` on it.
    #[serde(default)]
    pub in_body: bool,
    /// The local id of the element it sits in, or None for a body child.
    #[serde(default)]
    pub parent: Option<usize>,
    /// The first local id parsed after it: it sits before the parent's first child at or past it.
    #[serde(default)]
    pub next: usize,
}

/// Supported element capabilities are computed from the real owned document.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptNode {
    /// Stable local identity is inert data and grants no native control permission.
    pub local_id: usize,
    /// Actual author identifier; missing identifiers remain empty rather than synthetic selectors.
    pub html_id: String,
    /// Parent precedes its children and remains stable through text replacement.
    pub parent: Option<usize>,
    /// Only nonsecret input and textarea values may enter the bounded untainted interpreter.
    pub value: Option<String>,
    /// Current option selectedness, independent of its default selected attribute.
    #[serde(default)]
    pub selected: Option<bool>,
    /// Mirrors the document mutation validator rather than inventing another target policy.
    pub text_writable: bool,
    /// Only enabled visible explicit ordinary buttons accept programmatic click listeners.
    pub click_listener: bool,
    /// Existing supported inline button source participates in the same click transaction.
    pub inline_handler: Option<String>,
    /// Lowercase element name, or `#text`, so selectors and the divergence digest see structure.
    #[serde(default)]
    pub tag: String,
    /// Inert author attributes; secret field values never appear here.
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

/// When a classic script runs; declaration order is the execution order, each in document order.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ScriptMode {
    /// Runs first, in document order, after the parse finishes.
    #[default]
    Blocking,
    /// External scripts with `defer` run after every blocking script.
    Defer,
    /// External scripts with `async` run last; async wins over defer.
    Async,
}

/// Index of the node with `local_id == id`. Ids equal indexes until a script inserts or moves
/// nodes, so `nodes[id]` is tried first and a linear scan is the fallback; this keeps per-node
/// lookups linear overall at NODES while returning exactly what a linear find would.
pub fn local_position<T>(nodes: &[T], id: usize, local_id: impl Fn(&T) -> usize) -> Option<usize> {
    if nodes.get(id).is_some_and(|node| local_id(node) == id) {
        return Some(id);
    }
    nodes.iter().position(|node| local_id(node) == id)
}

/// Where inserted or moved nodes land relative to a target, as in insertAdjacentHTML.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InsertPosition {
    /// Immediately before the target, as its previous sibling.
    BeforeBegin,
    /// As the target's first child.
    AfterBegin,
    /// As the target's last child.
    BeforeEnd,
    /// Immediately after the target's subtree, as its next sibling.
    AfterEnd,
}

/// One ordered script change; engines replay the log in order so both trees end up identical.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomChange {
    /// Replace an element's descendants with text, exactly like TextChange.
    Text {
        /// Source HTML identifier, resolved against this candidate only.
        html_id: String,
        /// New text, validated before any change is applied.
        text: String,
    },
    /// Parse a fragment and insert it; `first_id` fixes the new identities so both sides agree.
    InsertHtml {
        /// Target node, or None for the body wrapper.
        target: Option<usize>,
        /// Placement relative to the target.
        position: InsertPosition,
        /// The fragment source, parsed again by the applying engine.
        html: String,
        /// Identity of the first fragment node; later nodes count up in preorder.
        first_id: usize,
    },
    /// Move an existing subtree; identities are preserved.
    Move {
        /// Root of the moved subtree.
        node: usize,
        /// Target node, or None for the body wrapper.
        target: Option<usize>,
        /// Placement relative to the target.
        position: InsertPosition,
    },
    /// Set one allowlisted reflected attribute.
    SetAttribute {
        /// Element whose attribute changes.
        node: usize,
        /// Attribute name from REFLECTED_ATTRIBUTES.
        name: String,
        /// New attribute value.
        value: String,
    },
}

impl DomChange {
    /// The replacement text of a Text change, for callers that only read text proposals.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

impl From<TextChange> for DomChange {
    /** Text proposals keep their existing shape inside the structural log. */
    fn from(change: TextChange) -> Self {
        Self::Text {
            html_id: change.html_id,
            text: change.text,
        }
    }
}

/// Lets V9 see freshly inserted nodes in the same run without depending on Eye's parser crate.
pub trait FragmentParser {
    /// Returns preorder nodes numbered from `first_id`; top-level nodes have parent None.
    fn parse(
        &self,
        context_tag: &str,
        html: &str,
        first_id: usize,
    ) -> EngineResult<Vec<ScriptNode>>;
}

/// Scripts may write only these attributes; id, href, src, style and handlers stay author-owned.
pub const REFLECTED_ATTRIBUTES: [&str; 2] = ["lang", "dir"];

impl ScriptDocument {
    /// Private wire snapshots are bounded trees, including fields not currently referenced by JS.
    pub fn validate(&self) -> EngineResult<()> {
        if self.identity.is_empty() || self.identity.len() > ID_BYTES {
            return Err(invalid_projection());
        }
        if self.nodes.len() > NODES {
            return Err(EngineError::limit("document nodes", NODES));
        }
        let mut seen = BTreeMap::new();
        for node in &self.nodes {
            let ancestors = Self::validate_node(node, &seen)?;
            seen.insert(node.local_id, ancestors);
        }
        validate_preorder(self.nodes.iter().map(|node| (node.local_id, node.parent)))
    }

    /** Parent ordering removes cycles before any interpreter traversal can occur; `seen` maps
    each earlier id to its ancestor count, and this node's count is returned. */
    fn validate_node(node: &ScriptNode, seen: &BTreeMap<usize, usize>) -> EngineResult<usize> {
        if node.local_id >= NODES
            || seen.contains_key(&node.local_id)
            || node.html_id.len() > crate::constants::HTML_ID_BYTES
            || node
                .parent
                .is_some_and(|parent| !seen.contains_key(&parent))
            || node
                .inline_handler
                .as_ref()
                .is_some_and(|source| source.len() > SCRIPT_BYTES)
        {
            return Err(invalid_projection());
        }
        if let Some(value) = &node.value {
            validate_text(value)?;
        }
        if node.tag.len() > ID_BYTES || node.attributes.keys().any(|name| name.len() > ID_BYTES) {
            return Err(invalid_projection());
        }
        node.attributes
            .values()
            .try_for_each(|value| validate_text(value))?;
        Self::validate_depth(node, seen)
    }

    /** The ordinary document depth limit also bounds retained projected ancestry; the parent's
    ancestor count is already known, so each node costs one map lookup. */
    fn validate_depth(node: &ScriptNode, seen: &BTreeMap<usize, usize>) -> EngineResult<usize> {
        let ancestors = node
            .parent
            .map_or(0, |parent| seen.get(&parent).map_or(0, |count| count + 1));
        if ancestors >= DOCUMENT_DEPTH {
            return Err(EngineError::limit("document depth", DOCUMENT_DEPTH));
        }
        Ok(ancestors)
    }

    /// Empty identifiers match nothing and duplicates resolve in current document order.
    pub fn get_by_id(&self, html_id: &str) -> Option<usize> {
        (!html_id.is_empty())
            .then(|| self.nodes.iter().find(|node| node.html_id == html_id))
            .flatten()
            .map(|node| node.local_id)
    }

    /// Replaces descendants immediately so later reads observe the same candidate tree.
    pub fn set_text(&mut self, local_id: usize, text: impl AsRef<str>) -> EngineResult<TextChange> {
        let text = text.as_ref();
        validate_text(text)?;
        let node = self
            .nodes
            .iter()
            .find(|node| node.local_id == local_id)
            .ok_or_else(invalid_projection)?;
        if !node.text_writable || self.get_by_id(&node.html_id) != Some(local_id) {
            return Err(EngineError::unsupported(
                "textContent requires a supported element with its first matching nonempty ID.",
            ));
        }
        let change = TextChange {
            html_id: node.html_id.clone(),
            text: text.into(),
        };
        let removed = descendant_ids(
            self.nodes.iter().map(|node| (node.local_id, node.parent)),
            local_id,
        );
        self.nodes.retain(|node| !removed.contains(&node.local_id));
        Ok(change)
    }

    /// Inserts parsed fragment nodes atomically and returns where the first one landed; a
    /// refused insert leaves the projection unchanged. Only the fragment is validated, against the depth of its insertion point: re-validating
    /// the whole document on every insert made a page of thousands of inserts quadratic.
    pub fn insert(
        &mut self,
        target: Option<usize>,
        position: InsertPosition,
        inserted: Vec<ScriptNode>,
    ) -> EngineResult<usize> {
        let (_, parent) = slot(&self.nodes, &node_key, target, position)?;
        self.validate_fragment(parent, &inserted)?;
        place(
            &mut self.nodes,
            node_key,
            reparent,
            target,
            position,
            inserted,
        )
    }

    /** Every fragment node passes the per-node checks of `validate`, with its ancestor count
    continuing from the node it is placed under, so the whole tree stays within DOCUMENT_DEPTH. */
    fn validate_fragment(
        &self,
        parent: Option<usize>,
        inserted: &[ScriptNode],
    ) -> EngineResult<()> {
        let mut seen = BTreeMap::new();
        if let Some(parent) = parent {
            seen.insert(parent, self.ancestors_of(parent)?);
        }
        for node in inserted {
            let fragment_parent = node.parent.or(parent);
            if !node.parent.is_none_or(|parent| seen.contains_key(&parent)) {
                return Err(invalid_projection());
            }
            let probe = ScriptNode {
                parent: fragment_parent,
                ..node.clone()
            };
            let ancestors = Self::validate_node(&probe, &seen)?;
            seen.insert(node.local_id, ancestors);
        }
        Ok(())
    }

    /** How many ancestors a node has; walks parents through an id-to-parent index. */
    fn ancestors_of(&self, id: usize) -> EngineResult<usize> {
        let parents: BTreeMap<usize, Option<usize>> = self
            .nodes
            .iter()
            .map(|node| (node.local_id, node.parent))
            .collect();
        let mut count = 0;
        let mut current = parents.get(&id).copied().ok_or_else(invalid_projection)?;
        while let Some(parent) = current {
            count += 1;
            if count >= DOCUMENT_DEPTH {
                return Err(EngineError::limit("document depth", DOCUMENT_DEPTH));
            }
            current = parents
                .get(&parent)
                .copied()
                .ok_or_else(invalid_projection)?;
        }
        Ok(count)
    }

    /// Moves a subtree atomically, keeping every identity.
    pub fn relocate(
        &mut self,
        node: usize,
        target: Option<usize>,
        position: InsertPosition,
    ) -> EngineResult<DomChange> {
        let mut next = self.clone();
        relocate(&mut next.nodes, node_key, reparent, node, target, position)?;
        next.validate()?;
        *self = next;
        Ok(DomChange::Move {
            node,
            target,
            position,
        })
    }

    /// Sets one allowlisted reflected attribute and returns the change to replay.
    pub fn set_attribute(
        &mut self,
        node: usize,
        name: &str,
        value: &str,
    ) -> EngineResult<DomChange> {
        if !REFLECTED_ATTRIBUTES.contains(&name) {
            return Err(EngineError::unsupported(
                "Scripts may set only the lang and dir attributes.",
            ));
        }
        validate_text(value)?;
        let entry = self
            .nodes
            .iter_mut()
            .find(|entry| entry.local_id == node && entry.tag != "#text")
            .ok_or_else(invalid_projection)?;
        entry.attributes.insert(name.into(), value.into());
        Ok(DomChange::SetAttribute {
            node,
            name: name.into(),
            value: value.into(),
        })
    }

    /// FNV-1a over structure in Vec order; a divergence check between engines, not a security hash.
    /// A password or hidden input's `value` attribute is left out: the document engine never
    /// projects it, so a script writing it must not read as divergence.
    pub fn digest(&self) -> u64 {
        let mut hash = Fnv::default();
        for node in &self.nodes {
            let secret = node.tag == "input"
                && node.attributes.get("type").is_some_and(|kind| {
                    ["password", "hidden"].contains(&kind.to_ascii_lowercase().as_str())
                });
            hash.write(&(node.local_id as u64).to_le_bytes());
            hash.write(
                &node
                    .parent
                    .map_or(0, |parent| parent as u64 + 1)
                    .to_le_bytes(),
            );
            hash.field(&node.tag);
            for (name, value) in &node.attributes {
                if secret && name == "value" {
                    continue;
                }
                hash.field(name);
                hash.field(value);
            }
            hash.write(&[FNV_NODE_END]);
        }
        hash.0
    }
}

/** Projection nodes expose their identity and parent to the shared tree helpers. */
fn node_key(node: &ScriptNode) -> (usize, Option<usize>) {
    (node.local_id, node.parent)
}

/** Projection nodes take a new parent when the helpers attach a root. */
fn reparent(node: &mut ScriptNode, parent: Option<usize>) {
    node.parent = parent;
}

/** FNV-1a 64-bit offset basis. */
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/** FNV-1a 64-bit prime. */
const FNV_PRIME: u64 = 0x0100_0000_01b3;
/** Separates variable-length fields so adjacent strings cannot alias. */
const FNV_FIELD_END: u8 = 0xff;
/** Separates nodes so attribute lists cannot alias the next node. */
const FNV_NODE_END: u8 = 0xfe;

/** Streaming FNV-1a state; no dependency is needed for a non-cryptographic digest. */
struct Fnv(u64);

impl Default for Fnv {
    /** Every digest starts from the published offset basis. */
    fn default() -> Self {
        Self(FNV_OFFSET)
    }
}

impl Fnv {
    /** Mix raw bytes. */
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
        }
    }

    /** Mix one string field with a terminator. */
    fn field(&mut self, value: &str) {
        self.write(value.as_bytes());
        self.write(&[FNV_FIELD_END]);
    }
}

/// Preorder with contiguous subtrees: each parent must be on the current ancestor path.
pub fn validate_preorder(nodes: impl Iterator<Item = (usize, Option<usize>)>) -> EngineResult<()> {
    let mut path: Vec<usize> = Vec::new();
    for (id, parent) in nodes {
        match parent {
            None => path.clear(),
            Some(parent) => {
                while path.last().is_some_and(|last| *last != parent) {
                    path.pop();
                }
                if path.is_empty() {
                    return Err(invalid_projection());
                }
            }
        }
        path.push(id);
    }
    Ok(())
}

/// Inserts preorder nodes (top-level parent None) at `position` of `target`; None is the body.
pub fn place<T>(
    nodes: &mut Vec<T>,
    key: impl Fn(&T) -> (usize, Option<usize>),
    mut reparent: impl FnMut(&mut T, Option<usize>),
    target: Option<usize>,
    position: InsertPosition,
    mut inserted: Vec<T>,
) -> EngineResult<usize> {
    let existing: BTreeSet<usize> = nodes.iter().map(|node| key(node).0).collect();
    let mut fresh = BTreeSet::new();
    for node in &inserted {
        let (id, parent) = key(node);
        if id >= NODES
            || existing.contains(&id)
            || !fresh.insert(id)
            || parent.is_some_and(|parent| !fresh.contains(&parent))
        {
            return Err(invalid_projection());
        }
    }
    if nodes.len() + inserted.len() > NODES {
        return Err(EngineError::limit("document nodes", NODES));
    }
    let (index, parent) = slot(nodes, &key, target, position)?;
    for node in inserted.iter_mut().filter(|node| key(node).1.is_none()) {
        reparent(node, parent);
    }
    nodes.splice(index..index, inserted);
    Ok(index)
}

/// Moves `node`'s subtree to `position` of `target`; moving into its own subtree is refused.
pub fn relocate<T>(
    nodes: &mut Vec<T>,
    key: impl Fn(&T) -> (usize, Option<usize>),
    mut reparent: impl FnMut(&mut T, Option<usize>),
    node: usize,
    target: Option<usize>,
    position: InsertPosition,
) -> EngineResult<()> {
    let start = index_of(nodes, &key, node)?;
    let end = subtree_end(nodes, &key, start);
    if target.is_some_and(|target| nodes[start..end].iter().any(|node| key(node).0 == target)) {
        return Err(EngineError::invalid(
            "A node cannot move into its own subtree.",
        ));
    }
    let mut moved: Vec<T> = nodes.drain(start..end).collect();
    let (index, parent) = slot(nodes, &key, target, position)?;
    reparent(&mut moved[0], parent);
    nodes.splice(index..index, moved);
    Ok(())
}

/** Resolve the Vec index and parent that a placement lands at. */
fn slot<T>(
    nodes: &[T],
    key: &impl Fn(&T) -> (usize, Option<usize>),
    target: Option<usize>,
    position: InsertPosition,
) -> EngineResult<(usize, Option<usize>)> {
    let Some(target) = target else {
        return match position {
            InsertPosition::AfterBegin => Ok((0, None)),
            InsertPosition::BeforeEnd => Ok((nodes.len(), None)),
            _ => Err(EngineError::unsupported(
                "The body accepts only afterbegin and beforeend insertion.",
            )),
        };
    };
    let index = index_of(nodes, key, target)?;
    let parent = key(&nodes[index]).1;
    Ok(match position {
        InsertPosition::BeforeBegin => (index, parent),
        InsertPosition::AfterBegin => (index + 1, Some(target)),
        InsertPosition::BeforeEnd => (subtree_end(nodes, key, index), Some(target)),
        InsertPosition::AfterEnd => (subtree_end(nodes, key, index), parent),
    })
}

/** Find a node's Vec index; after scripts run, ids no longer equal indexes. */
fn index_of<T>(
    nodes: &[T],
    key: &impl Fn(&T) -> (usize, Option<usize>),
    id: usize,
) -> EngineResult<usize> {
    nodes
        .iter()
        .position(|node| key(node).0 == id)
        .ok_or_else(invalid_projection)
}

/** One past the last node of the contiguous subtree rooted at `start`. */
fn subtree_end<T>(nodes: &[T], key: &impl Fn(&T) -> (usize, Option<usize>), start: usize) -> usize {
    let mut members = BTreeSet::from([key(&nodes[start]).0]);
    let mut end = start + 1;
    while end < nodes.len()
        && key(&nodes[end])
            .1
            .is_some_and(|parent| members.contains(&parent))
    {
        members.insert(key(&nodes[end]).0);
        end += 1;
    }
    end
}

/// Refuses an attribute write a script may not make, and a value that is not valid text. Shared by
/// both write paths so they refuse exactly the same writes.
fn check_any_attribute(name: &str, value: Option<&str>) -> EngineResult<()> {
    if !script_attribute_allowed(name) {
        return Err(EngineError::unsupported(
            "Scripts may not write event handler, srcdoc or http-equiv attributes.",
        ));
    }
    if let Some(value) = value {
        validate_text(value)?;
    }
    Ok(())
}

/** Hostile private state is rejected without exposing values or native identities. */
fn invalid_projection() -> EngineError {
    EngineError::invalid("Invalid script document projection.")
}

/** Both representations remove the same preorder descendant identities synchronously. */
pub fn descendant_ids(
    nodes: impl Iterator<Item = (usize, Option<usize>)>,
    parent: usize,
) -> Vec<usize> {
    let mut members = BTreeSet::from([parent]);
    let mut removed = Vec::new();
    for (id, parent) in nodes {
        if parent.is_some_and(|parent| members.contains(&parent)) {
            members.insert(id);
            removed.push(id);
        }
    }
    removed
}

/// Bounds decoded text, not just the source representation.
pub fn validate_text(text: &str) -> EngineResult<()> {
    if text.len() > TEXT_BYTES {
        return Err(EngineError::limit("node text bytes", TEXT_BYTES));
    }
    Ok(())
}

/// The extended script change log: everything DomChange expresses plus removal, any allowed
/// attribute, attribute removal, node text and field values. V9 emits it only when the app says
/// its document engine replays it (`PageEnvironment::dom_level`); otherwise V9 keeps to DomChange.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScriptChange {
    /// Final pixels from a canvas in this exact document candidate, independently validated by Eye.
    Canvas {
        /// Document-local canvas identity, after DOM adoption.
        node: usize,
        /// Complete backing, including empty backings after reset.
        bitmap: crate::canvas::Bitmap,
    },
    /// One of the original changes, replayed exactly as before.
    Dom {
        /// The wrapped change.
        change: DomChange,
    },
    /// Remove a node and its subtree.
    Remove {
        /// Root of the removed subtree.
        node: usize,
    },
    /// Set (Some) or remove (None) one attribute that `script_attribute_allowed` admits.
    Attribute {
        /// Element whose attribute changes.
        node: usize,
        /// Lowercase attribute name.
        name: String,
        /// New value, or None to remove the attribute.
        value: Option<String>,
    },
    /// Replace a node's children with text, like assigning textContent.
    NodeText {
        /// The element whose children are replaced.
        node: usize,
        /// The new text.
        text: String,
    },
    /// Transfer current selected option identities without changing default markup.
    Selection {
        /// The owning SELECT in this document.
        node: usize,
        /// Exact option identities, including an intentionally empty selection.
        selected: Vec<usize>,
    },
    /// Set a form field's current value (input and textarea).
    Value {
        /// The field.
        node: usize,
        /// Its new value.
        value: String,
    },
    /// A custom element was upgraded: it now matches `:defined`, whether or not it attached a
    /// shadow root.
    Defined {
        /// The upgraded element.
        node: usize,
    },
    /// Set (Some) or remove (None) one attribute of the document element (`body` false) or the
    /// body (`body` true). Those are virtual in V9's projection, so they travel apart from
    /// Attribute; without this `<html class>` written by a script (WPT's `reftest-wait`,
    /// `:root.dark` themes) never reached the document engine's style.
    RootAttribute {
        /// Whether the body rather than the document element changes.
        body: bool,
        /// Lowercase attribute name.
        name: String,
        /// New value, or None to remove the attribute.
        value: Option<String>,
    },
    /// Focus moved to an element, or left the page (None), so `:focus` and `:focus-within`
    /// match where the script put it.
    Focus {
        /// The newly focused element.
        node: Option<usize>,
    },
}

/// Attributes scripts may write through ScriptChange: everything except inline event handlers
/// (which would make attribute text executable) and document-replacing srcdoc and http-equiv.
/// Marks a popover the page opened, for the document engine only. Browsers hide a `popover`
/// element until it opens; the open state lives in script, so this attribute carries it to
/// layout. Its name starts with a control character no parser or `setAttribute` produces, and
/// V9 leaves it out of every attribute a page can read.
pub const POPOVER_OPEN_ATTRIBUTE: &str = "\u{1}popover-open";

/// Whether a script may set an attribute named `name`.
pub fn script_attribute_allowed(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !(name.is_empty()
        || name.starts_with("on")
        || name == "srcdoc"
        || name == "http-equiv"
        || name.len() > ID_BYTES
        || name
            .contains(|c: char| c.is_whitespace() || c == '/' || c == '>' || c == '=' || c == '"'))
}

impl ScriptDocument {
    /// Removes a subtree atomically and returns the change to replay.
    pub fn remove(&mut self, node: usize) -> EngineResult<ScriptChange> {
        if !self.nodes.iter().any(|entry| entry.local_id == node) {
            return Err(invalid_projection());
        }
        let mut removed = descendant_ids(self.nodes.iter().map(node_key), node);
        removed.push(node);
        self.nodes
            .retain(|entry| !removed.contains(&entry.local_id));
        Ok(ScriptChange::Remove { node })
    }

    /// Sets or removes one allowed attribute and returns the change to replay. Finds the node by
    /// identity, which scans every node; a caller that already holds the node's position uses
    /// `set_any_attribute_at`.
    pub fn set_any_attribute(
        &mut self,
        node: usize,
        name: &str,
        value: Option<&str>,
    ) -> EngineResult<ScriptChange> {
        check_any_attribute(name, value)?;
        let at = self
            .nodes
            .iter()
            .position(|entry| entry.local_id == node && entry.tag != "#text")
            .ok_or_else(invalid_projection)?;
        self.write_any_attribute(at, node, name, value)
    }

    /// The same write as `set_any_attribute` for a node whose position in `nodes` the caller knows,
    /// so the write costs O(1) on a page of thousands of nodes. `at` must be the position of `node`;
    /// a stale position is refused rather than written to another node.
    pub fn set_any_attribute_at(
        &mut self,
        at: usize,
        node: usize,
        name: &str,
        value: Option<&str>,
    ) -> EngineResult<ScriptChange> {
        check_any_attribute(name, value)?;
        self.write_any_attribute(at, node, name, value)
    }

    /// Applies an attribute write to the node at `at`, after the caller checked the name and value.
    fn write_any_attribute(
        &mut self,
        at: usize,
        node: usize,
        name: &str,
        value: Option<&str>,
    ) -> EngineResult<ScriptChange> {
        let name = name.to_ascii_lowercase();
        let entry = self
            .nodes
            .get_mut(at)
            .filter(|entry| entry.local_id == node && entry.tag != "#text")
            .ok_or_else(invalid_projection)?;
        match value {
            Some(value) => {
                entry.attributes.insert(name.clone(), value.into());
            }
            None => {
                entry.attributes.remove(&name);
            }
        }
        if name == "id" {
            entry.html_id = value.unwrap_or_default().into();
        }
        Ok(ScriptChange::Attribute {
            node,
            name,
            value: value.map(Into::into),
        })
    }

    /// Replaces a node's descendants with text and returns the change to replay.
    pub fn set_node_text(&mut self, node: usize, text: &str) -> EngineResult<ScriptChange> {
        validate_text(text)?;
        if !self.nodes.iter().any(|entry| entry.local_id == node) {
            return Err(invalid_projection());
        }
        let removed = descendant_ids(self.nodes.iter().map(node_key), node);
        self.nodes
            .retain(|entry| !removed.contains(&entry.local_id));
        Ok(ScriptChange::NodeText {
            node,
            text: text.into(),
        })
    }
}
