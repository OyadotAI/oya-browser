/** drawFocusIfNeeded: a focus ring is painted only when the element is the focused one, and only after its path is known. */
use super::*;

/** Ring width and color approximate the focus ring Chrome draws around focused canvas fallback content. */
const RING_WIDTH: f64 = 2.0;
const RING_COLOR: [u8; 4] = [16, 16, 16, u8::MAX];

/** Registered once on the context; refusals are catchable like every other canvas method. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "drawFocusIfNeeded", 1, method);
}

/** Translate resource refusals into catchable exceptions, as the other canvas entry points do. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = draw(vm, &call);
    resource_result(vm, result)
}

/** The optional path comes first, so the element is the last argument in both overloads. */
fn draw(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let source = if call.args.len() >= 2 {
        path2d::retained_argument(vm, &call.arg(0))
    } else {
        None
    };
    let element_value = call.arg(usize::from(source.is_some()));
    let element = dom::node_of(vm, &element_value)
        .ok_or_else(|| vm.type_error("drawFocusIfNeeded requires an element."))?;
    if vm.page.state.focused != Some(element) {
        return Ok(JsValue::Undefined);
    }
    let owned = path2d::snapshot(vm, source, node)?;
    let state = &vm.page.state.canvases[&node];
    let size = [state.bitmap.width, state.bitmap.height];
    let path = owned.as_ref().unwrap_or(&state.path);
    let pen = limits::StrokeStyle {
        width: RING_WIDTH,
        ..Default::default()
    };
    let plan = limits::plan_stroke_dashed(
        size,
        path,
        pen,
        state.drawing.transform,
        &limits::DashStyle::default(),
    )?
    .with_clip(clipping::current(state)?)?;
    work(vm, plan.work())?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    limits::paint_path(&mut state.bitmap, &plan, RING_COLOR, state.drawing.alpha)?;
    state.dirty |= !plan.is_empty();
    Ok(JsValue::Undefined)
}
