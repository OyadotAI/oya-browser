/** Current-path bindings construct real bounded geometry and paint it into the shared canvas backing. */
use super::*;
mod arcs;
mod curves;
mod round_rect;
mod styles;

/** Methods and properties are exposed only when their drawing behavior is implemented. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    styles::install(b, proto);
    arcs::install(b, proto);
    curves::install(b, proto);
    round_rect::install(b, proto);
    for (name, length) in [
        ("beginPath", 0),
        ("closePath", 0),
        ("moveTo", 2),
        ("lineTo", 2),
        ("rect", 4),
        ("fill", 0),
        ("stroke", 0),
        ("strokeRect", 4),
    ] {
        b.host_method(proto, name, length, method);
    }
}

/** Geometry and quota errors are catchable, with no partially painted path. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = dispatch(vm, &call);
    resource_result(vm, result)
}

/** Brand checks precede argument conversion; author callbacks finish before mutable state selection. */
fn dispatch(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let name = dom::native_name(call);
    if name == "fill" {
        return fill(vm, call);
    }
    if name == "stroke" {
        if !matches!(call.arg(0), JsValue::Undefined) {
            return Err(vm.type_error("Path2D stroke sources are not implemented."));
        }
        return paint(vm, node, true, limits::FillRule::Nonzero, None);
    }
    if name == "beginPath" {
        vm.page.state.canvases.get_mut(&node).expect("canvas").path = limits::Path::default();
    } else if name == "closePath" {
        vm.page
            .state
            .canvases
            .get_mut(&node)
            .expect("canvas")
            .path
            .close();
    } else {
        return construct(vm, call, name);
    }
    Ok(JsValue::Undefined)
}

/** Invalid fill-rule enum values throw before any state or pixels change. */
fn fill(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let rule = if matches!(call.arg(0), JsValue::Undefined) {
        "nonzero".into()
    } else {
        vm.to_text(&call.arg(0))?
    };
    let rule = match rule.as_str() {
        "nonzero" => limits::FillRule::Nonzero,
        "evenodd" => limits::FillRule::Evenodd,
        _ => return Err(vm.type_error("Canvas fill rule must be nonzero or evenodd.")),
    };
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    paint(vm, node, false, rule, None)
}

/** Nonfinite coordinates are ignored after every required conversion has run. */
fn coordinates(vm: &mut Vm, call: &Call, count: usize) -> Res<Option<[f64; 4]>> {
    if call.args.len() < count {
        return Err(vm.type_error("Not enough arguments for canvas path."));
    }
    let mut values = [0.0; 4];
    for (index, value) in values.iter_mut().take(count).enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    Ok(values
        .iter()
        .all(|value| value.is_finite())
        .then_some(values))
}

/** StrokeRect uses a temporary rectangle and leaves the context's current path untouched. */
fn construct(vm: &mut Vm, call: &Call, name: &str) -> Res<JsValue> {
    let count = if name.ends_with("To") { 2 } else { 4 };
    let Some(values) = coordinates(vm, call, count)? else {
        return Ok(JsValue::Undefined);
    };
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let transform = state.drawing.transform;
    match name {
        "moveTo" => state.path.move_to([values[0], values[1]], transform)?,
        "lineTo" => state.path.line_to([values[0], values[1]], transform)?,
        "rect" => state.path.rect(values, transform)?,
        "strokeRect" => {
            let mut path = limits::Path::default();
            path.rect(values, transform)?;
            return paint(vm, node, true, limits::FillRule::Nonzero, Some(path));
        }
        _ => unreachable!("registered path method"),
    }
    Ok(JsValue::Undefined)
}

/** Work admission and one fallible COW precede painting; fill/stroke never clear origin taint. */
fn paint(
    vm: &mut Vm,
    node: usize,
    stroke: bool,
    rule: limits::FillRule,
    temporary: Option<limits::Path>,
) -> Res<JsValue> {
    let state = &vm.page.state.canvases[&node];
    let size = [state.bitmap.width, state.bitmap.height];
    if Bitmap::length(size[0], size[1]).is_none() {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "Canvas dimensions exceed this build's limit.",
        ));
    }
    let path = temporary.as_ref().unwrap_or(&state.path);
    let plan = if stroke {
        limits::plan_stroke(size, path, state.drawing.pen, state.drawing.transform)?
    } else {
        limits::plan_fill(size, path, rule)?
    };
    work(vm, plan.work())?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let color = if stroke {
        state.drawing.stroke
    } else {
        state.drawing.fill
    };
    limits::paint_path(&mut state.bitmap, &plan, color, state.drawing.alpha)?;
    state.dirty |= !plan.is_empty();
    Ok(JsValue::Undefined)
}
