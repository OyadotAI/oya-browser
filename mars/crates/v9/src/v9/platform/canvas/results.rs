/** Branded read-only Canvas result objects expose actual measured values and copied pixel data. */
use super::*;
static METRICS: OnceLock<u32> = OnceLock::new();
static IMAGE: OnceLock<u32> = OnceLock::new();

/** Install real result interfaces before the compatibility shape table fills unsupported members. */
pub(super) fn install(b: &mut Builder) {
    let metrics = b.object(Some(b.ids.object_prototype));
    super::super::chrome_env::interface(b, "TextMetrics", metrics);
    b.getter(metrics, "width", metrics_get);
    let _ = METRICS.set(metrics);
    let image = b.object(Some(b.ids.object_prototype));
    let constructor = b.constructor("ImageData", 2, super::images::construct, image, None);
    b.global("ImageData", constructor);
    for field in ["width", "height", "data", "colorSpace", "pixelFormat"] {
        b.getter(image, field, image_get);
    }
    b.put(
        image,
        crate::v9::vm::heap::Key::Sym(crate::v9::vm::realm::symbol::TO_STRING_TAG),
        crate::v9::vm::realm::Static::Value(crate::v9::vm::realm::string("ImageData")),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    let _ = IMAGE.set(image);
}
/** Receiver slots are inaccessible to script, so forged prototype objects fail branding. */
fn get(vm: &mut Vm, call: Call, kind: &str) -> Res<JsValue> {
    if encoding::read_slot(vm, &call.this, "canvas.result.kind") != Some(JsValue::string(kind)?) {
        return Err(vm.type_error("Illegal invocation"));
    }
    encoding::read_slot(
        vm,
        &call.this,
        &format!("canvas.result.{}", dom::native_name(&call)),
    )
    .ok_or_else(|| vm.type_error("Illegal invocation"))
}
/** A metrics getter cannot borrow the identically named ImageData width slot. */
fn metrics_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    get(vm, call, "metrics")
}
/** An image getter remains branded even when its mutable typed array becomes detached. */
fn image_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    get(vm, call, "image")
}
/** Build one immutable interface result with GC-visible private references. */
fn record(vm: &mut Vm, prototype: u32, fields: Vec<(&str, JsValue)>) -> Res<JsValue> {
    let id = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    for (name, value) in fields {
        encoding::write_slot(vm, id, &format!("canvas.result.{name}"), value);
    }
    Ok(JsValue::Object(id))
}
/** Native width is reported without inventing unimplemented font metrics. */
pub(super) fn metrics(vm: &mut Vm, width: f64) -> Res<JsValue> {
    record(
        vm,
        *METRICS.get().expect("metrics interface"),
        vec![
            ("kind", JsValue::string("metrics")?),
            ("width", JsValue::Number(width)),
        ],
    )
}
/** Readback owns a new typed array, never a view of the mutable canvas backing. */
pub(super) fn image(vm: &mut Vm, width: usize, height: usize, data: ObjectId) -> Res<JsValue> {
    image_as(vm, width, height, data, image_prototype())
}
/** Constructors may derive from ImageData without losing its inaccessible data brand. */
pub(super) fn image_as(
    vm: &mut Vm,
    width: usize,
    height: usize,
    data: ObjectId,
    prototype: ObjectId,
) -> Res<JsValue> {
    record(
        vm,
        prototype,
        vec![
            ("kind", JsValue::string("image")?),
            ("width", JsValue::Number(width as f64)),
            ("height", JsValue::Number(height as f64)),
            ("data", JsValue::Object(data)),
            ("colorSpace", JsValue::string("srgb")?),
            ("pixelFormat", JsValue::string("rgba-unorm8")?),
        ],
    )
}

/** The constructor fallback is fixed even when script changes the public constructor property. */
pub(super) fn image_prototype() -> ObjectId {
    *IMAGE.get().expect("image data interface")
}

/** Read inaccessible slots, never user-shadowed public attributes. */
pub(super) fn image_slots(vm: &Vm, value: &JsValue) -> Option<(usize, usize, JsValue)> {
    let JsValue::String(kind) = encoding::read_slot(vm, value, "canvas.result.kind")? else {
        return None;
    };
    if kind.lossy() != "image" {
        return None;
    }
    let dimension = |name| match encoding::read_slot(vm, value, name)? {
        JsValue::Number(number)
            if number.is_finite()
                && number > 0.0
                && number <= f64::from(limits::EDGE)
                && number.fract() == 0.0 =>
        {
            Some(number as usize)
        }
        _ => None,
    };
    Some((
        dimension("canvas.result.width")?,
        dimension("canvas.result.height")?,
        encoding::read_slot(vm, value, "canvas.result.data")?,
    ))
}
