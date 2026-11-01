/** Canvas image copying snapshots source pixels before a destination can acquire mutable storage. */
use super::*;

/** Only genuine supported element wrappers enter pixel-source selection. */
fn source(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    let node = dom::node_of(vm, value)
        .filter(|node| matches!(vm.page.tag(*node).as_str(), "canvas" | "img"));
    node.ok_or_else(|| {
        vm.dom_exception(
            "NotSupportedError",
            "drawImage requires a supported canvas or image source.",
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
    let Some(source) = snapshot(vm, from)? else {
        return Ok(JsValue::Undefined);
    };
    drawable(vm, node)?;
    copy(vm, node, source, &values)
}

/** Natural dimensions determine overload defaults; bitmap dimensions determine sample coordinates. */
struct Source {
    bitmap: Bitmap,
    dimensions: [u32; 2],
    origin_clean: bool,
}

/** Pending images are no-ops, broken images throw, and unavailable decoded formats stay explicit. */
fn snapshot(vm: &mut Vm, from: usize) -> Res<Option<Source>> {
    if vm.page.tag(from) == "canvas" {
        let dimensions = size(&mut vm.page, from);
        if dimensions.contains(&0) || Bitmap::length(dimensions[0], dimensions[1]).is_none() {
            return Err(
                vm.dom_exception("InvalidStateError", "Source canvas has no admitted pixels.")
            );
        }
        // A canvas no context ever drew on is still a transparent bitmap of its size, as in
        // Chrome: drawing it composites (and clears under copy-like operators), not nothing.
        let Some(state) = vm.page.state.canvases.get(&from) else {
            let bitmap = Bitmap::new(dimensions[0], dimensions[1])?;
            return Ok(Some(Source {
                bitmap,
                dimensions,
                origin_clean: true,
            }));
        };
        return Ok(Some(Source {
            bitmap: state.bitmap.clone(),
            dimensions,
            origin_clean: state.origin_clean,
        }));
    }
    let Some(state) = vm.page.state.images.get(&from) else {
        return Ok(None);
    };
    if !state.complete || !state.requested {
        return Ok(None);
    }
    let Some((width, height)) = state.dimensions else {
        return Err(vm.dom_exception("InvalidStateError", "Source image is broken."));
    };
    let Some(bitmap) = state.bitmap.clone() else {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Decoded image pixels exceed this build's supported source formats or budget.",
        ));
    };
    Ok(Some(Source {
        bitmap,
        dimensions: [width, height],
        origin_clean: state.origin_clean,
    }))
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
fn copy(vm: &mut Vm, node: usize, source: Source, values: &[f64]) -> Res<JsValue> {
    let (mut crop, placement) = geometry(source.dimensions, values);
    if crop[2] == 0.0 || crop[3] == 0.0 || placement[2] == 0.0 || placement[3] == 0.0 {
        return Ok(JsValue::Undefined);
    }
    let dimensions = [source.bitmap.width, source.bitmap.height];
    for (index, value) in crop.iter_mut().enumerate() {
        let axis = index % 2;
        *value *= f64::from(dimensions[axis]) / f64::from(source.dimensions[axis]);
    }
    let state = &vm.page.state.canvases[&node];
    let plan = limits::plan_blit(
        dimensions,
        [state.bitmap.width, state.bitmap.height],
        crop,
        placement,
        state.drawing.transform,
        state.drawing.smoothing,
    )?
    .with_clip(clipping::current(state)?)?
    .with_composite(state.drawing.composite);
    work(vm, plan.work())?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    if plan.is_empty() {
        state.origin_clean &= source.origin_clean;
        return Ok(JsValue::Undefined);
    }
    limits::blit(
        &mut state.bitmap,
        &source.bitmap,
        &plan,
        state.drawing.alpha,
    )?;
    state.origin_clean &= source.origin_clean;
    state.dirty = true;
    Ok(JsValue::Undefined)
}
