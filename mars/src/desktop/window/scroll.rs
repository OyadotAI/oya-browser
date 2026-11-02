/** Scrolling stays local to AppKit; the engine hears where the person settled, never each frame. */
use std::time::{Duration, Instant};

/// Offsets closer than this are the same position (AppKit reports fractional points).
const SAME_POSITION: f64 = 0.5;
/// Idle time after the last scroll frame, momentum included, before the engine is told.
pub(super) const SETTLE: Duration = Duration::from_millis(150);

/** Where the person's scrolling has been seen and what the engine was last told. */
#[derive(Default)]
pub(super) struct ScrollReport {
    /// The offset the runtime holds, from a report or an adopted runtime scroll.
    pub sent: f64,
    /// The last observed offset and when it last moved.
    seen: Option<(f64, Instant)>,
}

impl ScrollReport {
    /** Returns the offset to report once the view has rested for `SETTLE`; a round trip per
    frame made scrolling wait on the engine, and its echo fought the person's own momentum. */
    pub fn settled(&mut self, y: f64, now: Instant) -> Option<f64> {
        if !self.seen.is_some_and(|(last, _)| same(last, y)) {
            self.seen = Some((y, now));
        }
        let (_, since) = self.seen?;
        (!same(y, self.sent) && now.duration_since(since) >= SETTLE).then_some(y)
    }
    /** A runtime offset moves the view only when the runtime itself changed it (agent scroll,
    navigation, fragment) and it is not the echo of the person's report; otherwise a late
    snapshot would snap the page back to where the person was a moment ago. */
    pub fn adopt(&mut self, previous: Option<f64>, runtime: f64, replaced: bool) -> bool {
        let moved = previous.is_none_or(|previous| !same(previous, runtime));
        let adopt = replaced || (moved && !same(runtime, self.sent));
        if adopt {
            self.sent = runtime;
        }
        adopt
    }
}

/** Fractional AppKit offsets within half a point are one position. */
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= SAME_POSITION
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Frames during a gesture or momentum are not reported. */
    #[test]
    fn moving_scroll_is_not_reported() {
        let mut report = ScrollReport::default();
        let start = Instant::now();
        for frame in 0..30u32 {
            let now = start + Duration::from_millis(8) * frame;
            assert_eq!(report.settled(f64::from(frame) * 40.0, now), None);
        }
    }

    /** The resting offset is reported once after the settle time, then not again. */
    #[test]
    fn settled_scroll_is_reported_once() {
        let mut report = ScrollReport::default();
        let start = Instant::now();
        assert_eq!(report.settled(600.0, start), None);
        assert_eq!(report.settled(600.0, start + SETTLE / 2), None);
        assert_eq!(report.settled(600.0, start + SETTLE), Some(600.0));
        report.sent = 600.0;
        assert_eq!(report.settled(600.0, start + SETTLE * 3), None);
    }

    /** Any movement restarts the settle time. */
    #[test]
    fn movement_restarts_the_settle_time() {
        let mut report = ScrollReport::default();
        let start = Instant::now();
        report.settled(100.0, start);
        assert_eq!(report.settled(140.0, start + SETTLE), None);
        assert_eq!(report.settled(140.0, start + SETTLE * 2), Some(140.0));
    }

    /** The echo of the person's own report never moves the view. */
    #[test]
    fn echo_of_the_persons_report_is_not_adopted() {
        let mut report = ScrollReport {
            sent: 800.0,
            ..Default::default()
        };
        assert!(!report.adopt(Some(500.0), 800.0, false));
    }

    /** A snapshot that leaves the runtime offset unchanged never moves the view mid-scroll. */
    #[test]
    fn unchanged_runtime_offset_is_not_adopted() {
        let mut report = ScrollReport {
            sent: 500.0,
            ..Default::default()
        };
        report.settled(900.0, Instant::now());
        assert!(!report.adopt(Some(500.0), 500.0, false));
    }

    /** An agent's or the page's own scroll moves the view and becomes the reported offset. */
    #[test]
    fn runtime_scroll_is_adopted() {
        let mut report = ScrollReport {
            sent: 500.0,
            ..Default::default()
        };
        assert!(report.adopt(Some(500.0), 1200.0, false));
        assert_eq!(report.settled(1200.0, Instant::now() + SETTLE * 2), None);
    }

    /** A new page or tab always takes its own offset, even when it equals the old one. */
    #[test]
    fn replaced_page_offset_is_adopted() {
        let mut report = ScrollReport::default();
        assert!(report.adopt(Some(0.0), 0.0, true));
    }
}
