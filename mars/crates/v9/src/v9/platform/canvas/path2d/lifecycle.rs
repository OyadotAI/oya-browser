/** Retained path quotas, GC ownership and restored associations share one admission contract. */
use super::*;

/** Reclaim unreachable wrappers once under pressure, while caller pins protect active native values. */
pub(super) fn admit(vm: &mut Vm, replacing: Option<ObjectId>, points: usize) -> Res<()> {
    if !fits(&vm.page.state.path2d, replacing, points) {
        vm.collect_now()?;
    }
    if !fits(&vm.page.state.path2d, replacing, points) {
        return Err(quota(vm));
    }
    Ok(())
}

/** A replacement releases its old point charge only when the new geometry is admitted atomically. */
fn fits(states: &BTreeMap<ObjectId, Retained>, replacing: Option<ObjectId>, points: usize) -> bool {
    let count = states.len() + usize::from(replacing.is_none());
    let total = states
        .iter()
        .filter(|(id, _)| Some(**id) != replacing)
        .fold(points, |total, (_, state)| {
            total.saturating_add(state.path.point_count())
        });
    count <= OBJECTS && total <= POINTS
}

/** Serialized native storage must satisfy the same bounds as every live insertion. */
pub(in crate::v9) fn validate(states: &BTreeMap<ObjectId, Retained>) -> EngineResult<()> {
    if states.len() > OBJECTS
        || states
            .values()
            .map(|state| state.path.point_count())
            .sum::<usize>()
            > POINTS
    {
        return Err(EngineError::invalid("Invalid retained canvas path budget."));
    }
    for state in states.values() {
        state.path.validate()?;
    }
    Ok(())
}

/** Every association must own an actual plain wrapper with the matching private native brand. */
pub(in crate::v9) fn validate_targets(vm: &mut Vm) -> EngineResult<()> {
    let ids: Vec<_> = vm.page.state.path2d.keys().copied().collect();
    for id in ids {
        let plain = vm
            .heap
            .objects
            .get(&id)
            .is_some_and(|object| matches!(object.class, Class::Plain));
        if !plain || identity(vm, &JsValue::Object(id)) != Some(id) {
            return Err(EngineError::invalid(
                "Invalid retained canvas path association.",
            ));
        }
    }
    Ok(())
}
