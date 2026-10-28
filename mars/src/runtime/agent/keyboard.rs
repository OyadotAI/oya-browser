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

impl Runtime {
    /** Enter at a field runs the page's keydown, keypress and keyup first, as a person's key
    does: Google submits its search from its own keydown handler and cancels the newline.
    None when nobody cancelled it, so the caller performs the native action; Some(result)
    when the page handled it. A page without scripts, or whose scripts fail, gets the native
    action, as in a browser. */
    pub(in crate::runtime::agent) fn press_enter(
        &mut self,
        page_id: &str,
        revision: u64,
        node_id: &str,
    ) -> EngineResult<Option<Value>> {
        let mut page = self.current()?.clone();
        if page.id() != page_id || page.revision != revision || !page.scripting.loaded {
            return Ok(None);
        }
        let Some(target) = node_id
            .strip_prefix(&format!("{}:node-", page.id()))
            .and_then(|value| value.parse::<usize>().ok())
        else {
            return Err(invalid_target());
        };
        let event = json!({"kind":"keyboard","key":"enter","target":target});
        let step = match self.script_step(&mut page, event) {
            Ok(step) => step,
            Err(error) if error.service.is_some() => return Err(error),
            Err(_) => return Ok(None),
        };
        let changed = self
            .settle_after_load(&mut page, Instant::now() + crate::constants::WEB_TIMEOUT)?
            || step.changed;
        let native = std::mem::take(&mut page.scripting.key_default);
        page.scripting.wake();
        let result = if changed {
            self.finish_mutation(page)?
        } else {
            self.commit_quietly(page)?;
            self.inspection()
        };
        if let Some(Ok(followed)) = self.follow_script_navigation(0) {
            return Ok(Some(followed));
        }
        Ok((!native).then_some(result))
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
