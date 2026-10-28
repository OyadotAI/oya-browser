/** Indexed reference contracts retain their owning module across JS globals, tables and serialization. */
use super::*;
const OWNER: &str = "wasm.type.module";
/** A concrete continuation is private to Wasm even when its object is null. */
pub(super) fn js_compatible(vm: &mut Vm, object: ObjectId, ty: Type) -> Res<bool> {
    if !ty.js_compatible() {
        return Ok(false);
    }
    if !ty.reference().is_some_and(|(heap, _)| heap >= 0) {
        return Ok(true);
    }
    let owner = get(vm, object, OWNER).ok_or_else(|| {
        vm.type_error("indexed WebAssembly reference requires its owning type graph")
    })?;
    let (_, m) = module(vm, &owner)?;
    Ok(gc_types::js_compatible(&m, ty))
}

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

/** Immutable global imports permit a canonical subtype from another module's type graph. */
pub(super) fn subtype(
    vm: &mut Vm,
    object: ObjectId,
    actual: Type,
    expected: &Module,
    ty: Type,
) -> Res<bool> {
    let (Some((source, source_null)), Some((target, target_null))) =
        (actual.reference(), ty.reference())
    else {
        return Ok(actual == ty);
    };
    if source_null && !target_null {
        return Ok(false);
    }
    if source < 0 {
        return Ok(gc_types::subtype(expected, actual, ty));
    }
    let Some(owner) = get(vm, object, OWNER) else {
        return Ok(false);
    };
    let (_, source_module) = module(vm, &owner)?;
    if gc_types::is_shared(&source_module, actual) != gc_types::is_shared(expected, ty) {
        return Ok(false);
    }
    if target < 0 {
        return Ok(gc_types::subtype(&source_module, actual, ty));
    }
    let mut current = Some(source as u32);
    for depth in 0..=source_module.definitions.len() {
        let Some(index) = current else {
            return Ok(false);
        };
        if gc_types::equivalent(&source_module, index, expected, target as u32) {
            return Ok(
                !matches!(ty, Type::Exact(..)) || depth == 0 && matches!(actual, Type::Exact(..))
            );
        }
        if matches!(ty, Type::Exact(..)) {
            return Ok(false);
        }
        current = source_module
            .definitions
            .get(index as usize)
            .and_then(|definition| definition.parent);
    }
    Ok(false)
}
