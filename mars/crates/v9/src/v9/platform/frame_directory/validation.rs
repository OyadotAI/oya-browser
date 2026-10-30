use super::super::{PageEnvironment, PageState};
/** Directory input and private incarnation state are bounded before execution. */
use super::{MAX_FRAME_WINDOW_ID, MAX_INCARNATION, OWNER_RECORDS, sibling_target};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeSet;

/** Reject forged or oversized environment metadata before executing any JavaScript. */
pub(in crate::v9) fn validate(environment: &PageEnvironment) -> EngineResult<()> {
    let routes = &environment.parent_frames;
    if routes.len() > crate::constants::NODES {
        return Err(EngineError::limit(
            "parent frame directory",
            crate::constants::NODES,
        ));
    }
    let mut owners = BTreeSet::new();
    let mut windows = BTreeSet::new();
    let mut bytes = 0_usize;
    for route in routes {
        bytes = bytes.saturating_add(route.name.len());
        if sibling_target(route.window).is_none()
            || route.owner > MAX_FRAME_WINDOW_ID as usize
            || !owners.insert(route.owner)
            || !windows.insert(route.window)
            || route.name.len() > crate::constants::TEXT_BYTES
        {
            return Err(invalid());
        }
    }
    if bytes > crate::constants::JS_ENVIRONMENT_UNITS {
        return Err(EngineError::limit(
            "parent frame names",
            crate::constants::JS_ENVIRONMENT_UNITS,
        ));
    }
    if (!routes.is_empty() || environment.own_window.is_some())
        && environment.embedder_origin.is_none()
        || environment
            .own_window
            .is_some_and(|window| !windows.contains(&window))
        || !routes.is_empty() && environment.own_window.is_none()
    {
        return Err(invalid());
    }
    Ok(())
}

/** Persisted incarnation counters remain bounded even if their detached nodes were collected. */
pub(in crate::v9) fn validate_state(state: &PageState) -> EngineResult<()> {
    if state.frame_incarnations.len() > OWNER_RECORDS
        || state
            .frame_incarnations
            .values()
            .any(|value| *value > MAX_INCARNATION)
    {
        return Err(invalid());
    }
    Ok(())
}

/** A single error keeps malformed routing metadata out of the execution path. */
pub(super) fn invalid() -> EngineError {
    EngineError::invalid("Invalid frame directory metadata.")
}
