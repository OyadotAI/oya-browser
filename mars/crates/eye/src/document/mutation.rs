/** Script change logs replay atomically on one candidate through the same core placement helpers as V9. */
use super::{Document, Node, TextChange, parse_fragment, validate_content};
use crate::{
    constants::STATEMENTS,
    protocol::{EngineError, EngineResult},
};
use mars_core::dom::{
    DomChange, InsertPosition, REFLECTED_ATTRIBUTES, ScriptChange, local_position, place, relocate,
    script_attribute_allowed, validate_preorder, validate_text,
};

/// Replays text, insert, move and attribute changes in script order; any refusal leaves the input unchanged.
pub fn apply_changes(document: &Document, changes: &[DomChange]) -> EngineResult<Document> {
    validate_content(document)?;
    if changes.len() > STATEMENTS {
        return Err(EngineError::limit("script changes", STATEMENTS));
    }
    let mut candidate = document.clone();
    let mut high_water = document
        .nodes
        .iter()
        .map(|node| node.local_id + 1)
        .max()
        .unwrap_or(0);
    for change in changes {
        apply_change(&mut candidate, change, &mut high_water, false)?;
    }
    validate_preorder(candidate.nodes.iter().map(key))?;
    validate_content(&candidate)?;
    Ok(candidate)
}

/// Replays V9's extended log (the original changes plus removal, attributes, node text and field
/// values) in order on one candidate; any refusal leaves the input unchanged. Attribute names are
/// re-checked here, so an inline handler, srcdoc or http-equiv never becomes document data.
pub fn apply_script_changes(
    document: &Document,
    changes: &[ScriptChange],
) -> EngineResult<Document> {
    validate_content(document)?;
    if changes.len() > STATEMENTS {
        return Err(EngineError::limit("script changes", STATEMENTS));
    }
    let mut candidate = document.clone();
    let mut high_water = document
        .nodes
        .iter()
        .map(|node| node.local_id + 1)
        .max()
        .unwrap_or(0);
    for change in changes {
        match change {
            ScriptChange::Canvas { node, bitmap } => {
                bitmap.validate()?;
                let target = candidate
                    .nodes
                    .iter_mut()
                    .find(|n| n.local_id == *node && n.tag == "canvas")
                    .ok_or_else(|| {
                        EngineError::invalid("Canvas publication requires an actual canvas.")
                    })?;
                target.canvas = Some(bitmap.clone());
            }
            ScriptChange::Dom { change } => {
                apply_change(&mut candidate, change, &mut high_water, true)?
            }
            ScriptChange::Remove { node } => remove(&mut candidate, *node)?,
            ScriptChange::Attribute { node, name, value } => {
                set_any_attribute(&mut candidate, *node, name, value.as_deref())?
            }
            ScriptChange::NodeText { node, text } => set_node_text(&mut candidate, *node, text)?,
            ScriptChange::Value { node, value } => set_field_value(&mut candidate, *node, value)?,
            ScriptChange::Defined { node } => mark_defined(&mut candidate, *node)?,
            ScriptChange::RootAttribute { body, name, value } => {
                set_root_attribute(&mut candidate, *body, name, value.as_deref())?
            }
            ScriptChange::Focus { node } => {
                for entry in &mut candidate.nodes {
                    entry.focused = Some(entry.local_id) == *node;
                }
            }
        }
    }
    validate_preorder(candidate.nodes.iter().map(key))?;
    validate_content(&candidate)?;
    Ok(candidate)
}

/** A script's write to `<html>` or `<body>` attributes, which live on the formatting wrappers
rather than in the node list; names are re-checked as for any other element. */
fn set_root_attribute(
    candidate: &mut Document,
    body: bool,
    name: &str,
    value: Option<&str>,
) -> EngineResult<()> {
    if !script_attribute_allowed(name) {
        return Err(EngineError::unsupported(
            "Scripts may not write event handler, srcdoc or http-equiv attributes.",
        ));
    }
    if let Some(value) = value {
        validate_text(value)?;
    }
    let formatting = &mut candidate.formatting;
    let element = if body {
        &mut formatting.body
    } else {
        &mut formatting.html
    };
    let name = name.to_ascii_lowercase();
    match value {
        Some(value) => element.attributes.insert(name, value.into()),
        None => element.attributes.remove(&name),
    };
    Ok(())
}

/** An upgraded custom element matches `:defined` from now on. */
fn mark_defined(candidate: &mut Document, node: usize) -> EngineResult<()> {
    let target = candidate
        .nodes
        .iter_mut()
        .find(|n| n.local_id == node)
        .ok_or_else(|| EngineError::invalid("Unknown node for a definedness change."))?;
    target.defined = true;
    Ok(())
}

/** Removal takes the node and its whole subtree, as V9's projection did. */
fn remove(candidate: &mut Document, node: usize) -> EngineResult<()> {
    find(candidate, node)?;
    let mut removed = super::descendants(candidate, node);
    removed.push(node);
    candidate
        .nodes
        .retain(|entry| !removed.contains(&entry.local_id));
    Ok(())
}

/**
 * Any attribute `script_attribute_allowed` admits, set or removed, with the fields Eye derives
 * from attributes at parse time kept in step (id, link, image, placeholder, disabled, checked).
 */
fn set_any_attribute(
    candidate: &mut Document,
    node: usize,
    name: &str,
    value: Option<&str>,
) -> EngineResult<()> {
    if !script_attribute_allowed(name) {
        return Err(EngineError::unsupported(
            "Scripts may not write event handler, srcdoc or http-equiv attributes.",
        ));
    }
    if let Some(value) = value {
        validate_text(value)?;
    }
    let name = name.to_ascii_lowercase();
    let entry = candidate
        .nodes
        .iter_mut()
        .find(|entry| entry.local_id == node && entry.tag != "#text")
        .ok_or_else(super::invalid_target)?;
    match value {
        Some(value) => entry.attributes.insert(name.clone(), value.into()),
        None => entry.attributes.remove(&name),
    };
    derive(entry, &name);
    Ok(())
}

/** Re-derive the one parse-time field an attribute feeds; `javascript:` targets stay inert. */
fn derive(node: &mut Node, name: &str) {
    let value = node.attributes.get(name).cloned();
    let present = value.is_some();
    match name {
        "id" => {
            node.html_id = value
                .filter(|id| !id.is_empty() && id.len() <= mars_core::constants::HTML_ID_BYTES)
                .unwrap_or_else(|| format!("@{}", node.local_id));
        }
        "href" if node.tag == "a" => {
            node.href = value.filter(|href| !super::markup::script_url(href))
        }
        "src" if node.tag == "img" => {
            node.image_src = value.filter(|src| !super::markup::script_url(src))
        }
        "alt" => node.image_alt = value.unwrap_or_default(),
        "placeholder" => node.placeholder = value.unwrap_or_default(),
        "disabled" => node.disabled = present,
        "checked" => node.checked = present,
        "selected" => node.selected = present,
        _ => {}
    }
}

/** textContent on any node: its descendants go (a host keeps its shadow tree) and its own
text becomes `text`. */
fn set_node_text(candidate: &mut Document, node: usize, text: &str) -> EngineResult<()> {
    validate_text(text)?;
    find(candidate, node)?;
    let mut removed = super::descendants(candidate, node);
    let roots: Vec<usize> = candidate
        .nodes
        .iter()
        .filter(|entry| entry.parent == Some(node) && entry.tag == super::shadow::SHADOW_ROOT)
        .map(|entry| entry.local_id)
        .collect();
    for root in roots {
        let mut kept = super::descendants(candidate, root);
        kept.push(root);
        removed.retain(|id| !kept.contains(id));
    }
    candidate
        .nodes
        .retain(|entry| !removed.contains(&entry.local_id));
    let position = local_position(&candidate.nodes, node, |entry| entry.local_id)
        .ok_or_else(super::invalid_target)?;
    candidate.nodes[position].text = text.into();
    Ok(())
}

/** A script-set field value is literal data on an input or textarea, never markup or source. */
fn set_field_value(candidate: &mut Document, node: usize, value: &str) -> EngineResult<()> {
    validate_text(value)?;
    let entry = candidate
        .nodes
        .iter_mut()
        .find(|entry| entry.local_id == node && ["input", "textarea"].contains(&entry.tag.as_str()))
        .ok_or_else(super::invalid_target)?;
    entry.value = value.into();
    Ok(())
}

/** One change mutates the shared candidate; the caller discards it on any error. */
/** `text_nodes` gives inserted elements' leading text its own text node, as V9's extended DOM
has; the legacy log keeps Eye's parse shape. */
fn apply_change(
    candidate: &mut Document,
    change: &DomChange,
    high_water: &mut usize,
    text_nodes: bool,
) -> EngineResult<()> {
    match change {
        DomChange::Text { html_id, text } => apply_text(
            candidate,
            &TextChange {
                html_id: html_id.clone(),
                text: text.clone(),
            },
        ),
        DomChange::InsertHtml {
            target,
            position,
            html,
            first_id,
        } => insert(
            candidate,
            (*target, *position),
            html,
            *first_id,
            high_water,
            text_nodes,
        ),
        DomChange::Move {
            node,
            target,
            position,
        } => relocate(
            &mut candidate.nodes,
            key,
            reparent,
            *node,
            *target,
            *position,
        ),
        DomChange::SetAttribute { node, name, value } => {
            set_attribute(candidate, *node, name, value)
        }
    }
}

/// Text replacement removes descendants and keeps the target's identity, exactly like `apply`.
pub(super) fn apply_text(candidate: &mut Document, change: &TextChange) -> EngineResult<()> {
    super::validate_change(candidate, change)?;
    let local = candidate
        .nodes
        .iter()
        .find(|node| node.html_id == change.html_id)
        .map(|node| node.local_id)
        .ok_or_else(super::invalid_target)?;
    let removed = super::descendants(candidate, local);
    candidate
        .nodes
        .retain(|node| !removed.contains(&node.local_id));
    candidate
        .nodes
        .iter_mut()
        .find(|node| node.local_id == local)
        .ok_or_else(super::invalid_target)?
        .text
        .clone_from(&change.text);
    Ok(())
}

/** Fresh identities start at or above the high-water mark, so no identity is reused in one execution. */
fn insert(
    candidate: &mut Document,
    (target, position): (Option<usize>, InsertPosition),
    html: &str,
    first_id: usize,
    high_water: &mut usize,
    text_nodes: bool,
) -> EngineResult<()> {
    if first_id < *high_water {
        return Err(EngineError::invalid(
            "Inserted nodes must use fresh identities.",
        ));
    }
    let context = context_tag(candidate, target, position)?;
    let mut fragment = parse_fragment(&context, html, first_id)?;
    let container = match position {
        InsertPosition::AfterBegin | InsertPosition::BeforeEnd => target,
        _ => target.and_then(|target| find(candidate, target).ok().and_then(|node| node.parent)),
    };
    if super::shadow::in_shadow(candidate, container) {
        super::shadow::keep_scoped_styles(&mut fragment.nodes, &mut fragment.styles);
    }
    let nodes = if text_nodes {
        super::script::text_children(fragment.nodes, first_id)?
    } else {
        fragment.nodes
    };
    *high_water = first_id + nodes.len();
    place(&mut candidate.nodes, key, reparent, target, position, nodes)?;
    candidate.styles.extend(fragment.styles);
    Ok(())
}

/** The parsing context is the element the fragment lands in; the body wrapper stands in for the root. */
fn context_tag(
    candidate: &Document,
    target: Option<usize>,
    position: InsertPosition,
) -> EngineResult<String> {
    let Some(target) = target else {
        return Ok("body".into());
    };
    let node = find(candidate, target)?;
    let container = match position {
        InsertPosition::AfterBegin | InsertPosition::BeforeEnd => Some(node),
        InsertPosition::BeforeBegin | InsertPosition::AfterEnd => match node.parent {
            Some(parent) => Some(find(candidate, parent)?),
            None => None,
        },
    };
    match container {
        Some(node) if node.tag == "#text" => Err(EngineError::invalid(
            "Text nodes cannot contain inserted markup.",
        )),
        Some(node) => Ok(node.tag.clone()),
        None => Ok("body".into()),
    }
}

/** Only the core allowlist is reflected; identifiers, links, sources, styles and handlers stay author-owned. */
fn set_attribute(
    candidate: &mut Document,
    node: usize,
    name: &str,
    value: &str,
) -> EngineResult<()> {
    if !REFLECTED_ATTRIBUTES.contains(&name) {
        return Err(EngineError::unsupported(
            "Scripts may set only the lang and dir attributes.",
        ));
    }
    validate_text(value)?;
    let node = candidate
        .nodes
        .iter_mut()
        .find(|entry| entry.local_id == node && entry.tag != "#text")
        .ok_or_else(super::invalid_target)?;
    node.attributes.insert(name.into(), value.into());
    Ok(())
}

/** Identity lookup never assumes local identities equal Vec indexes after scripts run. */
fn find(document: &Document, local: usize) -> EngineResult<&Node> {
    local_position(&document.nodes, local, |node| node.local_id)
        .map(|position| &document.nodes[position])
        .ok_or_else(super::invalid_target)
}

/** Documents expose identity and parent to the shared core tree helpers. */
fn key(node: &Node) -> (usize, Option<usize>) {
    (node.local_id, node.parent)
}

/** Placed roots take the parent the core helper resolved. */
fn reparent(node: &mut Node, parent: Option<usize>) {
    node.parent = parent;
}
