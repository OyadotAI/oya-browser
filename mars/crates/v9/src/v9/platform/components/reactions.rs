/**
 * Custom element reactions and mutation bookkeeping at the end of each DOM call: connected and
 * disconnected callbacks for inserted and removed subtrees, attributeChangedCallback for
 * observed attributes, upgrades of created nodes, observer records and slotchange signals.
 */
use super::{Mutation, REACTIONS, observer, registry, shadow, template};
use crate::v9::platform::Page;
use crate::v9::platform::dom::wrap;
use crate::v9::value::JsValue;
use crate::v9::vm::{Res, Thrown, Vm};

/** Run a DOM operation's steps, then the reactions and records it queued. */
pub(in crate::v9) fn reacting(
    vm: &mut Vm,
    steps: impl FnOnce(&mut Vm) -> Res<JsValue>,
) -> Res<JsValue> {
    let result = steps(vm);
    if matches!(result, Err(Thrown::Fatal(_))) {
        return result;
    }
    flush(vm)?;
    result
}

/** Process every queued mutation; reactions that mutate again queue (and flush) their own. */
pub(in crate::v9) fn flush(vm: &mut Vm) -> Res<()> {
    let mut processed = 0;
    while !vm.page.mutations.is_empty() && processed < REACTIONS {
        let mutations = std::mem::take(&mut vm.page.mutations);
        processed += mutations.len();
        for mutation in mutations {
            observer::record(vm, &mutation)?;
            react(vm, mutation)?;
        }
    }
    Ok(())
}

/** One mutation's custom element reactions and slot signals. */
fn react(vm: &mut Vm, mutation: Mutation) -> Res<()> {
    let components = !vm.page.state.components.definitions.is_empty();
    match mutation {
        Mutation::Children {
            parent,
            added,
            removed,
            ..
        } => {
            let changed: Vec<usize> = added.iter().chain(&removed).copied().collect();
            signal(vm, parent, &changed)?;
            for node in &removed {
                super::super::interfaces::popover::removed(vm, *node);
            }
            if components || !vm.page.state.components.connected.is_empty() {
                for node in removed {
                    disconnect_tree(vm, node)?;
                }
                for node in added {
                    connect_tree(vm, node)?;
                }
            }
        }
        Mutation::Attribute { node, name, old } => {
            if name == "popover" {
                super::super::interfaces::popover::attribute_changed(vm, node)?;
            }
            if (name == "slot" || name == "name")
                && let Some(parent) = vm.page.parent(node)
            {
                signal(vm, parent, &[node])?;
            }
            attribute_changed(vm, node, &name, old)?;
        }
        Mutation::Created { root } if components => upgrade_tree(vm, root)?,
        _ => {}
    }
    Ok(())
}

/** Queue slotchange for slots whose assigned nodes a change touched. */
fn signal(vm: &mut Vm, parent: usize, nodes: &[usize]) -> Res<()> {
    let slots = shadow::affected_slots(&mut vm.page, parent, nodes);
    if slots.is_empty() {
        return Ok(());
    }
    let pending = &mut vm.page.state.components.slot_changes;
    for slot in slots {
        if !pending.contains(&slot) {
            pending.push(slot);
        }
    }
    observer::schedule(vm)
}

/** A node and its descendants, shadow trees included, in shadow-including tree order. */
fn inclusive(page: &mut Page, root: usize) -> Vec<usize> {
    let mut nodes = vec![root];
    nodes.extend(page.all_descendants(root));
    nodes
}

/** After an insertion: connected custom elements get connectedCallback, others may upgrade. */
fn connect_tree(vm: &mut Vm, root: usize) -> Res<()> {
    if !vm.page.exists(root) || !vm.page.connected(root) {
        return Ok(());
    }
    for node in inclusive(&mut vm.page, root) {
        if !vm.page.exists(node) {
            continue;
        }
        if vm.page.state.components.custom.contains_key(&node) {
            connected(vm, node)?;
        } else {
            registry::upgrade(vm, node)?;
        }
    }
    Ok(())
}

/** After a removal: custom elements that left the document get disconnectedCallback. */
fn disconnect_tree(vm: &mut Vm, root: usize) -> Res<()> {
    if vm.page.state.components.connected.is_empty()
        || !vm.page.exists(root)
        || vm.page.connected(root)
    {
        return Ok(());
    }
    for node in inclusive(&mut vm.page, root) {
        if vm.page.state.components.connected.remove(&node) {
            let callback = definition_callback(vm, node, |definition| &definition.disconnected);
            call_reaction(vm, &callback, node, Vec::new())?;
        }
    }
    Ok(())
}

/** Upgrade defined custom elements in a subtree (never inside template content). */
pub(super) fn upgrade_tree(vm: &mut Vm, root: usize) -> Res<()> {
    if !vm.page.exists(root) || template::inert(&mut vm.page, root) {
        return Ok(());
    }
    for node in inclusive(&mut vm.page, root) {
        if vm.page.exists(node) && vm.page.tag(node) != "template" {
            registry::upgrade(vm, node)?;
        }
    }
    Ok(())
}

/** connectedCallback once per connection. */
pub(super) fn connected(vm: &mut Vm, node: usize) -> Res<()> {
    if !vm.page.state.components.connected.insert(node) {
        return Ok(());
    }
    let callback = definition_callback(vm, node, |definition| &definition.connected);
    call_reaction(vm, &callback, node, Vec::new())
}

/** attributeChangedCallback for an observed attribute of an upgraded element. */
fn attribute_changed(vm: &mut Vm, node: usize, name: &str, old: Option<String>) -> Res<()> {
    let Some(index) = vm.page.state.components.custom.get(&node).copied() else {
        return Ok(());
    };
    let definition = &vm.page.state.components.definitions[index];
    if !definition.observed.iter().any(|observed| observed == name) {
        return Ok(());
    }
    let callback = definition.attribute_changed.clone();
    let value = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get(name).cloned());
    let text = |value: Option<String>| -> Res<JsValue> {
        Ok(match value {
            Some(value) => JsValue::string(&value)?,
            None => JsValue::Null,
        })
    };
    let args = vec![JsValue::string(name)?, text(old)?, text(value)?];
    call_reaction(vm, &callback, node, args)
}

/** One callback of an upgraded element's definition. */
fn definition_callback(
    vm: &Vm,
    node: usize,
    pick: impl Fn(&super::Definition) -> &JsValue,
) -> JsValue {
    vm.page
        .state
        .components
        .custom
        .get(&node)
        .and_then(|index| vm.page.state.components.definitions.get(*index))
        .map_or(JsValue::Undefined, |definition| pick(definition).clone())
}

/** Call a lifecycle callback on an element; its exceptions are reported, not thrown. */
pub(super) fn call_reaction(
    vm: &mut Vm,
    callback: &JsValue,
    node: usize,
    args: Vec<JsValue>,
) -> Res<()> {
    if !vm.is_callable(callback) {
        return Ok(());
    }
    let this = wrap(vm, node)?;
    match vm.call(callback.clone(), this, args) {
        Ok(_) => Ok(()),
        Err(Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(fatal) => Err(fatal),
    }
}

/** Follow a node the document re-identified when it was inserted into the projection. */
pub(in crate::v9) fn adopt(page: &mut Page, old: usize, new: usize) {
    let state = &mut page.state.components;
    if let Some(index) = state.custom.remove(&old) {
        state.custom.insert(new, index);
    }
    if state.connected.remove(&old) {
        state.connected.insert(new);
    }
    if state.failed.remove(&old) {
        state.failed.insert(new);
    }
    if let Some(content) = state.contents.remove(&old) {
        state.contents.insert(new, content);
    }
    for entry in state.constructing.iter_mut().filter(|entry| **entry == old) {
        *entry = new;
    }
    for observation in state.observations.iter_mut().filter(|o| o.target == old) {
        observation.target = new;
    }
    for slot in state.slot_changes.iter_mut().filter(|slot| **slot == old) {
        *slot = new;
    }
    for mutation in &mut page.mutations {
        renumber(mutation, old, new);
    }
}

/** Queued mutations name nodes by identity, so they follow re-identification too. */
fn renumber(mutation: &mut Mutation, old: usize, new: usize) {
    let swap = |id: &mut usize| {
        if *id == old {
            *id = new;
        }
    };
    match mutation {
        Mutation::Children {
            parent,
            added,
            removed,
            previous,
            next,
        } => {
            swap(parent);
            added.iter_mut().for_each(swap);
            removed.iter_mut().for_each(swap);
            previous.iter_mut().for_each(swap);
            next.iter_mut().for_each(swap);
        }
        Mutation::Attribute { node, .. } | Mutation::Text { node, .. } => swap(node),
        Mutation::Created { root } => swap(root),
    }
}
