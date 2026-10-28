/** Hex decoding validates complete byte pairs and encoding always emits lowercase ASCII. */
use super::*;

/** Hex digit spelling is fixed by the standard, independent of locale. */
const DIGITS: &[u8; 16] = b"0123456789abcdef";

/** Invalid trailing input beyond a full destination is not inspected, but odd length is always rejected. */
fn decode(vm: &mut Vm, input: &JsString, capacity: usize) -> Res<(Vec<u8>, bool)> {
    if input.len() % 2 != 0 {
        return Err(vm.syntax_error("Invalid hexadecimal string"));
    }
    let mut output = Vec::with_capacity(capacity.min(input.len() / 2));
    for index in 0..capacity.min(input.len() / 2) {
        let (Some(high), Some(low)) = (
            input.unit_at(index * 2).and_then(digit),
            input.unit_at(index * 2 + 1).and_then(digit),
        ) else {
            return Ok((output, false));
        };
        output.push(high * 16 + low);
        vm.charge(1)?;
    }
    Ok((output, true))
}

/** Only the ASCII hexadecimal alphabet maps to nibbles; Unicode lookalikes are invalid. */
fn digit(unit: u16) -> Option<u8> {
    match unit {
        48..=57 => Some((unit - 48) as u8),
        65..=70 => Some((unit - 65 + 10) as u8),
        97..=102 => Some((unit - 97 + 10) as u8),
        _ => None,
    }
}

/** Static construction always creates the intrinsic Uint8Array and ignores an authored this constructor. */
pub(super) fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let input = input(vm, call.arg(0))?;
    let (bytes, valid) = decode(vm, &input, input.len() / 2)?;
    if !valid {
        return Err(vm.syntax_error("Invalid hexadecimal string"));
    }
    new_uint8(vm, bytes).map(JsValue::Object)
}

/** A malformed pair leaves preceding decoded bytes visible before throwing SyntaxError. */
pub(super) fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let view = write_receiver(vm, &call)?;
    let input = input(vm, call.arg(0))?;
    let (bytes, valid) = decode(vm, &input, view.length)?;
    commit(vm, &view, &bytes)?;
    if !valid {
        return Err(vm.syntax_error("Invalid hexadecimal string"));
    }
    result(vm, bytes.len() * 2, bytes.len())
}

/** Output allocation obeys the existing string allowance before reserving the UTF-16 result. */
pub(super) fn to(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let view = receiver(vm, &call)?;
    if view.length > crate::constants::JS_STRING_UNITS / 2 {
        return Err(crate::protocol::EngineError::limit(
            "JavaScript string UTF-16 units",
            crate::constants::JS_STRING_UNITS,
        )
        .into());
    }
    let bytes = read_range(vm, view.buffer, view.offset, view.length)
        .ok_or_else(|| vm.type_error("Invalid Uint8Array backing"))?;
    let mut output = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as u16);
        output.push(DIGITS[(byte & 15) as usize] as u16);
        vm.charge(1)?;
    }
    Ok(JsValue::String(JsString::from_units(output)?))
}
