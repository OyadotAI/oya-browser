/** Branded retained paths own bounded geometry without becoming permanent JavaScript GC roots. */
use super::*;
use mars_core::canvas::Affine;
mod arguments;
mod geometry;
mod lifecycle;
pub(super) use arguments::{fill_arguments, snapshot, stroke_argument};
pub(in crate::v9) use lifecycle::{validate, validate_targets};
/** Retained wrappers and native geometry have independent page bounds. */
const OBJECTS: usize = 64;
/** Aggregate geometry cannot grow with every retained wrapper up to its individual maximum. */
const POINTS: usize = 16_384;
/** Private self identity makes prototype spoofing and mismatched restored associations invalid. */
const BRAND: &str = "canvas.path2d";
static PROTOTYPE: OnceLock<u32> = OnceLock::new();

/** Only finite bounded geometry is serialized; no context or current drawing state is captured. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Retained {
    path: limits::Path,
}

/** All advertised methods build usable paths, including the existing curve and rounded geometry. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    for (name, length) in [
        ("closePath", 0),
        ("moveTo", 2),
        ("lineTo", 2),
        ("rect", 4),
        ("quadraticCurveTo", 4),
        ("bezierCurveTo", 6),
        ("arc", 5),
        ("arcTo", 5),
        ("ellipse", 7),
        ("roundRect", 4),
        ("addPath", 1),
    ] {
        b.host_method(proto, name, length, method);
    }
    let ctor = b.constructor("Path2D", 0, construct, proto, None);
    b.global("Path2D", ctor);
    let _ = PROTOTYPE.set(proto);
}

/** Constructor refusals and resource bounds remain catchable by feature-fallback code. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = create(vm, &call);
    resource_result(vm, result)
}

/** Copy construction snapshots independent geometry; strings use bounded SVG prefix recovery. */
fn create(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let target = call
        .new_target
        .as_ref()
        .ok_or_else(|| vm.type_error("Path2D requires 'new'"))?;
    let source = call.arg(0);
    let path = if matches!(source, JsValue::Undefined) {
        limits::Path::default()
    } else if let Some(id) = identity(vm, &source) {
        vm.page.state.path2d[&id].path.clone()
    } else {
        let text = vm.to_text(&source)?;
        work(vm, text.len())?;
        limits::Path::from_svg(&text)?
    };
    let prototype = vm.prototype_for(target, *PROTOTYPE.get().expect("Path2D installed"))?;
    vm.pin(JsValue::Object(prototype));
    lifecycle::admit(vm, None, path.point_count())?;
    let id = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    encoding::write_slot(vm, id, BRAND, JsValue::Object(id));
    vm.page.state.path2d.insert(id, Retained { path });
    Ok(JsValue::Object(id))
}

/** Native branding uses both association ownership and a private self edge. */
fn identity(vm: &mut Vm, value: &JsValue) -> Option<ObjectId> {
    let id = value.object()?;
    (vm.page.state.path2d.contains_key(&id)
        && encoding::read_slot(vm, value, BRAND) == Some(JsValue::Object(id)))
    .then_some(id)
}

/** Borrowed methods reject forged receivers before invoking author conversion callbacks. */
fn receiver(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    identity(vm, value).ok_or_else(|| vm.type_error("Illegal Path2D invocation"))
}

/** Geometry operates on a candidate; quota and numeric failures cannot partially modify the source. */
fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = geometry::dispatch(vm, &call);
    resource_result(vm, result)
}

/** Admission runs before replacing the retained geometry and may reclaim unreachable wrappers. */
fn replace(vm: &mut Vm, id: ObjectId, path: limits::Path) -> Res<()> {
    vm.pin(JsValue::Object(id));
    lifecycle::admit(vm, Some(id), path.point_count())?;
    vm.page.state.path2d.insert(id, Retained { path });
    Ok(())
}
