/** Shared vector interpolation preserves CSS transition timing and reversal across native properties. */
use super::TransitionEasing;
use crate::document::style::constants::TRANSITION_FRAME_MS;

/** An admitted effect keeps its original timing when only the transition declarations change. */
#[derive(Clone, Debug)]
pub(super) struct Interpolation<const N: usize> {
    pub(super) from: [f64; N],
    pub(super) to: [f64; N],
    start_ms: f64,
    duration_ms: f64,
    easing: TransitionEasing,
    reversing_start: [f64; N],
    shortening: f64,
}

impl<const N: usize> Interpolation<N> {
    /** Start from current presentation; exact reversal shortens duration to preserve apparent speed. */
    pub(super) fn start(
        before: [f64; N],
        active: Option<&Self>,
        to: [f64; N],
        timing: (f64, f64, TransitionEasing),
        now_ms: f64,
        bound: impl Fn(f64) -> f64,
    ) -> Option<Self> {
        let (mut duration, mut delay, easing) = timing;
        if duration + delay <= 0.0 {
            return None;
        }
        let from = active.map_or(before, |active| active.value(now_ms).map(&bound));
        if from == to {
            return None;
        }
        let (mut shortening, mut reversing_start) = (1.0, from);
        if let Some(active) = active
            && active.reversing_start == to
        {
            shortening = (active.progress(now_ms) * active.shortening + (1.0 - active.shortening))
                .abs()
                .clamp(0.0, 1.0);
            reversing_start = active.to;
            duration *= shortening;
            if delay < 0.0 {
                delay *= shortening;
            }
        }
        let effect = Self {
            from,
            to,
            start_ms: now_ms + delay,
            duration_ms: duration,
            easing,
            reversing_start,
            shortening,
        };
        (effect.end() > now_ms).then_some(effect)
    }

    /** Positive delay retains the start and zero duration jumps at its precise delayed end. */
    fn progress(&self, now_ms: f64) -> f64 {
        if now_ms < self.start_ms {
            return 0.0;
        }
        if self.duration_ms == 0.0 {
            return 1.0;
        }
        self.easing
            .sample(((now_ms - self.start_ms) / self.duration_ms).clamp(0.0, 1.0))
    }

    /** Property-specific constraints apply after easing, allowing supported timing overshoot. */
    pub(super) fn value(&self, now_ms: f64) -> [f64; N] {
        let progress = self.progress(now_ms);
        std::array::from_fn(|index| {
            self.from[index] + (self.to[index] - self.from[index]) * progress
        })
    }

    /** Negative delay and reversal scaling are already represented in this deadline. */
    pub(super) fn end(&self) -> f64 {
        self.start_ms + self.duration_ms
    }

    /** Completed effects stop ticking; delays wake directly at their active start. */
    pub(super) fn next_sample_ms(&self, now_ms: f64) -> Option<f64> {
        (now_ms < self.end()).then(|| {
            if now_ms < self.start_ms {
                self.start_ms
            } else {
                (now_ms + TRANSITION_FRAME_MS).min(self.end())
            }
        })
    }
}
