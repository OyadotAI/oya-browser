/** Externalization host API: V9 has one string form, so these calls validate and answer V8's observable contract without moving contents. */
use crate::v9::value::{JsString, JsValue};
use crate::v9::vm::error::ErrorKind;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};

/** Minimum externalizable length for one-byte strings in an uncompressed V8 build; only the empty string is refused. */
const EXTERNAL_MIN_ONE_BYTE_LENGTH: usize = 1;
/** Minimum externalizable length for two-byte strings in an uncompressed V8 build; only the empty string is refused. */
const EXTERNAL_MIN_TWO_BYTE_LENGTH: usize = 1;

/** Test string preparation creates real independently owned UTF-16 backing storage. */
pub(super) fn install(builder: &mut Builder) {
    for name in [
        "createExternalizableString",
        "createExternalizableTwoByteString",
    ] {
        let function = builder.function(name, 0, copy);
        builder.global(name, function);
    }
    let externalize = builder.function("externalizeString", 1, externalize);
    builder.global("externalizeString", externalize);
    let is_one_byte = builder.function("isOneByteString", 1, is_one_byte);
    builder.global("isOneByteString", is_one_byte);
    builder.global_value(
        "kExternalStringMinOneByteLength",
        JsValue::Number(EXTERNAL_MIN_ONE_BYTE_LENGTH as f64),
    );
    builder.global_value(
        "kExternalStringMinTwoByteLength",
        JsValue::Number(EXTERNAL_MIN_TWO_BYTE_LENGTH as f64),
    );
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
/** Single code units are cached in V8's read-only space and can never be externalized. */
const READ_ONLY_MAX_LENGTH: usize = 1;
/** Externalizing validates the operand and leaves contents untouched, since V9 has no second heap form to move the string into. */
fn externalize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(string) = call.arg(0) else {
        return Err(vm.throw(
            ErrorKind::Error,
            "First parameter to externalizeString() must be a string.",
        ));
    };
    if string.len() < minimum_length(&string) {
        return Err(vm.throw(ErrorKind::Error, "String is too short to be externalized."));
    }
    if string.len() <= READ_ONLY_MAX_LENGTH {
        return Err(vm.throw(ErrorKind::Error, "Cannot externalize a read-only string."));
    }
    Ok(JsValue::Undefined)
}
/** The minimum depends on the encoding a V8 build would pick, which is decided by the content. */
fn minimum_length(string: &JsString) -> usize {
    if string.is_latin1() {
        EXTERNAL_MIN_ONE_BYTE_LENGTH
    } else {
        EXTERNAL_MIN_TWO_BYTE_LENGTH
    }
}
/** Reports the content property: every code unit fits Latin-1, regardless of how the string was built. */
fn is_one_byte(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(string) = call.arg(0) else {
        return Err(vm.throw(
            ErrorKind::Error,
            "isOneByteString() requires a single string argument.",
        ));
    };
    Ok(JsValue::Boolean(string.is_latin1()))
}
