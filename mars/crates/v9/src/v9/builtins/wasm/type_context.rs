/** Indexed reference contracts retain their owning module across JS globals, tables and serialization. */
use super::*;
const OWNER: &str = "wasm.type.module";

/** Native wrappers root the source type graph independently of a live Instance or Module JS variable. */
pub(super) fn bind(vm: &mut Vm, object: ObjectId, module: ObjectId) {
    set(vm, object, OWNER, JsValue::Object(module))
}
/** JS mutations revalidate concrete/exact references against canonical shapes in the owning graph. */
pub(super) fn convert(vm: &mut Vm, object: ObjectId, ty: Type, value: &JsValue) -> Res<JsValue> {
    let value = global::convert(vm, ty, value)?;
    if ty.reference().is_some_and(|(heap, _)| heap >= 0) {
        let owner = get(vm, object, OWNER).ok_or_else(|| {
            vm.type_error("indexed WebAssembly reference requires its owning type graph")
        })?;
        let (_, m) = module(vm, &owner)?;
        if !gc::matches(vm, &m, ty, &value)? {
            return Err(vm.type_error("object is not a subtype of expected type"));
        }
    }
    Ok(value)
}
/** Module-local type numbers are compared canonically rather than mistaken for cross-module identities. */
pub(super) fn equivalent(
    vm: &mut Vm,
    object: ObjectId,
    actual: Type,
    expected: &Module,
    ty: Type,
) -> Res<bool> {
    if !actual.reference().is_some_and(|(heap, _)| heap >= 0)
        && !ty.reference().is_some_and(|(heap, _)| heap >= 0)
    {
        return Ok(actual == ty);
    }
    let Some(owner) = get(vm, object, OWNER) else {
        return Ok(false);
    };
    let (_, m) = module(vm, &owner)?;
    Ok(gc_types::equivalent_type(&m, actual, expected, ty))
}
