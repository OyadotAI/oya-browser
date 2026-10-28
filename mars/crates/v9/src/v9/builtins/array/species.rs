/** Array species creation preserves subclass constructors, foreign defaults and property failures. */
use super::*;

/** A foreign intrinsic Array defaults to the caller's Array, while author constructors remain visible. */
pub(super) fn create(vm: &mut Vm, original: &JsValue, length: usize) -> Res<ObjectId> {
    create_with_fresh(vm, original, length).map(|(result, _)| result)
}

/** Tell callers when ArraySpeciesCreate used a known fresh intrinsic allocation. */
pub(super) fn create_with_fresh(
    vm: &mut Vm,
    original: &JsValue,
    length: usize,
) -> Res<(ObjectId, bool)> {
    if !builtin_subclassing(vm) {
        return vm.new_array_length(length).map(|result| (result, true));
    }
    let mut constructor = JsValue::Undefined;
    if is_array_checked(vm, original)? {
        constructor = vm.get(original, &key("constructor"))?;
        if let Some(id) = constructor.object()
            && vm.object_realm(id) != vm.active_realm
            && id == vm.object_intrinsics(id).ids.array
        {
            constructor = JsValue::Undefined;
        }
        if constructor.object().is_some() {
            constructor = vm.get(&constructor, &Key::Sym(symbol::SPECIES))?;
            if constructor == JsValue::Null {
                constructor = JsValue::Undefined;
            }
        }
    }
    let fresh = constructor == JsValue::Undefined
        || constructor
            .object()
            .is_some_and(|id| id == vm.object_intrinsics(id).ids.array);
    let result = if constructor == JsValue::Undefined {
        vm.new_array_length(length)?
    } else {
        if !vm.is_constructor(&constructor) {
            return Err(vm.type_error("Array species is not a constructor"));
        }
        vm.construct(constructor, vec![JsValue::Number(length as f64)], None)?
            .object()
            .ok_or_else(|| vm.type_error("Array species did not create an object"))?
    };
    vm.pin(JsValue::Object(result));
    Ok((result, fresh))
}

/** CreateDataProperty never invokes a prototype setter, and refusal must throw. */
pub(super) fn define(vm: &mut Vm, result: ObjectId, index: usize, value: JsValue) -> Res<()> {
    vm.charge(1)?;
    if vm.define(
        result,
        index_key(index)?,
        crate::v9::vm::property::Descriptor::data(value),
    )? {
        return Ok(());
    }
    Err(vm.type_error("Cannot create array result property"))
}
