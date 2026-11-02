/* Native animation tasks run on the page's virtual clock and settle real Promise reaction jobs. */
use super::state::{Effect, State};

use super::*;
use crate::constants::{ANIMATION_FRAME_MS, VM_TIMERS};
use crate::v9::vm::data::PromiseState;

/** Playing or paused effects remain reachable until their visible contribution ends or they are cancelled. */
fn register(vm: &mut Vm, id: ObjectId) -> Res<()> {
    if !vm.page.state.active_animations.contains(&id) {
        if vm.page.state.active_animations.len() >= VM_TIMERS {
            return Err(
                crate::protocol::EngineError::limit("V9 active animations", VM_TIMERS).into(),
            );
        }
        vm.page.state.active_animations.push(id);
    }
    Ok(())
}
/** Replacing a scheduled sample removes the old timer instead of consuming stale timer quota. */
fn unschedule(vm: &mut Vm, state: &mut State) -> Res<()> {
    if let Some(timer) = state.timer.take() {
        vm.page.state.timers.retain(|pending| pending.id != timer)
    }
    state.generation = state.generation.checked_add(1).ok_or_else(|| {
        crate::protocol::EngineError::limit("V9 animation generations", u32::MAX as usize)
    })?;
    Ok(())
}
/** A native callback holds the actual Animation and its generation across state transport. */
fn schedule(vm: &mut Vm, id: ObjectId, state: &mut State, due: f64) -> Res<()> {
    if !due.is_finite() {
        return Err(vm.type_error("Animation sample time is not finite"));
    }
    if vm.page.state.timers.len() >= VM_TIMERS {
        return Err(crate::protocol::EngineError::limit("V9 timers", VM_TIMERS).into());
    }
    let callback = vm.native_closure(
        ids().tick,
        vec![
            JsValue::Object(id),
            JsValue::Number(f64::from(state.generation)),
        ],
    )?;
    let timer = vm
        .page
        .state
        .next_timer
        .checked_add(1)
        .ok_or_else(|| crate::protocol::EngineError::limit("V9 timers", VM_TIMERS))?;
    vm.page.state.next_timer = timer;
    let order = vm.page.state.next_order;
    vm.page.state.next_order = order.wrapping_add(1);
    vm.page.state.timers.push(super::super::Timer {
        id: timer,
        order,
        due,
        interval: None,
        callback: JsValue::Object(callback),
        args: Vec::new(),
        frame: false,
        nesting: 0,
    });
    state.timer = Some(timer);
    Ok(())
}
/** Zero playback rate holds time without scheduling an endless timer. */
fn next_sample(vm: &mut Vm, id: ObjectId, state: &mut State, effect: &Effect) -> Res<()> {
    if state.idle || state.paused || state.done || state.rate == 0.0 {
        return Ok(());
    }
    let now = vm.page.state.clock;
    let boundary = if state.rate > 0.0 { effect.end() } else { 0.0 };
    let current = state.current(now).unwrap_or(0.0);
    let due = now + ((boundary - current) / state.rate).clamp(0.0, ANIMATION_FRAME_MS);
    schedule(vm, id, state, due)
}
/** End state follows time and direction, not a timer count or an arbitrary successful Promise. */
fn ended(state: &State, effect: &Effect, now: f64) -> bool {
    state.current(now).is_some_and(|time| {
        state.rate > 0.0 && time >= effect.end() || state.rate < 0.0 && time <= 0.0
    })
}
/** A completed play cycle keeps its own finished Promise; replay creates a fresh one. */
fn complete(vm: &mut Vm, id: ObjectId, state: &mut State, effect: &Effect) -> Res<()> {
    state.hold = Some(if state.rate < 0.0 { 0.0 } else { effect.end() });
    state.start = None;
    state.done = true;
    state.pending = 0;
    vm.settle(state.ready, PromiseState::Fulfilled(JsValue::Object(id)))?;
    vm.settle(state.finished, PromiseState::Fulfilled(JsValue::Object(id)))?;
    if effect
        .progress_at(state.hold.unwrap_or(0.0), state.rate < 0.0)
        .is_none()
    {
        vm.page
            .state
            .active_animations
            .retain(|current| *current != id)
    }
    let callback = vm.native_closure(
        ids().event,
        vec![
            JsValue::Object(id),
            JsValue::string("finish")?,
            JsValue::Number(f64::from(state.generation)),
        ],
    )?;
    super::super::schedule_task(vm, JsValue::Object(callback), Vec::new())?;
    Ok(())
}
/** Native sampling resolves pending play/pause readiness before completion, then schedules the next actual time. */
pub(super) fn tick(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let captured = vm.slots(call.callee);
    let Some(id) = captured.first().and_then(JsValue::object) else {
        return Ok(JsValue::Undefined);
    };
    let Some(mut state) = State::load(vm, id) else {
        return Ok(JsValue::Undefined);
    };
    if captured.get(1) != Some(&JsValue::Number(f64::from(state.generation))) {
        return Ok(JsValue::Undefined);
    }
    state.timer = None;
    let effect =
        Effect::load(vm, state.effect).ok_or_else(|| vm.type_error("Invalid animation effect"))?;
    if state.pending != 0 {
        if state.pending == 1 && state.rate != 0.0 {
            state.start = Some(vm.page.state.clock - state.hold.unwrap_or(0.0) / state.rate);
            state.hold = None;
        }
        state.pending = 0;
        vm.settle(state.ready, PromiseState::Fulfilled(JsValue::Object(id)))?;
    }
    if !state.paused && !state.idle && ended(&state, &effect, vm.page.state.clock) {
        complete(vm, id, &mut state, &effect)?
    } else {
        next_sample(vm, id, &mut state, &effect)?
    }
    state.save(vm, id);
    Ok(JsValue::Undefined)
}
/** Completion and cancellation events are separate tasks, while Promise reactions are microtasks. */
pub(super) fn event(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let captured = vm.slots(call.callee);
    let Some(id) = captured.first().and_then(JsValue::object) else {
        return Ok(JsValue::Undefined);
    };
    let Some(state) = State::load(vm, id) else {
        return Ok(JsValue::Undefined);
    };
    if captured.get(2) != Some(&JsValue::Number(f64::from(state.generation))) {
        return Ok(JsValue::Undefined);
    }
    let kind = match captured.get(1) {
        Some(JsValue::String(value)) => value.lossy(),
        _ => return Ok(JsValue::Undefined),
    };
    super::super::events::fire(vm, super::super::Target::Object(id), &kind, false, false)?;
    Ok(JsValue::Undefined)
}
/** Explicit playback controls change time anchors and their actual scheduled tasks. */
pub(super) fn control(vm: &mut Vm, id: ObjectId, name: &str) -> Res<()> {
    let mut state =
        State::load(vm, id).ok_or_else(|| vm.type_error("Illegal Animation invocation"))?;
    let effect =
        Effect::load(vm, state.effect).ok_or_else(|| vm.type_error("Invalid animation effect"))?;
    let now = vm.page.state.clock;
    let current = state.current(now);
    if name == "play" && !state.idle && !state.paused && !state.done && state.pending != 2
        || name == "pause" && state.paused
    {
        return Ok(());
    }
    if name == "finish" {
        if state.done {
            return Ok(());
        }
        if state.rate == 0.0 {
            return Err(vm.dom_exception(
                "InvalidStateError",
                "Cannot finish an animation with zero playback rate",
            ));
        }
        unschedule(vm, &mut state)?;
        state.idle = false;
        state.paused = false;
        register(vm, id)?;
        complete(vm, id, &mut state, &effect)?;
        state.save(vm, id);
        return Ok(());
    }
    if name == "cancel" {
        cancel(vm, id, &mut state)?;
        state.save(vm, id);
        return Ok(());
    }
    unschedule(vm, &mut state)?;
    if name == "reverse" {
        if state.rate == 0.0 {
            return Err(vm.dom_exception(
                "InvalidStateError",
                "Cannot reverse an animation with zero playback rate",
            ));
        }
        state.rate = -state.rate;
    }
    if state.done {
        state.finished = vm.new_promise()?;
        state.observed = false;
        state.done = false;
    }
    state.hold = Some(current.unwrap_or(if state.rate < 0.0 { effect.end() } else { 0.0 }));
    if name != "pause"
        && current.is_some_and(|time| {
            state.rate > 0.0 && time >= effect.end() || state.rate < 0.0 && time <= 0.0
        })
    {
        state.hold = Some(if state.rate < 0.0 { effect.end() } else { 0.0 })
    }
    state.start = None;
    state.idle = false;
    state.paused = name == "pause";
    if state.pending == 0 {
        state.ready = vm.new_promise()?;
        state.ready_observed = false;
    }
    state.pending = if state.paused { 2 } else { 1 };
    register(vm, id)?;
    let due = ((now / ANIMATION_FRAME_MS).floor() + 1.0) * ANIMATION_FRAME_MS;
    schedule(vm, id, &mut state, due)?;
    state.save(vm, id);
    Ok(())
}
/** Cancellation rejects only an observed old promise, drops the contribution, and preserves a replayable idle object. */
fn cancel(vm: &mut Vm, id: ObjectId, state: &mut State) -> Res<()> {
    if state.idle {
        return Ok(());
    }
    unschedule(vm, state)?;
    if state.ready_observed && state.pending != 0 {
        let error = vm.dom_exception("AbortError", "The animation was cancelled");
        let crate::v9::vm::Thrown::Value(error) = error else {
            return Err(error);
        };
        vm.settle(state.ready, PromiseState::Rejected(error))?;
    }
    if state.observed && !state.done && !state.idle {
        let error = vm.dom_exception("AbortError", "The animation was cancelled");
        let crate::v9::vm::Thrown::Value(error) = error else {
            return Err(error);
        };
        vm.settle(state.finished, PromiseState::Rejected(error))?;
    }
    state.finished = vm.new_promise()?;
    state.ready = vm.promise_resolve(JsValue::Object(id))?;
    state.observed = false;
    state.ready_observed = false;
    state.hold = None;
    state.start = None;
    state.idle = true;
    state.paused = false;
    state.pending = 0;
    state.done = false;
    vm.page
        .state
        .active_animations
        .retain(|current| *current != id);
    let callback = vm.native_closure(
        ids().event,
        vec![
            JsValue::Object(id),
            JsValue::string("cancel")?,
            JsValue::Number(f64::from(state.generation)),
        ],
    )?;
    super::super::schedule_task(vm, JsValue::Object(callback), Vec::new())
}
/** Seeking and rate updates preserve the sampled position instead of restarting from zero. */
pub(super) fn seek(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) -> Res<()> {
    let mut state =
        State::load(vm, id).ok_or_else(|| vm.type_error("Illegal Animation invocation"))?;
    let effect =
        Effect::load(vm, state.effect).ok_or_else(|| vm.type_error("Invalid animation effect"))?;
    let now = vm.page.state.clock;
    if value.nullish() {
        return Err(vm.type_error("Unresolved animation time is not supported by this setter"));
    }
    let number = vm.to_number(&value)?;
    if !number.is_finite() {
        return Err(vm.type_error("Animation time or rate must be finite"));
    }
    let current = state.current(now).unwrap_or(0.0);
    unschedule(vm, &mut state)?;
    match name {
        "playbackRate" => {
            state.rate = number;
            state.hold = Some(current);
            if !state.paused && !state.idle && number != 0.0 {
                state.start = Some(now - current / number);
                state.hold = None;
            }
        }
        "startTime" => {
            state.start = Some(number);
            state.hold = None;
            state.idle = false;
            state.pending = 0;
        }
        _ => {
            state.hold = Some(number);
            state.idle = false;
            if !state.paused && state.pending == 0 && state.rate != 0.0 {
                state.start = Some(now - number / state.rate);
                state.hold = None;
            }
        }
    }
    if state.done && !ended(&state, &effect, now) {
        state.done = false;
        state.finished = vm.new_promise()?;
        state.observed = false;
    }
    if !state.idle {
        register(vm, id)?;
    }
    if state.pending != 0 {
        schedule(
            vm,
            id,
            &mut state,
            ((now / ANIMATION_FRAME_MS).floor() + 1.0) * ANIMATION_FRAME_MS,
        )?
    } else if !state.paused && ended(&state, &effect, now) {
        if !state.done {
            complete(vm, id, &mut state, &effect)?
        }
    } else {
        next_sample(vm, id, &mut state, &effect)?
    }
    state.save(vm, id);
    Ok(())
}
