/** ImageData and canvas copying use actual typed storage and immutable source snapshots. */
use super::*;
use crate::v9::builtins::typed;
use crate::v9::vm::data::ElementKind;
use crate::v9::vm::heap::Key;
mod copy;
mod data;
mod put;

/** Register only operations whose pixels are implemented; unsupported source types fail explicitly. */
pub(super) fn install(b: &mut Builder, prototype: u32) {
    for (name, length) in [
        ("createImageData", 1),
        ("putImageData", 3),
        ("drawImage", 3),
    ] {
        b.host_method(prototype, name, length, method);
    }
}

/** Brand the destination before argument conversion and translate resource failures consistently. */
pub(super) fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    type Handler = fn(&mut Vm, &Call) -> Res<JsValue>;
    let handlers: [(&str, Handler); 3] = [
        ("createImageData", data::create),
        ("putImageData", put::pixels),
        ("drawImage", copy::draw),
    ];
    let result = handlers
        .iter()
        .find(|(name, _)| *name == dom::native_name(&call))
        .expect("registered image operation")
        .1(vm, &call);
    resource_result(vm, result)
}

/** ImageData has a real constructor rather than the generic illegal-constructor placeholder. */
pub(super) fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = data::construct(vm, &call);
    resource_result(vm, result)
}

/** Detached and out-of-bounds data fail before any canvas copy-on-write starts. */
fn view(vm: &mut Vm, value: &JsValue) -> Res<crate::v9::vm::data::ViewData> {
    let view = value
        .object()
        .and_then(|id| typed::view_of(vm, id))
        .filter(|view| view.kind == ElementKind::Uint8Clamped)
        .ok_or_else(|| vm.type_error("ImageData requires a Uint8ClampedArray."))?;
    if view.invalid || typed::buffer_state_detached(vm, view.buffer) {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "ImageData buffer is detached or out of bounds.",
        ));
    }
    Ok(view)
}

/** Brand and length are checked from private fields, ignoring authored attribute shadows. */
fn image_info(vm: &mut Vm, value: &JsValue) -> Res<(usize, usize, JsValue)> {
    let info = results::image_slots(vm, value)
        .ok_or_else(|| vm.type_error("The value is not an ImageData."))?;
    if view(vm, &info.2)?.length != info.0 * info.1 * limits::CHANNELS {
        return Err(vm.dom_exception("InvalidStateError", "ImageData buffer length changed."));
    }
    Ok(info)
}

/** No supported image operation can bypass the existing backing-size ceiling. */
fn drawable(vm: &mut Vm, node: usize) -> Res<()> {
    let bitmap = &vm.page.state.canvases[&node].bitmap;
    if Bitmap::length(bitmap.width, bitmap.height).is_none() {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "Canvas dimensions exceed this build's limit.",
        ));
    }
    Ok(())
}

/** Image-data coordinates use WebIDL EnforceRange rather than wrapping large integers. */
pub(super) fn coordinate(vm: &mut Vm, value: &JsValue) -> Res<i64> {
    let number = vm.to_number(value)?.trunc();
    if !number.is_finite() || number < f64::from(i32::MIN) || number > f64::from(i32::MAX) {
        return Err(vm.type_error("ImageData coordinate is outside the signed long range."));
    }
    Ok(number as i64)
}
