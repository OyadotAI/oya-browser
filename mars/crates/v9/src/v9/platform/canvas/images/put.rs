/** Pixel insertion ignores drawing transforms and composites by direct byte replacement. */
use super::*;

/** Convert every supplied coordinate before observing mutable data or destination state. */
pub(super) fn pixels(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    if call.args.len() < 3 {
        return Err(vm.type_error("putImageData requires an image and two coordinates."));
    }
    let image = call.arg(0);
    results::image_slots(vm, &image)
        .ok_or_else(|| vm.type_error("The value is not an ImageData."))?;
    let (position, dirty) = coordinates(vm, call)?;
    let (width, height, data) = image_info(vm, &image)?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    drawable(vm, node)?;
    let dirty = dirty.unwrap_or([0, 0, width as i64, height as i64]);
    copy(vm, node, [width, height], &data, position, dirty)
}

/** Optional dirty geometry is used only by the seven-argument overload. */
fn coordinates(vm: &mut Vm, call: &Call) -> Res<([i64; 2], Option<[i64; 4]>)> {
    if (4..7).contains(&call.args.len()) {
        return Err(vm.type_error("putImageData requires either three or seven arguments."));
    }
    let mut values = [0_i64; 6];
    let count = if call.args.len() >= 7 { 6 } else { 2 };
    for (index, value) in values.iter_mut().take(count).enumerate() {
        *value = coordinate(vm, &call.arg(index + 1))?;
    }
    Ok((
        [values[0], values[1]],
        (count == 6).then_some([values[2], values[3], values[4], values[5]]),
    ))
}

/** Normalize and intersect source coordinates before their destination offset is applied. */
fn clip(
    source: [usize; 2],
    destination: [u32; 2],
    [dx, dy]: [i64; 2],
    [x, y, w, h]: [i64; 4],
) -> [i64; 4] {
    let left = x.min(x + w).max(0).max(-dx);
    let top = y.min(y + h).max(0).max(-dy);
    let right = x
        .max(x + w)
        .min(source[0] as i64)
        .min(i64::from(destination[0]) - dx)
        .max(left);
    let bottom = y
        .max(y + h)
        .min(source[1] as i64)
        .min(i64::from(destination[1]) - dy)
        .max(top);
    [left, top, right, bottom]
}

/** The temporary typed snapshot is reserved before copying; bitmap COW succeeds before any write. */
fn copy(
    vm: &mut Vm,
    node: usize,
    source: [usize; 2],
    data: &JsValue,
    position: [i64; 2],
    dirty: [i64; 4],
) -> Res<JsValue> {
    let bitmap = &vm.page.state.canvases[&node].bitmap;
    let [left, top, right, bottom] = clip(source, [bitmap.width, bitmap.height], position, dirty);
    if left == right || top == bottom {
        return Ok(JsValue::Undefined);
    }
    work(
        vm,
        source[0] * source[1] + ((right - left) * (bottom - top)) as usize,
    )?;
    let _allocation = limits::Allocation::new(source[0] * source[1] * limits::CHANNELS)?;
    let bytes = typed::source_bytes(vm, data)
        .ok_or_else(|| vm.dom_exception("InvalidStateError", "ImageData buffer is unavailable."))?;
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let width = state.bitmap.width as usize;
    let target = state.bitmap.pixels.make_mut()?;
    for row in top..bottom {
        let start = (row as usize * source[0] + left as usize) * limits::CHANNELS;
        let end = start + (right - left) as usize * limits::CHANNELS;
        let at = ((row + position[1]) as usize * width + (left + position[0]) as usize)
            * limits::CHANNELS;
        target[at..at + end - start].copy_from_slice(&bytes[start..end]);
    }
    state.dirty = true;
    Ok(JsValue::Undefined)
}
