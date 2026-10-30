/** Current clipping regions are immutable sample masks referenced by saved drawing states through a bounded pool. */
use super::*;
use std::collections::BTreeSet;

/** Only the implemented current-path overloads are exposed; Path2D remains outside this surface. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "clip", 0, method);
}

/** Geometry and resource refusals leave the current region, path and backing pixels untouched. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = intersect(vm, &call);
    resource_result(vm, result)
}

/** Fill-rule conversion can execute script, so context state is selected only after it finishes. */
fn intersect(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let text = if matches!(call.arg(0), JsValue::Undefined) {
        "nonzero".into()
    } else {
        vm.to_text(&call.arg(0))?
    };
    let rule = match text.as_str() {
        "nonzero" => limits::FillRule::Nonzero,
        "evenodd" => limits::FillRule::Evenodd,
        _ => return Err(vm.type_error("Canvas fill rule must be nonzero or evenodd.")),
    };
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let state = &vm.page.state.canvases[&node];
    let size = [state.bitmap.width, state.bitmap.height];
    if Bitmap::length(size[0], size[1]).is_none() {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "Canvas dimensions exceed this build's limit.",
        ));
    }
    let plan = limits::plan_fill(size, &state.path, rule)?.with_clip(current(state)?)?;
    work(vm, plan.clip_work())?;
    let mask = plan.clip_mask()?;
    if current(&vm.page.state.canvases[&node])? == Some(&mask) {
        return Ok(JsValue::Undefined);
    }
    admit(&vm.page, node, mask.bytes())?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let id = replacement(state)
        .or_else(|| (0..=state::SAVED_STATES as u8).find(|id| !state.clips.contains_key(id)))
        .ok_or_else(storage_error)?;
    state.clips.insert(id, mask);
    state.drawing.clip = Some(id);
    prune(state);
    Ok(JsValue::Undefined)
}

/** Missing references are contract errors, never a reason to silently drop clipping. */
pub(super) fn current(state: &State) -> EngineResult<Option<&limits::ClipMask>> {
    state
        .drawing
        .clip
        .map(|id| state.clips.get(&id).ok_or_else(invalid))
        .transpose()
}

/** A replaced current mask releases page allowance only when no saved drawing state still retains it. */
fn replacement(state: &State) -> Option<u8> {
    state
        .drawing
        .clip
        .filter(|id| !state.saved.iter().any(|drawing| drawing.clip == Some(*id)))
}

/** Candidates are process-accounted during construction; retained page admission happens before installation. */
fn admit(page: &Page, node: usize, additional: usize) -> EngineResult<()> {
    let replacing = replacement(&page.state.canvases[&node]);
    let mut bytes = additional;
    for (owner, state) in &page.state.canvases {
        for (id, mask) in &state.clips {
            if *owner != node || Some(*id) != replacing {
                bytes = bytes.saturating_add(mask.bytes());
            }
        }
    }
    if bytes > limits::CLIP_BYTES {
        return Err(storage_error());
    }
    Ok(())
}

/** Restore/replacement release unreachable masks; save duplicates references without duplicating pixels. */
pub(super) fn prune(state: &mut State) {
    let retained = references(state);
    state.clips.retain(|id, _| retained.contains(id));
}

/** Pool identities stay local to one context and cannot retain orphaned or mismatched mask storage. */
pub(super) fn validate(states: &BTreeMap<usize, State>) -> EngineResult<()> {
    let mut bytes = 0usize;
    for state in states.values() {
        let references = references(state);
        if state.clips.len() > state::SAVED_STATES + 1
            || references.len() != state.clips.len()
            || !references.iter().all(|id| state.clips.contains_key(id))
        {
            return Err(invalid());
        }
        for (id, mask) in &state.clips {
            if usize::from(*id) > state::SAVED_STATES {
                return Err(invalid());
            }
            mask.validate([state.bitmap.width, state.bitmap.height])?;
            bytes = bytes.saturating_add(mask.bytes());
        }
    }
    if bytes > limits::CLIP_BYTES {
        return Err(storage_error());
    }
    Ok(())
}

/** A saved state has no independent mask allocation; this set describes all live pool ownership. */
fn references(state: &State) -> BTreeSet<u8> {
    std::iter::once(&state.drawing)
        .chain(state.saved.iter())
        .filter_map(|drawing| drawing.clip)
        .collect()
}

/** Quota refusals are translated into catchable errors at the Canvas binding boundary. */
fn storage_error() -> EngineError {
    EngineError::invalid("QuotaExceededError: Canvas clipping mask budget exceeded.")
}

/** Serialized state must never disable a missing or forged clipping region implicitly. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid canvas clipping mask association.")
}
