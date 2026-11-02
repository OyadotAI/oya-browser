/** Bitmap reset, publication and validation preserve the page transaction boundary. */
use super::*;
use mars_core::canvas::dimension;

/** Default dimensions follow the HTML non-negative integer rule, not CSS sizing. */
pub(super) fn size(page: &mut Page, node: usize) -> [u32; 2] {
    ["width", "height"].map(|name| {
        let default = limits::DEFAULT_SIZE[usize::from(name == "height")];
        page.view(node)
            .and_then(|v| v.attributes.get(name).cloned())
            .and_then(|v| dimension(&v))
            .unwrap_or(default)
    })
}
/** Every dimension mutation, including a same-value write, clears pixels and state. */
pub(in crate::v9) fn prepare_resize(
    page: &mut Page,
    node: usize,
    name: &str,
    value: Option<&str>,
) -> EngineResult<Option<Bitmap>> {
    if !["width", "height"].contains(&name) || !page.state.canvases.contains_key(&node) {
        return Ok(None);
    }
    let mut dimensions = size(page, node);
    let index = usize::from(name == "height");
    dimensions[index] = value
        .and_then(dimension)
        .unwrap_or(limits::DEFAULT_SIZE[index]);
    let [width, height] = dimensions;
    admit(page, node, Bitmap::length(width, height).unwrap_or(0))?;
    Ok(Some(Bitmap::new(width, height)?))
}
/** Commit reset only after the attribute write succeeds; all allocation happened before it. */
pub(in crate::v9) fn resized(page: &mut Page, node: usize, bitmap: Bitmap) {
    let state = page.state.canvases.get_mut(&node).expect("canvas");
    state.bitmap = bitmap;
    state.origin_clean = true;
    state.drawing = state::DrawingState::default();
    state.saved.clear();
    state.path = limits::Path::default();
    state.clips.clear();
    state.dirty = true;
}
/** Account for replacement rather than charging both old and new live backing indefinitely. */
pub(super) fn admit(page: &Page, node: usize, bytes: usize) -> EngineResult<()> {
    let used: usize = page
        .state
        .canvases
        .iter()
        .filter(|(id, _)| **id != node)
        .map(|(_, s)| s.bitmap.pixels.len())
        .sum();
    if used.saturating_add(bytes) > limits::PAGE_BYTES {
        return Err(EngineError::invalid(
            "QuotaExceededError: Canvas bitmap budget exceeded.",
        ));
    }
    Ok(())
}
/** Publish projected canvases; head/detached pixels stay dirty until body adoption. */
pub(in crate::v9) fn publish(page: &mut Page) -> EngineResult<()> {
    let ids: Vec<usize> = page.state.canvases.keys().copied().collect();
    for node in ids {
        if !page.is_projected(node) {
            continue;
        }
        let state = page.state.canvases.get_mut(&node).expect("canvas");
        if !state.dirty {
            continue;
        }
        if page.script_changes.len() >= crate::constants::STATEMENTS {
            return Err(EngineError::limit(
                "Canvas changes",
                crate::constants::STATEMENTS,
            ));
        }
        page.script_changes
            .push(mars_core::dom::ScriptChange::Canvas {
                node,
                bitmap: state.bitmap.clone(),
            });
        state.dirty = false;
    }
    Ok(())
}
/** Reject forged backing data before a restored state can invoke drawing. */
pub(in crate::v9) fn validate(states: &BTreeMap<usize, State>) -> EngineResult<()> {
    if states.len() > limits::CONTEXTS
        || states
            .values()
            .map(|s| s.bitmap.pixels.len())
            .sum::<usize>()
            > limits::PAGE_BYTES
    {
        return Err(EngineError::limit("Canvas storage", limits::PAGE_BYTES));
    }
    for state in states.values() {
        state.bitmap.validate()?;
        state.path.validate()?;
        state::validate(&state.drawing)?;
        if state.saved.len() > state::SAVED_STATES {
            return Err(EngineError::limit(
                "Canvas saved states",
                state::SAVED_STATES,
            ));
        }
        for saved in &state.saved {
            state::validate(saved)?;
        }
    }
    clipping::validate(states)?;
    Ok(())
}

/** Restored associations must name the same real canvas and reciprocal private heap edges. */
pub(in crate::v9) fn validate_targets(vm: &mut Vm) -> EngineResult<()> {
    let associations: Vec<_> = vm
        .page
        .state
        .canvases
        .iter()
        .map(|(node, state)| (*node, state.context))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for (node, context) in associations {
        let context = JsValue::Object(context);
        let owner = encoding::read_slot(vm, &context, "canvas.owner");
        let valid = owner.as_ref().is_some_and(|owner| {
            dom::node_of(vm, owner) == Some(node)
                && encoding::read_slot(vm, owner, CONTEXT_SLOT) == Some(context.clone())
        });
        if !valid || vm.page.tag(node) != "canvas" || !seen.insert(context.object()) {
            return Err(EngineError::invalid("Invalid canvas context association."));
        }
    }
    Ok(())
}
