/* Private animation records remain ordinary traced heap data and are validated before restored execution. */
use super::*;
use crate::v9::vm::heap::{Prop, Slot};

/** Timing and playback cells shared by every Animation operation. */
#[derive(Clone)]
pub(super) struct State {
    pub effect: ObjectId,
    pub finished: ObjectId,
    pub ready: ObjectId,
    pub start: Option<f64>,
    pub hold: Option<f64>,
    pub rate: f64,
    pub null_effect: bool,
    pub idle: bool,
    pub paused: bool,
    pub pending: u8,
    pub done: bool,
    pub generation: u32,
    pub timer: Option<u32>,
    pub observed: bool,
    pub ready_observed: bool,
}
/** Normalized supported effect; numeric opacity is the first native composited property. */
#[derive(Clone)]
pub(super) struct Effect {
    pub target: JsValue,
    pub duration: f64,
    pub delay: f64,
    pub end_delay: f64,
    pub iterations: f64,
    pub fill: String,
    pub direction: String,
    pub from: Option<f64>,
    pub to: Option<f64>,
}
/** An unspellable key prevents author objects from acquiring the native brand. */
fn key(field: &str) -> Key {
    Key::private(&format!("animation.{field}"))
}
/** Internal cells never run authored getters or setters. */
pub(super) fn read(vm: &Vm, id: ObjectId, field: &str) -> Option<JsValue> {
    match &vm.heap.get(id)?.props.get(&key(field))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}
/** Heap insertion performs the usual property accounting. */
pub(super) fn write(vm: &mut Vm, id: ObjectId, field: &str, value: JsValue) {
    vm.heap.put(
        id,
        key(field),
        Prop {
            slot: Slot::Data(value),
            attrs: 0,
        },
    );
}
/** Numeric state is finite; null represents an unresolved time. */
fn number(vm: &Vm, id: ObjectId, field: &str) -> Option<f64> {
    match read(vm, id, field)? {
        JsValue::Number(value) if value.is_finite() => Some(value),
        _ => None,
    }
}
/** Null and finite numeric times have no executable wire representation. */
fn time(vm: &Vm, id: ObjectId, field: &str) -> Option<Option<f64>> {
    match read(vm, id, field)? {
        JsValue::Null => Some(None),
        JsValue::Number(value) if value.is_finite() => Some(Some(value)),
        _ => None,
    }
}
/** Native object links must resolve to actual heap records. */
fn object(vm: &Vm, id: ObjectId, field: &str) -> Option<ObjectId> {
    read(vm, id, field)?
        .object()
        .filter(|id| vm.heap.get(*id).is_some())
}
/** Flags have exact Boolean representation rather than JavaScript coercion. */
fn boolean(vm: &Vm, id: ObjectId, field: &str) -> Option<bool> {
    match read(vm, id, field)? {
        JsValue::Boolean(value) => Some(value),
        _ => None,
    }
}
/** Counters cannot wrap or accept fractional forged snapshots. */
fn counter(vm: &Vm, id: ObjectId, field: &str) -> Option<u32> {
    let value = number(vm, id, field)?;
    (value >= 0.0 && value <= u32::MAX as f64 && value.fract() == 0.0).then_some(value as u32)
}
/** Text enum cells are inert strings, checked against supported values by Effect::load. */
fn text(vm: &Vm, id: ObjectId, field: &str) -> Option<String> {
    match read(vm, id, field)? {
        JsValue::String(value) => Some(value.lossy()),
        _ => None,
    }
}
/** Resolve native Animation state without invoking author code. */
impl State {
    pub fn load(vm: &Vm, id: ObjectId) -> Option<Self> {
        if read(vm, id, "brand")? != JsValue::Boolean(true) {
            return None;
        }
        let pending = counter(vm, id, "pending")?;
        if pending > 2 {
            return None;
        }
        let timer = time(vm, id, "timer")?;
        let timer = match timer {
            Some(value) if value >= 1.0 && value <= u32::MAX as f64 && value.fract() == 0.0 => {
                Some(value as u32)
            }
            Some(_) => return None,
            None => None,
        };
        Some(Self {
            effect: object(vm, id, "effect")?,
            finished: object(vm, id, "finished")?,
            ready: object(vm, id, "ready")?,
            start: time(vm, id, "start")?,
            hold: time(vm, id, "hold")?,
            rate: number(vm, id, "rate")?,
            null_effect: boolean(vm, id, "nullEffect")?,
            idle: boolean(vm, id, "idle")?,
            paused: boolean(vm, id, "paused")?,
            pending: pending as u8,
            done: boolean(vm, id, "done")?,
            generation: counter(vm, id, "generation")?,
            timer,
            observed: boolean(vm, id, "observed")?,
            ready_observed: boolean(vm, id, "readyObserved")?,
        })
    }
    /** All state edges live beside the brand so ordinary collector traversal reaches them. */
    pub fn save(&self, vm: &mut Vm, id: ObjectId) {
        write(vm, id, "brand", JsValue::Boolean(true));
        for (name, value) in [
            ("effect", self.effect),
            ("finished", self.finished),
            ("ready", self.ready),
        ] {
            write(vm, id, name, JsValue::Object(value));
        }
        for (name, value) in [
            ("start", self.start),
            ("hold", self.hold),
            ("timer", self.timer.map(f64::from)),
        ] {
            write(
                vm,
                id,
                name,
                value.map(JsValue::Number).unwrap_or(JsValue::Null),
            );
        }
        for (name, value) in [
            ("rate", self.rate),
            ("pending", f64::from(self.pending)),
            ("generation", f64::from(self.generation)),
        ] {
            write(vm, id, name, JsValue::Number(value));
        }
        for (name, value) in [
            ("nullEffect", self.null_effect),
            ("idle", self.idle),
            ("paused", self.paused),
            ("done", self.done),
            ("observed", self.observed),
            ("readyObserved", self.ready_observed),
        ] {
            write(vm, id, name, JsValue::Boolean(value));
        }
    }
    /** Hold time wins while paused or waiting for a play task; running time follows the real page clock. */
    pub fn current(&self, now: f64) -> Option<f64> {
        self.hold
            .or_else(|| self.start.map(|start| (now - start) * self.rate))
    }
}
impl Effect {
    pub fn load(vm: &Vm, id: ObjectId) -> Option<Self> {
        if read(vm, id, "effectBrand")? != JsValue::Boolean(true) {
            return None;
        }
        let duration = number(vm, id, "duration")?;
        let iterations = number(vm, id, "iterations")?;
        if duration < 0.0 || iterations < 0.0 || !(duration * iterations).is_finite() {
            return None;
        }
        let fill = text(vm, id, "fill")?;
        let direction = text(vm, id, "direction")?;
        if !["auto", "none", "both", "forwards", "backwards"].contains(&fill.as_str())
            || !["normal", "reverse", "alternate", "alternate-reverse"]
                .contains(&direction.as_str())
        {
            return None;
        }
        let target = read(vm, id, "target")?;
        if target != JsValue::Null && super::super::dom::node_of(vm, &target).is_none() {
            return None;
        }
        let from = time(vm, id, "from")?;
        let to = time(vm, id, "to")?;
        if from.is_some() != to.is_some() {
            return None;
        }
        Some(Self {
            target,
            duration,
            delay: number(vm, id, "delay")?,
            end_delay: number(vm, id, "endDelay")?,
            iterations,
            fill,
            direction,
            from,
            to,
        })
    }
    pub fn save(&self, vm: &mut Vm, id: ObjectId) {
        write(vm, id, "effectBrand", JsValue::Boolean(true));
        write(vm, id, "target", self.target.clone());
        for (name, value) in [
            ("duration", self.duration),
            ("delay", self.delay),
            ("endDelay", self.end_delay),
            ("iterations", self.iterations),
        ] {
            write(vm, id, name, JsValue::Number(value));
        }
        for (name, value) in [("fill", &self.fill), ("direction", &self.direction)] {
            write(
                vm,
                id,
                name,
                JsValue::string(value).expect("supported timing enum"),
            );
        }
        for (name, value) in [("from", self.from), ("to", self.to)] {
            write(
                vm,
                id,
                name,
                value.map(JsValue::Number).unwrap_or(JsValue::Null),
            );
        }
    }
    pub fn end(&self) -> f64 {
        (self.delay + self.duration * self.iterations + self.end_delay).max(0.0)
    }
    /** Phase boundary equality follows playback direction, including reverse completion at zero. */
    pub fn progress_at(&self, time: f64, backwards: bool) -> Option<(f64, f64)> {
        let end = (self.delay + self.duration * self.iterations).min(self.end());
        let before_boundary = self.delay.min(self.end()).max(0.0);
        let after_boundary = end.max(0.0);
        let before = time < before_boundary || backwards && time == before_boundary;
        let after = time > after_boundary || !backwards && time == after_boundary;
        if before && !["backwards", "both"].contains(&self.fill.as_str())
            || after && !["forwards", "both"].contains(&self.fill.as_str())
        {
            return None;
        }
        let active = if before {
            0.0
        } else {
            (time - self.delay)
                .max(0.0)
                .min((end - self.delay).max(0.0))
        };
        let total = if self.duration == 0.0 {
            if before { 0.0 } else { self.iterations }
        } else {
            active / self.duration
        };
        let mut iteration = total.floor();
        let mut progress = total.fract();
        if (after || backwards && total == self.iterations) && total > 0.0 && progress == 0.0 {
            iteration -= 1.0;
            progress = 1.0;
        }
        if self.direction == "reverse"
            || self.direction == "alternate" && iteration % 2.0 == 1.0
            || self.direction == "alternate-reverse" && iteration % 2.0 == 0.0
        {
            progress = 1.0 - progress;
        }
        Some((progress, iteration.max(0.0)))
    }
}
