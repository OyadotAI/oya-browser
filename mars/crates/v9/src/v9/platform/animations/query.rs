/* Animation discovery follows connected tree scopes and actual live effect records, including ordinary subtree options. */
use super::*;
use crate::constants::{DOCUMENT_DEPTH, VM_DETACHED_NODES};

/** Only a real document's own tree scope is visible through Document.getAnimations. */
pub(super) fn document(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if super::super::dom::node_of(vm, &call.this) != Some(super::super::dom::DOCUMENT) {
        if matches!(
            super::super::host_data(vm, &call.this),
            Some(crate::v9::vm::data::HostData::Document(_))
        ) {
            return Err(vm.unsupported_error("child Document animation queries"));
        }
        return Err(vm.type_error("Illegal Document.getAnimations invocation"));
    }
    collect(vm, None, None)
}
/** A ShadowRoot cannot inspect nested shadow trees or its host's light descendants. */
pub(in crate::v9) fn shadow(vm: &mut Vm, root: usize) -> Res<JsValue> {
    if !vm.page.is_shadow_root(root) {
        return Err(vm.type_error("Illegal ShadowRoot.getAnimations invocation"));
    }
    collect(vm, Some(root), None)
}
/** Element queries default to the element itself and optionally include ordinary descendants in the same scope. */
pub(super) fn element(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = element_node(vm, &call.this)?;
    if !call.arg(0).nullish() && !matches!(call.arg(0), JsValue::Object(_)) {
        return Err(vm.type_error("getAnimations options must be a dictionary"));
    }
    let subtree = if call.arg(0).nullish() {
        false
    } else {
        vm.get(&call.arg(0), &Key::from("subtree"))?.truthy()
    };
    let scope = super::super::components::containing_root(&mut vm.page, node);
    collect(vm, scope, Some((node, subtree)))
}
/** Collection charges actual candidate traversal and keeps stable creation order without duplicate object identities. */
fn collect(vm: &mut Vm, scope: Option<usize>, element: Option<(usize, bool)>) -> Res<JsValue> {
    let active = vm.page.state.active_animations.clone();
    vm.charge(active.len())?;
    let mut values = Vec::new();
    for id in active {
        let Some(animation) = state::State::load(vm, id) else {
            continue;
        };
        if animation.idle || animation.null_effect {
            continue;
        }
        let Some(effect) = state::Effect::load(vm, animation.effect) else {
            continue;
        };
        let Some(node) = super::super::dom::node_of(vm, &effect.target) else {
            continue;
        };
        if !vm.page.connected(node)
            || super::super::components::containing_root(&mut vm.page, node) != scope
        {
            continue;
        }
        if let Some((ancestor, subtree)) = element
            && node != ancestor
            && (!subtree || !descendant(vm, node, ancestor)?)
        {
            continue;
        }
        values.push(JsValue::Object(id));
    }
    vm.new_array(values).map(JsValue::Object)
}
/** Parent traversal remains inside an already matched scope and cannot hang on a forged cyclic tree. */
fn descendant(vm: &mut Vm, node: usize, ancestor: usize) -> Res<bool> {
    let mut current = Some(node);
    for _ in 0..DOCUMENT_DEPTH + VM_DETACHED_NODES {
        let Some(node) = current else {
            return Ok(false);
        };
        vm.charge(1)?;
        if node == ancestor {
            return Ok(true);
        }
        current = vm.page.parent(node);
    }
    Ok(false)
}
