/* Native Web Animations timing and numeric-opacity effects use the existing page clock, jobs and collector. */
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::{Builder, Static, symbol};
use crate::v9::vm::{Call, Res, Vm};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
mod lifecycle;
mod query;
pub(in crate::v9) use query::shadow as shadow_animations;
mod sampling;
mod state;
mod timing;
pub(in crate::v9) use sampling::{opacity, snapshot, validate};

/** Native rendering consumes identities from the committed projection, never author style mutations. */
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationTarget {
    /// The document element's native formatting wrapper.
    Html,
    /// The body's native formatting wrapper.
    Body,
    /// A stable local identity from this page's validated projection.
    Node(usize),
}
/** One complete sampled animation-layer opacity; absence restores the actual cascade. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpacityEffect {
    /// The paint target in the same committed page generation.
    pub target: AnimationTarget,
    /// Finite sampled alpha, clamped to the CSS opacity range.
    pub value: f64,
}
/** Canonical intrinsic identities are remapped into the executing realm when objects are created. */
struct Ids {
    animation: ObjectId,
    effect: ObjectId,
    tick: u32,
    event: u32,
}
static IDS: OnceLock<Ids> = OnceLock::new();
fn ids() -> &'static Ids {
    IDS.get().expect("animation intrinsics installed")
}
/** Prototypes are ordinary EventTargets with branded native methods and exact promise getters. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let animation = b.object(Some(b.ids.event_target_prototype));
    let ctor = b.constructor("Animation", 0, construct, animation, None);
    b.global("Animation", ctor);
    for name in ["play", "pause", "finish", "cancel", "reverse"] {
        b.method(animation, name, 0, method);
    }
    b.method(animation, "updatePlaybackRate", 1, method);
    for name in ["currentTime", "startTime", "playbackRate", "effect"] {
        b.accessor(animation, name, Some(get), Some(set));
    }
    for name in [
        "playState",
        "pending",
        "finished",
        "ready",
        "replaceState",
        "timeline",
    ] {
        b.accessor(animation, name, Some(get), None);
    }
    b.put(
        animation,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(JsValue::string("Animation").expect("short tag")),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    let effect = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("KeyframeEffect", 1, effect_construct, effect, None);
    b.global("KeyframeEffect", ctor);
    for name in ["getTiming", "getComputedTiming", "getKeyframes"] {
        b.method(effect, name, 0, effect_method);
    }
    b.accessor(effect, "target", Some(effect_get), None);
    b.put(
        effect,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(JsValue::string("KeyframeEffect").expect("short tag")),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    let tick = b.native("", 0, lifecycle::tick);
    let event = b.native("", 0, lifecycle::event);
    let _ = IDS.set(Ids {
        animation,
        effect,
        tick,
        event,
    });
    b.method(b.ids.element_prototype, "animate", 1, animate);
    b.method(b.ids.element_prototype, "getAnimations", 0, query::element);
    b.method(
        b.ids.document_prototype,
        "getAnimations",
        0,
        query::document,
    );
}
/** An effect retains its actual target wrapper and normalized numeric endpoints as traced private data. */
fn new_effect(vm: &mut Vm, effect: state::Effect) -> Res<ObjectId> {
    let id = vm.alloc(Object::new(
        Class::Plain,
        Some(vm.intrinsic_id(ids().effect)),
    ))?;
    vm.pin(JsValue::Object(id));
    effect.save(vm, id);
    vm.check_heap_bounds()?;
    Ok(id)
}
/** Every play cycle starts with a genuinely pending finished promise. */
fn new_animation(vm: &mut Vm, effect: ObjectId, null_effect: bool) -> Res<ObjectId> {
    let id = vm.alloc(Object::new(
        Class::Plain,
        Some(vm.intrinsic_id(ids().animation)),
    ))?;
    vm.pin(JsValue::Object(id));
    let finished = vm.new_promise()?;
    vm.pin(JsValue::Object(finished));
    let ready = vm.promise_resolve(JsValue::Object(id))?;
    state::State {
        effect,
        finished,
        ready,
        start: None,
        hold: None,
        rate: 1.0,
        null_effect,
        idle: true,
        paused: false,
        pending: 0,
        done: false,
        generation: 0,
        timer: None,
        observed: false,
        ready_observed: false,
    }
    .save(vm, id);
    state::write(vm, effect, "owner", JsValue::Object(id));
    for name in ["onfinish", "oncancel", "onremove"] {
        vm.put(id, name, JsValue::Null)?;
    }
    vm.put(id, "id", JsValue::string("")?)?;
    vm.check_heap_bounds()?;
    Ok(id)
}
/** new Animation accepts a genuine KeyframeEffect or null; custom timelines are explicitly unsupported. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Animation constructor requires new"));
    }
    if !call.arg(1).nullish() {
        return Err(vm.unsupported_error("custom animation timelines"));
    }
    let null = call.arg(0).nullish();
    let effect = if null {
        let value = timing::parse(vm, JsValue::Null, JsValue::Null, JsValue::Undefined)?;
        new_effect(vm, value)?
    } else {
        effect_id(vm, &call.arg(0))?
    };
    new_animation(vm, effect, null).map(JsValue::Object)
}
/** Element operations reject document, text and comment receivers even when their methods are borrowed. */
pub(super) fn element_node(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    let node = super::dom::node_of(vm, value)
        .ok_or_else(|| vm.type_error("Animation target must be an Element"))?;
    if node == super::dom::DOCUMENT || vm.page.tag(node).starts_with('#') {
        return Err(vm.type_error("Animation target must be an Element"));
    }
    Ok(node)
}
/** KeyframeEffect construction performs conversion before allocating the branded effect. */
fn effect_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("KeyframeEffect constructor requires new"));
    }
    let target = call.arg(0);
    if !target.nullish() {
        element_node(vm, &target)?;
    }
    let effect = timing::parse(
        vm,
        if target.nullish() {
            JsValue::Null
        } else {
            target
        },
        call.arg(1),
        call.arg(2),
    )?;
    new_effect(vm, effect).map(JsValue::Object)
}
/** Element.animate creates the effect, then initiates actual pending playback. */
fn animate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    element_node(vm, &call.this)?;
    let effect = timing::parse(vm, call.this.clone(), call.arg(0), call.arg(1))?;
    let effect = new_effect(vm, effect)?;
    let id = new_animation(vm, effect, false)?;
    if let JsValue::Object(_) = call.arg(1) {
        let value = vm.get(&call.arg(1), &Key::from("id"))?;
        if value != JsValue::Undefined {
            let text = vm.to_string(&value)?;
            vm.put(id, "id", JsValue::String(text))?;
        }
    }
    lifecycle::control(vm, id, "play")?;
    Ok(JsValue::Object(id))
}
/** Branded methods cannot operate on proxies or objects that merely inherit the prototype. */
fn animation_id(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| state::State::load(vm, *id).is_some())
        .ok_or_else(|| vm.type_error("Illegal Animation invocation"))
}
fn effect_id(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    value
        .object()
        .filter(|id| state::Effect::load(vm, *id).is_some())
        .ok_or_else(|| vm.type_error("Illegal KeyframeEffect invocation"))
}
/** Animation properties reflect the same clock used by timer sampling. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = animation_id(vm, &call.this)?;
    let mut state = state::State::load(vm, id).expect("brand checked");
    let name = super::native_name(&call);
    Ok(match name {
        "currentTime" => state
            .current(vm.page.state.clock)
            .map(JsValue::Number)
            .unwrap_or(JsValue::Null),
        "startTime" => state.start.map(JsValue::Number).unwrap_or(JsValue::Null),
        "playbackRate" => JsValue::Number(state.rate),
        "pending" => JsValue::Boolean(state.pending != 0),
        "effect" => {
            if state.null_effect {
                JsValue::Null
            } else {
                JsValue::Object(state.effect)
            }
        }
        "finished" => {
            state.observed = true;
            state.save(vm, id);
            JsValue::Object(state.finished)
        }
        "ready" => {
            state.ready_observed = true;
            state.save(vm, id);
            JsValue::Object(state.ready)
        }
        "replaceState" => JsValue::string("active")?,
        "timeline" => return Err(vm.unsupported_error("Animation.timeline object reflection")),
        _ => JsValue::string(if state.idle {
            "idle"
        } else if state.paused {
            "paused"
        } else if state.done {
            "finished"
        } else {
            "running"
        })?,
    })
}
/** Unsupported effect replacement is rejected without losing an existing live animation. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = animation_id(vm, &call.this)?;
    let name = super::native_name(&call);
    if name == "effect" {
        return Err(vm.unsupported_error("Animation.effect replacement"));
    }
    lifecycle::seek(vm, id, name, call.arg(0))?;
    Ok(JsValue::Undefined)
}
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = animation_id(vm, &call.this)?;
    let name = super::native_name(&call);
    if name == "updatePlaybackRate" {
        lifecycle::seek(vm, id, "playbackRate", call.arg(0))?
    } else {
        lifecycle::control(vm, id, name)?;
    }
    Ok(JsValue::Undefined)
}
fn effect_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = effect_id(vm, &call.this)?;
    Ok(state::Effect::load(vm, id).expect("brand checked").target)
}
/** Timing results are fresh dictionaries, never mutable aliases of the private playback record. */
fn effect_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = effect_id(vm, &call.this)?;
    let effect = state::Effect::load(vm, id).expect("brand checked");
    let name = super::native_name(&call);
    if name == "getKeyframes" {
        return timing::frame_objects(vm, &effect);
    }
    let object = timing::timing_object(vm, &effect)?;
    if name == "getTiming" {
        return Ok(object);
    }
    let owner = state::read(vm, id, "owner")
        .and_then(|value| value.object())
        .and_then(|owner| state::State::load(vm, owner));
    let time = owner
        .as_ref()
        .and_then(|state| state.current(vm.page.state.clock));
    let target = object.object().expect("timing dictionary");
    vm.put(
        target,
        "activeDuration",
        JsValue::Number(effect.duration * effect.iterations),
    )?;
    vm.put(target, "endTime", JsValue::Number(effect.end()))?;
    vm.put(
        target,
        "localTime",
        time.map(JsValue::Number).unwrap_or(JsValue::Null),
    )?;
    let progress = time.and_then(|time| {
        effect.progress_at(time, owner.as_ref().is_some_and(|state| state.rate < 0.0))
    });
    vm.put(
        target,
        "progress",
        progress
            .map(|p| JsValue::Number(p.0))
            .unwrap_or(JsValue::Null),
    )?;
    vm.put(
        target,
        "currentIteration",
        progress
            .map(|p| JsValue::Number(p.1))
            .unwrap_or(JsValue::Null),
    )?;
    Ok(object)
}
