/** Test string preparation creates real independently owned UTF-16 backing storage. */
use crate::v9::value::{JsString, JsValue};
use crate::v9::vm::error::ErrorKind;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};

/** V9 has only two-byte strings; both creators preserve that actual representation without externalizing it. */
pub(super) fn install(builder: &mut Builder) {
    for name in [
        "createExternalizableString",
        "createExternalizableTwoByteString",
    ] {
        let function = builder.function(name, 0, copy);
        builder.global(name, function);
    }
}
/** Copy raw code units into a fresh Arc slice; no coercion, one-byte tag or externalization marker is invented. */
fn copy(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(source) = call.arg(0) else {
        return Err(vm.throw(
            ErrorKind::Error,
            "First parameter to createExternalizableString() must be a string.",
        ));
    };
    vm.charge(source.len())?;
    Ok(JsValue::String(JsString::from_units(
        source.as_units().to_vec(),
    )?))
}
