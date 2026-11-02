/**
 * TextDecoder after the WHATWG Encoding spec and Blink's core/encoding (text_decoder.cc):
 * labels resolve through the spec's label table, `fatal` turns malformed input into a TypeError,
 * `ignoreBOM` keeps a leading byte order mark, and `{stream: true}` holds an incomplete
 * trailing sequence for the next call. It replaces the UTF-8-only builtin decoder.
 *
 * Every legacy encoding decodes through encoding_rs, the WHATWG tables, one chunk at a time.
 * ponytail: a legacy stream does not carry decoder state across calls, so a multibyte sequence
 * split between two chunks decodes as U+FFFD; upgrade by holding an encoding_rs::Decoder per object.
 */
use crate::v9::builtins::typed;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Vm};
use mars_core::charset::{self, Encoding};
use std::sync::OnceLock;

/** The decoders this file runs: its own UTF-8 and UTF-16 paths, and every other encoding through
encoding_rs, x-user-defined included. */
#[derive(Clone, Copy)]
enum Decoder {
    Utf8,
    Utf16Le,
    Utf16Be,
    Legacy(&'static Encoding),
}

impl Decoder {
    /** The canonical name the `encoding` attribute reports: lowercase, as the spec spells it. */
    fn name(self) -> String {
        match self {
            Self::Utf8 => "utf-8".into(),
            Self::Utf16Le => "utf-16le".into(),
            Self::Utf16Be => "utf-16be".into(),
            Self::Legacy(encoding) => encoding.name().to_ascii_lowercase(),
        }
    }
}

/** The TextDecoder prototype. */
static PROTO: OnceLock<u32> = OnceLock::new();

/** Install TextDecoder over the builtin one. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.host_method(proto, "decode", 0, decode);
    for name in ["encoding", "fatal", "ignoreBOM"] {
        b.getter(proto, name, attribute);
    }
    let ctor = b.constructor("TextDecoder", 0, construct, proto, None);
    b.global("TextDecoder", ctor);
    let _ = PROTO.set(proto);
}

/** The decoder a label names: ASCII whitespace trimmed, ASCII case ignored. UTF-16 labels that
encoding_rs maps to LE or BE keep the native UTF-16 path, which handles streamed surrogates. */
fn lookup(label: &str) -> Option<Decoder> {
    let label = label
        .trim_matches(|c| matches!(c, '\t' | '\n' | '\x0c' | '\r' | ' '))
        .to_ascii_lowercase();
    let encoding = charset::for_label(&label)?;
    // WHATWG's replacement encoding exists for labels only; TextDecoder refuses it.
    if encoding.name() == "replacement" {
        return None;
    }
    Some(match encoding.name() {
        "UTF-8" => Decoder::Utf8,
        "UTF-16LE" => Decoder::Utf16Le,
        "UTF-16BE" => Decoder::Utf16Be,
        _ => Decoder::Legacy(encoding),
    })
}

/** A private slot key. */
fn slot(name: &str) -> Key {
    Key::private(name)
}

/** Read a private slot. */
pub(super) fn read_slot(vm: &Vm, value: &JsValue, name: &str) -> Option<JsValue> {
    match &vm.heap.get(value.object()?)?.props.get(&slot(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}

/** Write a private slot. */
pub(super) fn write_slot(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
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
    let Some(decoder) = lookup(&label) else {
        let message = format!(
            "Failed to construct 'TextDecoder': The encoding label provided ('{label}') is invalid."
        );
        return Err(vm.range_error(&message));
    };
    let options = call.arg(1);
    let fatal = flag(vm, &options, "fatal")?;
    let ignore_bom = flag(vm, &options, "ignoreBOM")?;
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let name = JsString::from_utf8(&decoder.name())?;
    write_slot(vm, id, "decEncoding", JsValue::String(name));
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

/** The receiver's decoder, or the TypeError for a receiver that is no TextDecoder. */
fn decoder_of(vm: &mut Vm, this: &JsValue) -> Res<Decoder> {
    // The slot is engine-written, but a bad value must still be a TypeError, never a panic.
    let decoder = match read_slot(vm, this, "decEncoding") {
        Some(JsValue::String(name)) => lookup(&name.lossy()),
        _ => None,
    };
    decoder.ok_or_else(|| vm.type_error("Illegal invocation: the receiver is not a TextDecoder"))
}

/** `encoding`, `fatal` and `ignoreBOM`. */
fn attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    decoder_of(vm, &call.this)?;
    Ok(match super::dom::native_name(&call) {
        "encoding" => read_slot(vm, &call.this, "decEncoding").unwrap_or(JsValue::Undefined),
        "fatal" => read_slot(vm, &call.this, "decFatal").unwrap_or(JsValue::Boolean(false)),
        _ => read_slot(vm, &call.this, "decIgnoreBOM").unwrap_or(JsValue::Boolean(false)),
    })
}

/** `decode(input, {stream})`. */
fn decode(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let decoder = decoder_of(vm, &call.this)?;
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
    let (units, rest, failed) = run(decoder, &bytes, stream);
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
fn run(decoder: Decoder, bytes: &[u8], stream: bool) -> (Vec<u16>, Vec<u8>, bool) {
    match decoder {
        Decoder::Utf8 => utf8(bytes, stream),
        Decoder::Utf16Le => utf16(bytes, stream, u16::from_le_bytes),
        Decoder::Utf16Be => utf16(bytes, stream, u16::from_be_bytes),
        Decoder::Legacy(encoding) => legacy(encoding, bytes),
    }
}

/** One whole legacy chunk through its WHATWG table; malformed bytes become U+FFFD and report. */
fn legacy(encoding: &'static Encoding, bytes: &[u8]) -> (Vec<u16>, Vec<u8>, bool) {
    let (text, failed) = encoding.decode_without_bom_handling(bytes);
    (text.encode_utf16().collect(), Vec::new(), failed)
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
