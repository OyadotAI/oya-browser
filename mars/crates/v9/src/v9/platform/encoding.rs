/**
 * TextDecoder after the WHATWG Encoding spec and Blink's core/encoding (text_decoder.cc):
 * labels resolve through the spec's label table, `fatal` turns malformed input into a TypeError,
 * `ignoreBOM` keeps a leading byte order mark, and `{stream: true}` holds an incomplete
 * trailing sequence for the next call. It replaces the UTF-8-only builtin decoder.
 *
 * ponytail: UTF-8, UTF-16LE/BE, windows-1252 (with its latin1 and ascii labels) and
 * x-user-defined only; the other legacy encodings need their index tables and stay RangeErrors.
 */
use crate::v9::builtins::typed;
use crate::v9::compile::PRIVATE_MARK;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The decoders this file implements. */
#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Windows1252,
    UserDefined,
}

/** The canonical names, in `Encoding` order, as the `encoding` attribute reports them. */
const NAMES: [&str; 5] = [
    "utf-8",
    "utf-16le",
    "utf-16be",
    "windows-1252",
    "x-user-defined",
];

/** The spec's label table, for the encodings above. */
const LABELS: &[(&str, Encoding)] = &[
    ("unicode-1-1-utf-8", Encoding::Utf8),
    ("unicode11utf8", Encoding::Utf8),
    ("unicode20utf8", Encoding::Utf8),
    ("utf-8", Encoding::Utf8),
    ("utf8", Encoding::Utf8),
    ("x-unicode20utf8", Encoding::Utf8),
    ("csunicode", Encoding::Utf16Le),
    ("iso-10646-ucs-2", Encoding::Utf16Le),
    ("ucs-2", Encoding::Utf16Le),
    ("unicode", Encoding::Utf16Le),
    ("unicodefeff", Encoding::Utf16Le),
    ("utf-16", Encoding::Utf16Le),
    ("utf-16le", Encoding::Utf16Le),
    ("unicodefffe", Encoding::Utf16Be),
    ("utf-16be", Encoding::Utf16Be),
    ("ansi_x3.4-1968", Encoding::Windows1252),
    ("ascii", Encoding::Windows1252),
    ("cp1252", Encoding::Windows1252),
    ("cp819", Encoding::Windows1252),
    ("csisolatin1", Encoding::Windows1252),
    ("ibm819", Encoding::Windows1252),
    ("iso-8859-1", Encoding::Windows1252),
    ("iso-ir-100", Encoding::Windows1252),
    ("iso8859-1", Encoding::Windows1252),
    ("iso88591", Encoding::Windows1252),
    ("iso_8859-1", Encoding::Windows1252),
    ("iso_8859-1:1987", Encoding::Windows1252),
    ("l1", Encoding::Windows1252),
    ("latin1", Encoding::Windows1252),
    ("us-ascii", Encoding::Windows1252),
    ("windows-1252", Encoding::Windows1252),
    ("x-cp1252", Encoding::Windows1252),
    ("x-user-defined", Encoding::UserDefined),
];

/** windows-1252's code points for bytes 0x80..=0x9F; the rest map to themselves. */
const WINDOWS_1252_HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x008D, 0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178,
];

/** The TextDecoder prototype. */
static PROTO: OnceLock<u32> = OnceLock::new();

/** Install TextDecoder over the builtin one. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(proto, "decode", 0, decode);
    for name in ["encoding", "fatal", "ignoreBOM"] {
        b.getter(proto, name, attribute);
    }
    let ctor = b.constructor("TextDecoder", 0, construct, proto, None);
    b.global("TextDecoder", ctor);
    let _ = PROTO.set(proto);
}

/** The encoding a label names: ASCII whitespace trimmed, ASCII case ignored. */
fn lookup(label: &str) -> Option<Encoding> {
    let label = label
        .trim_matches(|c| matches!(c, '\t' | '\n' | '\x0c' | '\r' | ' '))
        .to_ascii_lowercase();
    LABELS
        .iter()
        .find(|(name, _)| *name == label)
        .map(|(_, encoding)| *encoding)
}

/** A private slot key. */
fn slot(name: &str) -> Key {
    let mut units = vec![PRIVATE_MARK];
    units.extend(name.encode_utf16());
    Key::Str(JsString::from_units(units).expect("short key"))
}

/** Read a private slot. */
fn read_slot(vm: &Vm, value: &JsValue, name: &str) -> Option<JsValue> {
    match &vm.heap.get(value.object()?)?.props.get(&slot(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}

/** Write a private slot. */
fn write_slot(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    let prop = Prop {
        slot: Slot::Data(value),
        attrs: HIDDEN_ATTRS,
    };
    vm.heap.put(id, slot(name), prop);
}

/** A boolean dictionary member. */
fn flag(vm: &mut Vm, options: &JsValue, name: &str) -> Res<bool> {
    if !matches!(options, JsValue::Object(_)) {
        return Ok(false);
    }
    Ok(vm.get(options, &Key::from(name))?.truthy())
}

/** `new TextDecoder(label, {fatal, ignoreBOM})`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Failed to construct 'TextDecoder': Please use the 'new' operator, this DOM object constructor cannot be called as a function."));
    };
    let _ = realm();
    let proto = vm.prototype_for(&target, *PROTO.get().expect("installed"))?;
    let label = match call.arg(0) {
        JsValue::Undefined => String::from("utf-8"),
        value => vm.to_text(&value)?,
    };
    let Some(encoding) = lookup(&label) else {
        let message = format!(
            "Failed to construct 'TextDecoder': The encoding label provided ('{label}') is invalid."
        );
        return Err(vm.range_error(&message));
    };
    let options = call.arg(1);
    let fatal = flag(vm, &options, "fatal")?;
    let ignore_bom = flag(vm, &options, "ignoreBOM")?;
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    write_slot(
        vm,
        id,
        "decEncoding",
        JsValue::Number(encoding as u8 as f64),
    );
    write_slot(vm, id, "decFatal", JsValue::Boolean(fatal));
    write_slot(vm, id, "decIgnoreBOM", JsValue::Boolean(ignore_bom));
    reset(vm, id)?;
    Ok(JsValue::Object(id))
}

/** Clear the stream state: no pending bytes and the BOM not yet seen. */
fn reset(vm: &mut Vm, id: ObjectId) -> Res<()> {
    let pending = typed::new_uint8(vm, Vec::new())?;
    write_slot(vm, id, "decPending", JsValue::Object(pending));
    write_slot(vm, id, "decBomSeen", JsValue::Boolean(false));
    Ok(())
}

/** The receiver's encoding, or the TypeError for a receiver that is no TextDecoder. */
fn encoding_of(vm: &mut Vm, this: &JsValue) -> Res<Encoding> {
    let all = [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        Encoding::Windows1252,
        Encoding::UserDefined,
    ];
    match read_slot(vm, this, "decEncoding") {
        Some(JsValue::Number(index)) => Ok(all[index as usize]),
        _ => Err(vm.type_error("Illegal invocation: the receiver is not a TextDecoder")),
    }
}

/** `encoding`, `fatal` and `ignoreBOM`. */
fn attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let encoding = encoding_of(vm, &call.this)?;
    Ok(match super::dom::native_name(&call) {
        "encoding" => JsValue::String(JsString::from_utf8(NAMES[encoding as usize])?),
        "fatal" => read_slot(vm, &call.this, "decFatal").unwrap_or(JsValue::Boolean(false)),
        _ => read_slot(vm, &call.this, "decIgnoreBOM").unwrap_or(JsValue::Boolean(false)),
    })
}

/** `decode(input, {stream})`. */
fn decode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let encoding = encoding_of(vm, &call.this)?;
    let id = call.this.object().expect("checked receiver");
    let input = match call.arg(0) {
        JsValue::Undefined => Vec::new(),
        source => typed::source_bytes(vm, &source).ok_or_else(|| {
            vm.type_error("Failed to execute 'decode' on 'TextDecoder': The provided value is not of type '(ArrayBuffer or ArrayBufferView)'.")
        })?,
    };
    let stream = flag(vm, &call.arg(1), "stream")?;
    let pending = read_slot(vm, &call.this, "decPending");
    let mut bytes = pending
        .and_then(|p| typed::source_bytes(vm, &p))
        .unwrap_or_default();
    bytes.extend(input);
    let fatal = read_slot(vm, &call.this, "decFatal").is_some_and(|v| v.truthy());
    let (units, rest, failed) = run(encoding, &bytes, stream);
    if failed && fatal {
        reset(vm, id)?;
        return Err(vm.type_error(
            "Failed to execute 'decode' on 'TextDecoder': The encoded data was not valid.",
        ));
    }
    let units = strip_bom(vm, id, units)?;
    if stream {
        let held = typed::new_uint8(vm, rest)?;
        write_slot(vm, id, "decPending", JsValue::Object(held));
    } else {
        reset(vm, id)?;
    }
    Ok(JsValue::String(JsString::from_units(units)?))
}

/** Drop a leading U+FEFF once per stream, unless `ignoreBOM` asks to keep it. */
fn strip_bom(vm: &mut Vm, id: ObjectId, mut units: Vec<u16>) -> Res<Vec<u16>> {
    let this = JsValue::Object(id);
    let seen = read_slot(vm, &this, "decBomSeen").is_some_and(|v| v.truthy());
    if seen || units.is_empty() {
        return Ok(units);
    }
    let ignore = read_slot(vm, &this, "decIgnoreBOM").is_some_and(|v| v.truthy());
    if !ignore && units[0] == 0xFEFF {
        units.remove(0);
    }
    write_slot(vm, id, "decBomSeen", JsValue::Boolean(true));
    Ok(units)
}

/** Decode `bytes`: the UTF-16 units, the incomplete tail held for a streaming call, and whether
any input was malformed. */
fn run(encoding: Encoding, bytes: &[u8], stream: bool) -> (Vec<u16>, Vec<u8>, bool) {
    match encoding {
        Encoding::Utf8 => utf8(bytes, stream),
        Encoding::Utf16Le => utf16(bytes, stream, u16::from_le_bytes),
        Encoding::Utf16Be => utf16(bytes, stream, u16::from_be_bytes),
        Encoding::Windows1252 => (
            bytes.iter().map(|&b| windows_1252(b)).collect(),
            Vec::new(),
            false,
        ),
        Encoding::UserDefined => (
            bytes.iter().map(|&b| user_defined(b)).collect(),
            Vec::new(),
            false,
        ),
    }
}

/** One windows-1252 byte. */
fn windows_1252(byte: u8) -> u16 {
    match byte {
        0x80..=0x9F => WINDOWS_1252_HIGH[usize::from(byte - 0x80)],
        _ => u16::from(byte),
    }
}

/** One x-user-defined byte: ASCII as is, the high half in U+F780..U+F7FF. */
fn user_defined(byte: u8) -> u16 {
    if byte < 0x80 {
        u16::from(byte)
    } else {
        0xF780 + u16::from(byte - 0x80)
    }
}

/** UTF-8 with U+FFFD for each maximal malformed subpart, as the spec's decoder does. */
fn utf8(bytes: &[u8], stream: bool) -> (Vec<u16>, Vec<u8>, bool) {
    let (mut out, mut rest, mut failed) = (String::new(), bytes, false);
    loop {
        match std::str::from_utf8(rest) {
            Ok(text) => {
                out.push_str(text);
                return (out.encode_utf16().collect(), Vec::new(), failed);
            }
            Err(error) => {
                let (valid, after) = rest.split_at(error.valid_up_to());
                out.push_str(std::str::from_utf8(valid).unwrap_or_default());
                let Some(bad) = error.error_len() else {
                    if stream {
                        return (out.encode_utf16().collect(), after.to_vec(), failed);
                    }
                    out.push('\u{FFFD}');
                    return (out.encode_utf16().collect(), Vec::new(), true);
                };
                out.push('\u{FFFD}');
                failed = true;
                rest = &after[bad..];
            }
        }
    }
}

/** UTF-16 in either byte order: lone surrogates become U+FFFD; an odd byte or a trailing high
surrogate waits for more input when streaming. */
fn utf16(bytes: &[u8], stream: bool, unit: fn([u8; 2]) -> u16) -> (Vec<u16>, Vec<u8>, bool) {
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| unit([pair[0], pair[1]]))
        .collect();
    let mut rest = bytes[bytes.len() - bytes.len() % 2..].to_vec();
    if stream && units.last().is_some_and(|u| (0xD800..0xDC00).contains(u)) {
        let held = units.pop().expect("checked last").to_le_bytes();
        let held = if unit([1, 0]) == 1 {
            held
        } else {
            [held[1], held[0]]
        };
        rest.splice(0..0, held);
    }
    let mut out = Vec::with_capacity(units.len() + 1);
    let mut failed = false;
    for item in char::decode_utf16(units) {
        match item {
            Ok(c) => out.extend(c.encode_utf16(&mut [0; 2]).iter()),
            Err(_) => {
                out.push(0xFFFD);
                failed = true;
            }
        }
    }
    if !stream && !rest.is_empty() {
        out.push(0xFFFD);
        return (out, Vec::new(), true);
    }
    (out, rest, failed)
}
