/** Shadow properties and the layered paint path: a shape with a visible shadow is painted into a layer, then composited with its shadow. */
use super::*;
use mars_core::canvas::{shadow_layer, shadow_sigma};

/** A shadow composite touches each canvas pixel several times; this charge keeps the request budget honest. */
const SHADOW_WORK: usize = 16;

/** Shadow color, blur and offsets are value stores: nonfinite or negative blur assignments keep the old value. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    for name in [
        "shadowColor",
        "shadowBlur",
        "shadowOffsetX",
        "shadowOffsetY",
    ] {
        b.accessor(proto, name, Some(get), Some(set));
    }
}

/** Getters report the stored value in its canonical form. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &vm.page.state.canvases[&node].drawing;
    match dom::native_name(&call) {
        "shadowBlur" => Ok(JsValue::Number(drawing.shadow_blur)),
        "shadowOffsetX" => Ok(JsValue::Number(drawing.shadow_offset[0])),
        "shadowOffsetY" => Ok(JsValue::Number(drawing.shadow_offset[1])),
        _ => Ok(JsValue::string(&colors::serialize(drawing.shadow_color))?),
    }
}

/** Conversion can run script, so the target context is reacquired before the stored value changes. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let name = dom::native_name(&call);
    let color = if name == "shadowColor" {
        colors::parse(&vm.to_text(&call.arg(0))?)
    } else {
        None
    };
    let number = if name == "shadowColor" {
        0.0
    } else {
        vm.to_number(&call.arg(0))?
    };
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let drawing = &mut vm
        .page
        .state
        .canvases
        .get_mut(&node)
        .expect("canvas")
        .drawing;
    match name {
        "shadowColor" => drawing.shadow_color = color.unwrap_or(drawing.shadow_color),
        "shadowBlur" if number.is_finite() && number >= 0.0 => drawing.shadow_blur = number,
        "shadowOffsetX" if number.is_finite() => drawing.shadow_offset[0] = number,
        "shadowOffsetY" if number.is_finite() => drawing.shadow_offset[1] = number,
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/** A shadow is visible only with a nonzero color alpha and some blur or offset, as the canvas specification requires. */
pub(super) fn active(drawing: &state::DrawingState) -> bool {
    drawing.shadow_color[3] > 0
        && (drawing.shadow_blur > 0.0 || drawing.shadow_offset != [0.0, 0.0])
}

/** Charge the planned work, plus the layer passes when a shadow is visible, before any mutation. */
pub(super) fn charge(vm: &mut Vm, node: usize, planned: usize) -> Res<()> {
    let state = &vm.page.state.canvases[&node];
    let extra = if active(&state.drawing) {
        (state.bitmap.pixels.len() / limits::CHANNELS) * SHADOW_WORK
    } else {
        0
    };
    work(vm, planned + extra)
}

/** Paint one shape into a transparent canvas-size layer, then composite its shadow and then the shape. */
pub(super) fn paint_layered(
    vm: &mut Vm,
    node: usize,
    clean: bool,
    paint: impl FnOnce(&mut Bitmap) -> EngineResult<()>,
) -> Res<()> {
    let bitmap = &vm.page.state.canvases[&node].bitmap;
    let mut layer = Bitmap::new(bitmap.width, bitmap.height)?;
    paint(&mut layer)?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    composite(state, &layer)?;
    state.origin_clean &= clean;
    state.dirty = true;
    Ok(())
}

/** The shape layer and its shadow share the clip, operator and global alpha; the shadow sits beneath the shape. */
fn composite(state: &mut State, layer: &Bitmap) -> EngineResult<()> {
    let State {
        bitmap,
        clips,
        drawing,
        ..
    } = state;
    let clip = drawing.clip.and_then(|id| clips.get(&id));
    let sigma = shadow_sigma(drawing.shadow_blur);
    let shadow = shadow_layer(layer, drawing.shadow_color, sigma, drawing.shadow_offset)?;
    limits::composite_layer(bitmap, &shadow, clip, drawing.alpha, drawing.composite)?;
    limits::composite_layer(bitmap, layer, clip, drawing.alpha, drawing.composite)
}
