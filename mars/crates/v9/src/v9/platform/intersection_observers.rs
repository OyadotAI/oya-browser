/** Persistent viewport observations use measured boxes and context-owned, branded registrations. */
mod budget;
mod options;
mod sampling;
mod validation;
use super::{Target, dom, events};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, symbol};
use crate::v9::vm::{Call, Res, Vm};
pub(in crate::v9) use sampling::{deliver_all, effects, sample};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
pub(in crate::v9) use validation::{roots, validate, validate_summary};

use budget::{OBSERVERS, RECORDS, TARGETS};
const THRESHOLDS: usize = 128;
const MARGIN_LIMIT: f64 = 1_000_000.0;
const RECT_LIMIT: f64 = 1_000_000_000.0;

/** Realms share intrinsic code identities while their observer registrations stay independent. */
struct Ids {
    prototype: ObjectId,
}
static IDS: OnceLock<Ids> = OnceLock::new();

/** Last observable intersection state; only threshold or intersection changes enqueue an entry. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Previous {
    threshold: usize,
    intersects: bool,
}

/** Target wrappers remain alive while registered, including when temporarily detached. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    node: usize,
    object: ObjectId,
    previous: Option<Previous>,
}

/** Queued records capture actual geometry at observation time, before later DOM mutation. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    target: ObjectId,
    node: usize,
    bounds: [f64; 4],
    root: [f64; 4],
    intersection: [f64; 4],
    ratio: f64,
    intersects: bool,
    time: f64,
}

/** Inert page state is cloned and rolled back together with the heap and document candidate. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct State {
    options: options::Options,
    targets: Vec<Observation>,
    queued: Vec<Entry>,
}

/** Only this constructor creates brands; replacing a prototype or constructor name grants nothing. */
pub(super) fn install(b: &mut Builder) {
    let prototype = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("IntersectionObserver", 1, construct, prototype, None);
    b.global("IntersectionObserver", ctor);
    for name in ["observe", "unobserve", "disconnect", "takeRecords"] {
        b.host_method(
            prototype,
            name,
            u32::from(matches!(name, "observe" | "unobserve")),
            method,
        );
    }
    for name in [
        "root",
        "rootMargin",
        "thresholds",
        "scrollMargin",
        "delay",
        "trackVisibility",
    ] {
        b.getter(prototype, name, get);
    }
    let _ = IDS.set(Ids { prototype });
}

/** Keep the callback as a private heap edge so inactive but reachable observers retain it. */
fn callback_key() -> Key {
    Key::private("intersectionObserverCallback")
}

/** Convert bounded options before allocation so failed construction cannot leave a partial brand. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call
        .new_target
        .as_ref()
        .ok_or_else(|| vm.type_error("IntersectionObserver requires 'new'"))?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("IntersectionObserver requires a callback"));
    }
    vm.pin(callback.clone());
    vm.pin(target.clone());
    let options = options::parse(vm, &call.arg(1))?;
    let prototype = vm.prototype_for(target, IDS.get().expect("observer installed").prototype)?;
    vm.pin(JsValue::Object(prototype));
    // Prototype/options getters can reenter construction; admission runs only after all author code.
    budget::admit(vm, budget::Usage::observer(options.thresholds.len()))?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap.put(
        object,
        callback_key(),
        Prop {
            slot: Slot::Data(callback),
            attrs: HIDDEN_ATTRS,
        },
    );
    vm.page.state.intersection_observers.insert(
        object,
        State {
            options,
            targets: Vec::new(),
            queued: Vec::new(),
        },
    );
    Ok(JsValue::Object(object))
}

/** Every method and getter rejects forged or cross-context receiver objects. */
fn identity(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| vm.page.state.intersection_observers.contains_key(id))
        .ok_or_else(|| vm.type_error("Illegal IntersectionObserver invocation"))
}

/** EventTargets such as Window and Document are not Elements and cannot be observed. */
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
    Err(vm.type_error("IntersectionObserver target must be an Element"))
}

/** Removing a registration stops future observations; already queued records retain their lifetime. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    vm.charge(
        vm.page.state.intersection_observers[&id]
            .targets
            .len()
            .saturating_add(1),
    )?;
    match dom::native_name(&call) {
        "takeRecords" => return sampling::take_records(vm, id),
        "disconnect" => {
            vm.page
                .state
                .intersection_observers
                .get_mut(&id)
                .expect("brand")
                .targets = Vec::new();
        }
        name => {
            let (node, object) = element(vm, &call.arg(0))?;
            if name == "unobserve" {
                let targets = &mut vm
                    .page
                    .state
                    .intersection_observers
                    .get_mut(&id)
                    .expect("brand")
                    .targets;
                targets.retain(|target| target.node != node);
                // Keep retained capacity within the two-copy storage allowance after removals.
                if targets.capacity() > targets.len().saturating_mul(2) {
                    targets.shrink_to_fit();
                }
            } else {
                observe(vm, id, node, object)?;
            }
        }
    }
    Ok(JsValue::Undefined)
}

/** Duplicate observe calls preserve previous thresholds and never enqueue extra initial entries. */
fn observe(vm: &mut Vm, id: ObjectId, node: usize, object: ObjectId) -> Res<()> {
    vm.pin(JsValue::Object(id));
    vm.pin(JsValue::Object(object));
    vm.charge(
        vm.page.state.intersection_observers[&id]
            .targets
            .len()
            .saturating_add(1),
    )?;
    if vm.page.state.intersection_observers[&id]
        .targets
        .iter()
        .any(|target| target.node == node)
    {
        return Ok(());
    }
    budget::admit(vm, budget::Usage::target())?;
    vm.page
        .state
        .intersection_observers
        .get_mut(&id)
        .expect("brand")
        .targets
        .push(Observation {
            node,
            object,
            previous: None,
        });
    Ok(())
}

/** Read-only configuration is sourced from native state, never mutable author properties. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    let options = vm.page.state.intersection_observers[&id].options.clone();
    match dom::native_name(&call) {
        "root" => Ok(JsValue::Null),
        "rootMargin" => Ok(JsValue::string(&options.margin_text())?),
        "scrollMargin" => Ok(JsValue::string("0px 0px 0px 0px")?),
        "delay" => Ok(JsValue::Number(0.0)),
        "trackVisibility" => Ok(JsValue::Boolean(false)),
        _ => {
            let array = vm.new_array(
                options
                    .thresholds
                    .iter()
                    .copied()
                    .map(JsValue::Number)
                    .collect(),
            )?;
            crate::v9::builtins::freeze_object(vm, array)?;
            Ok(JsValue::Object(array))
        }
    }
}
