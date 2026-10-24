/**
 * The DOM domain's view of a page, read from Eye's committed document and display: every node
 * with its parent, attributes, text and painted box, the head's metadata elements, and the
 * identities V9 gives the virtual html, head and body, so the front door's node ids agree with
 * the realm's. Attribute writes and scrolling go through the same revision-checked paths as
 * every other mutation. Secret field values never leave the runtime.
 */
use super::super::{page::Page, supervisor::Runtime};
use crate::constants::HTML_BYTES;
use crate::document::{Node, ScriptChange};
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Map, Value, json};

/** Elements with no closing tag in the HTML serialization. */
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/** page.outer_html names one node by its document-local identity. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    node: usize,
}

/** page.set_attribute: a revision-scoped attribute write. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetAttribute {
    page_id: String,
    expected_revision: u64,
    node: usize,
    name: String,
    #[serde(default)]
    value: Option<String>,
}

/** page.scroll_into_view: a revision-scoped scroll to one node. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollIntoView {
    page_id: String,
    expected_revision: u64,
    node: usize,
}

impl Runtime {
    /** page.nodes: the whole document as the DOM domain needs it. */
    pub(in crate::runtime) fn nodes(&mut self, params: Value) -> EngineResult<Value> {
        let _: super::super::page::Empty = parameters(params)?;
        let page = self.current()?;
        let mut bounds = std::collections::BTreeMap::new();
        for item in page.display.iter().flat_map(|display| &display.items) {
            bounds
                .entry(item.local_id)
                .or_insert_with(|| json!([item.x, item.y, item.width, item.height]));
        }
        let nodes: Vec<Value> = page
            .document
            .nodes
            .iter()
            .map(|node| describe(node, bounds.get(&node.local_id)))
            .collect();
        let virtual_nodes = crate::v9::devtools::virtual_nodes();
        let head: Vec<Value> = page
            .document
            .metadata
            .iter()
            .enumerate()
            .map(|(index, element)| {
                json!({"id":virtual_nodes.detached_base + index,"tag":element.tag,"attributes":element.attributes,"text":element.text})
            })
            .collect();
        Ok(json!({
            "page_id": page.id(), "revision": page.revision, "url": page.url, "title": page.document.title,
            "nodes": nodes, "head": head,
            "virtual": {"document": virtual_nodes.document, "html": virtual_nodes.html, "head": virtual_nodes.head, "body": virtual_nodes.body},
            "html_attributes": page.document.formatting.html.attributes,
            "body_attributes": page.document.formatting.body.attributes,
            "viewport": {"width": self.render_context.width, "height": self.render_context.height, "scroll_y": self.scroll_y},
            "document_height": page.display.as_ref().map_or(0.0, |display| display.height),
        }))
    }

    /** page.outer_html: the node's subtree serialized as HTML, bounded like a document. */
    pub(in crate::runtime) fn outer_html(&mut self, params: Value) -> EngineResult<Value> {
        let input: Target = parameters(params)?;
        let page = self.current()?;
        let virtual_nodes = crate::v9::devtools::virtual_nodes();
        let mut out = String::new();
        if input.node == virtual_nodes.html || input.node == virtual_nodes.document {
            serialize_document(page, &mut out);
        } else if input.node == virtual_nodes.body {
            serialize_body(page, &mut out);
        } else {
            let position = position(page, input.node).ok_or_else(unknown_node)?;
            serialize_node(page, position, &mut out);
        }
        if out.len() > HTML_BYTES {
            return Err(EngineError::limit("outer HTML bytes", HTML_BYTES));
        }
        Ok(json!({"page_id":page.id(),"revision":page.revision,"outer_html":out}))
    }

    /** page.set_attribute, through Eye's attribute change and the ordinary mutation commit. */
    pub(in crate::runtime) fn set_attribute(&mut self, params: Value) -> EngineResult<Value> {
        let input: SetAttribute = parameters(params)?;
        let mut page = self.current()?.clone();
        check_revision(&page, &input.page_id, input.expected_revision)?;
        position(&page, input.node).ok_or_else(unknown_node)?;
        let change = ScriptChange::Attribute {
            node: input.node,
            name: input.name.to_ascii_lowercase(),
            value: input.value,
        };
        page.document = self.apply_all(&page.document, &[], &[change])?;
        page.styled = false;
        let result = self.finish_mutation(page)?;
        Ok(json!({"page_id":result["page_id"],"revision":result["revision"]}))
    }

    /** page.scroll_into_view: scroll only when the node's box is not already in the viewport. */
    pub(in crate::runtime) fn scroll_into_view(&mut self, params: Value) -> EngineResult<Value> {
        let input: ScrollIntoView = parameters(params)?;
        let page = self.current()?;
        check_revision(page, &input.page_id, input.expected_revision)?;
        let item = page
            .display
            .as_ref()
            .and_then(|display| {
                display
                    .items
                    .iter()
                    .find(|item| item.local_id == input.node)
            })
            .ok_or_else(|| {
                EngineError::new(
                    "invalid_target",
                    "Node does not have a layout object",
                    "Query the current page and use a painted node.",
                )
            })?;
        let (top, height) = (self.scroll_y, self.render_context.height);
        let visible = item.y >= top && item.y + item.height <= top + height;
        if visible {
            return Ok(json!({"page_id":page.id(),"revision":page.revision,"scroll_y":top}));
        }
        let y = (item.y - (height - item.height) / 2.0).max(0.0);
        let scrolled = self.scroll(
            json!({"page_id":input.page_id,"expected_revision":input.expected_revision,"y":y}),
        )?;
        Ok(
            json!({"page_id":scrolled["page_id"],"revision":scrolled["revision"],"scroll_y":self.scroll_y}),
        )
    }
}

/** The page and revision a client observed must still be current. */
fn check_revision(page: &Page, page_id: &str, revision: u64) -> EngineResult<()> {
    if page.id() != page_id {
        return Err(unknown_node());
    }
    if page.revision != revision {
        return Err(EngineError::new(
            "revision_conflict",
            "The document revision has changed.",
            "Inspect the current page before changing it.",
        ));
    }
    Ok(())
}

/** The node is not in this document. */
fn unknown_node() -> EngineError {
    EngineError::new(
        "invalid_target",
        "Could not find node with given id",
        "Read the page's nodes again; the document may have changed.",
    )
}

/** Index of a node by identity. */
fn position(page: &Page, node: usize) -> Option<usize> {
    mars_core::dom::local_position(&page.document.nodes, node, |node| node.local_id)
}

/** One node as the DOM domain reads it; a secret field's value and value attribute stay out. */
fn describe(node: &Node, bounds: Option<&Value>) -> Value {
    let mut attributes: Map<String, Value> = node
        .attributes
        .iter()
        .map(|(name, value)| (name.clone(), json!(value)))
        .collect();
    if node.secret() {
        attributes.remove("value");
    }
    let mut value = json!({"id":node.local_id,"parent":node.parent,"tag":node.tag,"attributes":attributes,"text":node.text});
    if !node.secret() && ["input", "textarea", "select", "option"].contains(&node.tag.as_str()) {
        value["value"] = json!(node.value);
    }
    if ["input"].contains(&node.tag.as_str()) {
        value["checked"] = json!(node.checked);
    }
    if node.tag == "option" {
        value["selected"] = json!(node.selected);
    }
    if node.disabled {
        value["disabled"] = json!(true);
    }
    if let Some(bounds) = bounds {
        value["bounds"] = bounds.clone();
    }
    value
}

/** `<html><head>…</head><body>…</body></html>` from the metadata and the projected roots. */
fn serialize_document(page: &Page, out: &mut String) {
    out.push_str("<html");
    attributes(&page.document.formatting.html.attributes, out);
    out.push_str("><head>");
    for element in &page.document.metadata {
        out.push('<');
        out.push_str(&element.tag);
        attributes(&element.attributes, out);
        out.push('>');
        if !VOID.contains(&element.tag.as_str()) {
            if element.tag == "style" || element.tag == "script" {
                out.push_str(&element.text);
            } else {
                escape(&element.text, out);
            }
            out.push_str(&format!("</{}>", element.tag));
        }
    }
    out.push_str("</head>");
    serialize_body(page, out);
    out.push_str("</html>");
}

/** The body element with every projected root inside it. */
fn serialize_body(page: &Page, out: &mut String) {
    out.push_str("<body");
    attributes(&page.document.formatting.body.attributes, out);
    out.push('>');
    for (index, node) in page.document.nodes.iter().enumerate() {
        if node.parent.is_none() {
            serialize_node(page, index, out);
        }
    }
    out.push_str("</body>");
}

/** One node and its subtree, children in document order. */
fn serialize_node(page: &Page, index: usize, out: &mut String) {
    let node = &page.document.nodes[index];
    if node.tag == "#text" {
        escape(&node.text, out);
        return;
    }
    if node.tag == "#comment" {
        out.push_str("<!--");
        out.push_str(&node.value);
        out.push_str("-->");
        return;
    }
    out.push('<');
    out.push_str(&node.tag);
    let mut attrs = node.attributes.clone();
    if node.secret() {
        attrs.remove("value");
    }
    attributes(&attrs, out);
    out.push('>');
    if VOID.contains(&node.tag.as_str()) {
        return;
    }
    escape(&node.text, out);
    for (child, candidate) in page.document.nodes.iter().enumerate().skip(index + 1) {
        if candidate.parent == Some(node.local_id) {
            serialize_node(page, child, out);
        }
    }
    out.push_str(&format!("</{}>", node.tag));
}

/** ` name="value"` for each attribute, quoted. */
fn attributes(attributes: &std::collections::BTreeMap<String, String>, out: &mut String) {
    for (name, value) in attributes {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        for character in value.chars() {
            match character {
                '&' => out.push_str("&amp;"),
                '"' => out.push_str("&quot;"),
                other => out.push(other),
            }
        }
        out.push('"');
    }
}

/** Text content escaping. */
fn escape(text: &str, out: &mut String) {
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Text and attributes are escaped; a password's value attribute never serializes. */
    #[test]
    fn serialization_escapes_and_hides_secrets() {
        let mut out = String::new();
        attributes(
            &[("title".to_string(), "a \"b\" & c".to_string())]
                .into_iter()
                .collect(),
            &mut out,
        );
        assert_eq!(out, " title=\"a &quot;b&quot; &amp; c\"");
        let mut text = String::new();
        escape("<b> & </b>", &mut text);
        assert_eq!(text, "&lt;b&gt; &amp; &lt;/b&gt;");
        let document =
            crate::document::parse(r#"<input type="password" value="hunter2">"#).unwrap();
        let node = document
            .nodes
            .iter()
            .find(|node| node.tag == "input")
            .unwrap();
        let described = describe(node, None);
        assert!(described["attributes"].get("value").is_none());
        assert!(described.get("value").is_none());
    }
}
