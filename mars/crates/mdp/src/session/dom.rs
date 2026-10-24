/**
 * DOM, Accessibility and DOMSnapshot domains over the runtime's node view (`page.nodes`): the
 * document tree with Blink's node ids and attribute arrays (inspector_dom_agent.cc's
 * BuildObjectForNode), selectors answered by V9's own selector engine, geometry from Eye's
 * display in viewport coordinates, the accessibility tree from Mars's roles and names, and a
 * DOMSnapshot built from the same nodes. Node ids are the document's own identities, so they
 * stay valid across commands of one page and die with it.
 */
use super::{
    DOCUMENT_NODE, Fault, Handler, NODE_OFFSET, Session, TEXT_NODE_BASE, runtime::ObjectId,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Chrome's error for a node id the document does not have.
const NODE_NOT_FOUND: &str = "Could not find node with given id";
/// Chrome's DOM node type numbers.
const ELEMENT_NODE: u64 = 1;
const TEXT_NODE: u64 = 3;
const COMMENT_NODE: u64 = 8;
const DOCUMENT_TYPE: u64 = 9;
/// Default depth of getDocument, as Blink's `depth.value_or(2)`.
const DEFAULT_DEPTH: i64 = 2;
/// Text kept in a node value, as Blink's kMaxTextSize.
const MAX_TEXT: usize = 10_000;
/// Roles Mars reports that have no accessibility node of their own.
const GENERIC_ROLES: &[&str] = &["generic", "", "none", "presentation"];

/** The DOM, Accessibility and DOMSnapshot commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("DOM.getDocument", Session::get_document),
    ("DOM.querySelector", Session::query_selector),
    ("DOM.querySelectorAll", Session::query_selector_all),
    ("DOM.describeNode", Session::describe_node),
    ("DOM.getBoxModel", Session::box_model),
    ("DOM.getContentQuads", Session::content_quads),
    ("DOM.resolveNode", Session::resolve_node),
    ("DOM.requestNode", Session::request_node),
    ("DOM.scrollIntoViewIfNeeded", Session::scroll_into_view),
    ("DOM.getOuterHTML", Session::outer_html),
    ("DOM.getAttributes", Session::attributes),
    ("DOM.setAttributeValue", Session::set_attribute),
    ("DOM.setAttributesAsText", Session::set_attributes_as_text),
    ("DOM.removeAttribute", Session::remove_attribute),
    ("DOM.focus", Session::focus),
    ("DOM.getNodeForLocation", Session::node_for_location),
    ("Accessibility.getFullAXTree", Session::full_ax_tree),
    ("Accessibility.getRootAXNode", Session::root_ax_node),
    ("Accessibility.getChildAXNodes", Session::child_ax_nodes),
    (
        "Accessibility.getAXNodeAndAncestors",
        Session::ax_node_and_ancestors,
    ),
    ("Accessibility.getPartialAXTree", Session::partial_ax_tree),
    ("Accessibility.queryAXTree", Session::query_ax_tree),
    ("DOMSnapshot.captureSnapshot", Session::capture_snapshot),
];

/** The runtime's node view, indexed. */
struct View {
    page: Value,
    by_id: BTreeMap<usize, Value>,
    children: BTreeMap<usize, Vec<usize>>,
    roots: Vec<usize>,
    html: usize,
    head: usize,
    body: usize,
    scroll_y: f64,
}

impl View {
    /** Index the view once per command. */
    fn new(page: Value) -> Self {
        let mut by_id = BTreeMap::new();
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut roots = Vec::new();
        for node in page["nodes"].as_array().cloned().unwrap_or_default() {
            let id = node["id"].as_u64().unwrap_or_default() as usize;
            match node["parent"].as_u64() {
                Some(parent) => children.entry(parent as usize).or_default().push(id),
                None => roots.push(id),
            }
            by_id.insert(id, node);
        }
        let virtual_nodes = &page["virtual"];
        Self {
            html: virtual_nodes["html"].as_u64().unwrap_or_default() as usize,
            head: virtual_nodes["head"].as_u64().unwrap_or_default() as usize,
            body: virtual_nodes["body"].as_u64().unwrap_or_default() as usize,
            scroll_y: page["viewport"]["scroll_y"].as_f64().unwrap_or_default(),
            page,
            by_id,
            children,
            roots,
        }
    }
    /** The head's metadata elements as nodes. */
    fn head_nodes(&self) -> Vec<Value> {
        self.page["head"].as_array().cloned().unwrap_or_default()
    }
    /** A DOM.Node for a Mars node, with children to `depth` (-1 for all). */
    fn node(&self, id: usize, depth: i64) -> Value {
        let Some(node) = self.by_id.get(&id) else {
            return self.virtual_node(id, depth);
        };
        let tag = node["tag"].as_str().unwrap_or_default();
        let node_id = id as u64 + NODE_OFFSET;
        if tag == "#text" || tag == "#comment" {
            let value = node["text"].as_str().unwrap_or_default();
            let kind = if tag == "#text" {
                TEXT_NODE
            } else {
                COMMENT_NODE
            };
            return json!({"nodeId":node_id,"backendNodeId":node_id,"nodeType":kind,"nodeName":tag,"localName":"","nodeValue":cap(value),"childNodeCount":0,"parentId":self.parent_id(id)});
        }
        let attributes: Vec<Value> = node["attributes"]
            .as_object()
            .map(|attributes| {
                attributes
                    .iter()
                    .flat_map(|(name, value)| [json!(name), value.clone()])
                    .collect()
            })
            .unwrap_or_default();
        let mut children: Vec<Value> = Vec::new();
        let text = node["text"].as_str().unwrap_or_default();
        let count = self.children.get(&id).map_or(0, Vec::len) + usize::from(!text.is_empty());
        if depth != 0 {
            if !text.is_empty() {
                let text_id = TEXT_NODE_BASE + id as u64;
                children.push(json!({"nodeId":text_id,"backendNodeId":text_id,"nodeType":TEXT_NODE,"nodeName":"#text","localName":"","nodeValue":cap(text),"childNodeCount":0,"parentId":node_id}));
            }
            for child in self.children.get(&id).into_iter().flatten() {
                children.push(self.node(*child, depth - 1));
            }
        }
        let mut value = json!({"nodeId":node_id,"backendNodeId":node_id,"nodeType":ELEMENT_NODE,"nodeName":tag.to_ascii_uppercase(),"localName":tag,"nodeValue":"","childNodeCount":count,"attributes":attributes,"parentId":self.parent_id(id)});
        if depth != 0 {
            value["children"] = json!(children);
        }
        value
    }
    /** The parent's node id. */
    fn parent_id(&self, id: usize) -> u64 {
        match self.by_id.get(&id).and_then(|node| node["parent"].as_u64()) {
            Some(parent) => parent + NODE_OFFSET,
            None => self.body as u64,
        }
    }
    /** The document, html, head, body and metadata nodes V9 numbers outside the projection. */
    fn virtual_node(&self, id: usize, depth: i64) -> Value {
        let base = self.page["head"].as_array().map_or(0, Vec::len);
        let detached: Vec<Value> = self.head_nodes();
        if id as u64 == DOCUMENT_NODE {
            let mut value = json!({"nodeId":DOCUMENT_NODE,"backendNodeId":DOCUMENT_NODE,"nodeType":DOCUMENT_TYPE,"nodeName":"#document","localName":"","nodeValue":"","childNodeCount":1,"documentURL":self.page["url"],"baseURL":self.page["url"],"xmlVersion":""});
            if depth != 0 {
                value["children"] = json!([self.virtual_node(self.html, depth - 1)]);
            }
            return value;
        }
        if id == self.html {
            let attributes = flat_attributes(&self.page["html_attributes"]);
            let mut value = json!({"nodeId":id,"backendNodeId":id,"nodeType":ELEMENT_NODE,"nodeName":"HTML","localName":"html","nodeValue":"","childNodeCount":2,"attributes":attributes,"frameId":"","parentId":DOCUMENT_NODE});
            if depth != 0 {
                value["children"] = json!([
                    self.virtual_node(self.head, depth - 1),
                    self.virtual_node(self.body, depth - 1)
                ]);
            }
            return value;
        }
        if id == self.head {
            let mut value = json!({"nodeId":id,"backendNodeId":id,"nodeType":ELEMENT_NODE,"nodeName":"HEAD","localName":"head","nodeValue":"","childNodeCount":detached.len(),"attributes":[],"parentId":self.html});
            if depth != 0 {
                let children: Vec<Value> = detached
                    .iter()
                    .map(|element| self.metadata_node(element, depth - 1))
                    .collect();
                value["children"] = json!(children);
            }
            return value;
        }
        if id == self.body {
            let attributes = flat_attributes(&self.page["body_attributes"]);
            let mut value = json!({"nodeId":id,"backendNodeId":id,"nodeType":ELEMENT_NODE,"nodeName":"BODY","localName":"body","nodeValue":"","childNodeCount":self.roots.len(),"attributes":attributes,"parentId":self.html});
            if depth != 0 {
                let children: Vec<Value> = self
                    .roots
                    .iter()
                    .map(|root| self.node(*root, depth - 1))
                    .collect();
                value["children"] = json!(children);
            }
            return value;
        }
        if let Some(element) = detached
            .iter()
            .find(|element| element["id"].as_u64() == Some(id as u64))
        {
            return self.metadata_node(element, depth);
        }
        let _ = base;
        json!({"nodeId":id,"backendNodeId":id,"nodeType":ELEMENT_NODE,"nodeName":"","localName":"","nodeValue":"","childNodeCount":0,"attributes":[]})
    }
    /** A head metadata element (meta, link, style, title, script data). */
    fn metadata_node(&self, element: &Value, depth: i64) -> Value {
        let id = element["id"].as_u64().unwrap_or_default();
        let tag = element["tag"].as_str().unwrap_or_default();
        let text = element["text"].as_str().unwrap_or_default();
        let mut value = json!({"nodeId":id,"backendNodeId":id,"nodeType":ELEMENT_NODE,"nodeName":tag.to_ascii_uppercase(),"localName":tag,"nodeValue":"","childNodeCount":usize::from(!text.is_empty()),"attributes":flat_attributes(&element["attributes"]),"parentId":self.head});
        if depth != 0 && !text.is_empty() {
            let text_id = TEXT_NODE_BASE + id;
            value["children"] = json!([{"nodeId":text_id,"backendNodeId":text_id,"nodeType":TEXT_NODE,"nodeName":"#text","localName":"","nodeValue":cap(text),"childNodeCount":0,"parentId":id}]);
        }
        value
    }
    /** A node's painted box in viewport coordinates. */
    fn quad(&self, id: usize) -> Option<(Vec<f64>, f64, f64)> {
        let bounds = self.by_id.get(&id)?["bounds"].as_array()?.clone();
        let value = |index: usize| {
            bounds
                .get(index)
                .and_then(Value::as_f64)
                .unwrap_or_default()
        };
        let (x, y, width, height) = (value(0), value(1) - self.scroll_y, value(2), value(3));
        Some((
            vec![x, y, x + width, y, x + width, y + height, x, y + height],
            width,
            height,
        ))
    }
}

/** Attribute map to Blink's flat name/value array. */
fn flat_attributes(attributes: &Value) -> Vec<Value> {
    attributes
        .as_object()
        .map(|attributes| {
            attributes
                .iter()
                .flat_map(|(name, value)| [json!(name), value.clone()])
                .collect()
        })
        .unwrap_or_default()
}

/** Node values are cut as Blink cuts them. */
fn cap(text: &str) -> String {
    if text.chars().count() <= MAX_TEXT {
        return text.into();
    }
    format!("{}…", text.chars().take(MAX_TEXT).collect::<String>())
}

/** The Mars node a DOM node id names; text children map to their element. */
fn mars_id(node_id: u64) -> Option<usize> {
    if node_id == DOCUMENT_NODE {
        return None;
    }
    if (TEXT_NODE_BASE..(1 << 31)).contains(&node_id) {
        return Some((node_id - TEXT_NODE_BASE) as usize);
    }
    if node_id >= (1 << 31) {
        return Some(node_id as usize);
    }
    node_id.checked_sub(NODE_OFFSET).map(|id| id as usize)
}

impl Session {
    /** The tab's node view. */
    fn view(&mut self, tab: &str) -> Result<View, Fault> {
        let page = self.call_tab(tab, "page.nodes", json!({}))?;
        Ok(View::new(page))
    }

    /** The Mars node a command names by nodeId, backendNodeId or objectId. */
    fn target_node(&mut self, tab: &str, params: &Value) -> Result<usize, Fault> {
        if let Some(text) = params["objectId"].as_str() {
            if let Some(node) = self
                .targets
                .get(tab)
                .and_then(|target| target.nodes.get(text))
                .copied()
            {
                return Ok(node);
            }
            let id = ObjectId::parse(text)?;
            let found = self.devtools(tab, json!({"kind":"node_of","handle":id.handle}))?;
            return found["node"]
                .as_u64()
                .map(|node| node as usize)
                .ok_or_else(|| Fault::server("Object id doesn't reference a Node"));
        }
        let node_id = params["nodeId"]
            .as_u64()
            .or_else(|| params["backendNodeId"].as_u64())
            .ok_or_else(|| {
                Fault::invalid("Either nodeId, backendNodeId or objectId must be specified")
            })?;
        if node_id == DOCUMENT_NODE {
            return Ok(DOCUMENT_NODE as usize);
        }
        mars_id(node_id).ok_or_else(|| Fault::server(NODE_NOT_FOUND))
    }

    /** DOM.getDocument. */
    fn get_document(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let view = self.view(&tab)?;
        let depth = params["depth"].as_i64().unwrap_or(DEFAULT_DEPTH);
        Ok(json!({"root":view.node(DOCUMENT_NODE as usize, depth)}))
    }

    /** Nodes matching a selector under a node, from V9's selector engine. */
    fn select(&mut self, tab: &str, params: &Value, all: bool) -> Result<Vec<u64>, Fault> {
        let mut scope = self.target_node(tab, params)?;
        if scope as u64 == DOCUMENT_NODE {
            // The document's own identity in V9 differs from the protocol's.
            scope = self.view(tab)?.page["virtual"]["document"]
                .as_u64()
                .unwrap_or_default() as usize;
        }
        let selector = params["selector"].as_str().unwrap_or_default();
        let found = self.devtools(
            tab,
            json!({"kind":"query_selector","node":scope,"selector":selector,"all":all}),
        )?;
        if let Some(error) = found.get("error").and_then(Value::as_str) {
            return Err(Fault::server(error));
        }
        Ok(found["nodes"]
            .as_array()
            .map(|nodes| nodes.iter().filter_map(Value::as_u64).map(dom_id).collect())
            .unwrap_or_default())
    }

    /** DOM.querySelector. */
    fn query_selector(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let found = self.select(&tab, params, false)?;
        Ok(json!({"nodeId":found.first().copied().unwrap_or(0)}))
    }

    /** DOM.querySelectorAll. */
    fn query_selector_all(
        &mut self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        Ok(json!({"nodeIds":self.select(&tab, params, true)?}))
    }

    /** DOM.describeNode. */
    fn describe_node(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let view = self.view(&tab)?;
        if node as u64 != DOCUMENT_NODE && node < (1 << 31) && !view.by_id.contains_key(&node) {
            return Err(Fault::server(NODE_NOT_FOUND));
        }
        Ok(json!({"node":view.node(node, params["depth"].as_i64().unwrap_or(0))}))
    }

    /** The node's quad, for box models and content quads. */
    fn node_quad(&mut self, tab: &str, params: &Value) -> Result<(Vec<f64>, f64, f64), Fault> {
        let node = self.target_node(tab, params)?;
        let view = self.view(tab)?;
        view.quad(node)
            .ok_or_else(|| Fault::server("Could not compute box model."))
    }

    /** DOM.getBoxModel: content, padding, border and margin share the painted box. */
    fn box_model(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let (quad, width, height) = self.node_quad(&tab, params)?;
        Ok(
            json!({"model":{"content":quad,"padding":quad,"border":quad,"margin":quad,"width":width,"height":height}}),
        )
    }

    /** DOM.getContentQuads. */
    fn content_quads(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let (quad, _, _) = self
            .node_quad(&tab, params)
            .map_err(|_| Fault::server("Could not compute content quads."))?;
        Ok(json!({"quads":[quad]}))
    }

    /** DOM.resolveNode: the node's wrapper as a remote object. */
    fn resolve_node(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        if params["nodeId"].is_null() == params["backendNodeId"].is_null() {
            return Err(Fault::server(
                "Either nodeId or backendNodeId must be specified.",
            ));
        }
        let node = self.target_node(&tab, params)?;
        if node as u64 == DOCUMENT_NODE {
            return Err(Fault::server(
                "Node with given id does not belong to the document",
            ));
        }
        let context = params["executionContextId"]
            .as_u64()
            .unwrap_or_else(|| self.targets.get(&tab).map_or(0, |target| target.context));
        let group = params["objectGroup"].as_str().unwrap_or_default();
        let mut result = self.devtools(
            &tab,
            json!({"kind":"resolve_node","node":node,"options":{"group":group}}),
        )?;
        let mut object = json!({"object":result["result"].take()});
        self.adopt_mirror(&mut object, context, &tab);
        Ok(object)
    }

    /** DOM.requestNode: the node behind a remote object. */
    fn request_node(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        Ok(json!({"nodeId":dom_id(node as u64)}))
    }

    /** DOM.scrollIntoViewIfNeeded. */
    fn scroll_into_view(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        self.mutate_revision(&tab, "page.scroll_into_view", json!({"node":node}))?;
        Ok(json!({}))
    }

    /** DOM.getOuterHTML. */
    fn outer_html(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let view = self.view(&tab)?;
        let target = if node as u64 == DOCUMENT_NODE {
            view.html
        } else {
            node
        };
        let html = self.call_tab(&tab, "page.outer_html", json!({"node":target}))?;
        Ok(json!({"outerHTML":html["outer_html"]}))
    }

    /** DOM.getAttributes. */
    fn attributes(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let view = self.view(&tab)?;
        let described = view.node(node, 0);
        if described["nodeType"] != ELEMENT_NODE {
            return Err(Fault::server("Node is not an Element"));
        }
        Ok(json!({"attributes":described["attributes"]}))
    }

    /** DOM.setAttributeValue. */
    fn set_attribute(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let name = params["name"]
            .as_str()
            .ok_or_else(|| Fault::invalid("name is required."))?;
        self.mutate_revision(
            &tab,
            "page.set_attribute",
            json!({"node":node,"name":name,"value":params["value"]}),
        )?;
        Ok(json!({}))
    }

    /** DOM.setAttributesAsText: `name="value"` pairs set one by one. */
    fn set_attributes_as_text(
        &mut self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let text = params["text"].as_str().unwrap_or_default();
        for pair in text.split_whitespace() {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = value.trim_matches(|c| c == '"' || c == '\'');
            self.mutate_revision(
                &tab,
                "page.set_attribute",
                json!({"node":node,"name":name,"value":value}),
            )?;
        }
        Ok(json!({}))
    }

    /** DOM.removeAttribute. */
    fn remove_attribute(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let name = params["name"]
            .as_str()
            .ok_or_else(|| Fault::invalid("name is required."))?;
        self.mutate_revision(
            &tab,
            "page.set_attribute",
            json!({"node":node,"name":name,"value":null}),
        )?;
        Ok(json!({}))
    }

    /** DOM.focus: later key events and insertText go to this node. */
    fn focus(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let node = self.target_node(&tab, params)?;
        let view = self.view(&tab)?;
        if !view.by_id.contains_key(&node) {
            return Err(Fault::server("Element is not focusable"));
        }
        if let Some(target) = self.targets.get_mut(&tab) {
            target.focused = Some(node);
        }
        Ok(json!({}))
    }

    /** DOM.getNodeForLocation: the painted node under a viewport point. */
    fn node_for_location(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let (x, y) = (
            params["x"].as_f64().unwrap_or_default(),
            params["y"].as_f64().unwrap_or_default(),
        );
        let hit = self.call_tab(&tab, "page.hover", json!({"x":x,"y":y}))?;
        let node = hit["hit"]["node_id"]
            .as_str()
            .and_then(super::input::node_number)
            .ok_or_else(|| Fault::server("No node found at given location"))?;
        let id = dom_id(node as u64);
        Ok(json!({"backendNodeId":id,"nodeId":id,"frameId":tab}))
    }

    /** Accessibility.getFullAXTree from Mars's roles and names. */
    fn full_ax_tree(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        Ok(json!({"nodes":self.ax_nodes(&tab)?}))
    }

    /** Accessibility.getRootAXNode. */
    fn root_ax_node(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.ax_nodes(&tab)?;
        Ok(json!({"node":nodes.first().cloned().unwrap_or(Value::Null)}))
    }

    /** Accessibility.getChildAXNodes. */
    fn child_ax_nodes(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.ax_nodes(&tab)?;
        let id = params["id"].as_str().unwrap_or_default();
        let parent = nodes
            .iter()
            .find(|node| node["nodeId"] == id)
            .ok_or_else(|| Fault::server("Node not found"))?;
        let children: Vec<Value> = parent["childIds"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|child| nodes.iter().find(|node| node["nodeId"] == *child).cloned())
                    .collect()
            })
            .unwrap_or_default();
        Ok(json!({"nodes":children}))
    }

    /** The accessibility node of a DOM node, by nodeId, backendNodeId or objectId. */
    fn ax_node_for(&mut self, tab: &str, params: &Value, nodes: &[Value]) -> Result<Value, Fault> {
        let mars = self.target_node(tab, params)?;
        let wanted = if mars as u64 == DOCUMENT_NODE {
            "root".to_string()
        } else {
            format!("ax-{mars}")
        };
        nodes
            .iter()
            .find(|node| node["nodeId"] == wanted)
            .cloned()
            .ok_or_else(|| {
                Fault::server("Node has no accessibility node; it is generic or ignored")
            })
    }

    /** Accessibility.getAXNodeAndAncestors. */
    fn ax_node_and_ancestors(
        &mut self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.ax_nodes(&tab)?;
        let mut chain = vec![self.ax_node_for(&tab, params, &nodes)?];
        while let Some(parent) = chain
            .last()
            .and_then(|node| node["parentId"].as_str())
            .map(str::to_owned)
        {
            let Some(next) = nodes.iter().find(|node| node["nodeId"] == parent).cloned() else {
                break;
            };
            chain.push(next);
        }
        Ok(json!({"nodes":chain}))
    }

    /** Accessibility.getPartialAXTree: the node, its ancestors and its children. */
    fn partial_ax_tree(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.ax_nodes(&tab)?;
        let node = self.ax_node_for(&tab, params, &nodes)?;
        let mut out = vec![node.clone()];
        if params["fetchRelatives"].as_bool().unwrap_or(true) {
            out.extend(
                node["childIds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|child| {
                        nodes
                            .iter()
                            .find(|candidate| candidate["nodeId"] == *child)
                            .cloned()
                    }),
            );
            let mut parent = node["parentId"].as_str().map(str::to_owned);
            while let Some(id) = parent {
                let Some(next) = nodes
                    .iter()
                    .find(|candidate| candidate["nodeId"] == id)
                    .cloned()
                else {
                    break;
                };
                parent = next["parentId"].as_str().map(str::to_owned);
                out.push(next);
            }
        }
        Ok(json!({"nodes":out}))
    }

    /** Accessibility.queryAXTree: descendants with the given role and accessible name. */
    fn query_ax_tree(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let nodes = self.ax_nodes(&tab)?;
        let root = if params["nodeId"].is_null()
            && params["backendNodeId"].is_null()
            && params["objectId"].is_null()
        {
            nodes.first().cloned().unwrap_or(Value::Null)
        } else {
            self.ax_node_for(&tab, params, &nodes)?
        };
        let mut pending: Vec<String> = root["childIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| id.as_str().map(str::to_owned))
            .collect();
        let mut found = Vec::new();
        while let Some(id) = pending.pop() {
            let Some(node) = nodes.iter().find(|candidate| candidate["nodeId"] == id) else {
                continue;
            };
            pending.extend(
                node["childIds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|child| child.as_str().map(str::to_owned)),
            );
            let role_ok = params["role"]
                .as_str()
                .is_none_or(|role| node["role"]["value"] == role);
            let name_ok = params["accessibleName"]
                .as_str()
                .is_none_or(|name| node["name"]["value"] == name);
            if role_ok && name_ok {
                found.push(node.clone());
            }
        }
        Ok(json!({"nodes":found}))
    }

    /** Every accessibility node of the page, root first. */
    fn ax_nodes(&mut self, tab: &str) -> Result<Vec<Value>, Fault> {
        let view = self.view(tab)?;
        let inspection = self.call_tab(
            tab,
            "page.inspect",
            json!({"fields":["role","name","tag","enabled","checked","value","protected"]}),
        )?;
        let mut roles: BTreeMap<usize, Value> = BTreeMap::new();
        for node in inspection["nodes"].as_array().cloned().unwrap_or_default() {
            if let Some(id) = node["node_id"].as_str().and_then(super::input::node_number) {
                roles.insert(id, node);
            }
        }
        let mut nodes = vec![
            json!({"nodeId":"root","ignored":false,"role":{"type":"role","value":"RootWebArea"},"name":{"type":"computedString","value":view.page["title"]},"properties":[],"childIds":view.roots.iter().filter_map(|root| ax_child_ids(&view, &roles, *root)).flatten().collect::<Vec<_>>(),"backendDOMNodeId":DOCUMENT_NODE,"frameId":tab}),
        ];
        for (id, node) in &roles {
            let role = node["role"].as_str().unwrap_or_default();
            if GENERIC_ROLES.contains(&role) {
                continue;
            }
            let mut properties = Vec::new();
            if node["enabled"] == false {
                properties.push(json!({"name":"disabled","value":{"type":"boolean","value":true}}));
            }
            if node["checked"] == true {
                properties
                    .push(json!({"name":"checked","value":{"type":"tristate","value":"true"}}));
            }
            if let Some(value) = node["value"].as_str().filter(|value| !value.is_empty()) {
                properties.push(json!({"name":"value","value":{"type":"string","value":value}}));
            }
            nodes.push(json!({"nodeId":format!("ax-{id}"),"ignored":false,"role":{"type":"role","value":role},"name":{"type":"computedString","value":node["name"].as_str().unwrap_or_default()},"properties":properties,"childIds":ax_child_ids(&view, &roles, *id).unwrap_or_default(),"backendDOMNodeId":dom_id(*id as u64),"parentId":ax_parent(&view, &roles, *id)}));
        }
        Ok(nodes)
    }

    /** DOMSnapshot.captureSnapshot: one document from the node view. */
    fn capture_snapshot(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let view = self.view(&tab)?;
        let want_rects = params["includeDOMRects"].as_bool().unwrap_or(false);
        let mut strings = Strings::default();
        let mut parent_index = Vec::new();
        let mut node_type = Vec::new();
        let mut node_name = Vec::new();
        let mut node_value = Vec::new();
        let mut backend = Vec::new();
        let mut attributes = Vec::new();
        let mut layout_index = Vec::new();
        let mut bounds = Vec::new();
        let mut text = Vec::new();
        let mut index_of: BTreeMap<usize, i64> = BTreeMap::new();
        let mut push = |strings: &mut Strings,
                        parent: i64,
                        kind: u64,
                        name: &str,
                        value: &str,
                        id: u64,
                        attrs: Vec<i64>|
         -> i64 {
            let index = node_type.len() as i64;
            parent_index.push(parent);
            node_type.push(kind);
            node_name.push(strings.add(name));
            node_value.push(strings.add(value));
            backend.push(id);
            attributes.push(attrs);
            index
        };
        let document = push(
            &mut strings,
            -1,
            DOCUMENT_TYPE,
            "#document",
            "",
            DOCUMENT_NODE,
            Vec::new(),
        );
        let html = push(
            &mut strings,
            document,
            ELEMENT_NODE,
            "HTML",
            "",
            view.html as u64,
            Vec::new(),
        );
        let head = push(
            &mut strings,
            html,
            ELEMENT_NODE,
            "HEAD",
            "",
            view.head as u64,
            Vec::new(),
        );
        for element in view.head_nodes() {
            let tag = element["tag"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_uppercase();
            let attrs = attribute_indexes(&mut strings, &element["attributes"]);
            push(
                &mut strings,
                head,
                ELEMENT_NODE,
                &tag,
                "",
                element["id"].as_u64().unwrap_or_default(),
                attrs,
            );
        }
        let body = push(
            &mut strings,
            html,
            ELEMENT_NODE,
            "BODY",
            "",
            view.body as u64,
            Vec::new(),
        );
        for (id, node) in &view.by_id {
            let parent = node["parent"]
                .as_u64()
                .and_then(|parent| index_of.get(&(parent as usize)).copied())
                .unwrap_or(body);
            let tag = node["tag"].as_str().unwrap_or_default();
            let (kind, name) = if tag == "#text" {
                (TEXT_NODE, "#text".to_string())
            } else if tag == "#comment" {
                (COMMENT_NODE, "#comment".to_string())
            } else {
                (ELEMENT_NODE, tag.to_ascii_uppercase())
            };
            let value = if kind == ELEMENT_NODE {
                ""
            } else {
                node["text"].as_str().unwrap_or_default()
            };
            let attrs = attribute_indexes(&mut strings, &node["attributes"]);
            let index = push(
                &mut strings,
                parent,
                kind,
                &name,
                value,
                *id as u64 + NODE_OFFSET,
                attrs,
            );
            index_of.insert(*id, index);
            if let Some(box_bounds) = node["bounds"].as_array() {
                layout_index.push(index);
                bounds.push(json!(box_bounds));
                text.push(strings.add(node["text"].as_str().unwrap_or_default()));
            }
        }
        let mut layout = json!({"nodeIndex":layout_index,"styles":layout_index.iter().map(|_| json!([])).collect::<Vec<_>>(),"bounds":bounds,"text":text,"stackingContexts":{"index":[]}});
        if want_rects {
            layout["offsetRects"] = layout["bounds"].clone();
            layout["clientRects"] = layout["bounds"].clone();
            layout["scrollRects"] = layout["bounds"].clone();
        }
        let document = json!({
            "documentURL": strings.add(view.page["url"].as_str().unwrap_or_default()),
            "title": strings.add(view.page["title"].as_str().unwrap_or_default()),
            "baseURL": strings.add(view.page["url"].as_str().unwrap_or_default()),
            "contentLanguage": strings.add(""), "encodingName": strings.add("UTF-8"), "publicId": strings.add(""), "systemId": strings.add(""),
            "frameId": strings.add(&tab),
            "nodes": {"parentIndex":parent_index,"nodeType":node_type,"nodeName":node_name,"nodeValue":node_value,"backendNodeId":backend,"attributes":attributes},
            "layout": layout, "textBoxes": {"layoutIndex":[],"bounds":[],"start":[],"length":[]},
            "scrollOffsetX": 0, "scrollOffsetY": view.scroll_y,
            "contentWidth": view.page["viewport"]["width"], "contentHeight": view.page["viewport"]["height"],
        });
        Ok(json!({"documents":[document],"strings":strings.table}))
    }
}

/** DOMSnapshot's string table. */
#[derive(Default)]
struct Strings {
    table: Vec<String>,
    index: BTreeMap<String, i64>,
}

impl Strings {
    /** The index of a string, added once. */
    fn add(&mut self, text: &str) -> i64 {
        if let Some(index) = self.index.get(text) {
            return *index;
        }
        self.table.push(text.to_owned());
        let index = self.table.len() as i64 - 1;
        self.index.insert(text.to_owned(), index);
        index
    }
}

/** Attribute name/value indexes, flat as Blink's. */
fn attribute_indexes(strings: &mut Strings, attributes: &Value) -> Vec<i64> {
    attributes
        .as_object()
        .map(|attributes| {
            attributes
                .iter()
                .flat_map(|(name, value)| {
                    [
                        strings.add(name),
                        strings.add(value.as_str().unwrap_or_default()),
                    ]
                })
                .collect()
        })
        .unwrap_or_default()
}

/** The DOM node id of a Mars node. */
fn dom_id(node: u64) -> u64 {
    if node >= (1 << 31) {
        node
    } else {
        node + NODE_OFFSET
    }
}

/** Accessibility children: the nearest descendants with a role of their own. */
fn ax_child_ids(view: &View, roles: &BTreeMap<usize, Value>, id: usize) -> Option<Vec<String>> {
    let mut ids = Vec::new();
    for child in view.children.get(&id)? {
        let role = roles
            .get(child)
            .and_then(|node| node["role"].as_str())
            .unwrap_or_default();
        if GENERIC_ROLES.contains(&role) {
            ids.extend(ax_child_ids(view, roles, *child).unwrap_or_default());
        } else {
            ids.push(format!("ax-{child}"));
        }
    }
    Some(ids)
}

/** The nearest ancestor with a role of its own, else the root. */
fn ax_parent(view: &View, roles: &BTreeMap<usize, Value>, id: usize) -> String {
    let mut current = view
        .by_id
        .get(&id)
        .and_then(|node| node["parent"].as_u64())
        .map(|parent| parent as usize);
    while let Some(parent) = current {
        let role = roles
            .get(&parent)
            .and_then(|node| node["role"].as_str())
            .unwrap_or_default();
        if !GENERIC_ROLES.contains(&role) {
            return format!("ax-{parent}");
        }
        current = view
            .by_id
            .get(&parent)
            .and_then(|node| node["parent"].as_u64())
            .map(|parent| parent as usize);
    }
    "root".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Node ids map both ways and keep the document, virtual and text children apart. */
    #[test]
    fn node_ids_round_trip() {
        assert_eq!(mars_id(DOCUMENT_NODE), None);
        assert_eq!(mars_id(dom_id(7)), Some(7));
        assert_eq!(mars_id(TEXT_NODE_BASE + 7), Some(7));
        assert_eq!(mars_id(1 << 31), Some(1 << 31));
    }
}
