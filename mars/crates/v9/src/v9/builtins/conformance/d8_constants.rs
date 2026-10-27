/** d8.constants exposes V9's real array bound under V8's name, so upstream tests size arrays against the engine's own limit. */
use super::*;
use crate::constants::VM_ARRAY_LENGTH;
use crate::v9::value::ObjectId;
use crate::v9::vm::heap::DEFAULT_ATTRS;

/** V8's name for the largest backing store a fixed array may hold. */
const MAX_FIXED_ARRAY_CAPACITY: &str = "maxFixedArrayCapacity";

/** The namespace is a plain object of numeric constants; V9 has one dense-array bound, so one value is exposed. */
pub(super) fn install(builder: &mut Builder, d8: ObjectId) {
    let constants = builder.object(Some(builder.ids.object_prototype));
    builder.value(
        constants,
        MAX_FIXED_ARRAY_CAPACITY,
        JsValue::Number(VM_ARRAY_LENGTH as f64),
        DEFAULT_ATTRS,
    );
    builder.alias(d8, "constants", constants);
}
