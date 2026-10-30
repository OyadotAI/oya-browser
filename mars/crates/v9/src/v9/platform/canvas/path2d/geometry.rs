/** Retained path commands share Core geometry while applying identity until a context uses the path. */
use super::*;

/** Receiver branding precedes conversion and candidate selection follows all author callbacks. */
pub(super) fn dispatch(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let id = receiver(vm, &call.this)?;
    let name = dom::native_name(call);
    if name == "addPath" {
        return add(vm, call, id);
    }
    if name == "roundRect" {
        return rounded(vm, call, id);
    }
    let count = match name {
        "closePath" => 0,
        "moveTo" | "lineTo" => 2,
        "rect" | "quadraticCurveTo" => 4,
        "arc" | "arcTo" => 5,
        "bezierCurveTo" => 6,
        "ellipse" => 7,
        _ => unreachable!("registered retained path method"),
    };
    let Some(values) = numbers(vm, call, name, count)? else {
        return Ok(JsValue::Undefined);
    };
    let mut path = vm.page.state.path2d[&id].path.clone();
    let result = construct(&mut path, name, values, call.arg(count).truthy());
    if result
        .as_ref()
        .is_err_and(|error| error.code == "canvas_radius")
    {
        replace(vm, id, path)?;
        return Err(radius_error(vm));
    }
    result?;
    replace(vm, id, path)?;
    Ok(JsValue::Undefined)
}

/** Every numeric conversion runs before nonfinite handling; arc and ellipse have different radius ordering. */
fn numbers(vm: &mut Vm, call: &Call, name: &str, count: usize) -> Res<Option<[f64; 7]>> {
    if call.args.len() < count {
        return Err(vm.type_error("Not enough arguments for Path2D geometry."));
    }
    let mut values = [0.0; 7];
    for (index, value) in values.iter_mut().take(count).enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    if name == "arc" && values[2].is_finite() && values[2] < 0.0 {
        return Err(radius_error(vm));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Ok(None);
    }
    if name == "ellipse" && (values[2] < 0.0 || values[3] < 0.0) {
        return Err(radius_error(vm));
    }
    Ok(Some(values))
}

/** This dispatcher invokes the same actual geometry used by current paths, without mutating a context. */
fn construct(path: &mut limits::Path, name: &str, v: [f64; 7], ccw: bool) -> EngineResult<()> {
    let transform = Affine::IDENTITY;
    match name {
        "closePath" => {
            path.close();
            Ok(())
        }
        "moveTo" => path.move_to([v[0], v[1]], transform),
        "lineTo" => path.line_to([v[0], v[1]], transform),
        "rect" => path.rect([v[0], v[1], v[2], v[3]], transform),
        "quadraticCurveTo" => path.quadratic_curve_to([v[0], v[1]], [v[2], v[3]], transform),
        "bezierCurveTo" => {
            path.bezier_curve_to([v[0], v[1]], [v[2], v[3]], [v[4], v[5]], transform)
        }
        "arcTo" => path.arc_to([v[0], v[1]], [v[2], v[3]], v[4], transform),
        "arc" => path.arc([v[0], v[1]], v[2], [v[3], v[4]], ccw, transform),
        "ellipse" => path.ellipse(
            [v[0], v[1]],
            [v[2], v[3]],
            v[4],
            [v[5], v[6]],
            ccw,
            transform,
        ),
        _ => unreachable!("registered retained path method"),
    }
}

/** DOMPoint and iterable radius conversion is shared with current-path roundRect. */
fn rounded(vm: &mut Vm, call: &Call, id: ObjectId) -> Res<JsValue> {
    let Some((rectangle, radii)) = paths::rounded_arguments(vm, call)? else {
        return Ok(JsValue::Undefined);
    };
    let mut path = vm.page.state.path2d[&id].path.clone();
    path.round_rect(rectangle, &radii, Affine::IDENTITY)?;
    replace(vm, id, path)?;
    Ok(JsValue::Undefined)
}

/** Source and target snapshots follow dictionary callbacks, so self-addition copies exactly once. */
fn add(vm: &mut Vm, call: &Call, id: ObjectId) -> Res<JsValue> {
    let source = receiver(vm, &call.arg(0))?;
    let dictionary = state::path_matrix(vm, &call.arg(1))?;
    let source = vm.page.state.path2d[&source].path.clone();
    if source.is_empty() {
        return Ok(JsValue::Undefined);
    }
    let transform = dictionary.resolve(vm)?;
    if !transform.finite() {
        return Ok(JsValue::Undefined);
    }
    let mut path = vm.page.state.path2d[&id].path.clone();
    path.add_path(&source, transform)?;
    replace(vm, id, path)?;
    Ok(JsValue::Undefined)
}

/** Domain errors remain distinct from point-budget or numeric-range refusals. */
fn radius_error(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception("IndexSizeError", "Canvas arc radii must be nonnegative.")
}
