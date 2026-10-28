/** Eye derives inert script capabilities only from the actual validated document. */
use super::{Document, Node, ScriptDocument, ScriptNode, text_writable};
use crate::{constants::ID_BYTES, protocol::EngineResult};
use std::collections::BTreeMap;

/// Derives a fresh snapshot from actual nodes, never from script-provided capabilities.
pub fn script_document(document: &Document, identity: &str) -> EngineResult<ScriptDocument> {
    super::validate_content(document)?;
    let first = first_ids(document);
    let projection = ScriptDocument {
        identity: identity.into(),
        charset: document.charset.clone(),
        nodes: document
            .nodes
            .iter()
            .map(|node| project_node(&first, node))
            .collect(),
    };
    projection.validate()?;
    Ok(projection)
}

/** The first node holding each targeting ID, built once so projection stays linear at the node limit. */
fn first_ids(document: &Document) -> BTreeMap<&str, usize> {
    let mut first = BTreeMap::new();
    for node in &document.nodes {
        first.entry(node.html_id.as_str()).or_insert(node.local_id);
    }
    first
}

/** A value scripts never see: a password field's, which a person types. A hidden input's value
is the page's own data, which Chrome lets scripts read; withholding it emptied Google's search
tokens (ei, iflsig, sxsrf) in the URL its script builds, and Google answered "unusual traffic".
Agent and DevTools views still hide both (`Node::secret`). */
fn typed_secret(node: &Node) -> bool {
    node.tag == "input" && node.input_type() == "password"
}

/** Explicit author identifiers cannot expose the parser's synthetic private targeting IDs. */
pub(super) fn project_node(first: &BTreeMap<&str, usize>, node: &Node) -> ScriptNode {
    let mut attributes = node.attributes.clone();
    if typed_secret(node) {
        attributes.remove("value");
    }
    // Over-long names are inert to scripts; dropping them keeps such pages loadable.
    attributes.retain(|name, _| name.len() <= ID_BYTES);
    ScriptNode {
        tag: node.tag.clone(),
        attributes,
        local_id: node.local_id,
        html_id: node
            .attributes
            .get("id")
            .filter(|id| id.len() <= mars_core::constants::HTML_ID_BYTES)
            .cloned()
            .unwrap_or_default(),
        parent: node.parent,
        value: (["input", "textarea"].contains(&node.tag.as_str()) && !typed_secret(node))
            .then(|| node.value.clone()),
        text_writable: text_writable(node)
            && first.get(node.html_id.as_str()) == Some(&node.local_id),
        click_listener: click_listener(node),
        inline_handler: node.handler.clone(),
    }
}

/// Eye's parser behind the core seam, so V9 sees inserted nodes without depending on Eye.
pub struct Fragments;

impl mars_core::dom::FragmentParser for Fragments {
    /** Projection reuses the page projector, so both engines see identical structure. */
    fn parse(
        &self,
        context_tag: &str,
        html: &str,
        first_id: usize,
    ) -> EngineResult<Vec<ScriptNode>> {
        project_fragment(super::parse_fragment(context_tag, html, first_id)?.nodes)
    }
}

/// Eye's parser for V9's extended DOM: an element's leading text becomes its own text node, as
/// `apply_script_changes` replays it, so both engines agree on inserted structure.
pub struct DomFragments;

impl mars_core::dom::FragmentParser for DomFragments {
    /** The same projection as Fragments, over nodes with text children. */
    fn parse(
        &self,
        context_tag: &str,
        html: &str,
        first_id: usize,
    ) -> EngineResult<Vec<ScriptNode>> {
        let fragment = super::parse_fragment(context_tag, html, first_id)?;
        project_fragment(text_children(fragment.nodes, first_id)?)
    }
}

/** Fragment nodes projected exactly as page nodes are. */
fn project_fragment(nodes: Vec<Node>) -> EngineResult<Vec<ScriptNode>> {
    let document = Document {
        nodes,
        scripts: Vec::new(),
        title: String::new(),
        styles: Vec::new(),
        diagnostics: Vec::new(),
        formatting: Default::default(),
        csp: Vec::new(),
        metadata: Vec::new(),
        charset: String::new(),
    };
    let first = first_ids(&document);
    Ok(document
        .nodes
        .iter()
        .map(|node| project_node(&first, node))
        .collect())
}

/** Native submission and command behavior remain outside the supported listener subset. */
fn click_listener(node: &Node) -> bool {
    node.tag == "button"
        && node
            .attributes
            .get("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("button"))
        && !node.disabled
        && !node.hidden()
        && node.style.interactive
        && ![
            "command",
            "commandfor",
            "popovertarget",
            "popovertargetaction",
        ]
        .iter()
        .any(|name| node.attributes.contains_key(*name))
}

/** Raw-text elements whose content never becomes child nodes in either engine. */
const RAW_TEXT: [&str; 4] = ["textarea", "style", "script", "title"];

/// Eye's parser keeps text that directly follows an element's start tag on the element itself;
/// a DOM (and V9) has a text node child there. Inserted fragments get that child, renumbered in
/// preorder from `first_id`, so both engines agree on the inserted structure. An option keeps
/// its text too, because select controls read their label from it, and a placeholder label on
/// embedded content is not page text at all.
pub(super) fn text_children(nodes: Vec<Node>, first_id: usize) -> EngineResult<Vec<Node>> {
    let mut split = Vec::with_capacity(nodes.len());
    let mut ids = BTreeMap::new();
    for mut node in nodes {
        let old = node.local_id;
        ids.insert(old, first_id + split.len());
        node.local_id = first_id + split.len();
        node.parent = node.parent.and_then(|parent| ids.get(&parent).copied());
        let text = (node.tag != "#text"
            && !node.text.is_empty()
            && !RAW_TEXT.contains(&node.tag.as_str())
            && !super::parser::labelled(&node))
        .then(|| node.text.clone());
        if text.is_some() && node.tag != "option" {
            node.text.clear();
        }
        let parent = node.local_id;
        split.push(node);
        if let Some(text) = text {
            let mut child = super::markup::node(
                "#text",
                first_id + split.len(),
                Some(parent),
                Default::default(),
            )?;
            child.text = text;
            split.push(child);
        }
    }
    for node in &mut split {
        if let Some(cell) = node.table_cell.as_mut() {
            let map = |id: &mut usize| *id = ids.get(id).copied().unwrap_or(*id);
            map(&mut cell.table);
            map(&mut cell.row);
            cell.headers.iter_mut().for_each(map);
        }
    }
    Ok(split)
}
