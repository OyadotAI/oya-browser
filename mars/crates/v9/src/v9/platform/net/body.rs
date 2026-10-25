/**
 * The Fetch standard's "extract a body" and "consume body", as Blink's BodyStreamBuffer,
 * FetchRequestData and FormDataEncoder apply them: a request or response body is a string,
 * URLSearchParams, BufferSource, Blob or FormData (multipart), each with the Content-Type it
 * implies, and any other object is its string conversion. A body is read back as text, JSON,
 * an ArrayBuffer, a Uint8Array, a Blob typed by the Content-Type, or (urlencoded) FormData.
 *
 * ponytail: a ReadableStream body stays unsupported (V9 has no byte streams), and formData()
 * reads back a multipart part's name and filename without undoing the %22 escapes.
 */
use crate::v9::value::{JsString, JsValue};
use crate::v9::vm::data::HostData;
use crate::v9::vm::heap::{Key, Slot};
use crate::v9::vm::{Res, Vm};

/** A body: text, or bytes when it is not UTF-8, plus the Content-Type extraction implies. */
#[derive(Default)]
pub(in crate::v9) struct Body {
    pub text: Option<String>,
    pub bytes: Vec<u8>,
    /** Fetch's extracted type, None for a BufferSource, a type-less Blob or a plain object. */
    pub kind: Option<String>,
}

/** The type a string body gets: Fetch's "text/plain;charset=UTF-8". */
pub(in crate::v9) const TEXT_TYPE: &str = "text/plain;charset=UTF-8";
/** The type a URLSearchParams body gets. */
const FORM_TYPE: &str = "application/x-www-form-urlencoded;charset=UTF-8";
/** Blink's multipart boundary prefix (FormDataEncoder::GenerateUniqueBoundaryString). */
const BOUNDARY_PREFIX: &str = "----WebKitFormBoundary";
/** Random alphanumerics after the prefix, as Blink makes. */
const BOUNDARY_CHARS: usize = 16;
/** Blink's boundary alphabet: 64 letters and digits, so six bits pick one. */
const BOUNDARY_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789AB";
/** SplitMix64 increment, mixing the page seed with the request counter for the boundary. */
const MIX_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
/** SplitMix64's two output multipliers. */
const MIX_A: u64 = 0xbf58_476d_1ce4_e5b9;
/** See MIX_A. */
const MIX_B: u64 = 0x94d0_49bb_1331_11eb;
/** Bits per boundary character. */
const BOUNDARY_BITS: u32 = 6;

impl Body {
    /** Text when the bytes are UTF-8 (so the common case stays a text body), else bytes. */
    fn of(bytes: Vec<u8>, kind: Option<String>) -> Self {
        match String::from_utf8(bytes) {
            Ok(text) => Body {
                text: Some(text),
                bytes: Vec::new(),
                kind,
            },
            Err(binary) => Body {
                text: None,
                bytes: binary.into_bytes(),
                kind,
            },
        }
    }
}

/** Push the extracted Content-Type unless the author set one (Fetch's Request constructor
step 36, XMLHttpRequest send() step 4, Response's "initialize a response"). */
pub(in crate::v9) fn label(headers: &mut Vec<(String, String)>, kind: Option<&str>) {
    if let Some(kind) = kind
        && !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        headers.push(("content-type".into(), kind.into()));
    }
}

/** Fetch's "extract a body" over a BodyInit value. */
pub(in crate::v9) fn extract(vm: &mut Vm, value: &JsValue) -> Res<Body> {
    if value.nullish() {
        return Ok(Body::default());
    }
    if let Some(HostData::SearchParams(pairs)) = super::super::host_data(vm, value) {
        let text = super::encode_pairs(&pairs);
        return Ok(Body::of(text.into_bytes(), Some(FORM_TYPE.into())));
    }
    if let Some(bytes) = crate::v9::builtins::typed::source_bytes(vm, value) {
        return Ok(Body::of(bytes, None));
    }
    if let Some(bytes) = super::super::blob::blob_bytes(vm, value) {
        let kind = Some(blob_type(vm, value)).filter(|kind| !kind.is_empty());
        return Ok(Body::of(bytes, kind));
    }
    if let Some(entries) = form_entries(vm, value)? {
        return multipart(vm, entries);
    }
    if is_stream(vm, value)? {
        return Err(vm.unsupported_error("ReadableStream request and response bodies"));
    }
    let text = vm.to_string(value)?.lossy();
    let kind = matches!(value, JsValue::String(_)).then(|| TEXT_TYPE.to_owned());
    Ok(Body::of(text.into_bytes(), kind))
}

/** A private slot's value on an object. */
fn private(vm: &Vm, value: &JsValue, name: &str) -> Option<JsValue> {
    match &vm
        .heap
        .get(value.object()?)?
        .props
        .get(&Key::private(name))?
        .slot
    {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}

/** A Blob's normalized type, from the slot blob.rs keeps it in. */
fn blob_type(vm: &Vm, value: &JsValue) -> String {
    match private(vm, value, "blobType") {
        Some(JsValue::String(text)) => text.lossy(),
        _ => String::new(),
    }
}

/** A FormData's entries, from the hidden list form_data.rs keeps; None for anything else. */
fn form_entries(vm: &mut Vm, value: &JsValue) -> Res<Option<Vec<(String, JsValue)>>> {
    let Some(list) = private(vm, value, "formEntries") else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for pair in vm.iterate_to_vec(&list)? {
        let name = vm.get(&pair, &Key::from("0"))?;
        let entry = vm.get(&pair, &Key::from("1"))?;
        out.push((vm.to_string(&name)?.lossy(), entry));
    }
    Ok(Some(out))
}

/** Whether the value is a ReadableStream. */
fn is_stream(vm: &mut Vm, value: &JsValue) -> Res<bool> {
    if !matches!(value, JsValue::Object(_)) {
        return Ok(false);
    }
    let ctor = vm.global_get(&JsString::from_utf8("ReadableStream")?, false)?;
    if !matches!(ctor, JsValue::Object(_)) {
        return Ok(false);
    }
    vm.ordinary_has_instance(&ctor, value)
}

/** A fresh boundary: Blink's prefix and 16 letters or digits, drawn from the page seed and
the request counter so two bodies on one page differ, without moving Math.random's state. */
fn boundary(vm: &Vm) -> String {
    let mut state = vm.page.state.random ^ u64::from(vm.page.state.next_request);
    let tail: String = (0..BOUNDARY_CHARS)
        .map(|_| {
            state = state.wrapping_add(MIX_GAMMA);
            let mut z = (state ^ (state >> 30)).wrapping_mul(MIX_A);
            z = (z ^ (z >> 27)).wrapping_mul(MIX_B);
            BOUNDARY_ALPHABET[((z ^ (z >> 31)) >> (u64::BITS - BOUNDARY_BITS)) as usize] as char
        })
        .collect();
    format!("{BOUNDARY_PREFIX}{tail}")
}

/** The HTML standard's multipart/form-data encoding: names and filenames escape `"`, CR and
LF as %22, %0D and %0A, string values normalize line breaks to CRLF, and a File part carries
its filename and type (application/octet-stream when it has none). */
fn multipart(vm: &mut Vm, entries: Vec<(String, JsValue)>) -> Res<Body> {
    let boundary = boundary(vm);
    let mut out = Vec::new();
    for (name, value) in entries {
        out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        let disposition = format!("Content-Disposition: form-data; name=\"{}\"", escape(&name));
        out.extend_from_slice(disposition.as_bytes());
        if let Some(bytes) = super::super::blob::blob_bytes(vm, &value) {
            let file = match private(vm, &value, "fileName") {
                Some(JsValue::String(text)) => text.lossy(),
                _ => "blob".into(),
            };
            let kind = Some(blob_type(vm, &value))
                .filter(|kind| !kind.is_empty())
                .unwrap_or_else(|| "application/octet-stream".into());
            let head = format!(
                "; filename=\"{}\"\r\nContent-Type: {kind}\r\n\r\n",
                escape(&file)
            );
            out.extend_from_slice(head.as_bytes());
            out.extend_from_slice(&bytes);
        } else {
            let text = vm.to_string(&value)?.lossy();
            out.extend_from_slice(b"\r\n\r\n");
            out.extend_from_slice(crlf(&text).as_bytes());
        }
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let kind = format!("multipart/form-data; boundary={boundary}");
    Ok(Body::of(out, Some(kind)))
}

/** Escape a multipart name or filename. */
fn escape(text: &str) -> String {
    text.replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/** Normalize every lone CR, lone LF and CRLF to CRLF. */
fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

/** Fetch's "consume body" for the readers JSON and text do not cover: `arrayBuffer`,
`bytes`, `blob` (typed by the Content-Type, lowercased as Blob normalizes it) and `formData`
(an application/x-www-form-urlencoded or multipart/form-data body; anything else is the spec's TypeError). None for
any other reader name. */
pub(in crate::v9) fn consume(
    vm: &mut Vm,
    name: &str,
    bytes: Vec<u8>,
    headers: &[(String, String)],
) -> Option<Res<JsValue>> {
    let kind = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    Some(match name {
        "arrayBuffer" => array_buffer(vm, bytes),
        "bytes" => crate::v9::builtins::typed::new_uint8(vm, bytes).map(JsValue::Object),
        "blob" => new_blob(vm, bytes, &kind.to_ascii_lowercase()),
        "formData" => form_data(vm, &bytes, &kind),
        _ => return None,
    })
}

/** A new ArrayBuffer holding `bytes`. */
fn array_buffer(vm: &mut Vm, bytes: Vec<u8>) -> Res<JsValue> {
    let view = crate::v9::builtins::typed::new_uint8(vm, bytes)?;
    vm.get(&JsValue::Object(view), &Key::from("buffer"))
}

/** `new Blob([bytes], {type})`, through the page's own Blob constructor. */
fn new_blob(vm: &mut Vm, bytes: Vec<u8>, kind: &str) -> Res<JsValue> {
    let view = crate::v9::builtins::typed::new_uint8(vm, bytes)?;
    let parts = vm.new_array(vec![JsValue::Object(view)])?;
    let options = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(crate::v9::vm::realm::realm().ids.object_prototype),
    ))?;
    vm.put(options, "type", JsValue::string(kind)?)?;
    let ctor = vm.global_get(&JsString::from_utf8("Blob")?, false)?;
    vm.construct(
        ctor,
        vec![JsValue::Object(parts), JsValue::Object(options)],
        None,
    )
}

/** A FormData over a urlencoded or multipart body's entries. */
fn form_data(vm: &mut Vm, bytes: &[u8], kind: &str) -> Res<JsValue> {
    let essence = kind.split(';').next().unwrap_or_default().trim();
    let parts = if essence.eq_ignore_ascii_case("application/x-www-form-urlencoded") {
        Some(
            url::form_urlencoded::parse(bytes)
                .map(|(name, value)| Part::text(&name, value.as_bytes()))
                .collect(),
        )
    } else {
        multipart_boundary(kind).and_then(|boundary| parse_multipart(bytes, &boundary))
    };
    let Some(parts) = parts else {
        return Err(vm.type_error("Failed to fetch: the body is not a form this browser can parse"));
    };
    let ctor = vm.global_get(&JsString::from_utf8("FormData")?, false)?;
    let form = vm.construct(ctor, Vec::new(), None)?;
    let append = vm.get(&form, &Key::from("append"))?;
    for part in parts {
        let mut args = vec![JsValue::string(&part.name)?];
        match part.file {
            Some(file) => {
                args.push(new_blob(vm, part.value, &part.kind)?);
                args.push(JsValue::string(&file)?);
            }
            None => args.push(JsValue::string(&String::from_utf8_lossy(&part.value))?),
        }
        vm.call(append.clone(), form.clone(), args)?;
    }
    Ok(form)
}

/** One form entry read back from a body. */
struct Part {
    name: String,
    value: Vec<u8>,
    /** The filename, for a file part. */
    file: Option<String>,
    kind: String,
}

impl Part {
    /** A text entry. */
    fn text(name: &str, value: &[u8]) -> Self {
        Part {
            name: name.into(),
            value: value.to_vec(),
            file: None,
            kind: String::new(),
        }
    }
}

/** The boundary parameter of a multipart/form-data type, unquoted. */
fn multipart_boundary(kind: &str) -> Option<String> {
    let (essence, params) = kind.split_once(';')?;
    if !essence.trim().eq_ignore_ascii_case("multipart/form-data") {
        return None;
    }
    params.split(';').find_map(|param| {
        let (key, value) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("boundary")
            .then(|| value.trim().trim_matches('"').to_owned())
    })
}

/** Parse a multipart/form-data body (RFC 7578): parts between `--boundary` delimiters, each
with headers, a blank line and its content. Any malformed part fails the whole body, as
Blink's FormDataBytesConsumer does. */
fn parse_multipart(bytes: &[u8], boundary: &str) -> Option<Vec<Part>> {
    let delimiter = format!("--{boundary}");
    let mut rest = bytes.strip_prefix(delimiter.as_bytes())?;
    let mut parts = Vec::new();
    let close = format!("\r\n{delimiter}");
    loop {
        if let Some(epilogue) = rest.strip_prefix(b"--") {
            // The close delimiter ends the line: only CRLF (and its epilogue) may follow.
            return (epilogue.is_empty() || epilogue.starts_with(b"\r\n")).then_some(parts);
        }
        rest = rest.strip_prefix(b"\r\n")?;
        let head_end = find(rest, b"\r\n\r\n")?;
        let head = std::str::from_utf8(&rest[..head_end]).ok()?;
        rest = &rest[head_end + 4..];
        let end = find(rest, close.as_bytes())?;
        parts.push(part(head, &rest[..end])?);
        rest = &rest[end + close.len()..];
    }
}

/** One part from its header block and content. */
fn part(head: &str, value: &[u8]) -> Option<Part> {
    let mut found = Part::text("", value);
    let mut named = false;
    for line in head.split("\r\n") {
        let (key, field) = line.split_once(':')?;
        match key.trim().to_ascii_lowercase().as_str() {
            "content-disposition" => {
                for param in field.split(';').skip(1) {
                    let (key, raw) = param.split_once('=')?;
                    let text = raw.trim().trim_matches('"').to_owned();
                    match key.trim() {
                        "name" => (found.name, named) = (text, true),
                        "filename" => found.file = Some(text),
                        _ => {}
                    }
                }
            }
            "content-type" => found.kind = field.trim().to_owned(),
            _ => {}
        }
    }
    named.then_some(found)
}

/** The first index of `needle` in `hay`. */
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}
