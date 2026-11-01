/*!
 * A stream player's playhead: media time that advances with the host clock while the element plays and holds while
 * it is paused. Eviction and the status the page hears are both read from it, so they agree with each other.
 */

/** Media time in seconds, anchored to the host clock while playing. */
#[derive(Debug, Default)]
pub struct Clock {
    /** The media time at the last change of state. */
    base: f64,
    /** The host time at which the clock last started, while it runs. */
    started: Option<f64>,
}

impl Clock {
    /** The media time at host time `host`. A clock that never started reads its base time. */
    pub fn time(&self, host: f64) -> f64 {
        self.base
            + self
                .started
                .map_or(0.0, |started| (host - started).max(0.0))
    }

    /** Whether the clock is running, so the element is playing. */
    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    /** Start or stop the clock at host time `host`; repeating the current state changes nothing. */
    pub fn set_running(&mut self, running: bool, host: f64) {
        match (running, self.started) {
            (true, None) => self.started = Some(host),
            (false, Some(_)) => {
                self.base = self.time(host);
                self.started = None;
            }
            _ => {}
        }
    }

    /** Move to media time `time`; a running clock keeps running from there. */
    pub fn seek(&mut self, time: f64, host: f64) {
        self.base = time;
        if self.started.is_some() {
            self.started = Some(host);
        }
    }
}
