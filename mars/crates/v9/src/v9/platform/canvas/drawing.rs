use super::paints;
use super::placement;
/** Glyph ink is rasterized as opaque black coverage, then composited through the paint source per pixel. */
const INK: [u8; 4] = [0, 0, 0, u8::MAX];
/** Synchronous drawing, text and pixel readback operate on the canvas's actual bounded backing. */
use super::*;
use crate::v9::builtins::typed;

/** Brand first, then run one supported operation through a fixed dispatch table. */
pub(super) fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    type Handler = fn(&mut Vm, &Call, usize) -> Res<JsValue>;
    let handlers: [(&str, Handler); 5] = [
        ("fillRect", rectangle),
        ("clearRect", rectangle),
        ("fillText", text),
        ("measureText", text),
        ("getImageData", readback),
    ];
    let result = handlers
        .iter()
        .find(|(name, _)| *name == dom::native_name(&call))
        .expect("registered operation")
        .1(vm, &call, node);
    resource_result(vm, result)
}
/** WebIDL double conversion can invoke script; acquire mutable state only after converting all arguments. */
fn coordinates(vm: &mut Vm, call: &Call) -> Res<[f64; 4]> {
    let mut values = [0.0; 4];
    for (index, value) in values.iter_mut().enumerate() {
        *value = vm.to_number(&call.arg(index))?;
    }
    Ok(values)
}
/** A nonempty excessive surface cannot be drawn until the page restores valid dimensions. */
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
/** Raster planning clips transformed geometry and charges every covered sample before mutation. */
fn rectangle(vm: &mut Vm, call: &Call, _: usize) -> Res<JsValue> {
    let values = coordinates(vm, call)?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if values.iter().any(|value| !value.is_finite()) {
        return Ok(JsValue::Undefined);
    }
    drawable(vm, node)?;
    let clearing = dom::native_name(call) == "clearRect";
    let state = &vm.page.state.canvases[&node];
    let plan = limits::plan_rect(
        [state.bitmap.width, state.bitmap.height],
        values,
        state.drawing.transform,
    )?
    .with_clip(clipping::current(state)?)?
    .with_composite(if clearing {
        limits::Composite::SourceOver
    } else {
        state.drawing.composite
    });
    work(vm, plan.work())?;
    if clearing {
        let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
        limits::clear_rect(&mut state.bitmap, &plan)?;
        state.dirty |= !plan.is_empty();
        return Ok(JsValue::Undefined);
    }
    let Some((shader, clean)) = paints::source(vm, node, false) else {
        return Ok(JsValue::Undefined);
    };
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    limits::fill_rect_with(&mut state.bitmap, &plan, &shader, state.drawing.alpha)?;
    if !plan.is_empty() {
        state.origin_clean &= clean;
    }
    state.dirty |= !plan.is_empty();
    Ok(JsValue::Undefined)
}
/** Native measurement and ink use the same font; never replace missing glyphs with fake bars. */
fn text(vm: &mut Vm, call: &Call, _: usize) -> Res<JsValue> {
    let value = vm.to_text(&call.arg(0))?;
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if value.encode_utf16().count() > limits::TEXT_UNITS {
        return Err(quota(vm));
    }
    if dom::native_name(call) == "measureText" {
        let fields = placement::measurement(vm, node, &value)?;
        return results::metrics(vm, &fields);
    }
    let at = [vm.to_number(&call.arg(1))?, vm.to_number(&call.arg(2))?];
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if at.iter().any(|v| !v.is_finite()) {
        return Ok(JsValue::Undefined);
    }
    if !matches!(call.arg(3), JsValue::Undefined) {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Canvas text maxWidth is not implemented.",
        ));
    }
    drawable(vm, node)?;
    let clip = clipping::current(&vm.page.state.canvases[&node])?.cloned();
    let pixels = vm.page.state.canvases[&node].bitmap.pixels.len() / limits::CHANNELS;
    work(
        vm,
        pixels * (limits::COMPOSITE_WORK + usize::from(clip.is_some())),
    )?;
    let origin = placement::origin(vm, node, &value, at)?;
    let Some((shader, clean)) = paints::source(vm, node, false) else {
        return Ok(JsValue::Undefined);
    };
    let state = &vm.page.state.canvases[&node];
    if !state.bitmap.drawable() {
        return Ok(JsValue::Undefined);
    }
    let ink = vm.page.canvas_host.text_transformed(
        &state.bitmap,
        &state.drawing.font,
        &value,
        origin,
        INK,
        state.drawing.transform,
    )?;
    ink.validate()?;
    if ink.width != state.bitmap.width || ink.height != state.bitmap.height {
        return Err(EngineError::invalid("Invalid native canvas text dimensions.").into());
    }
    let state = vm.page.state.canvases.get_mut(&node).expect("canvas");
    let (alpha, composite, width) = (
        state.drawing.alpha,
        state.drawing.composite,
        state.bitmap.width as usize,
    );
    for (index, (pixel, glyph)) in state
        .bitmap
        .pixels
        .make_mut()?
        .chunks_exact_mut(4)
        .zip(ink.pixels.chunks_exact(4))
        .enumerate()
    {
        let (x, y) = ((index % width) as u32, (index / width) as u32);
        let clip_coverage = clip.as_ref().map_or(1.0, |clip| clip.coverage(x, y));
        if clip_coverage == 0.0 {
            continue;
        }
        let color = shader.color_at(f64::from(x) + 0.5, f64::from(y) + 0.5);
        let covered = f64::from(glyph[3]) / f64::from(u8::MAX);
        limits::composite_coverage(pixel, color, alpha, composite, covered, clip_coverage);
    }
    state.dirty = true;
    state.origin_clean &= clean;
    Ok(JsValue::Undefined)
}
/** Readback returns a detached typed copy, with transparent padding outside the canvas. */
fn readback(vm: &mut Vm, call: &Call, _: usize) -> Res<JsValue> {
    let mut values = [0.0; 4];
    for (index, value) in values.iter_mut().enumerate() {
        *value = super::images::coordinate(vm, &call.arg(index))? as f64;
    }
    let node = context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let [mut x, mut y, w, h] = values;
    if w == 0.0 || h == 0.0 {
        return Err(vm.dom_exception("IndexSizeError", "ImageData dimensions must be nonzero."));
    }
    require_clean(vm, node)?;
    if w.abs() > f64::from(limits::EDGE) || h.abs() > f64::from(limits::EDGE) {
        return Err(quota(vm));
    }
    if w < 0.0 {
        x += w;
    }
    if h < 0.0 {
        y += h;
    }
    let (width, height) = (w.abs() as usize, h.abs() as usize);
    drawable(vm, node)?;
    work(vm, width * height)?;
    let bitmap = &vm.page.state.canvases[&node].bitmap;
    let _allocation = limits::Allocation::new(width * height * limits::CHANNELS)?;
    let mut bytes = vec![0; width * height * limits::CHANNELS];
    for row in 0..height {
        for col in 0..width {
            let (sx, sy) = (x + col as f64, y + row as f64);
            if sx < 0.0
                || sy < 0.0
                || sx >= f64::from(bitmap.width)
                || sy >= f64::from(bitmap.height)
            {
                continue;
            }
            let start = (sy as usize * bitmap.width as usize + sx as usize) * limits::CHANNELS;
            let target = (row * width + col) * limits::CHANNELS;
            bytes[target..target + limits::CHANNELS]
                .copy_from_slice(&bitmap.pixels[start..start + limits::CHANNELS]);
        }
    }
    let data = typed::new_uint8_clamped(vm, bytes)?;
    results::image(vm, width, height, data)
}
