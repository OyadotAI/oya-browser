/** TextEncoder and TextDecoder for UTF-8, over typed arrays. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;

/** TextEncoder and TextDecoder. */
pub(super) fn install(b: &mut Builder) {
    let encoder = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("TextEncoder", 0, construct, encoder, None);
    b.global("TextEncoder", ctor);
    b.method(encoder, "encode", 0, encode);
    b.method(encoder, "encodeInto", 2, encode_into);
    b.value(
        encoder,
        "encoding",
        crate::v9::vm::realm::string("utf-8"),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    let decoder = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("TextDecoder", 0, construct_decoder, decoder, None);
    b.global("TextDecoder", ctor);
    b.method(decoder, "decode", 1, decode);
    b.value(
        decoder,
        "encoding",
        crate::v9::vm::realm::string("utf-8"),
        crate::v9::vm::heap::CONFIGURABLE,
    );
}

/** `new TextEncoder()`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(vm.type_error("Constructor TextEncoder requires 'new'"));
    };
    let fallback = realm().ids.object_prototype;
    let proto = vm.prototype_for(target, fallback)?;
    Ok(JsValue::Object(
        vm.alloc(Object::new(Class::Plain, Some(proto)))?,
    ))
}

/** `new TextDecoder(label)`: only UTF-8 is available; other labels are a RangeError. */
fn construct_decoder(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let JsValue::String(label) = call.arg(0) {
        let label = label.lossy().trim().to_ascii_lowercase();
        if !matches!(label.as_str(), "utf-8" | "utf8" | "unicode-1-1-utf-8") {
            return Err(vm.range_error(&format!(
                "The encoding label provided ('{label}') is invalid."
            )));
        }
    }
    construct(vm, call)
}

/** The UTF-8 bytes of a string; lone surrogates become U+FFFD as the Encoding spec says. */
fn utf8(vm: &mut Vm, value: &JsValue) -> Res<Vec<u8>> {
    let text = match value {
        JsValue::Undefined => return Ok(Vec::new()),
        other => vm.to_string(other)?,
    };
    Ok(text.lossy().into_bytes())
}

/** TextEncoder.prototype.encode(text). */
fn encode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = utf8(vm, &call.arg(0))?;
    Ok(JsValue::Object(super::typed::new_uint8(vm, bytes)?))
}

/** TextEncoder.prototype.encodeInto(text, uint8Array): whole characters that fit. */
fn encode_into(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = vm.to_string(&call.arg(0))?.lossy();
    let target = call.arg(1);
    let room = super::typed::fill_uint8(vm, &target, &[])
        .map(|_| vm.length_of(&target))
        .transpose()?
        .ok_or_else(|| {
            vm.type_error(
                "Failed to execute 'encodeInto' on 'TextEncoder': parameter 2 is not a Uint8Array.",
            )
        })?;
    let (mut read, mut written) = (0usize, 0usize);
    for character in text.chars() {
        if written + character.len_utf8() > room {
            break;
        }
        written += character.len_utf8();
        read += character.len_utf16();
    }
    super::typed::fill_uint8(vm, &target, &text.as_bytes()[..written]);
    let result = vm.new_object()?;
    vm.put(result, "read", JsValue::Number(read as f64))?;
    vm.put(result, "written", JsValue::Number(written as f64))?;
    Ok(JsValue::Object(result))
}

/** TextDecoder.prototype.decode(bufferSource): malformed input becomes U+FFFD; a BOM is dropped. */
fn decode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = match call.arg(0) {
        JsValue::Undefined => Vec::new(),
        source => super::typed::source_bytes(vm, &source).ok_or_else(|| {
            vm.type_error("Failed to execute 'decode' on 'TextDecoder': The provided value is not of type '(ArrayBuffer or ArrayBufferView)'.")
        })?,
    };
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    super::text(vm, text)
}
