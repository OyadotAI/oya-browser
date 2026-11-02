/** isPointInPath and isPointInStroke answer from the same geometry that fill and stroke paint, so a hit and a painted pixel agree. */
use super::*;
use mars_core::canvas::FillRule;

/** Both members take the optional Path2D first, then the point, then (for fill) the fill rule. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "isPointInPath", 2, in_path);
    b.host_method(proto, "isPointInStroke", 2, in_stroke);
}

/** Fill containment; a refused geometry or brand error is catchable like any other drawing error. */
fn in_path(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = hit(vm, &call, false);
    resource_result(vm, result)
}

/** Stroke containment, measured against the outline the current pen produces. */
fn in_stroke(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = hit(vm, &call, true);
    resource_result(vm, result)
}

/** Converts every argument in WebIDL order before reading any state, then tests the point. */
fn hit(vm: &mut Vm, call: &Call, stroke: bool) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let source = path2d::retained_argument(vm, &call.arg(0));
    let first = usize::from(source.is_some());
    if call.args.len() < first + 2 {
        return Err(vm.type_error("Not enough arguments for canvas point test."));
    }
    let point = [
        vm.to_number(&call.arg(first))?,
        vm.to_number(&call.arg(first + 1))?,
    ];
    let rule = if stroke {
        FillRule::Nonzero
    } else {
        fill_rule(vm, &call.arg(first + 2))?
    };
    if point.iter().any(|value| !value.is_finite()) {
        return Ok(JsValue::Boolean(false));
    }
    let owned = path2d::snapshot(vm, source, node)?;
    let state = &vm.page.state.canvases[&node];
    let path = owned.as_ref().unwrap_or(&state.path);
    let plan = if stroke {
        limits::plan_stroke_dashed(
            [0, 0],
            path,
            state.drawing.pen,
            state.drawing.transform,
            &state.drawing.dash,
        )?
    } else {
        limits::plan_fill([0, 0], path, rule)?
    };
    work(vm, plan.work())?;
    let contained = plan.contains(point);
    Ok(JsValue::Boolean(contained))
}

/** An absent rule is nonzero; any other value must be one of the two spelled rules. */
fn fill_rule(vm: &mut Vm, value: &JsValue) -> Res<FillRule> {
    if matches!(value, JsValue::Undefined) {
        return Ok(FillRule::Nonzero);
    }
    match vm.to_text(value)?.as_str() {
        "nonzero" => Ok(FillRule::Nonzero),
        "evenodd" => Ok(FillRule::Evenodd),
        _ => Err(vm.type_error("Canvas fill rule must be nonzero or evenodd.")),
    }
}
