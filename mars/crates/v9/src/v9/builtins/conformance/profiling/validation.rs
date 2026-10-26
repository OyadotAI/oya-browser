/** Restored native profile records must remain a bounded, internally consistent call tree. */
use super::State;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::vm::{Limits, Vm};

/** Runtime and committed snapshots consume the same retained-unit and resident-entry budgets. */
pub(in crate::v9) fn validate_state(state: &State, limits: &Limits) -> EngineResult<()> {
    let Some(profile) = &state.session else {
        return Ok(());
    };
    if profile.nodes.is_empty()
        || profile.nodes.len() > limits.properties
        || profile.samples.len() > limits.array_length
        || profile.samples.len() != profile.time_deltas.len()
        || state.units() > limits.units
        || profile.end_time < profile.start_time
    {
        return Err(EngineError::invalid("Invalid bounded CPU profile state."));
    }
    let mut parents = vec![0; profile.nodes.len()];
    let mut hits = vec![0u32; profile.nodes.len()];
    for (index, node) in profile.nodes.iter().enumerate() {
        if node.id as usize != index + 1 {
            return Err(invalid());
        }
        for child in &node.children {
            let child = *child as usize;
            if child <= index + 1 || child > parents.len() {
                return Err(invalid());
            }
            parents[child - 1] += 1;
            if parents[child - 1] != 1 {
                return Err(invalid());
            }
        }
    }
    if parents.iter().skip(1).any(|count| *count != 1) {
        return Err(invalid());
    }
    for sample in &profile.samples {
        let Some(hit) = sample
            .checked_sub(1)
            .and_then(|id| hits.get_mut(id as usize))
        else {
            return Err(invalid());
        };
        *hit += 1;
    }
    let elapsed = profile
        .time_deltas
        .iter()
        .try_fold(0u64, |sum, delta| sum.checked_add(*delta));
    if profile
        .nodes
        .iter()
        .zip(hits)
        .any(|(node, hits)| node.hit_count != hits)
        || elapsed != Some(profile.end_time - profile.start_time)
    {
        return Err(invalid());
    }
    Ok(())
}

/** Listeners are actual callable heap roots, never saved native capabilities or forged frame references. */
pub(in crate::v9) fn validate(vm: &Vm) -> EngineResult<()> {
    let state = &vm.page.state.cpu_profile;
    validate_state(state, &vm.limits)?;
    if state.listener.is_some() != state.context.is_some()
        || state.context.is_some_and(|id| {
            !vm.realms
                .contexts
                .iter()
                .any(|context| context.global == Some(id))
        })
    {
        return Err(invalid());
    }
    if state
        .listener
        .is_some_and(|id| !vm.is_callable(&crate::v9::value::JsValue::Object(id)))
    {
        return Err(invalid());
    }
    if vm.page.environment.conformance_cpu_profiler_interval_us == 0 {
        return Err(invalid());
    }
    Ok(())
}

/** Invalid native metadata fails validation before script, collection or listener dispatch. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid CPU profile graph.")
}
