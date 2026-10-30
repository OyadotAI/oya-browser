/** Observer ownership survives collection and transport without accepting arbitrary callback or node identities. */
use super::*;
use crate::protocol::EngineResult;
use crate::v9::vm::heap::Heap;

/** Active targets keep their observer alive; its hidden callback remains an ordinary traced heap edge. */
pub(in crate::v9) fn roots(state: &super::super::PageState, out: &mut Vec<JsValue>) {
    for (id, observer) in &state.intersection_observers {
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
    let observers = &state.intersection_observers;
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
        options(&observer.options)?;
        let mut seen = std::collections::BTreeSet::new();
        for target in &observer.targets {
            if !seen.insert(target.node)
                || state.wrappers.get(&target.node) != Some(&target.object)
                || !target_object(heap, target.node, target.object)
                || target
                    .previous
                    .as_ref()
                    .is_some_and(|value| value.threshold > observer.options.thresholds.len())
            {
                return Err(invalid());
            }
        }
        for entry in &observer.queued {
            record(state, heap, entry)?;
        }
    }
    Ok(())
}

/** Restored thresholds remain sorted finite ratios and all four margins remain bounded. */
fn options(options: &options::Options) -> EngineResult<()> {
    if options.thresholds.is_empty()
        || options.thresholds.len() > THRESHOLDS
        || options
            .thresholds
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        || options.thresholds.windows(2).any(|pair| pair[0] > pair[1])
        || options
            .margins
            .iter()
            .any(|margin| !margin.value.is_finite() || margin.value.abs() > MARGIN_LIMIT)
    {
        return Err(invalid());
    }
    Ok(())
}

/** A restored record can reference only a retained genuine target and finite nonnegative extents. */
fn record(state: &super::super::PageState, heap: &Heap, entry: &Entry) -> EngineResult<()> {
    if state.wrappers.get(&entry.node) != Some(&entry.target)
        || !target_object(heap, entry.node, entry.target)
        || !entry.ratio.is_finite()
        || !(0.0..=1.0).contains(&entry.ratio)
        || !entry.time.is_finite()
        || entry.time < 0.0
        || [&entry.bounds, &entry.root, &entry.intersection]
            .iter()
            .any(|rect| {
                rect.iter()
                    .any(|value| !value.is_finite() || value.abs() > RECT_LIMIT)
                    || rect[2] < 0.0
                    || rect[3] < 0.0
            })
    {
        return Err(invalid());
    }
    Ok(())
}

/** Malformed observer state is rejected before any callback executes. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid IntersectionObserver state.")
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
