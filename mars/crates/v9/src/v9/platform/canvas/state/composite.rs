/** Canvas exposes only composition modes implemented consistently by every supported painter. */
use super::*;

/** Compositing is drawing state, so native save/restore/reset use the same accepted enum. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.accessor(proto, "globalCompositeOperation", Some(get), Some(set));
}

/** Canonical names reflect actual raster behavior rather than an inert JavaScript expando. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    JsValue::string(vm.page.state.canvases[&node].drawing.composite.name()).map_err(Into::into)
}

/** Conversion callbacks can resize the canvas, so assignment selects current state afterwards. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let text = vm.to_text(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if let Some(mode) = limits::Composite::named(&text) {
        vm.page
            .state
            .canvases
            .get_mut(&node)
            .expect("canvas")
            .drawing
            .composite = mode;
    }
    Ok(JsValue::Undefined)
}
