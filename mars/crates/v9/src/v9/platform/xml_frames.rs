/**
 * The document of an iframe whose source the app fetched, built by the source's media type: HTML
 * through the HTML tree builder, and XML (XML, XHTML and SVG) as the namespace-aware tree that
 * mars_xml's events describe. A source that is not well-formed becomes the parsererror document
 * the DOM Parsing specification describes: a root parsererror element in the XHTML namespace that
 * carries the error, so the frame still has a document to read.
 */
use super::dom::dom_error;
use super::frames::{self, HTML_TYPE, append, create};
use super::namespaces::{ElementNs, QName};
use crate::v9::vm::{Res, Vm};
use mars_xml::{Attribute, Event, Name};

/** The namespace of the parsererror element a document that is not well-formed becomes. */
const XHTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
/** The element that carries a parse error in place of the document. */
const PARSER_ERROR: &str = "parsererror";

/** Fill `document` (an empty document node) with `source`, as `media_type` says it is. An XML
document also records its media type, which its contentType reports. */
pub(super) fn build_source(
    vm: &mut Vm,
    document: usize,
    media_type: &str,
    source: &str,
) -> Res<()> {
    if media_type == HTML_TYPE {
        return frames::build(vm, document, source);
    }
    frames::set_content_type(vm, document, media_type);
    match mars_xml::parse(source) {
        Ok(events) => build_events(vm, document, &events),
        Err(error) => build_error(vm, document, &error.message),
    }
}

/** Build the tree the events describe under `document`. A start opens its element under the
innermost open one and an end closes it; the parser keeps the events balanced. */
fn build_events(vm: &mut Vm, document: usize, events: &[Event]) -> Res<()> {
    let mut open = vec![document];
    for event in events {
        let parent = open.last().copied().unwrap_or(document);
        match event {
            Event::Start { name, attributes } => {
                let node = element(vm, name, attributes)?;
                append(vm, parent, node)?;
                open.push(node);
            }
            Event::End => {
                if open.len() > 1 {
                    open.pop();
                }
            }
            Event::Text(text) => leaf(vm, parent, "#text", text)?,
            Event::Comment(text) => leaf(vm, parent, "#comment", text)?,
        }
    }
    Ok(())
}

/** An element in the namespace its name resolved to, with its attributes set. */
fn element(vm: &mut Vm, name: &Name, attributes: &[Attribute]) -> Res<usize> {
    let node = create(vm, &name.qualified(), None)?;
    let record = ElementNs {
        namespace: name.namespace.clone(),
        prefix: name.prefix.clone(),
        local: name.local.clone(),
    };
    vm.page.set_element_namespace(node, record);
    for attribute in attributes {
        set_attribute(vm, node, attribute)?;
    }
    Ok(node)
}

/** One attribute, set through the namespace-aware path so getAttributeNS finds it. */
fn set_attribute(vm: &mut Vm, node: usize, attribute: &Attribute) -> Res<()> {
    let name = &attribute.name;
    let qname = QName {
        prefix: name.prefix.clone(),
        local: name.local.clone(),
    };
    vm.page
        .set_namespaced_attribute(
            node,
            name.namespace.as_deref(),
            &qname,
            &name.qualified(),
            &attribute.value,
        )
        .map_err(|error| dom_error(vm, error))
}

/** A text or comment node under `parent`. */
fn leaf(vm: &mut Vm, parent: usize, tag: &str, text: &str) -> Res<()> {
    let node = create(vm, tag, Some(text.to_string()))?;
    append(vm, parent, node)
}

/** The document an XML source that is not well-formed becomes: a root parsererror element in the
XHTML namespace, carrying the error message as its text. */
fn build_error(vm: &mut Vm, document: usize, message: &str) -> Res<()> {
    let node = create(vm, PARSER_ERROR, None)?;
    let record = ElementNs {
        namespace: Some(XHTML_NAMESPACE.to_string()),
        prefix: None,
        local: PARSER_ERROR.to_string(),
    };
    vm.page.set_element_namespace(node, record);
    append(vm, document, node)?;
    leaf(vm, node, "#text", message)
}
