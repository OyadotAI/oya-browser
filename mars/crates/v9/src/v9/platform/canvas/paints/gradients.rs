/** CanvasGradient factories and addColorStop: WebIDL conversion, range checks, then stored geometry and stops. */
use super::*;
use mars_core::canvas::{GRADIENT_STOPS, Geometry, Gradient};
static PROTO: OnceLock<u32> = OnceLock::new();

/** The interface is illegal to construct directly, and its one method appends a validated stop. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    super::super::super::chrome_env::interface(b, "CanvasGradient", proto);
    b.host_method(proto, "addColorStop", 2, add_stop);
    let _ = PROTO.set(proto);
}

/** Build the requested geometry, then wrap it; a refused argument allocates nothing. */
pub(super) fn create(vm: &mut Vm, call: &Call, name: &str) -> Res<JsValue> {
    let geometry = match name {
        "createLinearGradient" => linear(vm, call)?,
        "createRadialGradient" => radial(vm, call)?,
        _ => conic(vm, call)?,
    };
    let entry = Entry {
        paint: mars_core::canvas::Paint::Gradient(Gradient::new(geometry)),
        clean: true,
    };
    wrap(vm, entry, *PROTO.get().expect("gradient prototype"))
}

/** WebIDL doubles reject nonfinite values with a TypeError before any geometry is built. */
fn finite<const N: usize>(vm: &mut Vm, call: &Call) -> Res<[f64; N]> {
    let mut values = [0.0; N];
    for (index, value) in values.iter_mut().enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(vm.type_error("The provided double value is non-finite."));
    }
    Ok(values)
}

/** Two points define the axis; coincident points are legal and paint nothing. */
fn linear(vm: &mut Vm, call: &Call) -> Res<Geometry> {
    let [x0, y0, x1, y1] = finite::<4>(vm, call)?;
    Ok(Geometry::Linear {
        from: [x0, y0],
        to: [x1, y1],
    })
}

/** Two circles; negative radii are an IndexSizeError, as the canvas specification requires. */
fn radial(vm: &mut Vm, call: &Call) -> Res<Geometry> {
    let [x0, y0, r0, x1, y1, r1] = finite::<6>(vm, call)?;
    if r0 < 0.0 || r1 < 0.0 {
        return Err(vm.dom_exception("IndexSizeError", "The radius provided is negative."));
    }
    Ok(Geometry::Radial {
        start: [x0, y0, r0],
        end: [x1, y1, r1],
    })
}

/** Conic gradients take their start angle in radians and their center in user space. */
fn conic(vm: &mut Vm, call: &Call) -> Res<Geometry> {
    let [angle, x, y] = finite::<3>(vm, call)?;
    Ok(Geometry::Conic {
        center: [x, y],
        start: angle,
    })
}

/** Receiver brand is checked first; then the offset and color are converted and validated in spec order. */
fn add_stop(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = stop(vm, &call);
    resource_result(vm, result)
}

/** Only gradient wrappers accept stops; a pattern or foreign object is an illegal invocation. */
fn stop(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let id = identity(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if gradient_mut(vm, id).is_none() {
        return Err(vm.type_error("Illegal invocation"));
    }
    let offset = vm.to_number(&call.arg(0))?;
    let text = vm.to_text(&call.arg(1))?;
    if !offset.is_finite() {
        return Err(vm.type_error("The provided double value is non-finite."));
    }
    if !(0.0..=1.0).contains(&offset) {
        return Err(vm.dom_exception("IndexSizeError", "The stop offset is outside [0, 1]."));
    }
    let color = colors::parse(&text)
        .ok_or_else(|| vm.dom_exception("SyntaxError", "The color is not a valid CSS color."))?;
    let gradient = gradient_mut(vm, id).expect("checked gradient");
    if gradient.stops.len() >= GRADIENT_STOPS {
        return Err(quota(vm));
    }
    gradient.add_stop(offset, color)?;
    Ok(JsValue::Undefined)
}
