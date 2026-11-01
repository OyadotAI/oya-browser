/**
 * A small HTML reader and writer for nodes scripts build outside the document. Connected
 * insertions always go through the app's own fragment parser; this one only lets detached
 * subtrees (innerHTML on a fresh element, `$("<div>...</div>")`) keep their structure and text.
 */
use super::Page;
use super::nodes::NodeView;
use crate::constants::{DOCUMENT_DEPTH, FRAGMENT_BYTES};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeMap;

/** Elements with no end tag. */
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
/** Elements whose content is raw text (WHATWG RAWTEXT, script data and PLAINTEXT): no tags
 * parse and no references decode, so `<xmp>&amp;</xmp>` keeps its literal `&amp;`. */
pub(super) const RAW: [&str; 7] = [
    "script",
    "style",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "plaintext",
];
/** Elements whose content is RCDATA (WHATWG): no tags parse, but references decode. */
const RCDATA: [&str; 2] = ["textarea", "title"];

/** Escape text content. */
fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            c => out.push(c),
        }
    }
}

/** Escape an attribute value. */
pub(super) fn escape_attribute(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            c => out.push(c),
        }
    }
}

/** Serialize a node (outer) or its children (inner), as scripts read it. */
pub(in crate::v9) fn serialize(page: &mut Page, id: usize, outer: bool) -> String {
    let mut out = String::new();
    if outer {
        write_node(page, id, &mut out, 0, false);
    } else {
        let children = inner_children(page, id);
        for child in children {
            write_node(page, child, &mut out, 0, false);
        }
    }
    out
}

/**
 * Serialize a subtree for the document engine: shadow roots are written as declarative
 * `<template shadowrootmode>` content, which the fragment parser turns back into the same
 * `#shadow-root` node, so a moved host keeps its shadow tree.
 */
pub(in crate::v9) fn serialize_all(page: &mut Page, id: usize) -> String {
    let mut out = String::new();
    write_node(page, id, &mut out, 0, true);
    out
}

/** Children a node serializes: a template's are its content's. */
fn inner_children(page: &mut Page, id: usize) -> Vec<usize> {
    match page.state.components.contents.get(&id).copied() {
        Some(content) if page.tag(id) == "template" => page.children(content),
        _ => page.children(id),
    }
}

/** Write one node; `shadow` includes shadow roots. */
fn write_node(page: &mut Page, id: usize, out: &mut String, depth: usize, shadow: bool) {
    let Some(view) = page.view(id) else {
        return;
    };
    match view.tag.as_str() {
        "#text" => {
            let parent_raw = page
                .parent(id)
                .map(|parent| page.tag(parent))
                .is_some_and(|tag| RAW.contains(&tag.as_str()));
            let text = view.text.unwrap_or_default();
            if parent_raw {
                out.push_str(&text);
            } else {
                escape_text(&text, out);
            }
        }
        super::frames::DOCTYPE => {
            out.push_str("<!DOCTYPE ");
            out.push_str(view.attributes.get("name").map_or("", String::as_str));
            out.push('>');
        }
        "#comment" => {
            out.push_str("<!--");
            out.push_str(view.text.as_deref().unwrap_or(""));
            out.push_str("-->");
        }
        super::components::SHADOW_ROOT => {
            let mode = if view
                .attributes
                .get("mode")
                .is_some_and(|mode| mode == "closed")
            {
                "closed"
            } else {
                "open"
            };
            out.push_str("<template shadowrootmode=\"");
            out.push_str(mode);
            out.push_str("\">");
            for child in page.children(id) {
                write_node(page, child, out, depth + 1, shadow);
            }
            out.push_str("</template>");
        }
        tag => {
            let qualified = page.qualified_element_name(id);
            write_open(&qualified, &view, out);
            if VOID.contains(&tag) || depth > DOCUMENT_DEPTH * 4 {
                return;
            }
            let children = if shadow {
                let mut children: Vec<usize> = page
                    .all_children(id)
                    .into_iter()
                    .filter(|child| page.is_shadow_root(*child))
                    .collect();
                children.extend(inner_children(page, id));
                children
            } else {
                inner_children(page, id)
            };
            if children.is_empty()
                && let Some(text) = &view.text
            {
                escape_text(text, out);
            }
            for child in children {
                write_node(page, child, out, depth + 1, shadow);
            }
            out.push_str("</");
            out.push_str(&qualified);
            out.push('>');
        }
    }
}

/** Write a start tag. */
fn write_open(tag: &str, view: &NodeView, out: &mut String) {
    out.push('<');
    out.push_str(tag);
    for (name, value) in &view.attributes {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape_attribute(value, out);
        out.push('"');
    }
    out.push('>');
}

/** One parsed node, in preorder; `parent` indexes an earlier node. */
pub(in crate::v9) struct Parsed {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
    pub text: Option<String>,
    pub parent: Option<usize>,
}

/** The input-stream newline rule: CRLF and a lone CR both become LF before any token is read, so attribute values see LF too. */
fn normalize_newlines(html: &str) -> String {
    html.replace("\r\n", "\n").replace('\r', "\n")
}

/** Parse HTML into a preorder node list. */
pub(in crate::v9) fn parse_tree(html: &str) -> EngineResult<Vec<Parsed>> {
    if html.len() > FRAGMENT_BYTES * 4 {
        return Err(EngineError::limit("fragment bytes", FRAGMENT_BYTES * 4));
    }
    let mut nodes: Vec<Parsed> = Vec::new();
    let mut open: Vec<(usize, String)> = Vec::new();
    let normalized = normalize_newlines(html);
    let mut rest = normalized.as_str();
    let push = |nodes: &mut Vec<Parsed>,
                open: &[(usize, String)],
                tag: &str,
                attributes,
                text|
     -> EngineResult<usize> {
        if nodes.len() >= crate::constants::VM_DETACHED_NODES {
            return Err(EngineError::limit(
                "V9 detached nodes",
                crate::constants::VM_DETACHED_NODES,
            ));
        }
        nodes.push(Parsed {
            tag: tag.into(),
            attributes,
            text,
            parent: open.last().map(|(index, _)| *index),
        });
        Ok(nodes.len() - 1)
    };
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").unwrap_or(after.len());
            push(
                &mut nodes,
                &open,
                "#comment",
                BTreeMap::new(),
                Some(after[..end].into()),
            )?;
            rest = after.get(end + 3..).unwrap_or("");
            continue;
        }
        if rest.starts_with("</") {
            let end = rest.find('>').unwrap_or(rest.len());
            let name = rest[2..end].trim().to_ascii_lowercase();
            if let Some(position) = open.iter().rposition(|(_, tag)| *tag == name) {
                open.truncate(position);
            }
            rest = rest.get(end + 1..).unwrap_or("");
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            let end = rest.find('>').unwrap_or(rest.len());
            rest = rest.get(end + 1..).unwrap_or("");
            continue;
        }
        if rest.starts_with('<') && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            let (tag, attributes, closed, remaining) = open_tag(&rest[1..]);
            implied_close(&mut open, &tag);
            let index = push(&mut nodes, &open, &tag, attributes, None)?;
            rest = remaining;
            if RAW.contains(&tag.as_str()) || RCDATA.contains(&tag.as_str()) {
                let close = format!("</{tag}");
                let end = rest.to_ascii_lowercase().find(&close).unwrap_or(rest.len());
                if end > 0 {
                    let text = decode_entities(&rest[..end], !RAW.contains(&tag.as_str()));
                    nodes.push(Parsed {
                        tag: "#text".into(),
                        attributes: BTreeMap::new(),
                        text: Some(text),
                        parent: Some(index),
                    });
                }
                rest = rest
                    .get(end..)
                    .map(|after| after.find('>').map_or("", |i| &after[i + 1..]))
                    .unwrap_or("");
                continue;
            }
            if !closed && !VOID.contains(&tag.as_str()) {
                if open.len() >= DOCUMENT_DEPTH * 4 {
                    return Err(EngineError::limit("document depth", DOCUMENT_DEPTH * 4));
                }
                open.push((index, tag));
            }
            continue;
        }
        let skip = rest.chars().next().map_or(1, char::len_utf8);
        let end = rest[skip..].find('<').map_or(rest.len(), |i| i + skip);
        let text = decode_entities(&rest[..end], true);
        if !text.is_empty() {
            push(&mut nodes, &open, "#text", BTreeMap::new(), Some(text))?;
        }
        rest = &rest[end..];
    }
    Ok(nodes)
}

/** Parse HTML into new detached nodes; returns the top-level nodes in order. */
pub(in crate::v9) fn parse_detached(page: &mut Page, html: &str) -> EngineResult<Vec<usize>> {
    let parsed = parse_tree(html)?;
    let mut ids = Vec::with_capacity(parsed.len());
    let mut roots = Vec::new();
    for node in parsed {
        let id = page.create(&node.tag, node.text)?;
        if let Some(record) = page.state.detached.get_mut(&id) {
            record.attributes = node.attributes;
        }
        match node.parent {
            Some(parent) => {
                let parent = page.content_or_self(ids[parent])?;
                page.insert(parent, id, None)?;
            }
            None => roots.push(id),
        }
        ids.push(id);
    }
    Ok(roots)
}

/** Text of the text nodes SimpleFragmentParser would number from `first_id`. */
pub(in crate::v9) fn fragment_texts(
    html: &str,
    first_id: usize,
) -> EngineResult<Vec<(usize, String)>> {
    let parsed = parse_tree(html)?;
    let mut kept: Vec<bool> = Vec::with_capacity(parsed.len());
    let mut templates: Vec<bool> = Vec::with_capacity(parsed.len());
    let mut next = first_id;
    let mut out = Vec::new();
    for node in parsed {
        let inert = node.parent.is_some_and(|index| templates[index]);
        templates.push(node.tag == "template" && !node.attributes.contains_key("shadowrootmode"));
        let keep = !inert && node.parent.is_none_or(|index| kept[index]);
        kept.push(keep);
        if !keep {
            continue;
        }
        if node.tag == "#text" || node.tag == "#comment" {
            out.push((next, node.text.unwrap_or_default()));
        }
        next += 1;
    }
    Ok(out)
}

/// A standalone fragment parser built on V9's own HTML reader, for callers without Eye. The
/// Mars app passes Eye's parser instead, so both engines agree on structure.
pub struct SimpleFragmentParser;

impl crate::document::FragmentParser for SimpleFragmentParser {
    /** Preorder nodes numbered from `first_id`; comments stay, as Eye keeps inserted ones. */
    fn parse(
        &self,
        _context: &str,
        html: &str,
        first_id: usize,
    ) -> EngineResult<Vec<crate::document::ScriptNode>> {
        let parsed = parse_tree(html)?;
        let mut kept: Vec<Option<usize>> = Vec::with_capacity(parsed.len());
        let mut out = Vec::new();
        let mut templates: Vec<bool> = Vec::with_capacity(parsed.len());
        for mut node in parsed {
            let parent = node.parent.and_then(|index| kept[index]);
            let inert = node.parent.is_some_and(|index| templates[index]);
            let shadow = node.tag == "template"
                && node
                    .attributes
                    .get("shadowrootmode")
                    .is_some_and(|mode| mode == "open" || mode == "closed");
            templates.push(node.tag == "template" && !shadow);
            if inert || node.parent.is_some_and(|index| kept[index].is_none()) {
                kept.push(None);
                continue;
            }
            if shadow {
                let mode = node.attributes.remove("shadowrootmode").unwrap_or_default();
                node.tag = super::components::SHADOW_ROOT.into();
                node.attributes = BTreeMap::from([("mode".to_string(), mode)]);
            }
            let local_id = first_id + out.len();
            kept.push(Some(local_id));
            let html_id = node.attributes.get("id").cloned().unwrap_or_default();
            out.push(crate::document::ScriptNode {
                selected: None,
                local_id,
                html_id,
                parent,
                value: (node.tag == "input" || node.tag == "textarea")
                    .then(|| node.attributes.get("value").cloned().unwrap_or_default()),
                text_writable: false,
                click_listener: false,
                inline_handler: None,
                tag: node.tag,
                attributes: node.attributes,
            });
        }
        Ok(out)
    }
}

/** Close elements an opening tag implicitly ends (p before block, li before li, ...). */
fn implied_close(open: &mut Vec<(usize, String)>, tag: &str) {
    let closes = |current: &str| match current {
        "p" => matches!(
            tag,
            "p" | "div"
                | "ul"
                | "ol"
                | "table"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "section"
                | "article"
                | "header"
                | "footer"
                | "form"
                | "pre"
                | "blockquote"
                | "dl"
        ),
        "li" => tag == "li",
        "option" => matches!(tag, "option" | "optgroup"),
        "td" | "th" => matches!(tag, "td" | "th" | "tr"),
        "tr" => tag == "tr",
        "dt" | "dd" => matches!(tag, "dt" | "dd"),
        _ => false,
    };
    while open.last().is_some_and(|(_, current)| closes(current)) {
        open.pop();
    }
}

/** Read a start tag after `<`: name, attributes, self-closing flag, and the rest of the input. */
fn open_tag(input: &str) -> (String, BTreeMap<String, String>, bool, &str) {
    let name_end = input
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(input.len());
    let tag = input[..name_end].to_ascii_lowercase();
    let mut rest = &input[name_end..];
    let mut attributes = BTreeMap::new();
    let mut closed = false;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        if let Some(after) = rest.strip_prefix('>') {
            rest = after;
            break;
        }
        if let Some(after) = rest.strip_prefix("/>") {
            closed = true;
            rest = after;
            break;
        }
        if let Some(after) = rest.strip_prefix('/') {
            rest = after;
            continue;
        }
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '=' || c == '>' || c == '/')
            .unwrap_or(rest.len())
            .max(1);
        let name = rest[..name_end].to_ascii_lowercase();
        rest = rest[name_end..].trim_start();
        let value = if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            if let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let body = &after[1..];
                let end = body.find(quote).unwrap_or(body.len());
                rest = body.get(end + 1..).unwrap_or("");
                mars_core::entities::decode(&body[..end], true)
            } else {
                let end = after
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after.len());
                rest = &after[end..];
                mars_core::entities::decode(&after[..end], true)
            }
        } else {
            String::new()
        };
        attributes.entry(name).or_insert(value);
    }
    (tag, attributes, closed, rest)
}

/** Decode character references in text with the shared WHATWG decoder; raw text keeps them. */
pub(in crate::v9) fn decode_entities(text: &str, decode: bool) -> String {
    if decode {
        mars_core::entities::decode(text, false)
    } else {
        text.to_string()
    }
}
