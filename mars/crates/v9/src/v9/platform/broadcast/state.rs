/** Endpoint lifetime is tied to VM reachability, with relevant listeners retaining open channels. */
use super::*;
use crate::v9::vm::heap::{FastSet, Heap};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/** No callback or heap object is embedded in the native route snapshot. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Handle {
    pub object: ObjectId,
    pub name: Vec<u16>,
    pub closed: bool,
}

/** The checked sequence never recycles an identity still referenced by a queued native task. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct State {
    pub(super) next: u64,
    pub(super) channels: BTreeMap<u64, Handle>,
}

/** Full states prove native branding; heap-less summaries retain the same count/name/ID limits. */
pub(in crate::v9) fn validate(state: &State, heap: Option<&Heap>) -> EngineResult<()> {
    if state.channels.len() > BROADCAST_CHANNELS {
        return Err(EngineError::limit(
            "BroadcastChannel handles",
            BROADCAST_CHANNELS,
        ));
    }
    let mut objects = BTreeSet::new();
    for (id, record) in &state.channels {
        if *id == 0
            || *id > state.next
            || record.name.len() > BROADCAST_NAME_UNITS
            || !objects.insert(record.object)
        {
            return Err(EngineError::invalid("Invalid BroadcastChannel state."));
        }
        if let Some(heap) = heap {
            let branded = heap.get(record.object).is_some_and(|object| {
                matches!(
                    object.props.get(&brand()),
                    Some(Prop {
                        slot: Slot::Data(JsValue::Boolean(true)),
                        ..
                    })
                )
            });
            if !branded {
                return Err(EngineError::invalid("Invalid BroadcastChannel identity."));
            }
        }
    }
    Ok(())
}

/** HTML retains an open channel with message or messageerror listeners, including handler attributes. */
pub(in crate::v9) fn roots(vm: &Vm, out: &mut Vec<JsValue>) {
    let targets: BTreeSet<_> = vm
        .page
        .state
        .listeners
        .iter()
        .filter_map(|listener| {
            if (listener.kind == "message" || listener.kind == "messageerror")
                && let super::super::Target::Object(object) = listener.target
            {
                Some(object)
            } else {
                None
            }
        })
        .collect();
    for record in vm
        .page
        .state
        .broadcast
        .channels
        .values()
        .filter(|record| !record.closed)
    {
        let handler = ["onmessage", "onmessageerror"].iter().any(|name| {
            vm.heap
                .get(record.object)
                .and_then(|object| object.props.get(&handler_key(name)))
                .is_some_and(
                    |property| matches!(&property.slot, Slot::Data(value) if vm.is_callable(value)),
                )
        });
        if targets.contains(&record.object) || handler {
            out.push(JsValue::Object(record.object));
        }
    }
}

/** GC never appends fallible effects; the final endpoint snapshot retires collected routes natively. */
pub(in crate::v9) fn sweep(state: &mut State, live: &FastSet<ObjectId>) {
    state
        .channels
        .retain(|_, record| live.contains(&record.object));
}

/** Emit even an empty snapshot so a context that closed or collected every endpoint is reconciled. */
pub(in crate::v9) fn publish(page: &mut super::super::Page) {
    page.effects.broadcast_channels = page
        .state
        .broadcast
        .channels
        .iter()
        .filter(|(_, record)| !record.closed)
        .map(|(id, record)| BroadcastEndpoint {
            id: *id,
            name: record.name.clone(),
        })
        .collect();
}
