/** Named native keys execute through V9 under the existing active-page, revision and ownership admission. */
use super::super::supervisor::Runtime;
use crate::protocol::{EngineError, EngineResult};
use serde_json::{Value, json};
use std::time::Instant;

impl Runtime {
    /** Escape uses the committed focus and cannot act on another page or an unobserved revision. */
    pub(in crate::runtime::agent) fn press_escape(
        &mut self,
        page_id: &str,
        revision: u64,
        node_id: Option<&str>,
    ) -> EngineResult<Value> {
        let mut page = self.current()?.clone();
        if page.id() != page_id {
            return Err(invalid_target());
        }
        if page.revision != revision {
            return Err(EngineError::new(
                "revision_conflict",
                "The document revision has changed.",
                "Inspect the current page before changing it.",
            ));
        }
        let target = node_id
            .map(|id| {
                let local = id
                    .strip_prefix(&format!("{}:node-", page.id()))
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(invalid_target)?;
                if page.node_id(local) != id {
                    return Err(invalid_target());
                }
                Ok(local)
            })
            .transpose()?;
        let step = self.script_step(
            &mut page,
            json!({"kind":"keyboard","key":"escape","target":target}),
        )?;
        let changed = self
            .settle_after_load(&mut page, Instant::now() + crate::constants::WEB_TIMEOUT)?
            || step.changed;
        // A key press is input even when its effect comes later from a timer.
        page.scripting.wake();
        let result = if changed {
            self.finish_mutation(page)?
        } else {
            self.commit_quietly(page)?;
            self.inspection()
        };
        Ok(match self.follow_script_navigation(0) {
            Some(Ok(followed)) => followed,
            _ => result,
        })
    }
}

/** Native key requests never interpret an arbitrary node string as current-page authority. */
fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The keyboard target is not on the current page.",
        "Inspect the active page and use its current identity.",
    )
}
