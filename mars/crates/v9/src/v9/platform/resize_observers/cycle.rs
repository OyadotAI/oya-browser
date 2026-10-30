/** A rendering opportunity advances only through fresh host layouts and strictly increasing gathered depths. */
use super::*;
use crate::protocol::EngineResult;

/** Cross-process broadcasts share a strict count cap even though each request has its own VM budget. */
pub const RESIZE_BROADCAST_LIMIT: usize = 16;
/** Shadow and detached ancestry cannot turn one observed target into unbounded traversal work. */
pub(super) const DEPTH_LIMIT: usize = crate::constants::DOCUMENT_DEPTH * 2;
/** The standard error names undelivered notifications without treating the document as failed. */
const LOOP_ERROR: &str = "ResizeObserver loop completed with undelivered notifications.";

/** The host must re-layout after a broadcast; completion defers skipped work to a later rendering opportunity. */
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResizeTurn {
    /// Apply callback changes and supply a fresh layout before continuing this same cycle.
    Relayout,
    /// The depth state has been reset, with undelivered sizes retained for a future rendering opportunity.
    Complete { deferred: bool },
}

/** Only depth and count cross the process boundary; callbacks and nodes remain in their branded registry. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Cycle {
    depth: usize,
    rounds: usize,
    /// Error handlers may change layout; intersection sampling waits for one final host layout.
    finishing: bool,
}

/** A begin cannot overwrite an unfinished cycle and a continuation cannot resurrect one already finalized. */
pub(in crate::v9) fn advance(vm: &mut Vm, begin: bool) -> Res<()> {
    vm.page.sampled_observer_turn = true;
    if begin {
        if vm.page.state.resize_cycle.is_some() {
            return Err(invalid().into());
        }
        vm.page.state.resize_cycle = Some(Cycle {
            depth: 0,
            rounds: 0,
            finishing: false,
        });
    }
    let current = vm
        .page
        .state
        .resize_cycle
        .as_ref()
        .ok_or_else(invalid)?
        .clone();
    if current.finishing {
        return finish_intersections(vm, true);
    }
    let skipped = sampling::gather(vm, current.depth)?;
    if !vm.page.effects.resize_observers_pending {
        return finalize(vm, skipped);
    }
    if current.rounds >= RESIZE_BROADCAST_LIMIT {
        return finalize(vm, true);
    }
    let Some(depth) = delivery::deliver_all(vm)? else {
        return finalize(vm, skipped);
    };
    if depth <= current.depth {
        return Err(invalid().into());
    }
    vm.page.state.resize_cycle = Some(Cycle {
        depth,
        rounds: current.rounds + 1,
        finishing: false,
    });
    sampling::effects(vm);
    vm.page.effects.resize_turn = Some(ResizeTurn::Relayout);
    Ok(())
}

/** Deadline exhaustion ends the cycle against the last fresh layout, with no further resize broadcasts. */
pub(in crate::v9) fn complete(vm: &mut Vm) -> Res<()> {
    vm.page.sampled_observer_turn = true;
    let cycle = vm.page.state.resize_cycle.as_ref().ok_or_else(invalid)?;
    if cycle.finishing {
        return finish_intersections(vm, true);
    }
    let depth = cycle.depth;
    let skipped = sampling::gather(vm, depth)?;
    let deferred = skipped || vm.page.effects.resize_observers_pending;
    finalize(vm, deferred)
}

/** Report one loop error, then yield for layout if its handler or microtasks changed the document. */
fn finalize(vm: &mut Vm, deferred: bool) -> Res<()> {
    for state in vm.page.state.resize_observers.values_mut() {
        state.queued.clear();
    }
    if !deferred {
        return finish_intersections(vm, false);
    }
    vm.page
        .state
        .resize_cycle
        .as_mut()
        .ok_or_else(invalid)?
        .finishing = true;
    events::report_window_error(vm, LOOP_ERROR)?;
    vm.drain_jobs()?;
    sampling::effects(vm);
    vm.page.effects.resize_turn = Some(ResizeTurn::Relayout);
    Ok(())
}

/** IO runs once against fresh post-broadcast/post-error layout; cycle depth resets for the next frame. */
fn finish_intersections(vm: &mut Vm, deferred: bool) -> Res<()> {
    vm.page.state.resize_cycle = None;
    super::super::intersection_observers::sample(vm)?;
    super::super::intersection_observers::deliver_all(vm)?;
    super::super::intersection_observers::effects(vm);
    sampling::effects(vm);
    let deferred = deferred || vm.page.resize_registration_changed;
    vm.page.effects.resize_turn = Some(ResizeTurn::Complete { deferred });
    Ok(())
}

/** Flattened ancestry includes slot assignment and crosses shadow roots without counting fragment boxes. */
pub(super) fn depth(vm: &mut Vm, node: usize) -> Res<usize> {
    let mut node = Some(node);
    let mut depth = 0;
    for _ in 0..DEPTH_LIMIT {
        let Some(current) = node else {
            return Ok(depth.max(1));
        };
        if !vm.page.is_shadow_root(current) {
            depth += 1;
        }
        node = super::super::components::composed_parent(&mut vm.page, current, true);
    }
    Err(EngineError::limit("ResizeObserver ancestry", DEPTH_LIMIT).into())
}

/** Restored state cannot bypass count limits or lower the depth threshold between broadcasts. */
pub(super) fn validate(cycle: Option<&Cycle>) -> EngineResult<()> {
    if cycle.is_some_and(|cycle| {
        cycle.rounds == 0
            || cycle.rounds > RESIZE_BROADCAST_LIMIT
            || cycle.depth == 0
            || cycle.depth > DEPTH_LIMIT
    }) {
        return Err(invalid());
    }
    Ok(())
}

/** Invalid cycle ordering is a host-state failure, not an author exception. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid ResizeObserver cycle continuation.")
}
