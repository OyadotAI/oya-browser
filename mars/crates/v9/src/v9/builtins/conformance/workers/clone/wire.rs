/** Bounded V8 ValueSerializer wire records preserve real structured graphs without a delegate or foreign VM. */
use super::*;
use crate::protocol::EngineError;
mod reader;
mod writer;
/** Pinned V8 writes version 16; versions 13-16 share these explicit ordinary-value records. */
const VERSION: u32 = 16;
/** The graph's existing one-million-unit message bound also caps encoded native storage. */
const MAX_BYTES: usize = 2 * MESSAGE_UNITS;
/** Date records pass through the same ECMAScript TimeClip range as ordinary Date construction. */
const DATE_CLIP_MS: f64 = 8.64e15;
/** Invalid bytes and refused native delegates remain distinct from actual engine resource exhaustion. */
enum Failure {
    Invalid,
    Version,
    Refused,
    Limit,
}
type Result<T> = std::result::Result<T, Failure>;
/** Emit every argument into one genuine serializer stream, retaining graph identity across arguments. */
pub(in crate::v9::builtins::conformance) fn serialize(
    vm: &mut Vm,
    args: &[JsValue],
) -> Res<Vec<u8>> {
    let mut graph = Graph {
        root: Atom::Primitive(JsValue::Undefined),
        nodes: Vec::new(),
        allowed_detached: Vec::new(),
        worker: false,
    };
    let mut seen = BTreeMap::new();
    let mut units = 0;
    let mut roots = Vec::new();
    for value in args {
        roots.push(atom(vm, value, &mut graph, &mut seen, &mut units, 0)?);
    }
    writer::encode(&graph, &roots).map_err(|failure| thrown(vm, failure))
}
/** Parse and validate the entire first value before allocating any receiving JS objects or invoking setters. */
pub(in crate::v9::builtins::conformance) fn deserialize(vm: &mut Vm, bytes: &[u8]) -> Res<JsValue> {
    if bytes.len() > MAX_BYTES {
        return Err(thrown(vm, Failure::Limit));
    }
    vm.charge(bytes.len())?;
    let graph = reader::parse(bytes, vm.limits.objects, crate::constants::VM_NESTING_DEPTH)
        .map_err(|failure| thrown(vm, failure))?;
    decode(vm, graph)
}
/** D8's standalone native serializer has no capability delegate; malformed records throw its documented Error. */
fn thrown(vm: &mut Vm, failure: Failure) -> crate::v9::vm::Thrown {
    match failure {
        Failure::Limit => EngineError::limit("ValueSerializer bytes", MAX_BYTES).into(),
        Failure::Refused => refused(vm, false),
        Failure::Invalid | Failure::Version => match vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            if matches!(failure, Failure::Version) {
                "Unable to deserialize cloned data due to invalid or unsupported version."
            } else {
                "Unable to deserialize cloned data."
            },
        ) {
            Ok(id) => crate::v9::vm::Thrown::Value(JsValue::Object(id)),
            Err(error) => error.into(),
        },
    }
}
/** Typed view tags are the actual stable byte values from V8's ArrayBufferViewTag. */
fn view_tag(kind: ElementKind) -> u8 {
    match kind {
        ElementKind::Int8 => b'b',
        ElementKind::Uint8 => b'B',
        ElementKind::Uint8Clamped => b'C',
        ElementKind::Int16 => b'w',
        ElementKind::Uint16 => b'W',
        ElementKind::Int32 => b'd',
        ElementKind::Uint32 => b'D',
        ElementKind::Float16 => b'h',
        ElementKind::Float32 => b'f',
        ElementKind::Float64 => b'F',
        ElementKind::BigInt64 => b'q',
        ElementKind::BigUint64 => b'Q',
        ElementKind::DataView => b'?',
    }
}
/** The width controls both alignment and the byte-length/element-count conversion. */
fn width(kind: ElementKind) -> usize {
    match kind {
        ElementKind::Int8
        | ElementKind::Uint8
        | ElementKind::Uint8Clamped
        | ElementKind::DataView => 1,
        ElementKind::Int16 | ElementKind::Uint16 | ElementKind::Float16 => 2,
        ElementKind::Int32 | ElementKind::Uint32 | ElementKind::Float32 => 4,
        _ => 8,
    }
}
/** RegExp flag bits are pinned by regexp-flags.h; unknown/duplicate and mutually exclusive modes are rejected. */
fn regexp_flags(text: &str) -> Result<u32> {
    let mut flags = 0;
    for flag in text.chars() {
        let bit = match flag {
            'g' => 0,
            'i' => 1,
            'm' => 2,
            'y' => 3,
            'u' => 4,
            's' => 5,
            'l' => 6,
            'd' => 7,
            'v' => 8,
            _ => return Err(Failure::Invalid),
        };
        if flags & (1 << bit) != 0 {
            return Err(Failure::Invalid);
        }
        flags |= 1 << bit;
    }
    if flags & (1 << 4) != 0 && flags & (1 << 8) != 0 {
        return Err(Failure::Invalid);
    }
    Ok(flags)
}
