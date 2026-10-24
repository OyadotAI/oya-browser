/**
 * Shadow trees in Eye. A shadow root is a `#shadow-root` child of its host (from declarative
 * `<template shadowrootmode>` markup or a script's attachShadow). Layout, paint and the cascade
 * run over the flat tree: a host's children are its shadow root's children, each `<slot>` holds
 * the light children assigned to it (or its fallback), and unassigned light children are not
 * rendered, so they are hidden from agents too. Styles inside a shadow root apply only within
 * it, plus `:host` and `::slotted()` rules, which are rewritten to match marked hosts and
 * slotted nodes in the flat tree.
 */
use super::{Document, Node};
use crate::constants::{DOCUMENT_DEPTH, NODES};
use std::collections::{BTreeMap, HashMap};

/// The tag of a shadow root node; no HTML tag can be spelled with `#`.
pub const SHADOW_ROOT: &str = "#shadow-root";
/** Flat-tree attribute listing the style groups a node's tree scope applies. */
pub(super) const SCOPE_ATTRIBUTE: &str = "mars-shadow-scope";
/** Flat-tree attribute listing the groups whose `:host` rules match a host. */
const HOST_ATTRIBUTE: &str = "mars-shadow-host";
/** Flat-tree attribute listing the groups whose `::slotted()` rules match a slotted node. */
const SLOTTED_ATTRIBUTE: &str = "mars-shadow-slotted";
/** The group of the document's own sheets. */
pub(super) const DOCUMENT_GROUP: usize = 0;
/** Rules anchored to marked hosts or slotted nodes apply in any scope. */
pub(super) const ANY_GROUP: usize = usize::MAX;

/// Whether a document holds any shadow root, so pages without components keep every old path.
pub fn has_shadow(document: &Document) -> bool {
    document.nodes.iter().any(|node| node.tag == SHADOW_ROOT)
}

/** Children of each node by Vec index, in order (top-level nodes under None). */
fn children(document: &Document) -> (HashMap<Option<usize>, Vec<usize>>, HashMap<usize, usize>) {
    let mut children: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    let mut index = HashMap::new();
    for (position, node) in document.nodes.iter().enumerate() {
        index.insert(node.local_id, position);
        children.entry(node.parent).or_default().push(position);
    }
    (children, index)
}

/** One shadow root: its host, its slots' assignments and its style groups. */
#[derive(Default)]
struct Root {
    host: usize,
    /** Light children (Vec indexes) each slot shows, by the slot's Vec index. */
    assigned: HashMap<usize, Vec<usize>>,
    /** The default slot, which also shows the host's own leading text. */
    default_slot: Option<usize>,
    /** Style groups (distinct sheet texts) inside this root. */
    groups: Vec<usize>,
}

/** The flat tree plus the scoped sheets the cascade adds. */
pub(super) struct Flat {
    pub document: Document,
    /** Rewritten scoped sheets and the group each applies to. */
    pub sheets: Vec<(String, usize)>,
}

/** Builds the flat tree in preorder. */
struct Flattener<'d> {
    document: &'d Document,
    children: HashMap<Option<usize>, Vec<usize>>,
    roots: HashMap<usize, Root>,
    /** Root Vec index by host Vec index. */
    hosts: HashMap<usize, usize>,
    /** Vec index by local identity. */
    index: HashMap<usize, usize>,
    out: Vec<Node>,
    marks: bool,
}

/// The flat tree for layout and paint, or None when the document has no shadow root.
pub fn flatten(document: &Document) -> Option<Document> {
    flat(document, false).map(|flat| flat.document)
}

/** The flat tree; `marks` adds the scope attributes and scoped sheets the cascade needs. */
pub(super) fn flat(document: &Document, marks: bool) -> Option<Flat> {
    if !has_shadow(document) {
        return None;
    }
    let (children, index) = children(document);
    let mut flattener = Flattener {
        document,
        children,
        roots: HashMap::new(),
        hosts: HashMap::new(),
        index,
        out: Vec::with_capacity(document.nodes.len()),
        marks,
    };
    let sheets = flattener.collect_roots();
    let top = flattener.children.get(&None).cloned().unwrap_or_default();
    for node in top {
        flattener.emit(node, None, 0, &[]);
    }
    let mut flat = document.clone();
    flat.nodes = flattener.out;
    Some(Flat {
        document: flat,
        sheets,
    })
}

impl Flattener<'_> {
    /** Find every root, assign its slots and number its distinct style texts. */
    fn collect_roots(&mut self) -> Vec<(String, usize)> {
        let mut groups: BTreeMap<String, usize> = BTreeMap::new();
        let mut sheets = Vec::new();
        for (position, node) in self.document.nodes.iter().enumerate() {
            if node.tag != SHADOW_ROOT {
                continue;
            }
            let Some(host) = node.parent.and_then(|id| self.index.get(&id).copied()) else {
                continue;
            };
            if self.hosts.contains_key(&host) {
                continue;
            }
            let mut root = Root {
                host,
                ..Root::default()
            };
            let scope = self.scope_nodes(position);
            self.assign(&mut root, &scope);
            for node in &scope {
                let style = &self.document.nodes[*node];
                if style.tag == "style" && !style.text.is_empty() {
                    let next = groups.len() + 1;
                    let group = *groups.entry(style.text.clone()).or_insert_with(|| {
                        sheets.extend(rewrite(&style.text, next));
                        next
                    });
                    if !root.groups.contains(&group) {
                        root.groups.push(group);
                    }
                }
            }
            self.hosts.insert(host, position);
            self.roots.insert(position, root);
        }
        sheets
    }
    /** A root's tree: its descendants, not entering nested shadow roots. */
    fn scope_nodes(&self, root: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.kids(root).into_iter().rev().collect();
        while let Some(next) = stack.pop() {
            if self.document.nodes[next].tag == SHADOW_ROOT || out.len() > NODES {
                continue;
            }
            out.push(next);
            stack.extend(self.kids(next).into_iter().rev());
        }
        out
    }
    /** Children by Vec index. */
    fn kids(&self, position: usize) -> Vec<usize> {
        let id = self.document.nodes[position].local_id;
        self.children.get(&Some(id)).cloned().unwrap_or_default()
    }
    /** Named slotting: each light child goes to the first slot with its `slot` name. */
    fn assign(&self, root: &mut Root, scope: &[usize]) {
        let mut first: BTreeMap<String, usize> = BTreeMap::new();
        for node in scope {
            let slot = &self.document.nodes[*node];
            if slot.tag == "slot" {
                let name = slot.attributes.get("name").cloned().unwrap_or_default();
                first.entry(name).or_insert(*node);
            }
        }
        root.default_slot = first.get("").copied();
        for child in self.kids(root.host) {
            let node = &self.document.nodes[child];
            if node.tag == SHADOW_ROOT {
                continue;
            }
            let name = if node.tag == "#text" {
                String::new()
            } else {
                node.attributes.get("slot").cloned().unwrap_or_default()
            };
            if let Some(slot) = first.get(&name) {
                root.assigned.entry(*slot).or_default().push(child);
            }
        }
    }
    /** Emit one node and its flat children. `scopes` holds the roots entered, innermost last. */
    fn emit(&mut self, position: usize, parent: Option<usize>, depth: usize, scopes: &[usize]) {
        if depth >= DOCUMENT_DEPTH || self.out.len() >= NODES {
            return;
        }
        let source = &self.document.nodes[position];
        let mut node = source.clone();
        node.parent = parent;
        let id = node.local_id;
        let root = self.hosts.get(&position).copied();
        if root.is_some() {
            node.text.clear();
        }
        let slot = scopes
            .last()
            .and_then(|root| self.roots.get(root))
            .and_then(|root| {
                root.assigned
                    .get(&position)
                    .cloned()
                    .map(|nodes| (root, nodes))
            })
            .map(|(root, nodes)| (nodes, root.default_slot == Some(position), root.host));
        if let Some((_, true, host)) = &slot {
            node.text = self.document.nodes[*host].text.clone();
        }
        if self.marks {
            self.mark(&mut node, position, root, scopes);
        }
        self.out.push(node);
        if let Some(root) = root {
            let mut inner = scopes.to_vec();
            inner.push(root);
            for child in self.kids(root) {
                self.emit(child, Some(id), depth + 1, &inner);
            }
            return;
        }
        if let Some((assigned, _, _)) = slot {
            let outer = &scopes[..scopes.len() - 1];
            for child in assigned {
                self.emit(child, Some(id), depth + 1, outer);
            }
            return;
        }
        for child in self.kids(position) {
            if self.document.nodes[child].tag != SHADOW_ROOT {
                self.emit(child, Some(id), depth + 1, scopes);
            }
        }
    }
    /** Scope, host and slotted marks for the cascade's copy of the flat tree. */
    fn mark(&self, node: &mut Node, position: usize, root: Option<usize>, scopes: &[usize]) {
        node.attributes.remove(SCOPE_ATTRIBUTE);
        node.attributes.remove(HOST_ATTRIBUTE);
        node.attributes.remove(SLOTTED_ATTRIBUTE);
        let list = |groups: &[usize]| -> String {
            groups
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        };
        if let Some(root) = scopes.last().and_then(|root| self.roots.get(root)) {
            node.attributes
                .insert(SCOPE_ATTRIBUTE.into(), format!(" {} ", list(&root.groups)));
        }
        if let Some(root) = root.and_then(|root| self.roots.get(&root))
            && !root.groups.is_empty()
        {
            node.attributes
                .insert(HOST_ATTRIBUTE.into(), list(&root.groups));
        }
        let parent = self.document.nodes[position]
            .parent
            .and_then(|id| self.index.get(&id).copied());
        if let Some(root) = parent
            .and_then(|host| self.hosts.get(&host))
            .and_then(|root| self.roots.get(root))
            && root
                .assigned
                .values()
                .any(|nodes| nodes.contains(&position))
            && !root.groups.is_empty()
        {
            node.attributes
                .insert(SLOTTED_ATTRIBUTE.into(), list(&root.groups));
        }
    }
}

/// Whether a rule group applies to an element with these flat-tree attributes.
pub(super) fn applies(group: usize, attributes: &BTreeMap<String, String>) -> bool {
    if group == ANY_GROUP {
        return true;
    }
    match attributes.get(SCOPE_ATTRIBUTE) {
        None => group == DOCUMENT_GROUP,
        Some(groups) => group != DOCUMENT_GROUP && groups.contains(&format!(" {group} ")),
    }
}

/**
 * Split a shadow sheet into its inner rules (group `group`) and its `:host`/`::slotted()` rules
 * rewritten against the flat-tree marks (ANY_GROUP). Nested conditional blocks keep their
 * wrappers; keyframes and font faces stay with the inner sheet.
 */
fn rewrite(css: &str, group: usize) -> Vec<(String, usize)> {
    let (inner, outer) = split_block(css, group);
    let mut out = vec![(inner, group)];
    if !outer.trim().is_empty() {
        out.push((outer, ANY_GROUP));
    }
    out
}

/** Walk one block's rules. */
fn split_block(css: &str, group: usize) -> (String, String) {
    let mut inner = String::new();
    let mut outer = String::new();
    let mut rest = css;
    while let Some((prelude, body, after)) = next_rule(rest) {
        rest = after;
        let prelude = prelude.trim();
        if let Some(at) = prelude.strip_prefix('@') {
            let name = at
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("");
            let Some(body) = body else {
                continue;
            };
            if ["media", "supports", "layer", "container", "document"].contains(&name) {
                let (nested_inner, nested_outer) = split_block(body, group);
                inner.push_str(&format!("{prelude} {{{nested_inner}}}\n"));
                if !nested_outer.trim().is_empty() {
                    outer.push_str(&format!("{prelude} {{{nested_outer}}}\n"));
                }
            } else {
                inner.push_str(&format!("{prelude} {{{body}}}\n"));
            }
            continue;
        }
        let Some(body) = body else {
            continue;
        };
        let (mine, anchored): (Vec<String>, Vec<String>) = split_selectors(prelude)
            .into_iter()
            .map(|selector| selector.trim().to_string())
            .filter(|selector| !selector.is_empty())
            .partition(|selector| !selector.contains(":host") && !selector.contains("::slotted"));
        if !mine.is_empty() {
            inner.push_str(&format!("{} {{{body}}}\n", mine.join(", ")));
        }
        let anchored: Vec<String> = anchored
            .iter()
            .filter_map(|selector| anchor(selector, group))
            .collect();
        if !anchored.is_empty() {
            outer.push_str(&format!("{} {{{body}}}\n", anchored.join(", ")));
        }
    }
    (inner, outer)
}

/** The next rule: prelude, block body (None for `;` statements) and the remaining text. */
fn next_rule(css: &str) -> Option<(&str, Option<&str>, &str)> {
    let start = skip_space(css);
    if start.is_empty() {
        return None;
    }
    let bytes = start.as_bytes();
    let mut position = 0;
    let mut quote: Option<u8> = None;
    while position < bytes.len() {
        let byte = bytes[position];
        match quote {
            Some(q) if byte == q => quote = None,
            Some(_) if byte == b'\\' => position += 1,
            Some(_) => {}
            None => match byte {
                b'"' | b'\'' => quote = Some(byte),
                b';' => return Some((&start[..position], None, &start[position + 1..])),
                b'{' => {
                    let end = block_end(start, position + 1);
                    let body = &start[position + 1..end.min(start.len())];
                    let after = start.get(end + 1..).unwrap_or("");
                    return Some((&start[..position], Some(body), after));
                }
                _ => {}
            },
        }
        position += 1;
    }
    None
}

/** Leading whitespace and comments. */
fn skip_space(mut css: &str) -> &str {
    loop {
        css = css.trim_start();
        match css.strip_prefix("/*") {
            Some(comment) => css = comment.find("*/").map_or("", |end| &comment[end + 2..]),
            None => return css,
        }
    }
}

/** The index of the brace closing a block opened just before `from`. */
fn block_end(css: &str, from: usize) -> usize {
    let bytes = css.as_bytes();
    let mut depth = 1usize;
    let mut position = from;
    let mut quote: Option<u8> = None;
    while position < bytes.len() {
        let byte = bytes[position];
        match quote {
            Some(q) if byte == q => quote = None,
            Some(_) if byte == b'\\' => position += 1,
            Some(_) => {}
            None => match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return position;
                    }
                }
                _ => {}
            },
        }
        position += 1;
    }
    bytes.len()
}

/** Selectors of a list, split at top-level commas. */
fn split_selectors(list: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (position, c) in list.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&list[start..position]);
                start = position + 1;
            }
            _ => {}
        }
    }
    out.push(&list[start..]);
    out
}

/** The parenthesized argument after `prefix` at the start of `text`, and what follows it. */
fn argument<'a>(text: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = text.strip_prefix(prefix)?;
    let mut depth = 1i32;
    for (position, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&rest[..position], &rest[position + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/** Rewrite one `:host` or `::slotted()` selector against the flat-tree marks. */
fn anchor(selector: &str, group: usize) -> Option<String> {
    if selector.contains(":host-context") {
        return None;
    }
    let host = format!("[{HOST_ATTRIBUTE}~=\"{group}\"]");
    let slotted = format!("[{SLOTTED_ATTRIBUTE}~=\"{group}\"]");
    let mut out = String::with_capacity(selector.len() + host.len());
    let mut rest = selector;
    while !rest.is_empty() {
        if let Some((inner, after)) = argument(rest, ":host(") {
            out.push_str(inner.trim());
            out.push_str(&host);
            rest = after;
        } else if let Some(after) = rest.strip_prefix(":host") {
            out.push_str(&host);
            rest = after;
        } else if let Some((inner, after)) = argument(rest, "::slotted(") {
            let compound_start = out
                .rfind(|c: char| c.is_whitespace() || c == '>' || c == '+' || c == '~')
                .map_or(0, |index| index + 1);
            let prefix = out[compound_start..].to_string();
            out.truncate(compound_start);
            if !prefix.is_empty() && prefix != "*" {
                out.push_str(&prefix);
                out.push_str(" > ");
            }
            out.push_str(inner.trim());
            out.push_str(&slotted);
            rest = after;
        } else {
            let c = rest.chars().next()?;
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    Some(out)
}

/// Styles computed on the flat tree return to the document's own nodes: a node the flat tree
/// leaves out (an unassigned light child, a shadow root) is hidden and never actionable.
pub(super) fn restore(document: &Document, styled: Document) -> Document {
    let mut out = document.clone();
    let styles: HashMap<usize, super::ComputedStyle> = styled
        .nodes
        .into_iter()
        .map(|node| (node.local_id, node.style))
        .collect();
    for node in &mut out.nodes {
        match styles.get(&node.local_id) {
            Some(style) => node.style = style.clone(),
            None => {
                node.style.hidden = true;
                node.style.interactive = false;
            }
        }
    }
    out.formatting = styled.formatting;
    out.diagnostics = styled.diagnostics;
    out
}

/// Fragment styles inserted into a shadow tree stay with their `<style>` nodes (scoped to that
/// tree) instead of joining the page's sheets; a sheet too large for node text stays global.
pub(super) fn keep_scoped_styles(nodes: &mut [Node], styles: &mut Vec<super::StyleSource>) {
    let elements: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.tag == "style")
        .map(|(index, _)| index)
        .collect();
    if elements.len() != styles.len() {
        return;
    }
    let mut kept = Vec::new();
    for (index, source) in elements.into_iter().zip(std::mem::take(styles)) {
        match source {
            super::StyleSource::Inline(css)
                if css.len() <= super::content::text_limit(&nodes[index]) =>
            {
                nodes[index].text = css;
            }
            other => kept.push(other),
        }
    }
    *styles = kept;
}

/// Whether the node with this identity lies in a shadow tree of the document.
pub(super) fn in_shadow(document: &Document, local: Option<usize>) -> bool {
    let mut current = local;
    for _ in 0..DOCUMENT_DEPTH {
        let Some(id) = current else {
            return false;
        };
        let Some(node) = document.nodes.iter().find(|node| node.local_id == id) else {
            return false;
        };
        if node.tag == SHADOW_ROOT {
            return true;
        }
        current = node.parent;
    }
    false
}
