/** Observer ownership survives collection and transport without accepting arbitrary callback or node identities. */
use super::*;
use crate::protocol::EngineResult;
use crate::v9::vm::heap::Heap;

/** Active targets keep their observer alive; its hidden callback remains an ordinary traced heap edge. */
pub(in crate::v9) fn roots(state: &super::super::PageState, out: &mut Vec<JsValue>) {
    for (id, observer) in &state.resize_observers {
        if !observer.targets.is_empty() || !observer.queued.is_empty() {
            out.push(JsValue::Object(*id));
        }
        out.extend(
            observer
                .targets
                .iter()
                .map(|target| JsValue::Object(target.object)),
        );
        out.extend(
            observer
                .queued
                .iter()
                .map(|record| JsValue::Object(record.target)),
        );
    }
}

/** Aggregate caps cover all observers, including records queued before registrations were removed. */
pub(in crate::v9) fn validate(state: &super::super::PageState, heap: &Heap) -> EngineResult<()> {
    cycle::validate(state.resize_cycle.as_ref())?;
    let observers = &state.resize_observers;
    if observers.len() > OBSERVERS
        || observers
            .values()
            .map(|item| item.targets.len())
            .sum::<usize>()
            > TARGETS
        || observers
            .values()
            .map(|item| item.queued.len())
            .sum::<usize>()
            > RECORDS
    {
        return Err(invalid());
    }
    for (id, observer) in observers {
        let callback = heap
            .get(*id)
            .and_then(|object| object.props.get(&callback_key()))
            .and_then(|prop| match &prop.slot {
                Slot::Data(value) => value.object(),
                _ => None,
            });
        if callback.is_none_or(|id| !callable(heap, id)) {
            return Err(invalid());
        }
        let mut seen = std::collections::BTreeSet::new();
        for target in &observer.targets {
            if !seen.insert(target.node)
                || state.wrappers.get(&target.node) != Some(&target.object)
                || !target_object(heap, target.node, target.object)
                || target.previous.as_ref().is_some_and(|size| {
                    size.iter()
                        .any(|value| !value.is_finite() || !(0.0..=SIZE_LIMIT).contains(value))
                })
            {
                return Err(invalid());
            }
        }
        let mut queued = std::collections::BTreeSet::new();
        for entry in &observer.queued {
            let target = observer
                .targets
                .iter()
                .find(|target| target.node == entry.node && target.object == entry.target)
                .ok_or_else(invalid)?;
            let selected = match target.kind {
                BoxKind::Content => entry.content_size,
                BoxKind::Border => entry.border_size,
                BoxKind::DevicePixelContent => entry.device_size,
            };
            if !queued.insert(entry.node) || entry.observed != selected {
                return Err(invalid());
            }
            record(state, heap, entry)?;
        }
    }
    Ok(())
}

/** A restored record can reference only a retained genuine target and finite nonnegative extents. */
fn record(state: &super::super::PageState, heap: &Heap, entry: &Entry) -> EngineResult<()> {
    if entry.depth == 0 || entry.depth > cycle::DEPTH_LIMIT
        || state.wrappers.get(&entry.node) != Some(&entry.target)
        || !target_object(heap, entry.node, entry.target)
        || entry
            .content
            .iter()
            .chain(&entry.content_size)
            .chain(&entry.border_size)
            .chain(&entry.device_size)
            .chain(&entry.observed)
            .any(|value| !value.is_finite() || !(0.0..=SIZE_LIMIT).contains(value))
    {
        return Err(invalid());
    }
    Ok(())
}

/** Malformed observer state is rejected before any callback executes. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid ResizeObserver state.")
}

/** A registry cannot promote an arbitrary heap object into an observed DOM target. */
fn target_object(heap: &Heap, node: usize, id: ObjectId) -> bool {
    matches!(heap.get(id).map(|object| &object.class), Some(Class::Host(data)) if matches!(data.as_ref(), crate::v9::vm::data::HostData::Node(value) if *value == node))
}

/** Callable proxies preserve their creation-time capability even after revocation. */
fn callable(heap: &Heap, mut id: ObjectId) -> bool {
    for _ in 0..OBSERVERS {
        match heap.get(id).map(|object| &object.class) {
            Some(Class::Function(_)) => return true,
            Some(Class::Proxy(proxy)) => {
                if let Some(value) = proxy.callable {
                    return value;
                }
                let Some(target) = proxy.target else {
                    return false;
                };
                id = target;
            }
            _ => return false,
        }
    }
    false
}
