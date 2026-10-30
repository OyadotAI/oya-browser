/** Persistent ResizeObservers retain branded registrations and deliver only measured layout changes. */
mod delivery;
mod sampling;
mod validation;
use super::{Target, dom, events};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
pub(in crate::v9) use delivery::deliver_all;
pub(in crate::v9) use sampling::sample;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
pub(in crate::v9) use validation::{roots, validate};

/** Per-page admission bounds callback work, retained DOM wrappers and queued measurements. */
const OBSERVERS: usize = 64;
const TARGETS: usize = 256;
const RECORDS: usize = TARGETS;
const SIZE_LIMIT: f64 = 1_000_000_000.0;
static PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();

/** Selected observation boxes compare logical sizes, independently of their visual transform. */
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum BoxKind {
    Content,
    Border,
    DevicePixelContent,
}

/** A target's last broadcast size survives state transfer and does not change until delivery. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    node: usize,
    object: ObjectId,
    kind: BoxKind,
    previous: Option<[f64; 2]>,
}

/** Snapshot metadata contains only actual measurements and genuine target wrappers. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    node: usize,
    target: ObjectId,
    content: [f64; 4],
    content_size: [f64; 2],
    border_size: [f64; 2],
    device_size: [f64; 2],
    observed: [f64; 2],
}

/** Registry state is candidate-owned and participates in heap collection and transport validation. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct State {
    targets: Vec<Observation>,
    queued: Vec<Entry>,
}

/** The dedicated constructor replaces the previous one-shot observer/timer compatibility binding. */
pub(super) fn install(builder: &mut Builder) {
    let prototype = builder.object(Some(builder.ids.object_prototype));
    let constructor = builder.constructor("ResizeObserver", 1, construct, prototype, None);
    builder.global("ResizeObserver", constructor);
    for name in ["observe", "unobserve", "disconnect", "takeRecords"] {
        builder.host_method(
            prototype,
            name,
            u32::from(matches!(name, "observe" | "unobserve")),
            method,
        );
    }
    let _ = PROTOTYPE.set(prototype);
}

/** Hidden callback edges preserve callable closures without exposing mutable author properties. */
fn callback_key() -> Key {
    Key::private("resizeObserverCallback")
}

/** Only construction creates a brand; prototype manipulation cannot register arbitrary receivers. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call
        .new_target
        .as_ref()
        .ok_or_else(|| vm.type_error("ResizeObserver requires 'new'"))?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("ResizeObserver requires a callback"));
    }
    if vm.page.state.resize_observers.len() >= OBSERVERS {
        return Err(EngineError::limit("ResizeObservers", OBSERVERS).into());
    }
    let prototype = vm.prototype_for(target, *PROTOTYPE.get().expect("observer installed"))?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap.put(
        object,
        callback_key(),
        Prop {
            slot: Slot::Data(callback),
            attrs: HIDDEN_ATTRS,
        },
    );
    vm.page.state.resize_observers.insert(
        object,
        State {
            targets: Vec::new(),
            queued: Vec::new(),
        },
    );
    Ok(JsValue::Object(object))
}

/** Genuine local Element wrappers are required, including detached elements that may later be inserted. */
fn element(vm: &mut Vm, value: &JsValue) -> Res<(usize, ObjectId)> {
    if let Some(Target::Node(node)) = events::target_of(vm, value)
        && let Some(object) = value.object()
        && matches!(
            super::host_data(vm, value),
            Some(crate::v9::vm::data::HostData::Node(_))
        )
        && !vm.page.tag(node).starts_with('#')
    {
        return Ok((node, object));
    }
    Err(vm.type_error("ResizeObserver target must be an Element"))
}

/** The box dictionary is converted before mutating registrations, so exceptions leave them intact. */
fn box_kind(vm: &mut Vm, value: &JsValue) -> Res<BoxKind> {
    if matches!(value, JsValue::Undefined | JsValue::Null) {
        return Ok(BoxKind::Content);
    }
    if value.object().is_none() {
        return Err(vm.type_error("ResizeObserver options must be a dictionary"));
    }
    let selected = vm.get(value, &Key::from("box"))?;
    if matches!(selected, JsValue::Undefined) {
        return Ok(BoxKind::Content);
    }
    match vm.to_text(&selected)?.as_str() {
        "content-box" => Ok(BoxKind::Content),
        "border-box" => Ok(BoxKind::Border),
        "device-pixel-content-box" => Ok(BoxKind::DevicePixelContent),
        _ => Err(vm.type_error("Invalid ResizeObserver box option")),
    }
}

/** DOM insertion rewrites native IDs without replacing the observed Element wrapper or its registration. */
pub(super) fn adopt(page: &mut super::Page, old: usize, new: usize) {
    for state in page.state.resize_observers.values_mut() {
        for target in &mut state.targets {
            if target.node == old {
                target.node = new;
            }
        }
        for record in &mut state.queued {
            if record.node == old {
                record.node = new;
            }
        }
    }
}

/** Cancellation clears pending entries as well as registrations; the legacy drain method remains bounded. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = call
        .this
        .object()
        .filter(|id| vm.page.state.resize_observers.contains_key(id))
        .ok_or_else(|| vm.type_error("Illegal ResizeObserver invocation"))?;
    match dom::native_name(&call) {
        "takeRecords" => return delivery::take_records(vm, id),
        "disconnect" => {
            let state = vm.page.state.resize_observers.get_mut(&id).expect("brand");
            state.targets.clear();
            state.queued.clear();
        }
        name => {
            let (node, object) = element(vm, &call.arg(0))?;
            let kind = if name == "observe" {
                Some(box_kind(vm, &call.arg(1))?)
            } else {
                None
            };
            let existing = vm.page.state.resize_observers[&id]
                .targets
                .iter()
                .any(|target| target.node == node);
            let count: usize = vm
                .page
                .state
                .resize_observers
                .values()
                .map(|state| state.targets.len())
                .sum();
            if kind.is_some() && !existing && count >= TARGETS {
                return Err(EngineError::limit("ResizeObserver targets", TARGETS).into());
            }
            let state = vm.page.state.resize_observers.get_mut(&id).expect("brand");
            state.targets.retain(|target| target.node != node);
            state.queued.retain(|entry| entry.node != node);
            if let Some(kind) = kind {
                state.targets.push(Observation {
                    node,
                    object,
                    kind,
                    previous: None,
                });
            }
        }
    }
    Ok(JsValue::Undefined)
}
