/** Host-owned render cycles re-layout between depth-gated broadcasts without running unrelated author tasks. */
use super::{page::Page, supervisor::Runtime};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{ResizeTurn, RESIZE_BROADCAST_LIMIT};
use serde_json::json;
use std::time::Instant;

impl Runtime {
    /** One candidate gets the broadcast cap plus error/finalization requests before publication. */
    pub(super) fn resize_observer_cycle(&mut self, page: &mut Page, until: Instant) -> EngineResult<bool> {
        if Instant::now() >= until { return Ok(false); }
        if !page.styled || page.display.is_none() { self.prepare_current(page)?; }
        if Instant::now() >= until { return Ok(false); }
        let mut changed = false;
        let mut action = json!({"kind":"observe"});
        for round in 0..=RESIZE_BROADCAST_LIMIT + 1 {
            let step = self.script_step(page, action)?;
            changed |= step.changed;
            match page.scripting.intersections.turn {
                Some(ResizeTurn::Relayout) => {
                    // Even a callback with no DOM changes may have changed what it observes.
                    // Supply the actual layout before gathering the next depth, never finish's old input.
                    self.prepare_current(page)?;
                    let complete = Instant::now() >= until || round + 1 >= RESIZE_BROADCAST_LIMIT;
                    action = json!({"kind":"resize","complete":complete});
                }
                Some(ResizeTurn::Complete { deferred }) => {
                    if step.changed { self.prepare_current(page)?; }
                    page.scripting.intersections.completed(deferred || step.changed);
                    return Ok(changed);
                }
                None => return Err(EngineError::invalid("Missing ResizeObserver cycle result.")),
            }
        }
        Err(EngineError::limit("ResizeObserver broadcasts", RESIZE_BROADCAST_LIMIT))
    }
}
