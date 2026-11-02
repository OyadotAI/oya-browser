/**
 * MutationObserver with real records: childList, attributes (with filters and old values),
 * characterData and subtree. Records queue as DOM calls make changes and are delivered from one
 * microtask after the current script or callback, together with pending slotchange events.
 */
use super::{Mutation, OBSERVATIONS, OBSERVER_RECORDS, Observation, get_slot, ids, set_slot};
use crate::v9::platform::dom::{self, native_name, node_of, wrap};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, Job};
use crate::v9::vm::heap::{FastSet, Key, Object};
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** `new MutationObserver(callback)`. */
pub(in crate::v9) fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(
            vm.type_error("Failed to construct 'MutationObserver': Please use the 'new' operator.")
        );
    };
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error(
            "Failed to construct 'MutationObserver': parameter 1 is not of type 'Function'.",
        ));
    }
    let proto = vm.prototype_for(&target, ids().mutation_observer)?;
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set_slot(vm, id, "callback", callback);
    Ok(JsValue::Object(id))
}

/** observe, disconnect and takeRecords. */
pub(in crate::v9) fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(observer) = call.this.object() else {
        return Err(vm.type_error("Illegal invocation"));
    };
    if get_slot(vm, &call.this, "callback") == JsValue::Undefined {
        return Err(vm.type_error("Illegal invocation"));
    }
    match native_name(&call) {
        "observe" => observe(vm, observer, &call),
        "disconnect" => {
            let state = &mut vm.page.state.components;
            state
                .observations
                .retain(|entry| entry.observer != observer);
            state.records.remove(&observer);
            Ok(JsValue::Undefined)
        }
        _ => {
            let records = vm
                .page
                .state
                .components
                .records
                .remove(&observer)
                .unwrap_or_default();
            Ok(JsValue::Object(vm.new_array(records)?))
        }
    }
}

/** Register (or replace) one observation, validating the options as browsers do. */
fn observe(vm: &mut Vm, observer: ObjectId, call: &Call) -> Res<JsValue> {
    let Some(target) = node_of(vm, &call.arg(0)) else {
        return Err(vm.type_error(
            "Failed to execute 'observe' on 'MutationObserver': parameter 1 is not of type 'Node'.",
        ));
    };
    let options = call.arg(1);
    let flag = |vm: &mut Vm, name: &str| super::option(vm, &options, name);
    let present = |vm: &mut Vm, name: &str| -> Res<bool> {
        if !matches!(options, JsValue::Object(_)) {
            return Ok(false);
        }
        Ok(vm.get(&options, &Key::from(name))? != JsValue::Undefined)
    };
    let filter = if present(vm, "attributeFilter")? {
        let list = vm.get(&options, &Key::from("attributeFilter"))?;
        let names = vm.iterate_to_vec(&list)?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            out.push(vm.to_text(&name)?.to_ascii_lowercase());
        }
        Some(out)
    } else {
        None
    };
    let attribute_old = flag(vm, "attributeOldValue")?;
    let character_old = flag(vm, "characterDataOldValue")?;
    let attributes = if present(vm, "attributes")? {
        flag(vm, "attributes")?
    } else {
        attribute_old || filter.is_some()
    };
    let character_data = if present(vm, "characterData")? {
        flag(vm, "characterData")?
    } else {
        character_old
    };
    let observation = Observation {
        observer,
        target,
        child_list: flag(vm, "childList")?,
        attributes,
        character_data,
        subtree: flag(vm, "subtree")?,
        attribute_old,
        character_old,
        filter,
    };
    if !(observation.child_list || observation.attributes || observation.character_data) {
        return Err(vm.type_error("Failed to execute 'observe' on 'MutationObserver': The options object must set at least one of 'attributes', 'characterData', or 'childList' to true."));
    }
    let state = &mut vm.page.state.components;
    state
        .observations
        .retain(|entry| !(entry.observer == observer && entry.target == target));
    if state.observations.len() >= OBSERVATIONS {
        return Err(vm.range_error("Too many mutation observations."));
    }
    state.observations.push(observation);
    Ok(JsValue::Undefined)
}

/** The mutation's target node and record type. */
fn subject(mutation: &Mutation) -> Option<(usize, &'static str)> {
    match mutation {
        Mutation::Children { parent, .. } => Some((*parent, "childList")),
        Mutation::Attribute { node, .. } => Some((*node, "attributes")),
        Mutation::Text { node, .. } => Some((*node, "characterData")),
        Mutation::Created { .. } => None,
    }
}

/** Whether an observation wants a record for this mutation, and with the old value. */
fn wants(observation: &Observation, mutation: &Mutation) -> Option<bool> {
    match mutation {
        Mutation::Children { .. } => observation.child_list.then_some(false),
        Mutation::Attribute { name, .. } => (observation.attributes
            && observation
                .filter
                .as_ref()
                .is_none_or(|names| names.contains(name)))
        .then_some(observation.attribute_old),
        Mutation::Text { .. } => observation
            .character_data
            .then_some(observation.character_old),
        Mutation::Created { .. } => None,
    }
}

/** Queue records for every observer interested in one mutation. */
pub(super) fn record(vm: &mut Vm, mutation: &Mutation) -> Res<()> {
    if vm.page.state.components.observations.is_empty() {
        return Ok(());
    }
    let Some((target, kind)) = subject(mutation) else {
        return Ok(());
    };
    let ancestors = observation_ancestors(vm, target);
    let mut interested: Vec<(ObjectId, bool)> = Vec::new();
    for observation in &vm.page.state.components.observations {
        let reaches = observation.target == target
            || observation.subtree && ancestors.contains(&observation.target);
        if !reaches {
            continue;
        }
        if let Some(old) = wants(observation, mutation) {
            match interested
                .iter_mut()
                .find(|(id, _)| *id == observation.observer)
            {
                Some(entry) => entry.1 |= old,
                None => interested.push((observation.observer, old)),
            }
        }
    }
    for (observer, old) in interested {
        let record = make_record(vm, mutation, target, kind, old)?;
        let records = vm
            .page
            .state
            .components
            .records
            .entry(observer)
            .or_default();
        if records.len() >= OBSERVER_RECORDS {
            records.remove(0);
        }
        records.push(record);
        schedule(vm)?;
    }
    Ok(())
}

/** Observations include the owning document but cannot cross a shadow root into its host. */
fn observation_ancestors(vm: &mut Vm, target: usize) -> FastSet<usize> {
    let mut ancestors = FastSet::default();
    let mut cursor = dom::traverse_parent(vm, target);
    while let Some(node) = cursor {
        if !ancestors.insert(node) {
            break;
        }
        cursor = dom::traverse_parent(vm, node);
    }
    ancestors
}

/** One MutationRecord. */
fn make_record(
    vm: &mut Vm,
    mutation: &Mutation,
    target: usize,
    kind: &str,
    old: bool,
) -> Res<JsValue> {
    let id = vm.alloc(Object::new(Class::Plain, Some(ids().mutation_record)))?;
    vm.put(id, "type", JsValue::string(kind)?)?;
    let target_value = wrap(vm, target)?;
    vm.put(id, "target", target_value)?;
    let (added, removed, previous, next) = match mutation {
        Mutation::Children {
            added,
            removed,
            previous,
            next,
            ..
        } => (added.clone(), removed.clone(), *previous, *next),
        _ => (Vec::new(), Vec::new(), None, None),
    };
    let added = dom::node_list(vm, added)?;
    vm.put(id, "addedNodes", added)?;
    let removed = dom::node_list(vm, removed)?;
    vm.put(id, "removedNodes", removed)?;
    let previous = dom::wrap_option(vm, previous)?;
    vm.put(id, "previousSibling", previous)?;
    let next = dom::wrap_option(vm, next)?;
    vm.put(id, "nextSibling", next)?;
    let (name, value) = match mutation {
        Mutation::Attribute {
            name, old: value, ..
        } => (
            JsValue::string(name)?,
            value.clone().map(|v| JsValue::string(&v)).transpose()?,
        ),
        Mutation::Text { old: value, .. } => (JsValue::Null, Some(JsValue::string(value)?)),
        _ => (JsValue::Null, None),
    };
    vm.put(id, "attributeName", name)?;
    vm.put(id, "attributeNamespace", JsValue::Null)?;
    let value = if old {
        value.unwrap_or(JsValue::Null)
    } else {
        JsValue::Null
    };
    vm.put(id, "oldValue", value)?;
    Ok(JsValue::Object(id))
}

/** Queue the delivery microtask once. */
pub(super) fn schedule(vm: &mut Vm) -> Res<()> {
    if vm.page.state.components.delivery {
        return Ok(());
    }
    vm.page.state.components.delivery = true;
    vm.enqueue(Job::Callback(JsValue::Object(ids().deliver)))
}

/** The delivery microtask: each observer's callback with its records, then slotchange events. */
pub(in crate::v9) fn deliver(vm: &mut Vm, _: Call) -> Res<JsValue> {
    vm.page.state.components.delivery = false;
    let observers: Vec<ObjectId> = {
        let state = &vm.page.state.components;
        let mut order: Vec<ObjectId> = Vec::new();
        for observation in &state.observations {
            if !order.contains(&observation.observer) {
                order.push(observation.observer);
            }
        }
        order.extend(
            state
                .records
                .keys()
                .filter(|id| !state.observations.iter().any(|o| o.observer == **id)),
        );
        order
    };
    for observer in observers {
        let Some(records) = vm.page.state.components.records.remove(&observer) else {
            continue;
        };
        if records.is_empty() {
            continue;
        }
        let this = JsValue::Object(observer);
        let callback = get_slot(vm, &this, "callback");
        let list = vm.new_array(records)?;
        match vm.call(callback, this.clone(), vec![JsValue::Object(list), this]) {
            Ok(_) => {}
            Err(Thrown::Value(error)) => vm.report_error(&error),
            Err(fatal) => return Err(fatal),
        }
    }
    let slots = std::mem::take(&mut vm.page.state.components.slot_changes);
    for slot in slots {
        if vm.page.exists(slot) {
            let event = crate::v9::platform::events::create_event(vm, "slotchange", true, false)?;
            if let Some(id) = event.object() {
                crate::v9::platform::events::dispatch(
                    vm,
                    crate::v9::platform::Target::Node(slot),
                    id,
                )?;
            }
        }
    }
    Ok(JsValue::Undefined)
}
