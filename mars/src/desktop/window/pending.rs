/** Orders the worker events a UI tick applies, so a busy page cannot hold up a screenshot. */
use super::*;

/** How one pending worker event applies within a tick. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lane {
    /** A command's own result or a navigation step; applied in arrival order. */
    Foreground,
    /** A window observation or screenshot; reads the frame on screen, so it runs after them. */
    Window,
    /** A snapshot the timers produced while idle, which no command is waiting on. */
    Background,
}

/** The lane an event applies in. Only background snapshots are ever coalesced. */
pub(super) fn lane(event: &Event) -> Lane {
    match event {
        Event::Window(..) => Lane::Window,
        Event::Update {
            background: true, ..
        } => Lane::Background,
        _ => Lane::Foreground,
    }
}

/**
 * The order a batch of pending events applies in. A busy page sends a background snapshot every few
 * seconds, and each one repaints the window; applying them in arrival order queued every screenshot
 * and inspection behind them. So the batch applies its foreground events in arrival order, then its
 * window requests (which read the frame on screen), then only the newest background snapshot that
 * arrived after the last foreground one. Older background snapshots are superseded and dropped:
 * applying one after a newer foreground snapshot would show the page going backwards.
 */
pub(super) fn order<T>(pending: Vec<T>, lane_of: impl Fn(&T) -> Lane) -> Vec<T> {
    let last_foreground = pending
        .iter()
        .rposition(|event| lane_of(event) == Lane::Foreground);
    let mut foreground = Vec::with_capacity(pending.len());
    let mut windows = Vec::new();
    let mut newest_background = None;
    for (index, event) in pending.into_iter().enumerate() {
        match lane_of(&event) {
            Lane::Foreground => foreground.push(event),
            Lane::Window => windows.push(event),
            Lane::Background if last_foreground.is_none_or(|last| index > last) => {
                newest_background = Some(event);
            }
            Lane::Background => {}
        }
    }
    foreground.append(&mut windows);
    foreground.extend(newest_background);
    foreground
}

#[cfg(test)]
mod tests {
    use super::{Lane, order};

    /** Test events name their lane and an id, so the order is visible in the assertion. */
    fn batch(lanes: &[(Lane, u32)]) -> Vec<(Lane, u32)> {
        order(lanes.to_vec(), |(lane, _)| *lane)
    }

    /** A foreground update that arrives after a background one supersedes it. */
    #[test]
    fn stale_background_snapshot_never_applies_after_a_command_snapshot() {
        let ordered = batch(&[(Lane::Background, 1), (Lane::Foreground, 2)]);
        assert_eq!(ordered, [(Lane::Foreground, 2)]);
    }

    /** Only the newest background snapshot of a storm applies, after the window requests. */
    #[test]
    fn background_storm_applies_once_and_after_window_requests() {
        let ordered = batch(&[
            (Lane::Background, 1),
            (Lane::Background, 2),
            (Lane::Window, 3),
            (Lane::Background, 4),
        ]);
        assert_eq!(ordered, [(Lane::Window, 3), (Lane::Background, 4)]);
    }

    /** Command results keep their arrival order, and screenshots follow all of them. */
    #[test]
    fn foreground_events_keep_arrival_order_before_window_requests() {
        let ordered = batch(&[
            (Lane::Foreground, 1),
            (Lane::Window, 2),
            (Lane::Foreground, 3),
        ]);
        assert_eq!(
            ordered,
            [
                (Lane::Foreground, 1),
                (Lane::Foreground, 3),
                (Lane::Window, 2)
            ]
        );
    }

    /** An empty batch and a batch with no background snapshot apply exactly what arrived. */
    #[test]
    fn batch_without_background_applies_everything_once() {
        assert!(batch(&[]).is_empty());
        let ordered = batch(&[(Lane::Window, 1), (Lane::Foreground, 2)]);
        assert_eq!(ordered, [(Lane::Foreground, 2), (Lane::Window, 1)]);
    }
}
