/** Script-visible live time advances within a task without changing the virtual timer scheduler. */
use super::Page;
use crate::protocol::{EngineError, EngineResult};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/** All platform clocks expose milliseconds while Instant measures seconds. */
const MILLIS_PER_SECOND: f64 = 1000.0;

/** A request-local monotonic anchor is never serialized or rolled back with script effects. */
pub(in crate::v9) struct ExecutionClock {
    /** The execution anchor begins after state restoration and compilation. */
    started: Instant,
    /** Saved observations, virtual advances and real idle time all establish a lower bound. */
    baseline_ms: f64,
}

impl Default for ExecutionClock {
    /** Construction is inert until session initialization installs the restored timeline. */
    fn default() -> Self {
        Self {
            started: Instant::now(),
            baseline_ms: 0.0,
        }
    }
}

/** One wall-clock sample fixes an epoch; later clock reads depend only on monotonic elapsed time. */
fn epoch_now() -> f64 {
    static ANCHOR: OnceLock<(Instant, f64)> = OnceLock::new();
    let (started, epoch) = ANCHOR.get_or_init(|| {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            * MILLIS_PER_SECOND;
        (Instant::now(), epoch)
    });
    epoch + started.elapsed().as_secs_f64() * MILLIS_PER_SECOND
}

/** Deserialized clocks must not inject NaNs, negative time or non-finite baselines into a realm. */
pub(in crate::v9) fn validate(clock: f64, observed: f64, epoch: f64) -> EngineResult<()> {
    if !clock.is_finite()
        || clock < 0.0
        || !observed.is_finite()
        || observed < 0.0
        || !epoch.is_finite()
    {
        return Err(EngineError::invalid("Invalid V9 script clock."));
    }
    Ok(())
}

impl Page<'_> {
    /** Start after restoring page facts; a stale coordinator clock never rewinds observed time. */
    pub(in crate::v9) fn start_execution_clock(&mut self) {
        let baseline_ms = self
            .state
            .observed_clock
            .max(self.state.clock)
            .max(epoch_now() - self.state.epoch)
            .max(0.0);
        self.execution_clock = ExecutionClock {
            started: Instant::now(),
            baseline_ms,
        };
    }

    /** Native clocks advance during synchronous JavaScript; standalone fixtures remain exact. */
    pub(in crate::v9) fn script_now(&self) -> f64 {
        if !self.environment.live_clock {
            return self.state.clock;
        }
        self.execution_clock.baseline_ms.max(self.state.clock)
            + self.execution_clock.started.elapsed().as_secs_f64() * MILLIS_PER_SECOND
    }

    /** Only successful request state carries observed time into the next restored execution. */
    pub(in crate::v9) fn retain_clock(&mut self) {
        if self.environment.live_clock {
            self.state.observed_clock = self.script_now();
        }
    }
}
