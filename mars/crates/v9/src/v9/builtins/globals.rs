/** Global functions: number parsing, URI coding, escape/unescape and indirect eval. */
use super::*;
use crate::v9::vm::realm::NativeFn;

/** The global function objects; the global object itself is installed by the platform. */
pub(super) fn install(b: &mut Builder) {
    let functions: [(&'static str, u32, NativeFn); 11] = [
        ("parseInt", 2, parse_int),
        ("parseFloat", 1, parse_float),
        ("isNaN", 1, is_nan),
        ("isFinite", 1, is_finite),
        ("encodeURIComponent", 1, |vm, call| {
            encode(vm, call, COMPONENT_SAFE)
        }),
        ("encodeURI", 1, |vm, call| encode(vm, call, URI_SAFE)),
        ("decodeURIComponent", 1, |vm, call| decode(vm, call, "")),
        ("decodeURI", 1, |vm, call| decode(vm, call, URI_RESERVED)),
        ("escape", 1, escape),
        ("unescape", 1, unescape),
        ("eval", 1, eval),
    ];
    for (name, length, function) in functions {
        let id = b.function(name, length, function);
        b.global(name, id);
        if matches!(name, "parseInt" | "parseFloat") {
            super::number::install_parser(b, name, id);
        }
    }
}

/** parseInt. */
fn parse_int(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?.lossy();
    let mut radix = crate::v9::value::to_int32(arg_number(vm, &call, 1)?);
    let trimmed = text.trim_start_matches(|c: char| crate::v9::source::whitespace(c));
    let (sign, rest) = match trimmed.as_bytes().first() {
        Some(b'-') => (-1.0, &trimmed[1..]),
        Some(b'+') => (1.0, &trimmed[1..]),
        _ => (1.0, trimmed),
    };
    let mut rest = rest;
    if (radix == 0 || radix == 16)
        && let Some(stripped) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X"))
    {
        rest = stripped;
        radix = 16;
    }
    if radix == 0 {
        radix = 10;
    }
    if !(2..=36).contains(&radix) {
        return Ok(JsValue::Number(f64::NAN));
    }
    let digits: String = rest
        .chars()
        .take_while(|c| c.is_digit(radix as u32))
        .collect();
    if digits.is_empty() {
        return Ok(JsValue::Number(f64::NAN));
    }
    let value = if radix == 10 {
        digits.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        digits.chars().fold(0.0, |acc, c| {
            acc * f64::from(radix) + f64::from(c.to_digit(radix as u32).unwrap_or(0))
        })
    };
    Ok(JsValue::Number(sign * value))
}

/** parseFloat. */
fn parse_float(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?.lossy();
    let trimmed = text.trim_start_matches(|c: char| crate::v9::source::whitespace(c));
    for infinity in ["Infinity", "+Infinity", "-Infinity"] {
        if trimmed.starts_with(infinity) {
            return Ok(JsValue::Number(if infinity.starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }));
        }
    }
    let bytes = trimmed.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    let digits_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    if end == digits_start || trimmed[digits_start..end] == *"." {
        return Ok(JsValue::Number(f64::NAN));
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        if bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
            end = exponent;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
        }
    }
    Ok(JsValue::Number(trimmed[..end].parse().unwrap_or(f64::NAN)))
}

/** isNaN. */
fn is_nan(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(arg_number(vm, &call, 0)?.is_nan()))
}

/** isFinite. */
fn is_finite(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(arg_number(vm, &call, 0)?.is_finite()))
}

/** Characters encodeURIComponent leaves alone. */
const COMPONENT_SAFE: &str = "-_.!~*'()";
/** Characters encodeURI also leaves alone. */
const URI_SAFE: &str = "-_.!~*'();/?:@&=+$,#";
/** Characters decodeURI keeps escaped. */
const URI_RESERVED: &str = ";/?:@&=+$,#";

/** Percent-encode UTF-8 except unreserved characters. */
fn encode(vm: &mut Vm, call: Call, safe: &str) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?;
    let decoded = match String::from_utf16(text.as_units()) {
        Ok(decoded) => decoded,
        Err(_) => return Err(vm.throw(crate::v9::vm::error::ErrorKind::Uri, "URI malformed")),
    };
    let mut out = String::with_capacity(decoded.len());
    for c in decoded.chars() {
        if c.is_ascii_alphanumeric() || safe.contains(c) {
            out.push(c);
        } else {
            let mut buffer = [0u8; 4];
            for byte in c.encode_utf8(&mut buffer).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    super::text(vm, &out)
}

/** Percent-decode UTF-8, keeping escapes of `reserved` characters. */
fn decode(vm: &mut Vm, call: Call, reserved: &str) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?.lossy();
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok());
            match hex {
                Some(byte) if byte < 0x80 && reserved.contains(byte as char) => {
                    out.extend_from_slice(&bytes[index..index + 3]);
                }
                Some(byte) => out.push(byte),
                None => return Err(vm.throw(crate::v9::vm::error::ErrorKind::Uri, "URI malformed")),
            }
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    match String::from_utf8(out) {
        Ok(decoded) => super::text(vm, &decoded),
        Err(_) => Err(vm.throw(crate::v9::vm::error::ErrorKind::Uri, "URI malformed")),
    }
}

/** escape (legacy). */
fn escape(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?;
    let mut out = String::new();
    for unit in text.as_units() {
        let c = char::from_u32(u32::from(*unit)).unwrap_or('\u{fffd}');
        if c.is_ascii_alphanumeric() || "@*_+-./".contains(c) {
            out.push(c);
        } else if *unit < 0x100 {
            out.push_str(&format!("%{unit:02X}"));
        } else {
            out.push_str(&format!("%u{unit:04X}"));
        }
    }
    super::text(vm, &out)
}

/** unescape (legacy). */
fn unescape(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?;
    let units = text.as_units();
    let mut out = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        if units[index] == u16::from(b'%') {
            let read = |start: usize, count: usize| -> Option<u16> {
                let slice = units.get(start..start + count)?;
                u16::from_str_radix(&String::from_utf16(slice).ok()?, 16).ok()
            };
            if units.get(index + 1) == Some(&u16::from(b'u'))
                && let Some(value) = read(index + 2, 4)
            {
                out.push(value);
                index += 6;
                continue;
            }
            if let Some(value) = read(index + 1, 2) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(units[index]);
        index += 1;
    }
    Ok(JsValue::String(JsString::from_units(out)?))
}

/** Indirect eval: compile and run at the top level with V9's parser. */
fn eval(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::String(source) = call.arg(0) else {
        return Ok(call.arg(0));
    };
    super::function::evaluate(vm, source.lossy())
}
