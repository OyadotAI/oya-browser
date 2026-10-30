/** Canvas image copying snapshots source pixels before a destination can acquire mutable storage. */
use super::*;

/** Canvas sources are real branded elements; unsupported external image types are explicit. */
fn source(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    let node = dom::node_of(vm, value).filter(|node| vm.page.tag(*node) == "canvas");
    node.ok_or_else(|| {
        vm.dom_exception(
            "NotSupportedError",
            "drawImage currently requires a canvas source.",
        )
    })
}

/** All conversion callbacks run before source snapshotting and destination state selection. */
pub(super) fn draw(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let value = call.arg(0);
    source(vm, &value)?;
    let values = coordinates(vm, call)?;
    if values.iter().any(|value| !value.is_finite()) {
        return Ok(JsValue::Undefined);
    }
    let from = source(vm, &value)?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let dimensions = size(&mut vm.page, from);
    if dimensions.contains(&0) {
        return Err(vm.dom_exception("InvalidStateError", "Source canvas has no pixels."));
    }
    drawable(vm, node)?;
    copy(vm, node, from, dimensions, &values)
}

/** Overload arity determines which numeric conversions can execute author code. */
fn coordinates(vm: &mut Vm, call: &Call) -> Res<Vec<f64>> {
    let count = match call.args.len() {
        3 => 2,
        5 => 4,
        9.. => 8,
        _ => return Err(vm.type_error("drawImage requires three, five or nine arguments.")),
    };
    let mut values = Vec::with_capacity(count);
    for index in 1..=count {
        values.push(vm.to_number(&call.arg(index))?);
    }
    Ok(values)
}

/** The two short overloads use the source's dimensions after coercion callbacks. */
fn geometry(size: [u32; 2], values: &[f64]) -> ([f64; 4], [f64; 4]) {
    let full = [0.0, 0.0, f64::from(size[0]), f64::from(size[1])];
    match values.len() {
        2 => (full, [values[0], values[1], full[2], full[3]]),
        4 => (full, [values[0], values[1], values[2], values[3]]),
        _ => (
            [values[0], values[1], values[2], values[3]],
            [values[4], values[5], values[6], values[7]],
        ),
    }
}

/** Source snapshots share immutable backing until the destination performs fallible COW. */
fn copy(
    vm: &mut Vm,
    node: usize,
    from: usize,
    dimensions: [u32; 2],
    values: &[f64],
) -> Res<JsValue> {
    if Bitmap::length(dimensions[0], dimensions[1]).is_none() {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "Source canvas dimensions exceed this build's limit.",
        ));
    }
    let (crop, placement) = geometry(dimensions, values);
    let state = &vm.page.state.canvases[&node];
    let plan = limits::plan_blit(
        dimensions,
        [state.bitmap.width, state.bitmap.height],
        crop,
        placement,
        state.drawing.transform,
        state.drawing.smoothing,
    )?;
    work(vm, plan.work())?;
    if plan.is_empty() {
        return Ok(JsValue::Undefined);
    }
    // An untouched canvas is transparent; source-over cannot change the destination.
    let Some(source) = vm
        .page
        .state
        .canvases
        .get(&from)
        .map(|state| state.bitmap.clone())
    else {
        return Ok(JsValue::Undefined);
    };
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    limits::blit(&mut state.bitmap, &source, &plan, state.drawing.alpha)?;
    state.dirty = true;
    Ok(JsValue::Undefined)
}
