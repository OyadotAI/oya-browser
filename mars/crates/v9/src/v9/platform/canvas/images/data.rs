/** ImageData constructor overloads preserve supplied typed-array identity and allocate bounded copies. */
use super::*;
use crate::v9::value::to_uint32;

/** Convert WebIDL arguments before checking source storage, since conversion may detach it. */
pub(super) fn construct(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let target = call
        .new_target
        .clone()
        .ok_or_else(|| vm.type_error("ImageData must be constructed with new."))?;
    if call.args.len() < 2 {
        return Err(vm.type_error("ImageData requires at least two arguments."));
    }
    let prototype = vm.prototype_for(&target, results::image_prototype())?;
    let first = call.arg(0);
    if let Some(view) = first.object().and_then(|id| typed::view_of(vm, id)) {
        if view.kind != ElementKind::Uint8Clamped {
            return Err(vm.type_error("ImageData requires a Uint8ClampedArray."));
        }
        return supplied(vm, call, first, prototype);
    }
    let width = to_uint32(vm.to_number(&first)?) as usize;
    let height = to_uint32(vm.to_number(&call.arg(1))?) as usize;
    settings(vm, &call.arg(2))?;
    empty(vm, width, height, Some(prototype))
}

/** Reusing a typed view must preserve both its identity and the exact requested shape. */
fn supplied(vm: &mut Vm, call: &Call, data: JsValue, prototype: ObjectId) -> Res<JsValue> {
    let width = to_uint32(vm.to_number(&call.arg(1))?) as usize;
    let height = if matches!(call.arg(2), JsValue::Undefined) {
        None
    } else {
        Some(to_uint32(vm.to_number(&call.arg(2))?) as usize)
    };
    settings(vm, &call.arg(3))?;
    let length = view(vm, &data)?.length;
    if length == 0 || !length.is_multiple_of(limits::CHANNELS) {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "ImageData data length must be a nonzero multiple of four.",
        ));
    }
    let pixels = length / limits::CHANNELS;
    if width == 0
        || !pixels.is_multiple_of(width)
        || height.is_some_and(|height| height != pixels / width)
    {
        return Err(size_error(vm));
    }
    dimensions(vm, width, pixels / width)?;
    results::image_as(
        vm,
        width,
        pixels / width,
        data.object().expect("typed view"),
        prototype,
    )
}

/** createImageData copies dimensions only; its data is always fresh transparent storage. */
pub(super) fn create(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    if call.args.len() == 1 {
        let (width, height, _) = results::image_slots(vm, &call.arg(0))
            .ok_or_else(|| vm.type_error("The value is not an ImageData."))?;
        return empty(vm, width, height, None);
    }
    if call.args.len() < 2 {
        return Err(vm.type_error("createImageData requires dimensions or an ImageData."));
    }
    let width = coordinate(vm, &call.arg(0))?.unsigned_abs() as usize;
    let height = coordinate(vm, &call.arg(1))?.unsigned_abs() as usize;
    settings(vm, &call.arg(2))?;
    empty(vm, width, height, None)
}

/** Admit before allocating a temporary vector; typed storage then remains under VM heap limits. */
fn empty(vm: &mut Vm, width: usize, height: usize, prototype: Option<ObjectId>) -> Res<JsValue> {
    let bytes = dimensions(vm, width, height)?;
    work(vm, width * height)?;
    let _allocation = limits::Allocation::new(bytes)?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(bytes).map_err(|_| quota(vm))?;
    pixels.resize(bytes, 0);
    let data = typed::new_uint8_clamped(vm, pixels)?;
    results::image_as(
        vm,
        width,
        height,
        data,
        prototype.unwrap_or_else(results::image_prototype),
    )
}

/** The same dimensions bound constructors and copying without unchecked products. */
fn dimensions(vm: &mut Vm, width: usize, height: usize) -> Res<usize> {
    if width == 0 || height == 0 {
        return Err(size_error(vm));
    }
    if width > limits::EDGE as usize || height > limits::EDGE as usize {
        return Err(quota(vm));
    }
    Ok(width * height * limits::CHANNELS)
}

/** Settings do not claim color conversion or high precision pixel formats that are not implemented. */
fn settings(vm: &mut Vm, value: &JsValue) -> Res<()> {
    if value.nullish() {
        return Ok(());
    }
    if value.object().is_none() {
        return Err(vm.type_error("ImageData settings must be an object."));
    }
    for (name, supported, recognized) in [
        ("colorSpace", "srgb", "display-p3"),
        ("pixelFormat", "rgba-unorm8", "rgba-float16"),
    ] {
        let value = vm.get(value, &Key::from(name))?;
        if matches!(value, JsValue::Undefined) {
            continue;
        }
        let text = vm.to_text(&value)?;
        if text == recognized {
            return Err(vm.dom_exception(
                "NotSupportedError",
                "This ImageData format is not implemented.",
            ));
        }
        if text != supported {
            return Err(vm.type_error("Invalid ImageData settings enum value."));
        }
    }
    Ok(())
}

/** Web-compatible dimension errors are separate from Mars's resource admission failures. */
fn size_error(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception(
        "IndexSizeError",
        "ImageData dimensions do not match its data.",
    )
}
