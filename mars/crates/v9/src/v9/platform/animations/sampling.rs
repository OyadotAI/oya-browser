/* Samples form a separate animation style layer; no authored attribute or inline declaration is rewritten. */
use super::state::{Effect, State};

use super::*;
use crate::constants::VM_TIMERS;
use crate::protocol::{EngineError, EngineResult};
use std::collections::{BTreeMap, BTreeSet};

/** A contribution uses one actual playback record and its normalized effect. */
fn contribution(vm: &Vm, id: ObjectId) -> Option<(usize, f64)> {
    let animation = State::load(vm, id)?;
    if animation.idle || animation.null_effect {
        return None;
    }
    let effect = Effect::load(vm, animation.effect)?;
    let node = super::super::dom::node_of(vm, &effect.target)?;
    let progress = effect
        .progress_at(
            animation.current(vm.page.state.clock)?,
            animation.rate < 0.0,
        )?
        .0;
    let value = (effect.from? * (1.0 - progress) + effect.to? * progress).clamp(0.0, 1.0);
    value.is_finite().then_some((node, value))
}
/** Later active animations replace earlier opacity contributions on the same element. */
pub(in crate::v9) fn opacity(vm: &Vm, node: usize) -> Option<f64> {
    vm.page.state.active_animations.iter().rev().find_map(|id| {
        contribution(vm, *id)
            .filter(|(target, _)| *target == node)
            .map(|(_, value)| value)
    })
}
/** Only committed projection identities enter host rendering; detached effects still keep genuine temporal state. */
pub(in crate::v9) fn snapshot(vm: &mut Vm) -> Vec<OpacityEffect> {
    let mut result = BTreeMap::new();
    for id in vm.page.state.active_animations.clone() {
        let Some((node, value)) = contribution(vm, id) else {
            continue;
        };
        let target = if node == super::super::HTML {
            AnimationTarget::Html
        } else if node == super::super::BODY {
            AnimationTarget::Body
        } else if vm.page.is_projected(node) && vm.page.connected(node) {
            AnimationTarget::Node(node)
        } else {
            continue;
        };
        result.insert(target, value);
    }
    result
        .into_iter()
        .map(|(target, value)| OpacityEffect { target, value })
        .collect()
}
/** Private marks grant no host authority; every inert cell, Promise edge and scheduling link is checked on restore. */
pub(in crate::v9) fn validate(vm: &Vm) -> EngineResult<()> {
    let invalid = || EngineError::invalid("Invalid V9 animation state.");
    if vm.page.state.active_animations.len() > VM_TIMERS {
        return Err(invalid());
    }
    let mut active = BTreeSet::new();
    for id in &vm.page.state.active_animations {
        if !active.insert(*id) || State::load(vm, *id).is_none_or(|state| state.idle) {
            return Err(invalid());
        }
    }
    for (id, object) in &vm.heap.objects {
        let branded = state::read(vm, *id, "brand").is_some();
        let effect = state::read(vm, *id, "effectBrand").is_some();
        if effect && (branded || Effect::load(vm, *id).is_none()) {
            return Err(invalid());
        }
        if !branded {
            continue;
        }
        if !matches!(object.class, Class::Plain) {
            return Err(invalid());
        }
        let state = State::load(vm, *id).ok_or_else(invalid)?;
        if Effect::load(vm, state.effect).is_none() {
            return Err(invalid());
        }
        if [state.finished, state.ready].iter().any(|id| {
            !matches!(
                vm.heap.get(*id).map(|object| &object.class),
                Some(Class::Promise(_))
            )
        }) {
            return Err(invalid());
        }
        if state.idle
            && (state.start.is_some()
                || state.hold.is_some()
                || state.pending != 0
                || state.timer.is_some())
            || state.paused && state.start.is_some()
        {
            return Err(invalid());
        }
        if let Some(timer) = state.timer {
            let timer = vm
                .page
                .state
                .timers
                .iter()
                .find(|entry| entry.id == timer)
                .ok_or_else(invalid)?;
            let Some(callee) = timer.callback.object() else {
                return Err(invalid());
            };
            let slots = vm.slots(callee);
            if slots
                != vec![
                    JsValue::Object(*id),
                    JsValue::Number(f64::from(state.generation)),
                ]
            {
                return Err(invalid());
            }
        }
        if !state.idle && !state.done && !active.contains(id) {
            return Err(invalid());
        }
    }
    Ok(())
}
