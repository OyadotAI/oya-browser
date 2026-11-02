/** Directory input and private incarnation state are bounded before execution. */
use super::super::{PageEnvironment, PageState};
use super::{MAX_FRAME_WINDOW_ID, MAX_INCARNATION, OWNER_RECORDS, sibling_target};
use crate::protocol::{EngineError, EngineResult};
use std::collections::BTreeSet;

/** Reject forged or oversized environment metadata before executing any JavaScript. */
pub(in crate::v9) fn validate(environment: &PageEnvironment) -> EngineResult<()> {
    contexts(environment)?;
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

/** A directory is one bounded acyclic rooted tree; child IDs never belong to two parents. */
fn contexts(environment: &PageEnvironment) -> EngineResult<()> {
    const CONTEXTS: usize = 17;
    const DEPTH: usize = 3;
    let contexts = &environment.frame_contexts;
    if contexts.is_empty() {
        return if environment.context_window.is_none() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    if contexts.len() > CONTEXTS
        || contexts
            .iter()
            .filter(|context| context.parent.is_none())
            .count()
            != 1
    {
        return Err(invalid());
    }
    let windows: BTreeSet<_> = contexts.iter().map(|context| context.window).collect();
    if windows.len() != contexts.len()
        || windows
            .iter()
            .any(|window| sibling_target(*window).is_none())
        || environment
            .context_window
            .is_none_or(|own| !windows.contains(&own))
    {
        return Err(invalid());
    }
    let mut children = BTreeSet::new();
    let mut bytes = 0usize;
    for context in contexts {
        let mut owners = BTreeSet::new();
        for child in &context.children {
            bytes = bytes.saturating_add(child.name.len());
            if !owners.insert(child.owner)
                || !children.insert(child.window)
                || sibling_target(child.window).is_none()
                || contexts.iter().any(|entry| {
                    entry.window == child.window && entry.parent != Some(context.window)
                })
                || child.owner > MAX_FRAME_WINDOW_ID as usize
                || child.name.len() > crate::constants::TEXT_BYTES
            {
                return Err(invalid());
            }
        }
        let mut cursor = context;
        let mut depth = 0usize;
        while let Some(parent) = cursor.parent {
            depth += 1;
            if depth >= DEPTH {
                return Err(invalid());
            }
            let ancestor = contexts
                .iter()
                .find(|entry| entry.window == parent)
                .ok_or_else(invalid)?;
            if !ancestor
                .children
                .iter()
                .any(|child| child.window == cursor.window)
            {
                return Err(invalid());
            }
            cursor = ancestor;
        }
    }
    if children.len() > crate::constants::NODES || bytes > crate::constants::JS_ENVIRONMENT_UNITS {
        return Err(invalid());
    }
    let own = contexts
        .iter()
        .find(|context| Some(context.window) == environment.context_window)
        .ok_or_else(invalid)?;
    if own.parent.is_some() != environment.embedder_origin.is_some()
        || environment
            .own_window
            .is_some_and(|id| Some(id) != environment.context_window)
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
