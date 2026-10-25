/**
 * fetch, XMLHttpRequest, sendBeacon, URL, URLSearchParams, Headers and Response. Requests are
 * recorded as HostRequest for the app to perform through Relay; responses come back through
 * `resume_requests`. V9 never opens a connection.
 */
use super::dom::native_name;
use super::events::fire;
use super::{HostRequest, HostResponse, Pending, Target};
use crate::constants::{VM_HOST_REQUESTS, VM_REQUEST_BODY_BYTES, VM_RESPONSE_BYTES};
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::{Class, HostData, PromiseState, ResponseData};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Thrown, Vm};
use headers::Guard;
use serde::{Deserialize, Serialize};

mod headers;

/** Parse an absolute URL. */
pub(in crate::v9) fn parse_url(text: &str) -> Option<url::Url> {
    url::Url::parse(text).ok()
}

/** Resolve a possibly relative URL against the document URL. */
pub(in crate::v9) fn resolve(vm: &Vm, text: &str) -> Option<String> {
    let base = parse_url(&vm.page.state.href);
    let resolved = match base {
        Some(base) => base.join(text.trim()).ok()?,
        None => url::Url::parse(text.trim()).ok()?,
    };
    Some(resolved.to_string())
}

/** One XMLHttpRequest's state. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::v9) struct XhrState {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub ready: u8,
    pub status: u16,
    pub status_text: String,
    pub response: String,
    pub response_headers: Vec<(String, String)>,
    pub response_url: String,
    pub response_type: String,
    pub with_credentials: bool,
    pub sent: bool,
}

/** Record a request, failing like a network error past the pending bound. */
fn record(vm: &mut Vm, mut request: HostRequest, owner: ObjectId, xhr: bool) -> Res<bool> {
    if vm.page.state.pending.len() >= VM_HOST_REQUESTS {
        return Ok(false);
    }
    if request.body.as_ref().map_or(0, String::len) + request.body_bytes.len()
        > VM_REQUEST_BODY_BYTES
    {
        return Err(EngineError::limit("request body bytes", VM_REQUEST_BODY_BYTES).into());
    }
    vm.page.state.next_request += 1;
    request.id = vm.page.state.next_request;
    if request.kind != "beacon" {
        vm.page.state.pending.push(Pending {
            id: request.id,
            owner,
            xhr,
        });
    }
    vm.page.effects.requests.push(request);
    Ok(true)
}

/** Header pairs from a Headers object, an object or an array of pairs. */
fn header_list(vm: &mut Vm, value: &JsValue) -> Res<Vec<(String, String)>> {
    if value.nullish() {
        return Ok(Vec::new());
    }
    if let Some(HostData::Headers(list)) = super::host_data(vm, value) {
        return Ok(list);
    }
    let mut out = Vec::new();
    if crate::v9::builtins::is_array(vm, value) {
        for pair in vm.iterate_to_vec(value)? {
            let name = vm.get(&pair, &Key::from("0"))?;
            let item = vm.get(&pair, &Key::from("1"))?;
            out.push((vm.to_text(&name)?.to_ascii_lowercase(), vm.to_text(&item)?));
        }
        return Ok(out);
    }
    if let JsValue::Object(id) = value {
        for key in vm.enumerable_keys(*id)? {
            let item = vm.get(value, &key)?;
            out.push((
                crate::v9::vm::property::key_text(&key).to_ascii_lowercase(),
                vm.to_text(&item)?,
            ));
        }
    }
    Ok(out)
}

/** A URLSearchParams body is a form: label it so unless the page set its own Content-Type.
(The app labels any other text body text/plain, as browsers do.) */
fn form_type(vm: &mut Vm, body: &JsValue, headers: &mut Vec<(String, String)>) {
    if matches!(super::host_data(vm, body), Some(HostData::SearchParams(_)))
        && !headers.iter().any(|(name, _)| name == "content-type")
    {
        headers.push((
            "content-type".into(),
            "application/x-www-form-urlencoded;charset=UTF-8".into(),
        ));
    }
}

/** A request body: text, or bytes when a BufferSource holds something other than UTF-8. */
#[derive(Default)]
struct Body {
    text: Option<String>,
    bytes: Vec<u8>,
}

/** A request body as a browser reads it: strings and URLSearchParams as text, and an
ArrayBuffer, typed array or DataView as its bytes (as text when they are UTF-8, so the common
case stays a text body). Other objects are refused as unsupported. */
fn request_body(vm: &mut Vm, value: &JsValue) -> Res<Body> {
    if value.nullish() {
        return Ok(Body::default());
    }
    if let Some(HostData::SearchParams(pairs)) = super::host_data(vm, value) {
        return Ok(Body {
            text: Some(encode_pairs(&pairs)),
            bytes: Vec::new(),
        });
    }
    if let Some(bytes) = crate::v9::builtins::typed::source_bytes(vm, value) {
        return Ok(match String::from_utf8(bytes) {
            Ok(text) => Body {
                text: Some(text),
                bytes: Vec::new(),
            },
            Err(binary) => Body {
                text: None,
                bytes: binary.into_bytes(),
            },
        });
    }
    if matches!(value, JsValue::Object(_))
        && super::host_data(vm, value).is_none()
        && !matches!(
            vm.heap.get(value.object().unwrap_or(0)).map(|o| &o.class),
            Some(Class::String(_))
        )
    {
        return Err(
            vm.unsupported_error("request bodies other than strings, URLSearchParams and buffers")
        );
    }
    Ok(Body {
        text: Some(vm.to_text(value)?),
        bytes: Vec::new(),
    })
}

/** fetch(input, init). */
pub(in crate::v9) fn fetch(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    let request = match request_from(vm, &call.arg(0), &call.arg(1))? {
        Ok(request) => request,
        Err(message) => {
            reject(vm, promise, &message)?;
            return Ok(JsValue::Object(promise));
        }
    };
    if !record(vm, request, promise, false)? {
        reject(vm, promise, "Failed to fetch")?;
    }
    Ok(JsValue::Object(promise))
}

/** The request `fetch(input, init)` or `new Request(input, init)` describes: `input` is a URL
string, a URL, or a Request whose fields the init overrides; Err is the TypeError's message. */
fn request_from(vm: &mut Vm, input: &JsValue, init: &JsValue) -> Res<Result<HostRequest, String>> {
    let base = match super::host_data(vm, input) {
        Some(HostData::Request(request)) => Some(*request),
        _ => None,
    };
    let target = match (&base, input) {
        (Some(request), _) => request.url.clone(),
        (None, JsValue::Object(_)) => match super::host_data(vm, input) {
            Some(HostData::Url(href)) => href,
            _ => {
                let url = vm.get(input, &Key::from("url"))?;
                vm.to_text(&url)?
            }
        },
        (None, other) => vm.to_text(other)?,
    };
    let Some(url) = resolve(vm, &target) else {
        return Ok(Err(format!("Failed to parse URL from {target}")));
    };
    let read = |vm: &mut Vm, name: &str| -> Res<JsValue> {
        if matches!(init, JsValue::Object(_)) {
            vm.get(init, &Key::from(name))
        } else {
            Ok(JsValue::Undefined)
        }
    };
    let text = |vm: &mut Vm, value: JsValue, fallback: String| -> Res<String> {
        if value.nullish() {
            Ok(fallback)
        } else {
            vm.to_text(&value)
        }
    };
    let method = read(vm, "method")?;
    let method = text(
        vm,
        method,
        base.as_ref().map_or("GET".into(), |r| r.method.clone()),
    )?;
    let method = match normalize_method(&method) {
        Ok(method) => method,
        Err(message) => return Ok(Err(message)),
    };
    let mode = read(vm, "mode")?;
    let mode = text(
        vm,
        mode,
        base.as_ref().map_or("cors".into(), |r| r.mode.clone()),
    )?;
    let headers = read(vm, "headers")?;
    let mut headers = if headers.nullish() {
        live_headers(vm, input)
            .unwrap_or_else(|| base.as_ref().map(|r| r.headers.clone()).unwrap_or_default())
    } else {
        match guarded_pairs(vm, &headers, request_guard(&mode))? {
            Ok(pairs) => pairs,
            Err(message) => return Ok(Err(message.into())),
        }
    };
    let body = read(vm, "body")?;
    let body = if body.nullish() {
        base.as_ref().map_or_else(Body::default, |r| Body {
            text: r.body.clone(),
            bytes: r.body_bytes.clone(),
        })
    } else {
        if matches!(method.as_str(), "GET" | "HEAD") {
            return Ok(Err("Request with GET/HEAD method cannot have body.".into()));
        }
        form_type(vm, &body, &mut headers);
        if matches!(body, JsValue::String(_))
            && !headers.iter().any(|(name, _)| name == "content-type")
        {
            headers.push(("content-type".into(), "text/plain;charset=UTF-8".into()));
        }
        request_body(vm, &body)?
    };
    if let Err(message) = check_init(vm, init, &url, &method, &mode)? {
        return Ok(Err(message));
    }
    let credentials = read(vm, "credentials")?;
    let credentials = text(
        vm,
        credentials,
        base.as_ref()
            .map_or("same-origin".into(), |r| r.credentials.clone()),
    )?;
    // Inherited headers were admitted under the input Request's mode; the outgoing list must pass
    // this request's own guard, so no-cors cannot inherit a non-safelisted header.
    let headers = refiltered(headers, request_guard(&mode));
    Ok(Ok(HostRequest {
        id: 0,
        kind: "fetch".into(),
        method,
        url,
        headers,
        body: body.text,
        body_bytes: body.bytes,
        credentials,
        mode,
    }))
}

/** Methods Fetch normalises to upper case; any other method keeps the page's spelling. */
const NORMALIZED_METHODS: [&str; 6] = ["DELETE", "GET", "HEAD", "OPTIONS", "POST", "PUT"];

/** Fetch's method check: a token, not CONNECT, TRACE or TRACK, and normalised; Err is the
TypeError's message. */
fn normalize_method(method: &str) -> Result<String, String> {
    if !headers::valid_name(method) {
        return Err(format!("'{method}' is not a valid HTTP method."));
    }
    let upper = method.to_ascii_uppercase();
    if ["CONNECT", "TRACE", "TRACK"].contains(&upper.as_str()) {
        return Err(format!("'{method}' HTTP method is unsupported."));
    }
    Ok(if NORMALIZED_METHODS.contains(&upper.as_str()) {
        upper
    } else {
        method.to_owned()
    })
}

/** The values each RequestInit enum member accepts (WebIDL enums, so others are TypeErrors). */
const INIT_ENUMS: [(&str, &[&str]); 5] = [
    ("mode", &["navigate", "same-origin", "no-cors", "cors"]),
    ("credentials", &["omit", "same-origin", "include"]),
    (
        "cache",
        &[
            "default",
            "no-store",
            "reload",
            "no-cache",
            "force-cache",
            "only-if-cached",
        ],
    ),
    ("redirect", &["follow", "error", "manual"]),
    (
        "referrerPolicy",
        &[
            "",
            "no-referrer",
            "no-referrer-when-downgrade",
            "same-origin",
            "origin",
            "strict-origin",
            "origin-when-cross-origin",
            "strict-origin-when-cross-origin",
            "unsafe-url",
        ],
    ),
];

/** The Request constructor's init checks, in Blink's Request::CreateRequestWithRequestOrString
order: enum values, a non-null window, a navigate mode, credentials in the URL, an unparsable
referrer, only-if-cached outside same-origin, and a no-cors method other than GET, HEAD or
POST. Err is the TypeError's message. */
fn check_init(
    vm: &mut Vm,
    init: &JsValue,
    url: &str,
    method: &str,
    mode: &str,
) -> Res<Result<(), String>> {
    let mut cache = String::new();
    for (name, allowed) in INIT_ENUMS {
        let value = member(vm, init, name)?;
        if matches!(value, JsValue::Undefined) {
            continue;
        }
        let value = vm.to_text(&value)?;
        if !allowed.contains(&value.as_str()) {
            return Ok(Err(format!(
                "The provided value '{value}' is not a valid {name}."
            )));
        }
        if name == "cache" {
            cache = value;
        }
    }
    let window = member(vm, init, "window")?;
    let referrer = member(vm, init, "referrer")?;
    let referrer = if matches!(referrer, JsValue::Undefined) {
        None
    } else {
        Some(vm.to_text(&referrer)?)
    };
    Ok(request_rules(
        vm,
        url,
        method,
        mode,
        &cache,
        !window.nullish(),
        referrer.as_deref(),
    ))
}

/** The checks of `check_init` that need no further script values. */
fn request_rules(
    vm: &Vm,
    url: &str,
    method: &str,
    mode: &str,
    cache: &str,
    window: bool,
    referrer: Option<&str>,
) -> Result<(), String> {
    let parsed = parse_url(url);
    if window {
        Err("'window' must be null.".into())
    } else if mode == "navigate" {
        Err(
            "Cannot construct a Request with a RequestInit whose mode member is set as 'navigate'."
                .into(),
        )
    } else if parsed.is_some_and(|u| !u.username().is_empty() || u.password().is_some()) {
        Err(format!(
            "Request cannot be constructed from a URL that includes credentials: {url}"
        ))
    } else if referrer.is_some_and(|r| !r.is_empty() && resolve(vm, r).is_none()) {
        Err("The referrer is not a valid URL.".into())
    } else if cache == "only-if-cached" && mode != "same-origin" {
        Err("'only-if-cached' can be set only with 'same-origin' mode".into())
    } else if mode == "no-cors" && !matches!(method, "GET" | "HEAD" | "POST") {
        Err(format!("'{method}' is unsupported in no-cors mode."))
    } else {
        Ok(())
    }
}

/** `new Request(input, init)`: the request fetch would send, kept for `fetch(request)`. */
pub(in crate::v9) fn request_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Failed to construct 'Request': Please use the 'new' operator."));
    }
    let request = match request_from(vm, &call.arg(0), &call.arg(1))? {
        Ok(request) => request,
        Err(message) => return Err(vm.type_error(&message)),
    };
    request_object(vm, request)
}

/** A Request object over its data. */
fn request_object(vm: &mut Vm, request: HostRequest) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Request(Box::new(request)))),
        Some(realm().ids.request_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** Request accessors and methods: the fields fetch reads, `clone`, and the body readers. */
pub(in crate::v9) fn request_member(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(HostData::Request(request)) = super::host_data(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match native_name(&call) {
        "url" => JsValue::string(&request.url).map_err(Into::into),
        "method" => JsValue::string(&request.method).map_err(Into::into),
        "headers" => {
            let guard = request_guard(&request.mode);
            owned_headers(vm, &call.this, request.headers.clone(), guard)
        }
        "credentials" => JsValue::string(&request.credentials).map_err(Into::into),
        "mode" => JsValue::string(&request.mode).map_err(Into::into),
        "cache" => JsValue::string("default").map_err(Into::into),
        "redirect" => JsValue::string("follow").map_err(Into::into),
        "referrer" => JsValue::string("about:client").map_err(Into::into),
        "destination" => JsValue::string("").map_err(Into::into),
        "integrity" => JsValue::string("").map_err(Into::into),
        "keepalive" | "bodyUsed" => Ok(JsValue::Boolean(false)),
        "signal" => {
            let signal = vm.alloc(Object::new(
                Class::Plain,
                Some(realm().ids.abort_signal_prototype),
            ))?;
            vm.put(signal, "aborted", JsValue::Boolean(false))?;
            vm.put(signal, "reason", JsValue::Undefined)?;
            vm.put(signal, "onabort", JsValue::Null)?;
            Ok(JsValue::Object(signal))
        }
        "clone" => {
            let mut copy = *request;
            if let Some(pairs) = live_headers(vm, &call.this) {
                copy.headers = pairs;
            }
            request_object(vm, copy)
        }
        name @ ("text" | "json") => {
            let promise = vm.new_promise()?;
            let body = request.body.clone().unwrap_or_default();
            let outcome = if name == "text" {
                JsString::from_utf8(&body)
                    .map(JsValue::String)
                    .map_err(Thrown::from)
            } else {
                crate::v9::builtins::json_parse_text(vm, &body)
            };
            match outcome {
                Ok(value) => vm.resolve_promise(promise, value)?,
                Err(Thrown::Value(error)) => vm.settle(promise, PromiseState::Rejected(error))?,
                Err(fatal) => return Err(fatal),
            }
            Ok(JsValue::Object(promise))
        }
        name => Err(vm.unsupported_error(&format!("Request.{name}"))),
    }
}

/** Reject a promise with a TypeError. */
fn reject(vm: &mut Vm, promise: ObjectId, message: &str) -> Res<()> {
    let error = vm.type_error(message);
    let Thrown::Value(error) = error else {
        return Err(error);
    };
    vm.settle(promise, PromiseState::Rejected(error))
}

/** navigator.sendBeacon(url, data): fire-and-forget. */
pub(in crate::v9) fn send_beacon(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = vm.to_text(&call.arg(0))?;
    let Some(url) = resolve(vm, &target) else {
        return Err(vm.type_error("Failed to execute 'sendBeacon': Invalid URL"));
    };
    let body = request_body(vm, &call.arg(1))?;
    let request = HostRequest {
        id: 0,
        kind: "beacon".into(),
        method: "POST".into(),
        url,
        headers: Vec::new(),
        body: body.text,
        body_bytes: body.bytes,
        credentials: "include".into(),
        mode: "no-cors".into(),
    };
    Ok(JsValue::Boolean(record(
        vm,
        request,
        crate::v9::vm::GLOBAL,
        false,
    )?))
}

impl Vm<'_> {
    /** Deliver responses: fetch promises settle, XMLHttpRequests fire their events. */
    pub fn deliver(&mut self, responses: Vec<HostResponse>) -> Res<()> {
        for response in responses {
            let Some(index) = self
                .page
                .state
                .pending
                .iter()
                .position(|pending| pending.id == response.id)
            else {
                continue;
            };
            let pending = self.page.state.pending.remove(index);
            if response.body.len() > VM_RESPONSE_BYTES {
                return Err(EngineError::limit("response body bytes", VM_RESPONSE_BYTES).into());
            }
            let result = if pending.xhr {
                self.deliver_xhr(pending.owner, response)
            } else {
                self.deliver_fetch(pending.owner, response)
            };
            match result {
                Ok(()) => {}
                Err(Thrown::Value(error)) => self.report_error(&error),
                Err(fatal) => return Err(fatal),
            }
            self.drain_jobs()?;
        }
        Ok(())
    }
    /** Settle a fetch promise. */
    fn deliver_fetch(&mut self, promise: ObjectId, response: HostResponse) -> Res<()> {
        if let Some(error) = response.error {
            let _ = error;
            return reject(self, promise, "Failed to fetch");
        }
        let data = ResponseData {
            status: response.status,
            status_text: response.status_text,
            url: response.url,
            headers: response.headers,
            body: response.body,
            used: false,
        };
        let object = self.alloc(Object::new(
            Class::Host(Box::new(HostData::Response(Box::new(data)))),
            Some(realm().ids.response_prototype),
        ))?;
        self.resolve_promise(promise, JsValue::Object(object))
    }
    /** Complete an XMLHttpRequest. */
    fn deliver_xhr(&mut self, xhr: ObjectId, response: HostResponse) -> Res<()> {
        let failed = response.error.is_some();
        if let Some(state) = self.page.state.xhrs.get_mut(&xhr) {
            state.ready = 4;
            state.status = if failed { 0 } else { response.status };
            state.status_text = response.status_text;
            state.response = response.body;
            state.response_headers = response.headers;
            state.response_url = response.url;
        }
        let target = Target::Object(xhr);
        fire(self, target, "readystatechange", false, false)?;
        fire(
            self,
            target,
            if failed { "error" } else { "load" },
            false,
            false,
        )?;
        fire(self, target, "loadend", false, false)?;
        Ok(())
    }
}

/** `new XMLHttpRequest()`. */
pub(in crate::v9) fn xhr_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(
            vm.type_error("Failed to construct 'XMLHttpRequest': Please use the 'new' operator.")
        );
    }
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Xhr(0))),
        Some(realm().ids.xhr_prototype),
    ))?;
    vm.page.state.xhrs.insert(id, XhrState::default());
    Ok(JsValue::Object(id))
}

/** The XMLHttpRequest a receiver is. */
fn this_xhr(vm: &mut Vm, call: &Call) -> Res<ObjectId> {
    match call.this {
        JsValue::Object(id) if vm.page.state.xhrs.contains_key(&id) => Ok(id),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** XMLHttpRequest methods. */
pub(in crate::v9) fn xhr_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_xhr(vm, &call)?;
    match native_name(&call) {
        "open" => {
            let method = xhr_method_name(vm, &call.arg(0))?;
            let target = vm.to_text(&call.arg(1))?;
            let Some(url) = resolve(vm, &target) else {
                return Err(vm.dom_exception(
                    "SyntaxError",
                    "Failed to execute 'open' on 'XMLHttpRequest': Invalid URL",
                ));
            };
            if call.args.len() > 2 && !call.arg(2).truthy() {
                return Err(vm.unsupported_error("synchronous XMLHttpRequest"));
            }
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                *state = XhrState {
                    method,
                    url,
                    ready: 1,
                    response_type: state.response_type.clone(),
                    with_credentials: state.with_credentials,
                    ..XhrState::default()
                };
            }
            fire(vm, Target::Object(id), "readystatechange", false, false)?;
        }
        "setRequestHeader" => {
            let name = vm.to_text(&call.arg(0))?;
            let value = vm.to_text(&call.arg(1))?;
            xhr_set_header(vm, id, &name, &value)?;
        }
        "send" => {
            let body = request_body(vm, &call.arg(0))?;
            let Some(mut state) = vm.page.state.xhrs.get(&id).cloned() else {
                return Ok(JsValue::Undefined);
            };
            form_type(vm, &call.arg(0), &mut state.headers);
            if state.ready != 1 || state.sent {
                return Err(vm.dom_exception(
                    "InvalidStateError",
                    "Failed to execute 'send' on 'XMLHttpRequest': The object's state must be OPENED.",
                ));
            }
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                state.sent = true;
            }
            let credentials = if state.with_credentials {
                "include"
            } else {
                "same-origin"
            };
            let request = HostRequest {
                id: 0,
                kind: "xhr".into(),
                method: state.method,
                url: state.url,
                headers: state.headers,
                body: body.text,
                body_bytes: body.bytes,
                credentials: credentials.into(),
                mode: "cors".into(),
            };
            if !record(vm, request, id, true)? {
                vm.deliver_xhr(
                    id,
                    HostResponse {
                        error: Some("too many requests".into()),
                        ..HostResponse::default()
                    },
                )?;
            }
        }
        "abort" => {
            vm.page.state.pending.retain(|pending| pending.owner != id);
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                state.ready = 0;
                state.sent = false;
            }
            fire(vm, Target::Object(id), "abort", false, false)?;
        }
        "getResponseHeader" => {
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let state = vm.page.state.xhrs.get(&id).cloned().unwrap_or_default();
            let value = (state.ready >= XHR_HEADERS_RECEIVED)
                .then(|| headers::combined(&state.response_headers, &name))
                .flatten();
            return match value {
                Some(value) => JsValue::string(&value).map_err(Into::into),
                None => Ok(JsValue::Null),
            };
        }
        "getAllResponseHeaders" => {
            let state = vm.page.state.xhrs.get(&id).cloned().unwrap_or_default();
            let text = all_response_headers(&state);
            return JsValue::string(&text).map_err(Into::into);
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/** XMLHttpRequest's HEADERS_RECEIVED state: response headers are readable from here on. */
const XHR_HEADERS_RECEIVED: u8 = 2;

/** `open()`'s method, checked and normalised as Fetch does (Blink's
XMLHttpRequest::open): not a token is a SyntaxError, CONNECT, TRACE or TRACK a SecurityError. */
fn xhr_method_name(vm: &mut Vm, value: &JsValue) -> Res<String> {
    let method = vm.to_text(value)?;
    if !headers::valid_name(&method) {
        let message = format!("'{method}' is not a valid HTTP method.");
        return Err(vm.dom_exception("SyntaxError", &message));
    }
    normalize_method(&method).map_err(|message| vm.dom_exception("SecurityError", &message))
}

/** `setRequestHeader()` (XHR standard): only while OPENED and unsent, a bad name or value is a
SyntaxError, a forbidden header is ignored, and a repeated name joins its values with ", ". */
fn xhr_set_header(vm: &mut Vm, id: ObjectId, name: &str, value: &str) -> Res<()> {
    let opened = vm
        .page
        .state
        .xhrs
        .get(&id)
        .is_some_and(|state| state.ready == 1 && !state.sent);
    if !opened {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "Failed to execute 'setRequestHeader' on 'XMLHttpRequest': The object's state must be OPENED.",
        ));
    }
    let (name, value) = match headers::checked(name, value) {
        Ok(pair) => pair,
        Err(message) => return Err(vm.dom_exception("SyntaxError", message)),
    };
    if headers::forbidden_request(&name, &value) {
        return Ok(());
    }
    if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
        match state.headers.iter_mut().find(|(header, _)| *header == name) {
            Some((_, existing)) => {
                existing.push_str(", ");
                existing.push_str(&value);
            }
            None => state.headers.push((name, value)),
        }
    }
    Ok(())
}

/** `getAllResponseHeaders()`: empty before HEADERS_RECEIVED, then lowercase names in byte
order, repeats joined with ", ", each line ending in CRLF. */
fn all_response_headers(state: &XhrState) -> String {
    if state.ready < XHR_HEADERS_RECEIVED {
        return String::new();
    }
    let lowered: Vec<(String, String)> = state
        .response_headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .collect();
    headers::sort_and_combine(&lowered)
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect()
}

/** XMLHttpRequest accessors. */
pub(in crate::v9) fn xhr_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_xhr(vm, &call)?;
    let state = vm.page.state.xhrs.get(&id).cloned().unwrap_or_default();
    Ok(match native_name(&call) {
        "readyState" => JsValue::Number(f64::from(state.ready)),
        "status" => JsValue::Number(f64::from(state.status)),
        "statusText" => JsValue::string(&state.status_text)?,
        "responseText" => JsValue::string(&state.response)?,
        "responseURL" => JsValue::string(&state.response_url)?,
        "responseType" => JsValue::string(&state.response_type)?,
        "withCredentials" => JsValue::Boolean(state.with_credentials),
        "response" => {
            if state.ready != 4 {
                return Ok(JsValue::string("")?);
            }
            if state.response_type == "json" {
                return crate::v9::builtins::json_parse_text(vm, &state.response)
                    .or(Ok(JsValue::Null));
            }
            JsValue::string(&state.response)?
        }
        "responseXML" => JsValue::Null,
        "timeout" => JsValue::Number(0.0),
        "upload" => JsValue::Object(vm.new_object()?),
        _ => JsValue::Undefined,
    })
}

/** XMLHttpRequest setters. */
pub(in crate::v9) fn xhr_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_xhr(vm, &call)?;
    let value = call.arg(0);
    match native_name(&call) {
        "responseType" => {
            let text = vm.to_text(&value)?;
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                state.response_type = text;
            }
        }
        "withCredentials" => {
            let state = vm.page.state.xhrs.get(&id).cloned().unwrap_or_default();
            if state.ready > 1 || state.sent {
                return Err(vm.dom_exception(
                    "InvalidStateError",
                    "Failed to set the 'withCredentials' property on 'XMLHttpRequest': The value may only be set if the object's state is UNSENT or OPENED.",
                ));
            }
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                state.with_credentials = value.truthy();
            }
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/** The response a receiver holds. */
fn this_response(vm: &mut Vm, call: &Call) -> Res<(ObjectId, ResponseData)> {
    if let JsValue::Object(id) = call.this
        && let Some(HostData::Response(data)) = super::host_data(vm, &call.this)
    {
        return Ok((id, *data));
    }
    Err(vm.type_error("Illegal invocation"))
}

/** Response accessors. */
pub(in crate::v9) fn response_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, data) = this_response(vm, &call)?;
    Ok(match native_name(&call) {
        "ok" => JsValue::Boolean((200..300).contains(&data.status)),
        "status" => JsValue::Number(f64::from(data.status)),
        "statusText" => JsValue::string(&data.status_text)?,
        "url" => JsValue::string(&data.url)?,
        "redirected" => JsValue::Boolean(false),
        "type" => JsValue::string(&response_type(vm, &call.this, &data))?,
        "bodyUsed" => JsValue::Boolean(data.used),
        "headers" => {
            let guard = response_guard(vm, &call.this);
            owned_headers(vm, &call.this, data.headers, guard)?
        }
        _ => JsValue::Undefined,
    })
}

/// What kind of response the app handed back: an opaque one has status 0 and no URL, a CORS
/// one ended on another origin, and anything else is basic. A Response script built says so
/// in its private type ("default" or "error").
// ponytail: a CORS response redirected back to the page's own origin reads as "basic"; carry the
// tainting on HostResponse if a page ever depends on it.
fn response_type(vm: &Vm, this: &JsValue, data: &ResponseData) -> String {
    if let Some(JsValue::String(kind)) = this
        .object()
        .and_then(|id| headers::get_private(vm, id, "responseType"))
    {
        return String::from_utf16_lossy(kind.as_units());
    }
    let page = parse_url(&vm.page.state.href).map(|url| url.origin());
    let own = parse_url(&data.url).map(|url| url.origin());
    if data.status == 0 && data.url.is_empty() {
        "opaque"
    } else if own.is_some() && own != page {
        "cors"
    } else {
        "basic"
    }
    .into()
}

/** A Response's headers guard: "response" for one script built with `new Response`, and
immutable for fetched, error and redirect responses (Fetch's Response constructor and statics). */
fn response_guard(vm: &Vm, this: &JsValue) -> Guard {
    let built = this
        .object()
        .and_then(|id| headers::get_private(vm, id, "responseGuard"));
    if matches!(built, Some(JsValue::Boolean(true))) {
        Guard::Response
    } else {
        Guard::Immutable
    }
}

/** A Response object script built: its type, and whether its headers stay writable. */
fn built_response(vm: &mut Vm, data: ResponseData, kind: &str, writable: bool) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Response(Box::new(data)))),
        Some(realm().ids.response_prototype),
    ))?;
    headers::put_private(vm, id, "responseType", JsValue::string(kind)?);
    headers::put_private(vm, id, "responseGuard", JsValue::Boolean(writable));
    Ok(JsValue::Object(id))
}

/** Statuses whose responses have no body (Fetch's null body status). */
const NULL_BODY_STATUSES: [u16; 5] = [101, 103, 204, 205, 304];
/** Statuses `Response.redirect` accepts (Fetch's redirect status). */
const REDIRECT_STATUSES: [u16; 5] = [301, 302, 303, 307, 308];
/** The range `new Response` accepts for `status`. */
const RESPONSE_STATUSES: std::ops::RangeInclusive<u16> = 200..=599;
/** Response.redirect's default status. */
const DEFAULT_REDIRECT: u16 = 302;

/** RFC 9110 reason-phrase: tab, space, visible ASCII and obs-text. */
fn reason_phrase(text: &str) -> bool {
    text.chars().all(|c| {
        let c = u32::from(c);
        c == 0x09 || c == 0x20 || (0x21..=0x7E).contains(&c) || (0x80..=0xFF).contains(&c)
    })
}

/** WebIDL unsigned short from a value, or `fallback` when undefined. */
fn unsigned_short(vm: &mut Vm, value: &JsValue, fallback: u16) -> Res<u16> {
    if matches!(value, JsValue::Undefined) {
        return Ok(fallback);
    }
    let number = vm.to_number(value)?;
    Ok(if number.is_finite() {
        (number.trunc().rem_euclid(65536.0)) as u16
    } else {
        0
    })
}

/** Read one member of an init dictionary. */
fn member(vm: &mut Vm, init: &JsValue, name: &str) -> Res<JsValue> {
    if matches!(init, JsValue::Object(_)) {
        vm.get(init, &Key::from(name))
    } else {
        Ok(JsValue::Undefined)
    }
}

/** `new Response(body, init)`: Fetch's Response constructor and "initialize a response". */
pub(in crate::v9) fn response_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Failed to construct 'Response': Please use the 'new' operator."));
    }
    initialize_response(vm, &call.arg(0), &call.arg(1), "text/plain;charset=UTF-8")
}

/** Fetch's "initialize a response" over a body and init; `label` is the Content-Type a text
body gets when the init's headers set none. */
fn initialize_response(vm: &mut Vm, body: &JsValue, init: &JsValue, label: &str) -> Res<JsValue> {
    let status = member(vm, init, "status")?;
    let status = unsigned_short(vm, &status, 200)?;
    if !RESPONSE_STATUSES.contains(&status) {
        return Err(vm.range_error("The status provided is outside the range [200, 599]."));
    }
    let text = member(vm, init, "statusText")?;
    let status_text = if matches!(text, JsValue::Undefined) {
        String::new()
    } else {
        vm.to_text(&text)?
    };
    if !reason_phrase(&status_text) {
        return Err(vm.type_error("Invalid statusText"));
    }
    let given = member(vm, init, "headers")?;
    let mut pairs = if given.nullish() {
        Vec::new()
    } else {
        match guarded_pairs(vm, &given, Guard::Response)? {
            Ok(pairs) => pairs,
            Err(message) => return Err(vm.type_error(message)),
        }
    };
    let text = response_text(vm, body, status, &mut pairs, label)?;
    let data = ResponseData {
        status,
        status_text,
        url: String::new(),
        headers: pairs,
        body: text,
        used: false,
    };
    built_response(vm, data, "default", true)
}

/** A constructed Response's body as text, labelling its Content-Type the way Fetch's "extract a
body" does when the page set none. */
fn response_text(
    vm: &mut Vm,
    body: &JsValue,
    status: u16,
    pairs: &mut Vec<(String, String)>,
    label: &str,
) -> Res<String> {
    if body.nullish() {
        return Ok(String::new());
    }
    if NULL_BODY_STATUSES.contains(&status) {
        return Err(vm.type_error("Response with null body status cannot have body"));
    }
    let is_text = matches!(body, JsValue::String(_));
    form_type(vm, body, pairs);
    if is_text && !pairs.iter().any(|(name, _)| name == "content-type") {
        pairs.push(("content-type".into(), label.into()));
    }
    let read = request_body(vm, body)?;
    Ok(read
        .text
        .unwrap_or_else(|| String::from_utf8_lossy(&read.bytes).into_owned()))
}

/** `Response.json(data, init)`: the data as JSON text, labelled application/json; data JSON
cannot represent is a TypeError. */
fn response_json(vm: &mut Vm, data: &JsValue, init: &JsValue) -> Res<JsValue> {
    let json = vm.global_get(&JsString::from_utf8("JSON")?, false)?;
    let stringify = vm.get(&json, &Key::from("stringify"))?;
    let text = vm.call(stringify, json, vec![data.clone()])?;
    if !matches!(text, JsValue::String(_)) {
        return Err(vm.type_error("Value is not JSON serializable"));
    }
    initialize_response(vm, &text, init, "application/json")
}

/** `Response.json(data, init)`, `Response.error()` and `Response.redirect(url, status)`. */
pub(in crate::v9) fn response_static(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let blank = |status, headers| ResponseData {
        status,
        status_text: String::new(),
        url: String::new(),
        headers,
        body: String::new(),
        used: false,
    };
    if native_name(&call) == "json" {
        return response_json(vm, &call.arg(0), &call.arg(1));
    }
    if native_name(&call) == "error" {
        return built_response(vm, blank(0, Vec::new()), "error", false);
    }
    let target = vm.to_text(&call.arg(0))?;
    let Some(url) = resolve(vm, &target) else {
        return Err(vm.type_error(&format!("Failed to parse URL from {target}")));
    };
    let status = unsigned_short(vm, &call.arg(1), DEFAULT_REDIRECT)?;
    if !REDIRECT_STATUSES.contains(&status) {
        return Err(vm.range_error("Invalid status code"));
    }
    built_response(
        vm,
        blank(status, vec![("location".into(), url)]),
        "default",
        false,
    )
}

/** `response.clone()`: the same status, type and body, with its own copy of the headers. */
fn clone_response(vm: &mut Vm, this: &JsValue, mut data: ResponseData) -> Res<JsValue> {
    if data.used {
        return Err(
            vm.type_error("Failed to execute 'clone' on 'Response': Response body is already used")
        );
    }
    if let Some(pairs) = live_headers(vm, this) {
        data.headers = pairs;
    }
    let copy = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Response(Box::new(data)))),
        Some(realm().ids.response_prototype),
    ))?;
    if let Some(source) = this.object() {
        for key in ["responseType", "responseGuard"] {
            if let Some(value) = headers::get_private(vm, source, key) {
                headers::put_private(vm, copy, key, value);
            }
        }
    }
    Ok(JsValue::Object(copy))
}

/** Response body methods. */
pub(in crate::v9) fn response_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, data) = this_response(vm, &call)?;
    let promise = vm.new_promise()?;
    let name = native_name(&call);
    if name == "clone" {
        return clone_response(vm, &call.this, data);
    }
    if data.used {
        reject(vm, promise, "Failed to execute: body stream already read")?;
        return Ok(JsValue::Object(promise));
    }
    if let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class)
        && let HostData::Response(response) = host.as_mut()
    {
        response.used = true;
    }
    let outcome = match name {
        "text" => JsString::from_utf8(&data.body)
            .map(JsValue::String)
            .map_err(Thrown::from),
        "json" => crate::v9::builtins::json_parse_text(vm, &data.body),
        _ => Err(vm.unsupported_error(&format!("Response.{name}"))),
    };
    match outcome {
        Ok(value) => vm.resolve_promise(promise, value)?,
        Err(Thrown::Value(error)) => vm.settle(promise, PromiseState::Rejected(error))?,
        Err(Thrown::Fatal(error)) if error.code == "limit_exceeded" => {
            let error = vm.range_error(&error.message);
            if let Thrown::Value(error) = error {
                vm.settle(promise, PromiseState::Rejected(error))?;
            }
        }
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(promise))
}

/** A Headers object over pairs under a guard. */
fn headers_object(vm: &mut Vm, pairs: Vec<(String, String)>, guard: Guard) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Headers(pairs))),
        Some(realm().ids.headers_prototype),
    ))?;
    headers::set_guard(vm, id, guard);
    Ok(JsValue::Object(id))
}

/** The guard a Request's headers get from its mode. */
fn request_guard(mode: &str) -> Guard {
    if mode == "no-cors" {
        Guard::RequestNoCors
    } else {
        Guard::Request
    }
}

/** Pairs from a HeadersInit filled in under `guard`, as Fetch's "fill" does: an invalid name or
value is a TypeError (the Err message) and a guarded name is dropped. */
fn guarded_pairs(
    vm: &mut Vm,
    init: &JsValue,
    guard: Guard,
) -> Res<Result<Vec<(String, String)>, &'static str>> {
    let source = match super::host_data(vm, init) {
        Some(HostData::Headers(pairs)) => pairs,
        _ => match headers::init_pairs(vm, init)? {
            Ok(pairs) => pairs,
            Err(message) => return Ok(Err(message)),
        },
    };
    let mut pairs = Vec::new();
    for (name, value) in source {
        if let Err(headers::Refusal::Throw(message)) =
            headers::append(&mut pairs, guard, &name, &value)
        {
            return Ok(Err(message));
        }
    }
    Ok(Ok(pairs))
}

/** Header pairs passed again through `guard`, dropping any it refuses. */
fn refiltered(source: Vec<(String, String)>, guard: Guard) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (name, value) in source {
        let _ = headers::append(&mut pairs, guard, &name, &value);
    }
    pairs
}

/** The pairs of the Headers object a Request or Response handed out, which script may have
changed since. */
fn live_headers(vm: &Vm, owner: &JsValue) -> Option<Vec<(String, String)>> {
    let owner = owner.object()?;
    let held = headers::get_private(vm, owner, "headers")?;
    match super::host_data(vm, &held) {
        Some(HostData::Headers(pairs)) => Some(pairs),
        _ => None,
    }
}

/** A Request's or Response's one Headers object, made on first read, so `r.headers` is the same
object each time and changes to it stick. */
fn owned_headers(
    vm: &mut Vm,
    owner: &JsValue,
    pairs: Vec<(String, String)>,
    guard: Guard,
) -> Res<JsValue> {
    let Some(id) = owner.object() else {
        return headers_object(vm, pairs, guard);
    };
    if let Some(held) = headers::get_private(vm, id, "headers") {
        return Ok(held);
    }
    let made = headers_object(vm, pairs, guard)?;
    headers::put_private(vm, id, "headers", made.clone());
    Ok(made)
}

/** `new Headers(init)`. */
pub(in crate::v9) fn headers_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let init = call.arg(0);
    let pairs = if matches!(init, JsValue::Undefined) {
        Vec::new()
    } else {
        match guarded_pairs(vm, &init, Guard::None)? {
            Ok(pairs) => pairs,
            Err(message) => return Err(vm.type_error(message)),
        }
    };
    headers_object(vm, pairs, Guard::None)
}

/** A guarded write's outcome: a TypeError when refused loudly, nothing when dropped. */
fn written(vm: &mut Vm, outcome: Result<(), headers::Refusal>) -> Res<bool> {
    match outcome {
        Ok(()) => Ok(true),
        Err(headers::Refusal::Drop) => Ok(false),
        Err(headers::Refusal::Throw(message)) => Err(vm.type_error(message)),
    }
}

/** Headers methods. */
pub(in crate::v9) fn headers_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(HostData::Headers(mut pairs)), JsValue::Object(id)) =
        (super::host_data(vm, &call.this), call.this.clone())
    else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let guard = headers::guard(vm, id);
    let method = native_name(&call);
    let name = if matches!(method, "get" | "has" | "set" | "append" | "delete") {
        vm.to_text(&call.arg(0))?
    } else {
        String::new()
    };
    if matches!(method, "get" | "has") && !headers::valid_name(&name) {
        return Err(vm.type_error("Invalid header name."));
    }
    let lower = name.to_ascii_lowercase();
    let changed = match method {
        "get" => {
            return match headers::combined(&pairs, &lower) {
                Some(value) => JsValue::string(&value).map_err(Into::into),
                None => Ok(JsValue::Null),
            };
        }
        "has" => return Ok(JsValue::Boolean(pairs.iter().any(|(h, _)| *h == lower))),
        "getSetCookie" => {
            let mut values = Vec::new();
            for (_, value) in pairs.iter().filter(|(h, _)| h == "set-cookie") {
                values.push(JsValue::string(value)?);
            }
            return Ok(JsValue::Object(vm.new_array(values)?));
        }
        "set" | "append" => {
            let value = vm.to_text(&call.arg(1))?;
            let outcome = if method == "set" {
                headers::set(&mut pairs, guard, &name, &value)
            } else {
                headers::append(&mut pairs, guard, &name, &value)
            };
            written(vm, outcome)?
        }
        "delete" => {
            let outcome = headers::delete(&mut pairs, guard, &name);
            written(vm, outcome)?
        }
        "forEach" => {
            if !vm.is_callable(&call.arg(0)) {
                return Err(
                    vm.type_error("The callback provided as parameter 1 is not a function.")
                );
            }
            for (header, value) in headers::sort_and_combine(&pairs) {
                vm.call(
                    call.arg(0),
                    call.arg(1),
                    vec![
                        JsValue::string(&value)?,
                        JsValue::string(&header)?,
                        call.this.clone(),
                    ],
                )?;
            }
            return Ok(JsValue::Undefined);
        }
        kind => return pair_iterator(vm, &headers::sort_and_combine(&pairs), kind),
    };
    if changed && let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        **host = HostData::Headers(pairs);
    }
    Ok(JsValue::Undefined)
}

/** entries/keys/values over pairs, as an array iterator. */
fn pair_iterator(vm: &mut Vm, pairs: &[(String, String)], kind: &str) -> Res<JsValue> {
    let mut values = Vec::new();
    for (name, value) in pairs {
        values.push(match kind {
            "keys" => JsValue::string(name)?,
            "values" => JsValue::string(value)?,
            _ => {
                let pair = vec![JsValue::string(name)?, JsValue::string(value)?];
                JsValue::Object(vm.new_array(pair)?)
            }
        });
    }
    let array = vm.new_array(values)?;
    let method = vm.get(&JsValue::Object(array), &Key::from("values"))?;
    vm.call(method, JsValue::Object(array), Vec::new())
}

/** application/x-www-form-urlencoded serialization. */
pub(in crate::v9) fn encode_pairs(pairs: &[(String, String)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter())
        .finish()
}

/** `new URL(url, base)`. */
pub(in crate::v9) fn url_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let input = vm.to_text(&call.arg(0))?;
    let parsed = if call.arg(1).nullish() {
        url::Url::parse(&input)
    } else {
        let base = vm.to_text(&call.arg(1))?;
        url::Url::parse(&base).and_then(|base| base.join(&input))
    };
    let Ok(parsed) = parsed else {
        return Err(vm.type_error(&format!("Failed to construct 'URL': Invalid URL '{input}'")));
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Url(parsed.to_string()))),
        Some(realm().ids.url_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** The URL behind a URL object or `location`. */
fn this_url(vm: &mut Vm, call: &Call) -> Res<(Option<ObjectId>, url::Url)> {
    if call.this == JsValue::Object(realm().ids.location) {
        let href = vm.page.state.href.clone();
        return Ok((
            None,
            url::Url::parse(&href)
                .unwrap_or_else(|_| url::Url::parse("about:blank").expect("about:blank parses")),
        ));
    }
    if let (Some(HostData::Url(href)), JsValue::Object(id)) =
        (super::host_data(vm, &call.this), call.this.clone())
    {
        return Ok((
            Some(id),
            url::Url::parse(&href).map_err(|_| EngineError::invalid("Invalid V9 URL."))?,
        ));
    }
    Err(vm.type_error("Illegal invocation"))
}

/** URL and location component getters. */
pub(in crate::v9) fn url_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, url) = this_url(vm, &call)?;
    let text = match native_name(&call) {
        "href" | "toString" | "toJSON" => url.to_string(),
        "protocol" => format!("{}:", url.scheme()),
        "host" => match (url.host_str(), url.port()) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (Some(host), None) => host.to_string(),
            _ => String::new(),
        },
        "hostname" => url.host_str().unwrap_or("").to_string(),
        "port" => url.port().map(|port| port.to_string()).unwrap_or_default(),
        "pathname" => url.path().to_string(),
        "search" => url
            .query()
            .filter(|query| !query.is_empty())
            .map(|query| format!("?{query}"))
            .unwrap_or_default(),
        "hash" => url
            .fragment()
            .filter(|fragment| !fragment.is_empty())
            .map(|fragment| format!("#{fragment}"))
            .unwrap_or_default(),
        "origin" => url.origin().ascii_serialization(),
        "username" => url.username().to_string(),
        "password" => url.password().unwrap_or("").to_string(),
        "searchParams" => {
            let pairs: Vec<(String, String)> = url
                .query_pairs()
                .map(|(a, b)| (a.into_owned(), b.into_owned()))
                .collect();
            let id = vm.alloc(Object::new(
                Class::Host(Box::new(HostData::SearchParams(pairs))),
                Some(realm().ids.search_params_prototype),
            ))?;
            return Ok(JsValue::Object(id));
        }
        _ => String::new(),
    };
    JsValue::string(&text).map_err(Into::into)
}

/** URL and location component setters; location changes become navigation requests. */
pub(in crate::v9) fn url_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (owner, mut url) = this_url(vm, &call)?;
    let value = vm.to_text(&call.arg(0))?;
    let name = native_name(&call);
    match name {
        "href" => match resolve(vm, &value) {
            Some(href) => {
                url = url::Url::parse(&href).map_err(|_| EngineError::invalid("Invalid V9 URL."))?
            }
            None => return Err(vm.type_error(&format!("Invalid URL '{value}'"))),
        },
        "protocol" => {
            let _ = url.set_scheme(value.trim_end_matches(':'));
        }
        "username" => {
            let _ = url.set_username(&value);
        }
        "password" => {
            let _ = url.set_password(
                Some(&value)
                    .filter(|value| !value.is_empty())
                    .map(|v| v.as_str()),
            );
        }
        "host" => set_host_and_port(&mut url, &value),
        "hostname" => {
            let _ = url.set_host(Some(&value));
        }
        "port" => {
            let _ = url.set_port(value.parse().ok());
        }
        "pathname" => url.set_path(&value),
        "search" => {
            url.set_query(Some(value.trim_start_matches('?')).filter(|query| !query.is_empty()))
        }
        "hash" => url.set_fragment(
            Some(value.trim_start_matches('#')).filter(|fragment| !fragment.is_empty()),
        ),
        _ => {}
    }
    match owner {
        Some(id) => {
            if let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
                **host = HostData::Url(url.to_string());
            }
        }
        None => super::window::navigate(vm, "assign", url.to_string(), name == "hash")?,
    }
    Ok(JsValue::Undefined)
}

/** The `host` setter: a host, and the port after it when one is given (`example.com:81`),
as the URL Standard's host state reads it; an IPv6 literal's colons stay in the host. */
fn set_host_and_port(url: &mut url::Url, value: &str) {
    let split = value
        .rfind(':')
        .filter(|colon| !value[*colon..].contains(']'))
        .map(|colon| (&value[..colon], &value[colon + 1..]));
    match split {
        Some((host, port)) => {
            if url.set_host(Some(host)).is_ok() && !port.is_empty() {
                let _ = url.set_port(port.parse().ok());
            }
        }
        None => {
            let _ = url.set_host(Some(value));
        }
    }
}

/** URL statics that need blobs. */
pub(in crate::v9) fn url_object_url(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Err(vm.unsupported_error(&format!("URL.{}", native_name(&call))))
}

/** `new URLSearchParams(init)`. */
pub(in crate::v9) fn params_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let init = call.arg(0);
    let pairs = match &init {
        JsValue::Undefined | JsValue::Null => Vec::new(),
        JsValue::Object(_) if super::host_data(vm, &init).is_none() => header_list(vm, &init)?,
        other => {
            let text = vm.to_text(other)?;
            url::form_urlencoded::parse(text.trim_start_matches('?').as_bytes())
                .map(|(a, b)| (a.into_owned(), b.into_owned()))
                .collect()
        }
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::SearchParams(pairs))),
        Some(realm().ids.search_params_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** URLSearchParams methods. */
pub(in crate::v9) fn params_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(HostData::SearchParams(mut pairs)), JsValue::Object(id)) =
        (super::host_data(vm, &call.this), call.this.clone())
    else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let name = if call.args.is_empty() {
        String::new()
    } else {
        vm.to_text(&call.arg(0))?
    };
    let result = match native_name(&call) {
        "get" => {
            return match pairs.iter().find(|(key, _)| *key == name) {
                Some((_, value)) => JsValue::string(value).map_err(Into::into),
                None => Ok(JsValue::Null),
            };
        }
        "getAll" => {
            let values: Vec<JsValue> = pairs
                .iter()
                .filter(|(key, _)| *key == name)
                .filter_map(|(_, value)| JsValue::string(value).ok())
                .collect();
            return Ok(JsValue::Object(vm.new_array(values)?));
        }
        "has" => return Ok(JsValue::Boolean(pairs.iter().any(|(key, _)| *key == name))),
        "toString" => return JsValue::string(&encode_pairs(&pairs)).map_err(Into::into),
        "size" => return Ok(JsValue::Number(pairs.len() as f64)),
        "set" => {
            let value = vm.to_text(&call.arg(1))?;
            match pairs.iter().position(|(key, _)| *key == name) {
                Some(position) => {
                    pairs[position].1 = value;
                    let mut seen = false;
                    pairs.retain(|(key, _)| *key != name || !std::mem::replace(&mut seen, true));
                }
                None => pairs.push((name, value)),
            }
            JsValue::Undefined
        }
        "append" => {
            let value = vm.to_text(&call.arg(1))?;
            pairs.push((name, value));
            JsValue::Undefined
        }
        "delete" => {
            pairs.retain(|(key, _)| *key != name);
            JsValue::Undefined
        }
        "sort" => {
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            JsValue::Undefined
        }
        "forEach" => {
            for (key, value) in pairs.clone() {
                vm.call(
                    call.arg(0),
                    call.arg(1),
                    vec![
                        JsValue::string(&value)?,
                        JsValue::string(&key)?,
                        call.this.clone(),
                    ],
                )?;
            }
            return Ok(JsValue::Undefined);
        }
        kind => return pair_iterator(vm, &pairs, kind),
    };
    if let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        **host = HostData::SearchParams(pairs);
    }
    Ok(result)
}
