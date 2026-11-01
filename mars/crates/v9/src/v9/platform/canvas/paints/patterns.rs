/** CanvasPattern creation from a canvas or image source, and its pattern-space transform. */
use super::*;
use mars_core::canvas::{Affine, Paint, Pattern, Repeat};
static PROTO: OnceLock<u32> = OnceLock::new();

/** The interface is illegal to construct directly; its one method replaces the pattern transform. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    super::super::super::chrome_env::interface(b, "CanvasPattern", proto);
    b.host_method(proto, "setTransform", 0, set_transform);
    let _ = PROTO.set(proto);
}

/** Repetition is validated before the source is read, and an empty source yields null as the specification requires. */
pub(super) fn create(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let repeat_text = match call.arg(1) {
        JsValue::Undefined => String::new(),
        value => vm.to_text(&value)?,
    };
    let repeat = Repeat::named(&repeat_text)
        .ok_or_else(|| vm.dom_exception("SyntaxError", "Invalid repetition keyword."))?;
    let source = call.arg(0);
    let node = dom::node_of(vm, &source)
        .filter(|node| matches!(vm.page.tag(*node).as_str(), "canvas" | "img"))
        .ok_or_else(|| vm.type_error("createPattern requires a canvas or image source."))?;
    if vm.page.tag(node) == "canvas" {
        let [width, height] = size(&mut vm.page, node);
        if width == 0 || height == 0 {
            return Ok(JsValue::Null);
        }
    }
    let Some(source) = images::copy::snapshot(vm, node)? else {
        return Ok(JsValue::Null);
    };
    if !source.bitmap.drawable() {
        return Ok(JsValue::Null);
    }
    let entry = Entry {
        paint: Paint::Pattern(Pattern::new(source.bitmap, repeat)),
        clean: source.origin_clean,
    };
    wrap(vm, entry, *PROTO.get().expect("pattern prototype"))
}

/** An absent matrix is the identity; a dictionary goes through the same conversion as the context's transforms. */
fn set_transform(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    if !matches!(
        vm.page
            .state
            .canvas_paints
            .get(&id)
            .map(|entry| &entry.paint),
        Some(Paint::Pattern(_))
    ) {
        return Err(vm.type_error("Illegal invocation"));
    }
    let transform = match call.arg(0) {
        JsValue::Undefined => Affine::IDENTITY,
        value => state::matrix_dictionary(vm, &value)?,
    };
    if let Some(Paint::Pattern(pattern)) = vm
        .page
        .state
        .canvas_paints
        .get_mut(&id)
        .map(|entry| &mut entry.paint)
    {
        pattern.transform = transform;
    }
    Ok(JsValue::Undefined)
}
