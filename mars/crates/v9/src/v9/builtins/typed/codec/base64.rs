/** Base64 option parsing and encoding preserve access order and byte view bounds after callbacks. */
use super::*;

/** The URL alphabet differs only in its two final symbols. */
const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/** The URL-safe alphabet uses dash and underscore in place of plus and slash. */
const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/** Decode modes determine whether an unpadded final quantum is decoded, rejected, or left unread. */
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Tail {
    Loose,
    Strict,
    Stop,
}

/** Alphabet is checked before the last chunk getter, without coercing either value. */
fn alphabet(vm: &mut Vm, options: &JsValue) -> Res<bool> {
    match option(vm, options, "alphabet")? {
        JsValue::Undefined => Ok(false),
        JsValue::String(text) if text.is("base64") => Ok(false),
        JsValue::String(text) if text.is("base64url") => Ok(true),
        _ => Err(vm.type_error("Invalid base64 alphabet option")),
    }
}

/** Invalid option values throw TypeError before any source character is decoded. */
fn decode_options(vm: &mut Vm, value: JsValue) -> Res<(bool, Tail)> {
    let options = options(vm, value)?;
    let url = alphabet(vm, &options)?;
    let tail = match option(vm, &options, "lastChunkHandling")? {
        JsValue::Undefined => Tail::Loose,
        JsValue::String(text) if text.is("loose") => Tail::Loose,
        JsValue::String(text) if text.is("strict") => Tail::Strict,
        JsValue::String(text) if text.is("stop-before-partial") => Tail::Stop,
        _ => return Err(vm.type_error("Invalid base64 lastChunkHandling option")),
    };
    Ok((url, tail))
}

/** Static decoding creates a fresh intrinsic typed array after full validation succeeds. */
pub(super) fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let input = input(vm, call.arg(0))?;
    let (url, tail) = decode_options(vm, call.arg(1))?;
    let decoded = decode::parse(vm, &input, url, tail, BUFFER_BYTES)?;
    if decoded.invalid {
        return Err(vm.syntax_error("Invalid base64 string"));
    }
    new_uint8(vm, decoded.bytes).map(JsValue::Object)
}

/** Getter-induced resize or detachment is observed before selecting the destination capacity. */
pub(super) fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    receiver(vm, &call)?;
    let input = input(vm, call.arg(0))?;
    let (url, tail) = decode_options(vm, call.arg(1))?;
    let view = receiver(vm, &call)?;
    let decoded = decode::parse(vm, &input, url, tail, view.length)?;
    commit(vm, &view, &decoded.bytes)?;
    if decoded.invalid {
        return Err(vm.syntax_error("Invalid base64 string"));
    }
    result(vm, decoded.read, decoded.bytes.len())
}

/** Options run before copying live bytes and omitPadding uses ordinary ToBoolean. */
pub(super) fn to(vm: &mut Vm, call: Call) -> Res<JsValue> {
    brand(vm, &call)?;
    let options = options(vm, call.arg(0))?;
    let url = alphabet(vm, &options)?;
    let omit = option(vm, &options, "omitPadding")?.truthy();
    let view = receiver(vm, &call)?;
    let output_length =
        view.length.div_ceil(3) * 4 - if omit { (3 - view.length % 3) % 3 } else { 0 };
    if output_length > crate::constants::JS_STRING_UNITS {
        return Err(crate::protocol::EngineError::limit(
            "JavaScript string UTF-16 units",
            crate::constants::JS_STRING_UNITS,
        )
        .into());
    }
    let bytes = read_range(vm, view.buffer, view.offset, view.length)
        .ok_or_else(|| vm.type_error("Invalid Uint8Array backing"))?;
    let mut output = Vec::with_capacity(output_length);
    let alphabet = if url { URL } else { STANDARD };
    for chunk in bytes.chunks(3) {
        encode(chunk, alphabet, omit, &mut output);
        vm.charge(1)?;
    }
    Ok(JsValue::String(JsString::from_units(output)?))
}

/** Each complete or partial quantum emits the correct number of symbols and optional padding. */
fn encode(bytes: &[u8], alphabet: &[u8; 64], omit: bool, output: &mut Vec<u16>) {
    let word = (u32::from(bytes[0]) << 16)
        | (u32::from(*bytes.get(1).unwrap_or(&0)) << 8)
        | u32::from(*bytes.get(2).unwrap_or(&0));
    for index in 0..4 {
        if index < bytes.len() + 1 {
            output.push(alphabet[((word >> (18 - 6 * index)) & 63) as usize] as u16)
        } else if !omit {
            output.push(u16::from(b'='))
        }
    }
}
