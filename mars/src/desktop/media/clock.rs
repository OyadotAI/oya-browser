/*!
 * A stream player's playhead: the timebase of an AVSampleBufferRenderSynchronizer. The audio renderer attaches to
 * the same synchronizer, so the sound the OS plays and the pictures chosen against this clock agree, and eviction and
 * the status the page hears read it too. The clock runs at the element's rate whether or not a renderer holds
 * media: the page gates play on its own buffered ranges, and a synchronizer that waited for media would never start
 * a video-only stream.
 */
use super::constants::TIME_SCALE;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_av_foundation::{AVQueuedSampleBufferRendering, AVSampleBufferRenderSynchronizer};
use objc2_core_media::CMTime;

/** Media time in seconds, read from the synchronizer's timebase. */
pub struct Clock {
    synchronizer: Retained<AVSampleBufferRenderSynchronizer>,
    /** The rate last set, so a repeated setting costs no call and `running` needs no framework read. */
    rate: f64,
}

impl Default for Clock {
    /** A stopped clock at time zero. */
    fn default() -> Self {
        Self::new()
    }
}

impl Clock {
    /** A stopped clock at time zero, which starts as soon as it is given a rate. */
    pub fn new() -> Self {
        // SAFETY: construction and a property set on an object only this clock holds.
        let synchronizer = unsafe { AVSampleBufferRenderSynchronizer::new() };
        unsafe { synchronizer.setDelaysRateChangeUntilHasSufficientMediaData(false) };
        Self {
            synchronizer,
            rate: 0.0,
        }
    }

    /** The media time now; never negative or NaN. */
    pub fn time(&self) -> f64 {
        // SAFETY: a getter on the synchronizer this clock holds; a CMTime reads as seconds.
        let seconds = unsafe { self.synchronizer.currentTime().seconds() };
        if seconds.is_finite() {
            seconds.max(0.0)
        } else {
            0.0
        }
    }

    /** Whether the clock is running, so the element is playing. */
    pub fn running(&self) -> bool {
        self.rate != 0.0
    }

    /** Run at `rate` (zero stops); repeating the current rate changes nothing. */
    pub fn set_rate(&mut self, rate: f64) {
        if rate == self.rate {
            return;
        }
        self.rate = rate;
        // SAFETY: a setter on the synchronizer this clock holds.
        unsafe { self.synchronizer.setRate(rate as f32) };
    }

    /** Move to media time `time`; a running clock keeps running from there. */
    pub fn seek(&self, time: f64) {
        // SAFETY: a CMTime from plain numbers, set on the synchronizer this clock holds.
        unsafe {
            self.synchronizer
                .setRate_time(self.rate as f32, CMTime::with_seconds(time, TIME_SCALE));
        }
    }

    /** Put a renderer on this clock, so what it plays follows the playhead. */
    pub fn attach(&self, renderer: &ProtocolObject<dyn AVQueuedSampleBufferRendering>) {
        // SAFETY: the synchronizer retains the renderer; the caller keeps its own reference too.
        unsafe { self.synchronizer.addRenderer(renderer) };
    }
}

#[cfg(test)]
mod tests {
    use super::Clock;

    /** A seek moves a stopped clock, and it holds there until it is given a rate. */
    #[test]
    fn a_stopped_clock_holds_the_time_it_was_seeked_to() {
        let mut clock = Clock::new();
        assert!(!clock.running());
        clock.seek(5.0);
        assert!((clock.time() - 5.0).abs() < 0.001, "{}", clock.time());
        clock.set_rate(1.0);
        assert!(clock.running());
    }
}
