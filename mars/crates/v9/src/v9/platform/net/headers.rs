/**
 * The Fetch standard's Headers semantics, as Blink's Headers and FetchHeaderList and Chromium's
 * network-service CORS helpers apply them: header names must be tokens and values byte strings
 * without NUL, CR or LF, values lose leading and trailing HTTP whitespace, a guard decides which
 * writes are refused (immutable), dropped (forbidden request and response names, non-safelisted
 * no-cors headers) or kept, and reading sorts names and combines repeats with ", " except
 * Set-Cookie. Without the guard a page could forge Cookie or Host on a request, or read
 * Set-Cookie off a response.
 */
use crate::v9::compile::PRIVATE_MARK;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Prop, Slot};
use crate::v9::vm::{Res, Vm};

/** Who may change a Headers object, per the Fetch standard's header guard. */
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::v9) enum Guard {
    /** `new Headers()`: anything valid goes. */
    None,
    /** A Request's headers: forbidden request headers are dropped. */
    Request,
    /** A no-cors Request's headers: only no-CORS-safelisted headers are kept. */
    RequestNoCors,
    /** A constructed Response's headers: Set-Cookie and Set-Cookie2 are dropped. */
    Response,
    /** A fetched or error Response's headers: every write throws. */
    Immutable,
}

impl Guard {
    /** The guard as the number kept in the object's private slot. */
    fn code(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Request => 1.0,
            Self::RequestNoCors => 2.0,
            Self::Response => 3.0,
            Self::Immutable => 4.0,
        }
    }
    /** The guard a private slot's number names; anything else is no guard. */
    fn from_code(code: f64) -> Self {
        [
            Self::Request,
            Self::RequestNoCors,
            Self::Response,
            Self::Immutable,
        ]
        .into_iter()
        .find(|guard| guard.code() == code)
        .unwrap_or(Self::None)
    }
}

/** A private key script cannot reach: Object.keys and reflection skip PRIVATE_MARK keys. */
pub(in crate::v9) fn private_key(name: &str) -> Key {
    let mut units = vec![PRIVATE_MARK];
    units.extend(name.encode_utf16());
    Key::Str(JsString::from_units(units).expect("short key"))
}

/** Store a hidden value on an object. */
pub(in crate::v9) fn put_private(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(
        id,
        private_key(name),
        Prop {
            slot: Slot::Data(value),
            attrs: HIDDEN_ATTRS,
        },
    );
}

/** Read a hidden value off an object. */
pub(in crate::v9) fn get_private(vm: &Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    match vm.heap.get(id)?.props.get(&private_key(name)) {
        Some(Prop {
            slot: Slot::Data(value),
            ..
        }) => Some(value.clone()),
        _ => None,
    }
}

/** Set a Headers object's guard. */
pub(in crate::v9) fn set_guard(vm: &mut Vm, id: ObjectId, guard: Guard) {
    put_private(vm, id, "headersGuard", JsValue::Number(guard.code()));
}

/** A Headers object's guard. */
pub(in crate::v9) fn guard(vm: &Vm, id: ObjectId) -> Guard {
    match get_private(vm, id, "headersGuard") {
        Some(JsValue::Number(code)) => Guard::from_code(code),
        _ => Guard::None,
    }
}

/** RFC 9110 token characters: what a header name may hold. */
pub(in crate::v9) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        && name.is_ascii()
}

/** A header value after normalisation: a byte string (no code point above U+00FF) without NUL,
CR or LF. */
pub(in crate::v9) fn valid_value(value: &str) -> bool {
    value
        .chars()
        .all(|c| u32::from(c) <= 0xFF && !matches!(c, '\0' | '\r' | '\n'))
}

/** Strip leading and trailing HTTP whitespace (tab, LF, CR, space) and nothing else. */
pub(in crate::v9) fn normalize(value: &str) -> String {
    value
        .trim_matches(|c| matches!(c, '\t' | '\n' | '\r' | ' '))
        .to_owned()
}

/** Methods a request may never carry, so the override headers naming them are forbidden. */
const FORBIDDEN_METHODS: [&str; 3] = ["connect", "trace", "track"];

/** Request headers only the browser sets (Fetch's forbidden request-header). */
const FORBIDDEN_REQUEST: [&str; 21] = [
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "expect",
    "host",
    "keep-alive",
    "origin",
    "referer",
    "set-cookie",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "via",
];

/** Whether a page may not set this request header: the fixed list, the `proxy-` and `sec-`
prefixes, and a method override naming a forbidden method. */
pub(in crate::v9) fn forbidden_request(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if FORBIDDEN_REQUEST.contains(&name.as_str())
        || name.starts_with("proxy-")
        || name.starts_with("sec-")
    {
        return true;
    }
    matches!(
        name.as_str(),
        "x-http-method" | "x-http-method-override" | "x-method-override"
    ) && value
        .split(',')
        .any(|method| FORBIDDEN_METHODS.contains(&normalize(method).to_ascii_lowercase().as_str()))
}

/** Response headers script never sees or sets. */
pub(in crate::v9) fn forbidden_response(name: &str) -> bool {
    name.eq_ignore_ascii_case("set-cookie") || name.eq_ignore_ascii_case("set-cookie2")
}

/** The longest value a CORS-safelisted request header may have. */
const SAFELISTED_VALUE_BYTES: usize = 128;

/** Bytes Fetch calls CORS-unsafe request-header bytes. */
fn unsafe_byte(b: u8) -> bool {
    (b < 0x20 && b != b'\t') || b"\"():<>?@[\\]{}".contains(&b) || b == 0x7F
}

/** Fetch's CORS-safelisted request-header, which no-cors requests are limited to. */
pub(in crate::v9) fn cors_safelisted(name: &str, value: &str) -> bool {
    if value.len() > SAFELISTED_VALUE_BYTES {
        return false;
    }
    match name.to_ascii_lowercase().as_str() {
        "accept" => !value.bytes().any(unsafe_byte),
        "accept-language" | "content-language" => value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" *,-.;=".contains(&b)),
        "content-type" => {
            !value.bytes().any(unsafe_byte)
                && matches!(
                    essence(value).as_str(),
                    "application/x-www-form-urlencoded" | "multipart/form-data" | "text/plain"
                )
        }
        _ => false,
    }
}

/** A MIME type's lowercased type/subtype. */
fn essence(value: &str) -> String {
    normalize(value.split(';').next().unwrap_or_default()).to_ascii_lowercase()
}

/** Names a no-cors request may carry at all. */
fn no_cors_name(name: &str) -> bool {
    matches!(
        name,
        "accept" | "accept-language" | "content-language" | "content-type"
    )
}

/** Why a Headers write did not happen: a TypeError to throw, or a silent drop. */
pub(in crate::v9) enum Refusal {
    Throw(&'static str),
    Drop,
}

/** The guard check shared by append, set and delete: Err means skip the write. */
fn admit(
    guard: Guard,
    name: &str,
    value: &str,
    pairs: &[(String, String)],
    appending: bool,
) -> Result<(), Refusal> {
    match guard {
        Guard::Immutable => Err(Refusal::Throw("Headers are immutable")),
        Guard::Request if forbidden_request(name, value) => Err(Refusal::Drop),
        Guard::Response if forbidden_response(name) => Err(Refusal::Drop),
        Guard::RequestNoCors => {
            let combined = match combined(pairs, name) {
                Some(existing) if appending => format!("{existing}, {value}"),
                _ => value.to_owned(),
            };
            if no_cors_name(name) && cors_safelisted(name, &combined) {
                Ok(())
            } else {
                Err(Refusal::Drop)
            }
        }
        _ => Ok(()),
    }
}

/** The values for `name` joined with ", ", or None when absent. */
pub(in crate::v9) fn combined(pairs: &[(String, String)], name: &str) -> Option<String> {
    let values: Vec<&str> = pairs
        .iter()
        .filter(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
        .collect();
    (!values.is_empty()).then(|| values.join(", "))
}

/** Validate and normalise a name and value; Err is the TypeError message. */
pub(in crate::v9) fn checked(name: &str, value: &str) -> Result<(String, String), &'static str> {
    if !valid_name(name) {
        return Err("Invalid header name.");
    }
    let value = normalize(value);
    if !valid_value(&value) {
        return Err("Invalid header value.");
    }
    Ok((name.to_ascii_lowercase(), value))
}

/** Headers.append under a guard. */
pub(in crate::v9) fn append(
    pairs: &mut Vec<(String, String)>,
    guard: Guard,
    name: &str,
    value: &str,
) -> Result<(), Refusal> {
    let (name, value) = checked(name, value).map_err(Refusal::Throw)?;
    admit(guard, &name, &value, pairs, true)?;
    pairs.push((name, value));
    Ok(())
}

/** Headers.set under a guard: the first entry takes the value in place, the rest go. */
pub(in crate::v9) fn set(
    pairs: &mut Vec<(String, String)>,
    guard: Guard,
    name: &str,
    value: &str,
) -> Result<(), Refusal> {
    let (name, value) = checked(name, value).map_err(Refusal::Throw)?;
    admit(guard, &name, &value, pairs, false)?;
    match pairs.iter().position(|(header, _)| *header == name) {
        Some(first) => {
            pairs[first].1 = value;
            let mut index = 0;
            pairs.retain(|(header, _)| {
                index += 1;
                index - 1 == first || *header != name
            });
        }
        None => pairs.push((name, value)),
    }
    Ok(())
}

/** Headers.delete under a guard. */
pub(in crate::v9) fn delete(
    pairs: &mut Vec<(String, String)>,
    guard: Guard,
    name: &str,
) -> Result<(), Refusal> {
    if !valid_name(name) {
        return Err(Refusal::Throw("Invalid header name."));
    }
    let name = name.to_ascii_lowercase();
    match guard {
        Guard::Immutable => return Err(Refusal::Throw("Headers are immutable")),
        Guard::Request if forbidden_request(&name, "") => return Err(Refusal::Drop),
        Guard::RequestNoCors if !no_cors_name(&name) => return Err(Refusal::Drop),
        Guard::Response if forbidden_response(&name) => return Err(Refusal::Drop),
        _ => {}
    }
    pairs.retain(|(header, _)| *header != name);
    Ok(())
}

/** Fetch's "sort and combine": names in byte order, repeats joined with ", ", and each
Set-Cookie its own entry, since joining cookies with commas is ambiguous. */
pub(in crate::v9) fn sort_and_combine(pairs: &[(String, String)]) -> Vec<(String, String)> {
    let mut names: Vec<&str> = pairs.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let mut out = Vec::new();
    for name in names {
        if name == "set-cookie" {
            out.extend(pairs.iter().filter(|(header, _)| header == name).cloned());
        } else if let Some(value) = combined(pairs, name) {
            out.push((name.to_owned(), value));
        }
    }
    out
}

/** Pairs from a `new Headers(init)` argument, Err being the TypeError message: an iterable of
two-item iterables, or a record of own enumerable string keys. */
pub(in crate::v9) fn init_pairs(
    vm: &mut Vm,
    value: &JsValue,
) -> Res<Result<Vec<(String, String)>, &'static str>> {
    let JsValue::Object(id) = value else {
        return Ok(Err(
            "The provided value is not of type '(record or sequence)'.",
        ));
    };
    let iterator = vm.get(value, &Key::Sym(crate::v9::vm::realm::symbol::ITERATOR))?;
    let mut out = Vec::new();
    if !iterator.nullish() {
        for pair in vm.iterate_to_vec(value)? {
            if !matches!(pair, JsValue::Object(_)) {
                return Ok(Err("The provided value cannot be converted to a sequence."));
            }
            let items = vm.iterate_to_vec(&pair)?;
            if items.len() != 2 {
                return Ok(Err(
                    "Invalid value: a header pair needs a name and a value.",
                ));
            }
            out.push((vm.to_text(&items[0])?, vm.to_text(&items[1])?));
        }
        return Ok(Ok(out));
    }
    for key in vm.enumerable_keys(*id)? {
        let item = vm.get(value, &key)?;
        out.push((crate::v9::vm::property::key_text(&key), vm.to_text(&item)?));
    }
    Ok(Ok(out))
}
