/** Timers on the virtual clock: setTimeout, setInterval, requestAnimationFrame, queueMicrotask. */
use super::Timer;
use super::dom::native_name;
use crate::constants::{ANIMATION_FRAME_MS, TIMER_MIN_DELAY_MS, VM_TIMERS};
use crate::protocol::EngineError;
use crate::v9::value::JsValue;
use crate::v9::vm::data::Job;
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** setTimeout, setInterval, requestAnimationFrame, requestIdleCallback and their cancels. */
pub(in crate::v9) fn timer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    match name {
        "clearTimeout" | "clearInterval" | "cancelAnimationFrame" | "cancelIdleCallback" => {
            let id = vm.to_number(&call.arg(0))?;
            vm.page
                .state
                .timers
                .retain(|timer| f64::from(timer.id) != id);
            return Ok(JsValue::Undefined);
        }
        "queueMicrotask" => {
            let callback = call.arg(0);
            if !vm.is_callable(&callback) {
                return Err(vm.type_error(
                    "Failed to execute 'queueMicrotask': parameter 1 is not of type 'Function'.",
                ));
            }
            vm.enqueue(Job::Callback(callback))?;
            return Ok(JsValue::Undefined);
        }
        _ => {}
    }
    let callback = call.arg(0);
    if !vm.is_callable(&callback) && !matches!(callback, JsValue::String(_)) {
        return Err(vm.type_error(&format!(
            "Failed to execute '{name}': parameter 1 is not a function."
        )));
    }
    if vm.page.state.timers.len() >= VM_TIMERS {
        return Err(EngineError::limit("V9 timers", VM_TIMERS).into());
    }
    let now = vm.page.state.clock;
    let (due, interval, frame, args) = match name {
        "requestAnimationFrame" => (next_frame(now), None, true, Vec::new()),
        "requestIdleCallback" => (now + 1.0, None, false, vec![idle_deadline(vm)?]),
        _ => {
            let delay = vm.to_number(&call.arg(1))?;
            let delay = if delay.is_finite() && delay > 0.0 {
                delay
            } else {
                0.0
            };
            let args = call
                .args
                .get(2..)
                .map(<[JsValue]>::to_vec)
                .unwrap_or_default();
            if name == "setInterval" {
                let delay = delay.max(TIMER_MIN_DELAY_MS);
                (now + delay, Some(delay), false, args)
            } else {
                (now + delay, None, false, args)
            }
        }
    };
    vm.page.state.next_timer += 1;
    let id = vm.page.state.next_timer;
    let order = vm.page.state.next_order;
    vm.page.state.next_order = order.wrapping_add(1);
    vm.page.state.timers.push(Timer {
        id,
        order,
        due,
        interval,
        callback,
        args,
        frame,
    });
    Ok(JsValue::Number(f64::from(id)))
}

/** Queue `callback(...args)` as a zero-delay task (observer deliveries). */
pub(in crate::v9) fn schedule(vm: &mut Vm, callback: JsValue, args: Vec<JsValue>) -> Res<()> {
    if vm.page.state.timers.len() >= VM_TIMERS {
        return Err(EngineError::limit("V9 timers", VM_TIMERS).into());
    }
    vm.page.state.next_timer += 1;
    let id = vm.page.state.next_timer;
    let order = vm.page.state.next_order;
    vm.page.state.next_order = order.wrapping_add(1);
    let due = vm.page.state.clock;
    vm.page.state.timers.push(Timer {
        id,
        order,
        due,
        interval: None,
        callback,
        args,
        frame: false,
    });
    Ok(())
}

/** An IdleDeadline for requestIdleCallback; its prototype's timeRemaining() reports a full idle
period, since V9 has no frame budget to share. */
fn idle_deadline(vm: &mut Vm) -> Res<JsValue> {
    let proto = crate::v9::vm::realm::realm().ids.idle_deadline_prototype;
    let deadline = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    vm.put(deadline, "didTimeout", JsValue::Boolean(false))?;
    Ok(JsValue::Object(deadline))
}

/** The next animation frame boundary after `now`. */
fn next_frame(now: f64) -> f64 {
    ((now / ANIMATION_FRAME_MS).floor() + 1.0) * ANIMATION_FRAME_MS
}

impl Vm<'_> {
    /** The earliest pending timer, if any. */
    pub fn next_timer(&self) -> Option<f64> {
        self.page
            .state
            .timers
            .iter()
            .map(|timer| timer.due)
            .fold(None, |best, due| match best {
                Some(best) if best <= due => Some(best),
                _ => Some(due),
            })
    }
    /** Run timers due by `now`, earliest first, up to `budget` callbacks; then set the clock to `now`. */
    pub fn run_due_timers(&mut self, now: f64, budget: usize) -> Res<usize> {
        let mut ran = 0;
        while ran < budget {
            let due = self
                .page
                .state
                .timers
                .iter()
                .enumerate()
                .filter(|(_, timer)| timer.due <= now)
                .min_by(|(_, a), (_, b)| a.due.total_cmp(&b.due).then(a.order.cmp(&b.order)))
                .map(|(index, _)| index);
            let Some(index) = due else {
                break;
            };
            let timer = self.page.state.timers.remove(index);
            self.page.state.clock = self.page.state.clock.max(timer.due);
            if let Some(interval) = timer.interval {
                let mut next = timer.clone();
                next.due = timer.due + interval;
                next.order = self.page.state.next_order;
                self.page.state.next_order = next.order.wrapping_add(1);
                self.page.state.timers.push(next);
            }
            self.fire_timer(&timer)?;
            self.drain_jobs()?;
            ran += 1;
        }
        self.page.state.clock = self.page.state.clock.max(now);
        Ok(ran)
    }
    /** Run one timer callback; its exceptions go to the console. */
    fn fire_timer(&mut self, timer: &Timer) -> Res<()> {
        let result = match &timer.callback {
            JsValue::String(source) => {
                crate::v9::builtins::evaluate_source(self, source.lossy()).map(|_| ())
            }
            callback => {
                let args = if timer.frame {
                    vec![JsValue::Number(self.page.state.clock)]
                } else {
                    timer.args.clone()
                };
                self.call(callback.clone(), JsValue::Undefined, args)
                    .map(|_| ())
            }
        };
        match result {
            Ok(()) => Ok(()),
            Err(Thrown::Value(error)) => {
                self.report_error(&error);
                Ok(())
            }
            Err(fatal) => Err(fatal),
        }
    }
}
