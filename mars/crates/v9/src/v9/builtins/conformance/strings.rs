/** The opt-in d8 extension owns real bounded external string resources and encoding metadata. */
use crate::v9::value::{JsString, JsValue};
use crate::v9::vm::error::ErrorKind;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};

/** The extension is absent from d8 globals unless its explicit host flag is enabled. */
pub(super) fn expose(vm: &mut Vm, global: crate::v9::value::ObjectId, bank: u32) {
    if vm.page.environment.conformance_expose_externalize_string {
        install_lexicals(vm, bank);
        return;
    }
    for name in [
        "createExternalizableString",
        "createExternalizableTwoByteString",
        "externalizeString",
        "isOneByteString",
        "kExternalStringMinOneByteLength",
        "kExternalStringMinTwoByteLength",
        "kExternalStringMinOneByteCachedLength",
        "kExternalStringMinTwoByteCachedLength",
    ] {
        vm.heap
            .remove(global, &crate::v9::vm::heap::Key::from(name));
    }
}

/** Install the extension's actual per-context global lexical declarations. */
pub(in crate::v9) fn install_lexicals(vm: &mut Vm, bank: u32) {
    let bindings = if bank == vm.active_realm {
        &mut vm.globals
    } else {
        &mut vm.realms.contexts[bank as usize].globals
    };
    for (name, value) in [
        (
            "kExternalStringMinOneByteLength",
            EXTERNAL_MIN_ONE_BYTE_LENGTH,
        ),
        (
            "kExternalStringMinTwoByteLength",
            EXTERNAL_MIN_TWO_BYTE_LENGTH,
        ),
        (
            "kExternalStringMinOneByteCachedLength",
            EXTERNAL_MIN_ONE_BYTE_CACHED_LENGTH,
        ),
        (
            "kExternalStringMinTwoByteCachedLength",
            EXTERNAL_MIN_TWO_BYTE_CACHED_LENGTH,
        ),
    ] {
        bindings.insert(
            name.into(),
            crate::v9::binding::Binding {
                kind: crate::v9::binding::BindingKind::Let,
                value: Some(JsValue::Number(value as f64)),
            },
        );
    }
}

/** Minimum externalizable length for one-byte strings in an uncompressed V8 build; only the empty string is refused. */
const EXTERNAL_MIN_ONE_BYTE_LENGTH: usize = 1;
/** Minimum externalizable length for two-byte strings in an uncompressed V8 build; only the empty string is refused. */
const EXTERNAL_MIN_TWO_BYTE_LENGTH: usize = 1;
/** Cached resources require space for a native data pointer in an uncompressed 64-bit build. */
const EXTERNAL_MIN_ONE_BYTE_CACHED_LENGTH: usize = 9;
const EXTERNAL_MIN_TWO_BYTE_CACHED_LENGTH: usize = 5;

/** Test string preparation creates real independently owned UTF-16 backing storage. */
pub(super) fn install(builder: &mut Builder) {
    let function = builder.function("createExternalizableString", 0, copy);
    builder.global("createExternalizableString", function);
    let function = builder.function("createExternalizableTwoByteString", 0, copy_two_byte);
    builder.global("createExternalizableTwoByteString", function);
    let externalize = builder.function("externalizeString", 1, externalize);
    builder.global("externalizeString", externalize);
    let is_one_byte = builder.function("isOneByteString", 1, is_one_byte);
    builder.global("isOneByteString", is_one_byte);
}
/** Copy raw code units into a fresh Arc slice; no coercion, one-byte tag or externalization marker is invented. */
fn copy(vm: &mut Vm, call: Call) -> Res<JsValue> {
    copy_impl(vm, call, false)
}
/** The separate host constructor deliberately keeps a two-byte externalizable representation. */
fn copy_two_byte(vm: &mut Vm, call: Call) -> Res<JsValue> {
    copy_impl(vm, call, true)
}
fn copy_impl(vm: &mut Vm, call: Call, two_byte: bool) -> Res<JsValue> {
    let JsValue::String(source) = call.arg(0) else {
        return Err(vm.throw(
            ErrorKind::Error,
            "First parameter to createExternalizableString() must be a string.",
        ));
    };
    if source.is_external() {
        return Ok(JsValue::String(source));
    }
    if read_only(&source) {
        return Err(vm.throw(
            ErrorKind::Error,
            "Read-only strings cannot be externalized.",
        ));
    }
    let shared_allowed = !source.is_shared()
        || vm.page.environment.conformance_shared_string_table
        || vm.page.environment.conformance_harmony_struct;
    if source.is_old() && shared_allowed && (!two_byte || !source.is_latin1()) {
        return Ok(JsValue::String(source));
    }
    if source.len() > crate::constants::JS_STRING_UNITS {
        return Err(crate::protocol::EngineError::limit(
            "external string UTF-16 units",
            crate::constants::JS_STRING_UNITS,
        )
        .into());
    }
    if let Some((left, right)) = source.externalizable_cons_parts() {
        if two_byte && left.is_latin1() && right.is_latin1() {
            return Err(vm.throw(
                ErrorKind::Error,
                "Cannot create externalizable two-byte string from one-byte ConsString.",
            ));
        }
        vm.charge(source.len())?;
        let value = left
            .construct_cons(&right)?
            .ok_or_else(|| vm.unsupported_error("externalizable cons string"))?;
        value.mark_old();
        return Ok(JsValue::String(value));
    }
    vm.charge(source.len())?;
    let value = vm.copy_string(&source, two_byte || !source.is_latin1())?;
    value.mark_old();
    Ok(JsValue::String(value))
}
/** Single code units are cached in V8's read-only space and can never be externalized. */
const READ_ONLY_MAX_LENGTH: usize = 1;
/** Externalizing publishes owned immutable external storage shared by every existing alias. */
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
    if read_only(&string) {
        return Err(vm.throw(ErrorKind::Error, "Cannot externalize a read-only string."));
    }
    if !string.is_old()
        || (string.is_shared()
            && !vm.page.environment.conformance_shared_string_table
            && !vm.page.environment.conformance_harmony_struct)
    {
        return Err(vm.throw(ErrorKind::Error, "string does not support externalization."));
    }
    if string.len() > crate::constants::JS_STRING_UNITS {
        return Err(crate::protocol::EngineError::limit(
            "external string UTF-16 units",
            crate::constants::JS_STRING_UNITS,
        )
        .into());
    }
    vm.charge(string.len())?;
    string.externalize()?;
    vm.check_heap_bounds()?;
    Ok(JsValue::Undefined)
}
/** V8 caches the empty string and Latin-1 single-unit strings in read-only space. */
fn read_only(string: &JsString) -> bool {
    string.is_empty()
        || (string.len() == READ_ONLY_MAX_LENGTH
            && string.unit_at(0).is_some_and(|unit| unit <= 0xff))
}
/** The minimum depends on the encoding a V8 build would pick, which is decided by the content. */
fn minimum_length(string: &JsString) -> usize {
    if string.is_latin1() {
        EXTERNAL_MIN_ONE_BYTE_LENGTH
    } else {
        EXTERNAL_MIN_TWO_BYTE_LENGTH
    }
}
/** Reports the actual one-byte or two-byte representation retained by the string. */
fn is_one_byte(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.args.len() != 1 {
        return Err(vm.throw(
            ErrorKind::Error,
            "isOneByteString() requires a single string argument.",
        ));
    }
    let JsValue::String(string) = call.arg(0) else {
        return Err(vm.throw(
            ErrorKind::Error,
            "isOneByteString() requires a single string argument.",
        ));
    };
    Ok(JsValue::Boolean(string.is_latin1()))
}
