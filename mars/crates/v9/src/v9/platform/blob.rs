/**
 * Blob and File, after Blink's core/fileapi (blob.cc, file.cc) and the File API spec: a Blob is
 * an immutable byte sequence with a normalized MIME type, built from strings (UTF-8, with the
 * `endings` option), BufferSources and other Blobs. `slice`, `text`, `arrayBuffer` and `bytes`
 * read it back. The bytes live in a hidden Uint8Array on the wrapper.
 *
 * ponytail: no `stream()` and no blob: URLs; the data is in memory, never in a blob registry.
 */
use crate::v9::builtins::typed;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, Static, realm, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The Blob and File prototypes, set once while the realm is built. */
struct Ids {
    blob: u32,
    file: u32,
}

/** Set once at install. */
static IDS: OnceLock<Ids> = OnceLock::new();

/** The blob intrinsics. */
fn ids() -> &'static Ids {
    let _ = realm();
    IDS.get()
        .expect("blob intrinsics are installed with the realm")
}

/** Install Blob and File. */
pub(super) fn install(b: &mut Builder) {
    let blob = b.object(Some(b.ids.object_prototype));
    b.getter(blob, "size", size);
    b.getter(blob, "type", kind);
    b.host_method(blob, "slice", 0, slice);
    b.host_method(blob, "text", 0, read_text);
    b.host_method(blob, "arrayBuffer", 0, read_array_buffer);
    b.host_method(blob, "bytes", 0, read_bytes);
    tag(b, blob, "Blob");
    let blob_ctor = b.constructor("Blob", 0, construct_blob, blob, None);
    b.global("Blob", blob_ctor);
    let file = b.object(Some(blob));
    b.getter(file, "name", file_name);
    b.getter(file, "lastModified", last_modified);
    b.getter(file, "webkitRelativePath", relative_path);
    tag(b, file, "File");
    let file_ctor = b.constructor("File", 2, construct_file, file, Some(blob_ctor));
    b.global("File", file_ctor);
    let _ = IDS.set(Ids { blob, file });
}

/** Symbol.toStringTag on a prototype, so `String(blob)` is `[object Blob]`. */
fn tag(b: &mut Builder, target: u32, name: &'static str) {
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        CONFIGURABLE,
    );
}

/** A private slot key; scripts cannot name it. */
fn slot(name: &str) -> Key {
    Key::private(name)
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

/** A Blob's bytes, or None when the value is no Blob. */
pub(super) fn blob_bytes(vm: &Vm, value: &JsValue) -> Option<Vec<u8>> {
    typed::source_bytes(vm, &read_slot(vm, value, "blobBytes")?)
}

/** Whether the value is a File. */
pub(super) fn is_file(vm: &Vm, value: &JsValue) -> bool {
    read_slot(vm, value, "fileName").is_some()
}

/** A Blob's normalized type. */
fn type_of(vm: &Vm, value: &JsValue) -> String {
    match read_slot(vm, value, "blobType") {
        Some(JsValue::String(text)) => text.lossy(),
        _ => String::new(),
    }
}

/** A new Blob (or File, by prototype) over `bytes` with an already normalized type. */
fn new_blob(vm: &mut Vm, proto: ObjectId, bytes: Vec<u8>, kind: &str) -> Res<ObjectId> {
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let store = typed::new_uint8(vm, bytes)?;
    write_slot(vm, id, "blobBytes", JsValue::Object(store));
    write_slot(
        vm,
        id,
        "blobType",
        JsValue::String(JsString::from_utf8(kind)?),
    );
    Ok(id)
}

/** A File over `bytes`, as FormData makes when it wraps a Blob entry. */
pub(super) fn new_file(vm: &mut Vm, bytes: Vec<u8>, kind: &str, name: &str) -> Res<ObjectId> {
    let id = new_blob(vm, ids().file, bytes, kind)?;
    write_slot(
        vm,
        id,
        "fileName",
        JsValue::String(JsString::from_utf8(name)?),
    );
    let now = now_ms(vm);
    write_slot(vm, id, "fileModified", JsValue::Number(now));
    Ok(id)
}

/** Blink's Blob::NormalizeType: a type with any character outside U+0020..U+007E is the empty
string, anything else is ASCII-lowercased. */
fn normalize_type(text: &str) -> String {
    if text.chars().any(|c| !(' '..='~').contains(&c)) {
        return String::new();
    }
    text.to_ascii_lowercase()
}

/** Line endings to the platform's native `\n`, as BlobPropertyBag's `native` endings ask. */
fn native_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/** The BlobPropertyBag members in WebIDL dictionary order (`endings`, then `type`). */
fn property_bag(vm: &mut Vm, options: &JsValue) -> Res<(bool, String)> {
    match options {
        JsValue::Undefined | JsValue::Null => return Ok((false, String::new())),
        JsValue::Object(_) => {}
        _ => {
            return Err(vm.type_error(
                "Failed to construct 'Blob': The provided value is not of type 'BlobPropertyBag'.",
            ));
        }
    }
    let endings = vm.get(options, &Key::from("endings"))?;
    let native = match endings {
        JsValue::Undefined => false,
        value => match vm.to_text(&value)?.as_str() {
            "transparent" => false,
            "native" => true,
            other => {
                let message = format!(
                    "Failed to construct 'Blob': The provided value '{other}' is not a valid enum value of type EndingType."
                );
                return Err(vm.type_error(&message));
            }
        },
    };
    let kind = match vm.get(options, &Key::from("type"))? {
        JsValue::Undefined => String::new(),
        value => normalize_type(&vm.to_text(&value)?),
    };
    Ok((native, kind))
}

/** The bytes of a `sequence<BlobPart>`: each element a Blob, a BufferSource or a USVString,
converted in iteration order. */
fn parts_bytes(vm: &mut Vm, parts: &JsValue, options: &JsValue) -> Res<Vec<u8>> {
    let items =
        match parts {
            JsValue::Undefined => Vec::new(),
            JsValue::Object(_) => vm.iterate_to_vec(parts)?,
            _ => return Err(vm.type_error(
                "Failed to construct 'Blob': The provided value cannot be converted to a sequence.",
            )),
        };
    let mut converted = Vec::with_capacity(items.len());
    for item in &items {
        converted.push(part(vm, item)?);
    }
    let (native, _) = property_bag(vm, options)?;
    let mut out = Vec::new();
    for item in converted {
        match item {
            Ok(bytes) => out.extend(bytes),
            Err(text) if native => out.extend(native_endings(&text).into_bytes()),
            Err(text) => out.extend(text.into_bytes()),
        }
    }
    Ok(out)
}

/** One BlobPart: bytes for a Blob or BufferSource, text (still subject to `endings`) for a
string. Lone surrogates become U+FFFD, as USVString conversion says. */
fn part(vm: &mut Vm, item: &JsValue) -> Res<Result<Vec<u8>, String>> {
    if let Some(bytes) = blob_bytes(vm, item).or_else(|| typed::source_bytes(vm, item)) {
        return Ok(Ok(bytes));
    }
    Ok(Err(vm.to_string(item)?.lossy()))
}

/** The prototype a construction uses, or the TypeError for a call without `new`. */
fn construction(vm: &mut Vm, call: &Call, name: &str, fallback: ObjectId) -> Res<ObjectId> {
    let Some(target) = call.new_target.clone() else {
        let message = format!(
            "Failed to construct '{name}': Please use the 'new' operator, this DOM object constructor cannot be called as a function."
        );
        return Err(vm.type_error(&message));
    };
    vm.prototype_for(&target, fallback)
}

/** `new Blob(blobParts, options)`. */
fn construct_blob(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let proto = construction(vm, &call, "Blob", ids().blob)?;
    let options = call.arg(1);
    let bytes = parts_bytes(vm, &call.arg(0), &options)?;
    let (_, kind) = property_bag(vm, &options)?;
    Ok(JsValue::Object(new_blob(vm, proto, bytes, &kind)?))
}

/** `new File(fileBits, fileName, options)`: a Blob with a name and a modification time. */
fn construct_file(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let proto = construction(vm, &call, "File", ids().file)?;
    if call.args.len() < 2 {
        let message = format!(
            "Failed to construct 'File': 2 arguments required, but only {} present.",
            call.args.len()
        );
        return Err(vm.type_error(&message));
    }
    let options = call.arg(2);
    let bytes = parts_bytes(vm, &call.arg(0), &options)?;
    let name = vm.to_string(&call.arg(1))?.lossy();
    let (_, kind) = property_bag(vm, &options)?;
    let modified = match &options {
        JsValue::Object(_) => vm.get(&options, &Key::from("lastModified"))?,
        _ => JsValue::Undefined,
    };
    let modified = match modified {
        JsValue::Undefined => now_ms(vm),
        value => long_long(vm.to_number(&value)?),
    };
    let id = new_blob(vm, proto, bytes, &kind)?;
    write_slot(
        vm,
        id,
        "fileName",
        JsValue::String(JsString::from_utf8(&name)?),
    );
    write_slot(vm, id, "fileModified", JsValue::Number(modified));
    Ok(JsValue::Object(id))
}

/** The page's virtual time in milliseconds, the default `lastModified`. */
fn now_ms(vm: &Vm) -> f64 {
    (vm.page.state.epoch + vm.page.state.clock).floor()
}

/** WebIDL `long long` conversion: NaN and infinities are 0, everything else truncates. */
fn long_long(number: f64) -> f64 {
    if number.is_finite() {
        number.trunc() + 0.0
    } else {
        0.0
    }
}

/** The receiver's bytes, or the TypeError for a receiver that is no Blob. */
fn this_bytes(vm: &mut Vm, call: &Call, member: &str) -> Res<Vec<u8>> {
    blob_bytes(vm, &call.this).ok_or_else(|| {
        vm.type_error(&format!(
            "Failed to execute '{member}' on 'Blob': Illegal invocation"
        ))
    })
}

/** `blob.size`. */
fn size(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Number(this_bytes(vm, &call, "size")?.len() as f64))
}

/** `blob.type`. */
fn kind(vm: &mut Vm, call: Call) -> Res<JsValue> {
    this_bytes(vm, &call, "type")?;
    let kind = type_of(vm, &call.this);
    Ok(JsValue::String(JsString::from_utf8(&kind)?))
}

/** A relative slice offset clamped into `0..=size`, as Blink's ClampSliceOffsets does. */
fn clamp(vm: &mut Vm, value: &JsValue, size: f64, default: f64) -> Res<f64> {
    let offset = match value {
        JsValue::Undefined => return Ok(default),
        value => long_long(vm.to_number(value)?),
    };
    Ok(if offset < 0.0 {
        (size + offset).max(0.0)
    } else {
        offset.min(size)
    })
}

/** `blob.slice(start, end, contentType)`: a new Blob over the clamped range. */
fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = this_bytes(vm, &call, "slice")?;
    let size = bytes.len() as f64;
    let start = clamp(vm, &call.arg(0), size, 0.0)?;
    let end = clamp(vm, &call.arg(1), size, size)?.max(start);
    let kind = match call.arg(2) {
        JsValue::Undefined => String::new(),
        value => normalize_type(&vm.to_text(&value)?),
    };
    let range = bytes[start as usize..end as usize].to_vec();
    Ok(JsValue::Object(new_blob(vm, ids().blob, range, &kind)?))
}

/** A promise already fulfilled with `value`. */
fn fulfilled(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    Ok(JsValue::Object(vm.promise_resolve(value)?))
}

/** `blob.text()`: the bytes decoded as UTF-8, a BOM dropped and malformed input U+FFFD. */
fn read_text(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = this_bytes(vm, &call, "text")?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let value = JsValue::String(JsString::from_utf8(text)?);
    fulfilled(vm, value)
}

/** `blob.arrayBuffer()`: a fresh ArrayBuffer with a copy of the bytes. */
fn read_array_buffer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = this_bytes(vm, &call, "arrayBuffer")?;
    let view = typed::new_uint8(vm, bytes)?;
    let buffer = vm.get(&JsValue::Object(view), &Key::from("buffer"))?;
    fulfilled(vm, buffer)
}

/** `blob.bytes()`: a fresh Uint8Array with a copy of the bytes. */
fn read_bytes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = this_bytes(vm, &call, "bytes")?;
    let view = typed::new_uint8(vm, bytes)?;
    fulfilled(vm, JsValue::Object(view))
}

/** The receiver's File slot, or the TypeError for a receiver that is no File. */
fn file_slot(vm: &mut Vm, call: &Call, name: &str) -> Res<JsValue> {
    read_slot(vm, &call.this, name)
        .ok_or_else(|| vm.type_error("Failed to read a File member: Illegal invocation"))
}

/** `file.name`. */
fn file_name(vm: &mut Vm, call: Call) -> Res<JsValue> {
    file_slot(vm, &call, "fileName")
}

/** `file.lastModified`. */
fn last_modified(vm: &mut Vm, call: Call) -> Res<JsValue> {
    file_slot(vm, &call, "fileModified")
}

/** `file.webkitRelativePath`: always empty, since no file comes from a directory picker. */
fn relative_path(vm: &mut Vm, call: Call) -> Res<JsValue> {
    file_slot(vm, &call, "fileName")?;
    Ok(string(""))
}
