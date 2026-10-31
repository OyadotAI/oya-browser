/** Timers on the virtual clock: setTimeout, setInterval, requestAnimationFrame, queueMicrotask. */
use super::Timer;
use super::dom::native_name;
use crate::constants::{ANIMATION_FRAME_MS, TIMER_MIN_DELAY_MS, TIMER_SLICE, VM_TIMERS};
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
            #[cfg(feature = "conformance")]
            if vm.page.environment.conformance_d8 {
                crate::v9::builtins::enqueue_shell_microtask(vm, callback)?;
                return Ok(JsValue::Undefined);
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
            // HTML clamps a timeout scheduled more than NESTING_CLAMP levels deep to at least
            // TIMER_MIN_DELAY_MS. Without it the virtual clock never moved past a stream of
            // zero-delay timeouts and animation frames starved (YouTube's scheduler waited on one
            // forever, so its icons and thumbnails never rendered).
            let delay = if nesting(vm) > NESTING_CLAMP {
                delay.max(TIMER_MIN_DELAY_MS)
            } else {
                delay
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
    let nesting = if frame || name == "requestIdleCallback" {
        0
    } else {
        nesting(vm)
    };
    vm.page.state.timers.push(Timer {
        id,
        order,
        due,
        interval,
        callback,
        args,
        frame,
        nesting,
    });
    Ok(JsValue::Number(f64::from(id)))
}

/** Real time between rendering opportunities inside one timer run: one display frame. */
const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_micros(16_667);

/** Levels of nested timeouts HTML allows before clamping their delay. */
const NESTING_CLAMP: u8 = 5;

/** The nesting level a timeout scheduled now gets: one deeper than the running timeout's. */
fn nesting(vm: &Vm) -> u8 {
    vm.page.timer_nesting.saturating_add(1)
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
        nesting: 0,
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
    /** Run timers due by `now`, earliest first, up to `budget` callbacks or until TIMER_SLICE
    has passed after the first; then set the clock to `now`. Timers left due run late, next. */
    pub fn run_due_timers(&mut self, now: f64, budget: usize) -> Res<usize> {
        let started = std::time::Instant::now();
        // Due order alone rules a quick run; only a run that has gone a frame of real time
        // without one forces the frames due, which is when zero-delay timeouts starve them.
        let mut last_frame = started;
        let mut ran = 0;
        while ran < budget && (ran == 0 || started.elapsed() < TIMER_SLICE) {
            if last_frame.elapsed() >= FRAME_INTERVAL {
                let frames = self.run_frame_batch(now)?;
                last_frame = std::time::Instant::now();
                if frames > 0 {
                    ran += frames;
                    continue;
                }
            }
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
            self.page.timer_nesting = timer.nesting;
            let fired = self.fire_timer(&timer).and_then(|()| self.drain_jobs());
            self.page.timer_nesting = 0;
            fired?;
            ran += 1;
        }
        self.page.state.clock = self.page.state.clock.max(now);
        Ok(ran)
    }
    /**
     * One rendering opportunity: every animation frame callback queued and due by `now`, in the
     * order they were requested, ahead of any timeout. A browser renders between tasks about
     * once a frame of real time; ordering frames among timeouts by due time instead let a
     * stream of zero-delay timeouts on a slow step starve them (YouTube's scheduler waited on
     * a frame forever). Callbacks a frame callback requests wait for the next opportunity.
     */
    fn run_frame_batch(&mut self, now: f64) -> Res<usize> {
        let mut batch: Vec<Timer> = Vec::new();
        let timers = std::mem::take(&mut self.page.state.timers);
        for timer in timers {
            if timer.frame && timer.due <= now {
                batch.push(timer);
            } else {
                self.page.state.timers.push(timer);
            }
        }
        batch.sort_by_key(|timer| timer.order);
        let Some(frame_time) = batch.iter().map(|timer| timer.due).reduce(f64::max) else {
            return Ok(0);
        };
        self.page.state.clock = self.page.state.clock.max(frame_time);
        for timer in &batch {
            self.fire_timer(timer)?;
            self.drain_jobs()?;
        }
        Ok(batch.len())
    }
    /** Shell callbacks report thrown values while their actual creation context is entered. */
    #[cfg(feature = "conformance")]
    fn fire_shell_timer(&mut self, timer: &Timer) -> Res<()> {
        let bank = crate::v9::builtins::shell_callback_realm(self, &timer.callback)?;
        let depth = self.realm_entries.len();
        self.realm_entries.push(bank);
        let result = self.with_realm(bank, |vm| {
            let args = if timer.frame {
                vec![JsValue::Number(vm.page.state.clock)]
            } else {
                timer.args.clone()
            };
            match vm.call(timer.callback.clone(), JsValue::Undefined, args) {
                Ok(_) => Ok(()),
                Err(Thrown::Value(error)) => {
                    crate::v9::builtins::report_shell_exception(vm, error.clone())?;
                    // d8 fails the run when a task throws, whether or not onerror swallowed the
                    // printout (ProcessMessages' verbose TryCatch still sees it), so a test whose
                    // only assertion sits in a timer cannot pass by throwing quietly.
                    let text = vm.describe(&error);
                    vm.page
                        .effects
                        .conformance_error
                        .get_or_insert(format!("Uncaught {text}"));
                    Ok(())
                }
                Err(fatal) => Err(fatal),
            }
        });
        self.realm_entries.truncate(depth);
        result
    }
    /** Run one timer callback; its exceptions go to the console. */
    fn fire_timer(&mut self, timer: &Timer) -> Res<()> {
        #[cfg(feature = "conformance")]
        if self.page.environment.conformance_d8 && self.is_callable(&timer.callback) {
            return self.fire_shell_timer(timer);
        }
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
                super::workers::report_exception(self, &error)?;
                Ok(())
            }
            Err(fatal) => Err(fatal),
        }
    }
}
