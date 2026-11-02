/**
 * `<template>` content: each template owns one detached DocumentFragment that holds the markup
 * parsed into it, so `template.content` can be cloned and stamped (importNode, cloneNode) while
 * the template element itself stays empty in the document, as Eye keeps it.
 */
use super::FRAGMENT;
use crate::protocol::EngineResult;
use crate::v9::platform::Page;
use crate::v9::platform::dom::{self, this_node, wrap};
use crate::v9::value::JsValue;
use crate::v9::vm::{Call, Res, Vm};

impl Page<'_> {
    /** A template's content fragment (made on first use); any other node is itself. */
    pub fn content_or_self(&mut self, node: usize) -> EngineResult<usize> {
        if self.tag(node) != "template" {
            return Ok(node);
        }
        if let Some(content) = self.state.components.contents.get(&node) {
            return Ok(*content);
        }
        let content = self.create(FRAGMENT, None)?;
        self.state.components.contents.insert(node, content);
        Ok(content)
    }
}

/**
 * `content`: a template's fragment. On other elements the name is the `content` attribute
 * (`<meta content>`), which the same accessor reflects.
 */
pub(in crate::v9) fn content_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    if vm.page.tag(node) == "template" {
        let content = vm
            .page
            .content_or_self(node)
            .map_err(|error| dom::dom_error(vm, error))?;
        return wrap(vm, content);
    }
    let value = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get("content").cloned())
        .unwrap_or_default();
    Ok(JsValue::string(&value)?)
}

/** `content` assignment on non-template elements writes the attribute. */
pub(in crate::v9) fn content_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    if vm.page.tag(node) == "template" {
        return Ok(JsValue::Undefined);
    }
    let text = vm.to_text(&call.arg(0))?;
    vm.page
        .set_attribute(node, "content", Some(&text))
        .map_err(|error| dom::dom_error(vm, error))?;
    super::flush(vm)?;
    Ok(JsValue::Undefined)
}

/** A deep clone of a template copies its content into the copy's own content. */
pub(in crate::v9) fn clone_content(vm: &mut Vm, original: usize, copy: usize) -> Res<()> {
    let Some(content) = vm.page.state.components.contents.get(&original).copied() else {
        return Ok(());
    };
    let target = vm
        .page
        .content_or_self(copy)
        .map_err(|error| dom::dom_error(vm, error))?;
    for child in vm.page.children(content) {
        let child_copy = dom::clone_tree(vm, child, true)?;
        vm.page
            .insert(target, child_copy, None)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    Ok(())
}

/** Whether a node lies in some template's content, where custom elements never upgrade. */
pub(in crate::v9) fn inert(page: &mut Page, node: usize) -> bool {
    if page.state.components.contents.is_empty() {
        return false;
    }
    let mut root = node;
    for ancestor in page.ancestors(node) {
        root = ancestor;
    }
    page.state
        .components
        .contents
        .values()
        .any(|content| *content == root)
}
