/** Geometry sampling and asynchronous delivery share one bounded queue per genuine observer. */
use super::*;
use std::collections::{BTreeMap, BTreeSet};

/** Entry materialization writes 32 properties plus four objects and an array slot; charge bounded native work. */
const ENTRY_WORK: usize = 64;
/** Sampling keeps one geometry result per unique target during this callback-free turn. */
type Bounds = BTreeMap<usize, (bool, [f64; 4])>;

/** Sample after a V9 turn; the host supplies refreshed visual boxes before dedicated observe turns. */
pub(in crate::v9) fn sample(vm: &mut Vm) -> Res<()> {
    vm.charge(
        vm.page
            .state
            .intersection_observers
            .len()
            .saturating_add(vm.page.environment.geometry.len()),
    )?;
    let mut usage = budget::sampling(vm)?;
    let known: BTreeSet<_> = vm
        .page
        .environment
        .geometry
        .iter()
        .map(|item| item.node)
        .collect();
    let mut bounds = Bounds::new();
    let observers: Vec<_> = vm
        .page
        .state
        .intersection_observers
        .keys()
        .copied()
        .collect();
    for id in observers {
        if vm.page.state.intersection_observers[&id].targets.is_empty() {
            continue;
        }
        let state = &vm.page.state.intersection_observers[&id];
        vm.charge(
            state
                .targets
                .len()
                .saturating_add(state.queued.len())
                .saturating_add(state.options.thresholds.len()),
        )?;
        let mut state = vm.page.state.intersection_observers[&id].clone();
        for target in &mut state.targets {
            let entry = measure(vm, target, &state.options, &known, &mut bounds)?;
            vm.charge(state.options.thresholds.len().saturating_add(1))?;
            let previous = Previous {
                threshold: state
                    .options
                    .thresholds
                    .partition_point(|threshold| *threshold <= entry.ratio),
                intersects: entry.intersects,
            };
            if target.previous.as_ref() != Some(&previous) {
                usage.record()?;
                state.queued.push(entry);
                target.previous = Some(previous);
            }
        }
        vm.page.state.intersection_observers.insert(id, state);
    }
    effects(vm);
    Ok(())
}

/** Delivery refreshes queue flags without sampling geometry invalidated by an author callback. */
pub(in crate::v9) fn effects(vm: &mut Vm) {
    vm.page.effects.intersection_observers = vm
        .page
        .state
        .intersection_observers
        .values()
        .any(|state| !state.targets.is_empty() || !state.queued.is_empty());
    vm.page.effects.intersection_observers_pending =
        vm.page.state.intersection_observers.values().any(|state| {
            !state.queued.is_empty() || state.targets.iter().any(|target| target.previous.is_none())
        });
}

/** Missing boxes and disconnected targets cannot be converted into visible one-pixel rectangles. */
fn measure(
    vm: &mut Vm,
    target: &Observation,
    options: &options::Options,
    known: &BTreeSet<usize>,
    cache: &mut Bounds,
) -> Res<Entry> {
    let (in_scope, base) = root_rect(vm, target.node, options, known, cache)?;
    let margin = options.margins.map(|margin| {
        if margin.percent {
            margin.value * base[2] / 100.0
        } else {
            margin.value
        }
    });
    let root = [
        base[0] - margin[3],
        base[1] - margin[0],
        base[2] + margin[1] + margin[3],
        base[3] + margin[0] + margin[2],
    ];
    let (has_box, bounds) = target_bounds(vm, target.node, known, cache)?;
    let has_box = has_box && in_scope;
    if root
        .iter()
        .chain(&bounds)
        .any(|value| !value.is_finite() || value.abs() > RECT_LIMIT)
    {
        return Err(EngineError::invalid("Invalid IntersectionObserver geometry.").into());
    }
    let left = bounds[0].max(root[0]);
    let top = bounds[1].max(root[1]);
    let right = (bounds[0] + bounds[2]).min(root[0] + root[2]);
    let bottom = (bounds[1] + bounds[3]).min(root[1] + root[3]);
    let intersects = has_box
        && bounds[2] >= 0.0
        && bounds[3] >= 0.0
        && root[2] >= 0.0
        && root[3] >= 0.0
        && right >= left
        && bottom >= top;
    let intersection = if intersects {
        [left, top, right - left, bottom - top]
    } else {
        [0.0; 4]
    };
    let area = bounds[2] * bounds[3];
    let ratio = if area > 0.0 {
        (intersection[2] * intersection[3] / area).clamp(0.0, 1.0)
    } else if intersects {
        1.0
    } else {
        0.0
    };
    Ok(Entry {
        target: target.object,
        node: target.node,
        bounds,
        root: [root[0], root[1], root[2].max(0.0), root[3].max(0.0)],
        intersection,
        ratio,
        intersects,
        time: vm.page.script_now(),
    })
}

/** The root's rectangle before margins, and whether the target can intersect it at all: the
viewport for an implicit root; for an element root its border box, reachable only by targets
inside it, as the spec's containing-block chain check requires.
ponytail: border box, not the content box of a clipping root; refine if a scroller's padding
ever decides whether a row loads. */
fn root_rect(
    vm: &mut Vm,
    target: usize,
    options: &options::Options,
    known: &BTreeSet<usize>,
    cache: &mut Bounds,
) -> Res<(bool, [f64; 4])> {
    let Some(root) = options.root else {
        let environment = &vm.page.environment;
        return Ok((
            true,
            [
                0.0,
                0.0,
                environment.viewport_width,
                environment.viewport_height,
            ],
        ));
    };
    let (has_box, bounds) = target_bounds(vm, root.node, known, cache)?;
    Ok((has_box && descends(vm, target, root.node), bounds))
}

/** Whether `node` is a proper descendant of `ancestor`. */
fn descends(vm: &mut Vm, node: usize, ancestor: usize) -> bool {
    let mut current = vm.page.parent(node);
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        current = vm.page.parent(parent);
    }
    false
}

/** Repeated observers of the same element reuse geometry without changing per-observer margins or thresholds. */
fn target_bounds(
    vm: &mut Vm,
    node: usize,
    known: &BTreeSet<usize>,
    cache: &mut Bounds,
) -> Res<(bool, [f64; 4])> {
    if let Some(value) = cache.get(&node) {
        return Ok(*value);
    }
    let nodes = vm
        .page
        .document
        .as_ref()
        .map_or(0, |document| document.nodes.len());
    vm.charge(
        vm.page
            .environment
            .geometry
            .len()
            .saturating_add(nodes)
            .saturating_add(vm.page.state.detached.len())
            .saturating_add(1),
    )?;
    let has_box = vm.page.connected(node)
        && (matches!(node, super::super::HTML | super::super::BODY) || known.contains(&node));
    let (x, y, width, height) = dom::client_box(vm, node);
    let result = (
        has_box,
        if has_box {
            [x, y, width, height]
        } else {
            [0.0; 4]
        },
    );
    cache.insert(node, result);
    Ok(result)
}

/** A dedicated host observation turn drains only this task source, leaving author timers untouched. */
pub(in crate::v9) fn deliver_all(vm: &mut Vm) -> Res<()> {
    vm.charge(vm.page.state.intersection_observers.len().saturating_add(1))?;
    let ids: Vec<_> = vm
        .page
        .state
        .intersection_observers
        .iter()
        .filter(|(_, state)| !state.queued.is_empty())
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        let result = deliver_id(vm, id).and_then(|_| vm.drain_jobs());
        match result {
            Err(crate::v9::vm::Thrown::Value(error)) => {
                super::super::workers::report_exception(vm, &error)?
            }
            result => result?,
        }
    }
    Ok(())
}

/** Queued records are removed before invoking author code, so reentrant sampling cannot duplicate them. */
fn deliver_id(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let Some(state) = vm.page.state.intersection_observers.get(&id) else {
        return Ok(());
    };
    if state.queued.is_empty() {
        return Ok(());
    }
    let callback = vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&callback_key()))
        .and_then(|prop| match &prop.slot {
            Slot::Data(value) => Some(value.clone()),
            _ => None,
        })
        .ok_or_else(|| EngineError::invalid("Invalid IntersectionObserver callback."))?;
    vm.pin(JsValue::Object(id));
    vm.pin(callback.clone());
    let entries = take_records(vm, id)?;
    vm.call(
        callback,
        JsValue::Object(id),
        vec![entries, JsValue::Object(id)],
    )?;
    Ok(())
}

/** takeRecords drains without invoking the callback and keeps detached targets alive while allocating. */
pub(super) fn take_records(vm: &mut Vm, id: ObjectId) -> Res<JsValue> {
    vm.charge(
        vm.page.state.intersection_observers[&id]
            .queued
            .len()
            .saturating_mul(ENTRY_WORK)
            .saturating_add(1),
    )?;
    let records = std::mem::take(
        &mut vm
            .page
            .state
            .intersection_observers
            .get_mut(&id)
            .expect("brand")
            .queued,
    );
    for record in &records {
        vm.pin(JsValue::Object(record.target));
    }
    let mut values = Vec::new();
    for record in records {
        values.push(entry(vm, record)?);
    }
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Record fields preserve the measured dimensions, including zero-area and edge-adjacent cases. */
fn entry(vm: &mut Vm, record: Entry) -> Res<JsValue> {
    let id = vm.new_object()?;
    vm.pin(JsValue::Object(id));
    vm.put(id, "target", JsValue::Object(record.target))?;
    for (name, bounds) in [
        ("boundingClientRect", record.bounds),
        ("rootBounds", record.root),
        ("intersectionRect", record.intersection),
    ] {
        let rect = rect(vm, bounds)?;
        vm.put(id, name, rect)?;
    }
    vm.put(id, "isIntersecting", JsValue::Boolean(record.intersects))?;
    vm.put(id, "isVisible", JsValue::Boolean(false))?;
    vm.put(id, "intersectionRatio", JsValue::Number(record.ratio))?;
    vm.put(id, "time", JsValue::Number(record.time))?;
    Ok(JsValue::Object(id))
}

/** DOMRect-shaped snapshots contain no live references to mutable native geometry. */
fn rect(vm: &mut Vm, [x, y, width, height]: [f64; 4]) -> Res<JsValue> {
    let id = vm.new_object()?;
    vm.pin(JsValue::Object(id));
    for (name, value) in [
        ("x", x),
        ("y", y),
        ("width", width),
        ("height", height),
        ("top", y),
        ("left", x),
        ("right", x + width),
        ("bottom", y + height),
    ] {
        vm.put(id, name, JsValue::Number(value))?;
    }
    Ok(JsValue::Object(id))
}
