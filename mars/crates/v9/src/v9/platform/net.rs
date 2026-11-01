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

pub(super) mod abort;
mod blob_fetch;
mod body;
mod headers;
mod response_body;
use body::Body;
/** Saved fetch state validates body storage, captured sources and native cancellation identities together. */
pub(in crate::v9) fn validate_state(
    page: &super::PageState,
    heap: &crate::v9::vm::heap::Heap,
) -> crate::protocol::EngineResult<()> {
    response_body::validate(heap)?;
    blob_fetch::validate(page, heap)?;
    abort::validate(page, heap)
}

/** Resident summaries retain pending ownership and capture budgets without duplicating service heap objects. */
pub(in crate::v9) fn validate_summary(
    page: &super::PageState,
) -> crate::protocol::EngineResult<()> {
    blob_fetch::validate_summary(page)?;
    abort::validate_summary(page)
}

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
    /// Opened with async false: send() completes from a recorded answer, not an event loop turn.
    #[serde(default)]
    pub sync: bool,
    /// Bumped by open() and abort(): a delivery whose handlers reopened or aborted the request
    /// stops firing the old request's events (XHR's "terminate the fetch").
    pub generation: u32,
}

/** Record a request, failing like a network error past the pending bound. */
fn record(
    vm: &mut Vm,
    mut request: HostRequest,
    owner: ObjectId,
    xhr: bool,
    signal: Option<ObjectId>,
) -> Res<bool> {
    if vm.page.state.pending.len() >= VM_HOST_REQUESTS {
        return Ok(false);
    }
    if !payload_admitted(&vm.page.state, &request) {
        return Ok(false);
    }
    let next = vm
        .page
        .state
        .next_request
        .checked_add(1)
        .ok_or_else(|| EngineError::invalid("Host request identities exhausted."))?;
    vm.page.state.next_request = next;
    request.id = next;
    if request.kind != "beacon" {
        vm.page.state.pending.push(Pending {
            id: request.id,
            owner,
            xhr,
            platform: request.kind == super::host_bridge::KIND,
            signal,
            blob_bytes: blob_fetch::charge(&request),
            indexed: false,
        });
    }
    vm.page.effects.requests.push(request);
    Ok(true)
}

/** Record an IndexedDB coordinator request whose answer goes to `callback`; false when the host queue is full. */
pub(in crate::v9) fn record_indexed(
    vm: &mut Vm,
    request: HostRequest,
    callback: ObjectId,
) -> Res<bool> {
    if !record(vm, request, callback, false, None)? {
        return Ok(false);
    }
    if let Some(pending) = vm.page.state.pending.last_mut() {
        pending.indexed = true;
    }
    Ok(true)
}

/** The bytes of a BufferSource, Blob or string argument (empty for undefined), for bridge payloads. */
pub(super) fn body_bytes(vm: &mut Vm, value: &JsValue) -> Res<Vec<u8>> {
    let body = body::extract(vm, value)?;
    Ok(match body.text {
        Some(text) if body.bytes.is_empty() => text.into_bytes(),
        _ => body.bytes,
    })
}

/** Record a platform bridge request whose answer settles `promise`; false past the bound. */
pub(super) fn record_platform(vm: &mut Vm, request: HostRequest, promise: ObjectId) -> Res<bool> {
    record(vm, request, promise, false, None)
}

/** fetch(input, init). */
pub(in crate::v9) fn fetch(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    match start_fetch(vm, &call, promise) {
        Ok(()) => {}
        Err(Thrown::Value(reason)) => vm.settle(promise, PromiseState::Rejected(reason))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(promise))
}

/** Conversion, signal admission and source capture finish before any host request becomes visible. */
fn start_fetch(vm: &mut Vm, call: &Call, promise: ObjectId) -> Res<()> {
    let mut request = match request_from(vm, &call.arg(0), &call.arg(1))? {
        Ok(request) => request,
        Err(message) => {
            reject(vm, promise, &message)?;
            return Ok(());
        }
    };
    let signal = abort::for_request(vm, &call.arg(0), &call.arg(1))?;
    if let Some(reason) = abort::reason(vm, signal) {
        return vm.settle(promise, PromiseState::Rejected(reason));
    }
    if let Err(message) = blob_fetch::capture(vm, &mut request) {
        return reject(vm, promise, &message);
    }
    if !record(vm, request, promise, false, Some(signal))? {
        reject(vm, promise, "Failed to fetch")?;
    }
    Ok(())
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
            kind: None,
        })
    } else {
        if matches!(method.as_str(), "GET" | "HEAD") {
            return Ok(Err("Request with GET/HEAD method cannot have body.".into()));
        }
        let body = body::extract(vm, &body)?;
        body::label(&mut headers, body.kind.as_deref());
        body
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
        blob_source: None,
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
const INIT_ENUMS: [(&str, &[&str]); 6] = [
    ("duplex", &["half"]),
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
    let signal = abort::for_request(vm, &call.arg(0), &call.arg(1))?;
    request_object(vm, request, signal)
}

/** A Request object over its data. */
fn request_object(vm: &mut Vm, request: HostRequest, signal: ObjectId) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Request(Box::new(request)))),
        Some(realm().ids.request_prototype),
    ))?;
    abort::bind(vm, id, signal);
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
            abort::request_signal(vm, call.this.object().expect("Request receiver checked"))
        }
        "clone" => {
            let mut copy = *request;
            if let Some(pairs) = live_headers(vm, &call.this) {
                copy.headers = pairs;
            }
            let signal = abort::for_request(vm, &call.this, &JsValue::Undefined)?;
            request_object(vm, copy, signal)
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
        name => {
            let mut bytes = request.body_bytes.clone();
            if let Some(text) = &request.body {
                bytes = text.clone().into_bytes();
            }
            let headers = live_headers(vm, &call.this).unwrap_or(request.headers.clone());
            match body::consume(vm, name, bytes, &headers) {
                Some(outcome) => settled(vm, outcome),
                None => Err(vm.unsupported_error(&format!("Request.{name}"))),
            }
        }
    }
}

/** A promise settled by a body reader's outcome: fulfilled, or rejected with what it threw. */
fn settled(vm: &mut Vm, outcome: Res<JsValue>) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    match outcome {
        Ok(value) => vm.resolve_promise(promise, value)?,
        Err(Thrown::Value(error)) => vm.settle(promise, PromiseState::Rejected(error))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(promise))
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
    let body = body::extract(vm, &call.arg(1))?;
    let mut headers = Vec::new();
    body::label(&mut headers, body.kind.as_deref());
    let request = HostRequest {
        id: 0,
        kind: "beacon".into(),
        method: "POST".into(),
        url,
        headers,
        body: body.text,
        body_bytes: body.bytes,
        blob_source: None,
        credentials: "include".into(),
        mode: "no-cors".into(),
    };
    Ok(JsValue::Boolean(record(
        vm,
        request,
        crate::v9::vm::GLOBAL,
        false,
        None,
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
            let limit = if pending.indexed {
                super::indexed_db::INDEXED_DB_RESPONSE_BYTES
            } else {
                VM_RESPONSE_BYTES
            };
            if response
                .body
                .len()
                .saturating_add(response.body_bytes.len())
                > limit
            {
                return Err(EngineError::limit("response body bytes", limit).into());
            }
            if !pending.platform && !response.body.is_empty() && !response.body_bytes.is_empty() {
                return Err(EngineError::invalid("Ambiguous response body representation.").into());
            }
            let result = if pending.platform {
                super::host_bridge::deliver(self, pending.owner, response)
            } else if pending.indexed {
                self.deliver_indexed(pending.owner, response)
            } else if pending.xhr {
                let mut response = response;
                if !response.body_bytes.is_empty() {
                    response.body = response_body::text(&response.body_bytes);
                }
                self.deliver_xhr(pending.owner, response)
            } else {
                self.deliver_fetch(pending.owner, pending.signal, response)
            };
            match result {
                Ok(()) => {}
                Err(Thrown::Value(error)) => super::workers::report_exception(self, &error)?,
                Err(fatal) => return Err(fatal),
            }
            self.drain_jobs()?;
        }
        Ok(())
    }
    /** Hand an IndexedDB answer to its callback: the text, or undefined when the coordinator failed. */
    fn deliver_indexed(&mut self, callback: ObjectId, response: HostResponse) -> Res<()> {
        let answer = match response.error {
            Some(_) => JsValue::Undefined,
            None => JsValue::string(&response.body)?,
        };
        self.call(JsValue::Object(callback), JsValue::Undefined, vec![answer])
            .map(|_| ())
    }
    /** Settle a fetch promise. */
    fn deliver_fetch(
        &mut self,
        promise: ObjectId,
        signal: Option<ObjectId>,
        response: HostResponse,
    ) -> Res<()> {
        if let Some(reason) = signal.and_then(|signal| abort::reason(self, signal)) {
            return self.settle(promise, PromiseState::Rejected(reason));
        }
        if let Some(error) = response.error {
            let _ = error;
            return reject(self, promise, "Failed to fetch");
        }
        let data = ResponseData {
            status: response.status,
            status_text: status_text(response.status, response.status_text),
            url: response.url,
            headers: response.headers,
            body: response.body,
            used: false,
        };
        let object = self.alloc(Object::new(
            Class::Host(Box::new(HostData::Response(Box::new(data)))),
            Some(realm().ids.response_prototype),
        ))?;
        response_body::store(self, object, response.body_bytes)?;
        if let Some(signal) = signal {
            abort::bind(self, object, signal);
        }
        self.resolve_promise(promise, JsValue::Object(object))
    }
    /** Complete an XMLHttpRequest the way XHR's "process response" steps do: HEADERS_RECEIVED,
    then LOADING with one progress event, then DONE with load and loadend. A network error goes
    straight to DONE with error and loadend and no response. A handler that calls open() or
    abort() ends the old request's events at once. */
    fn deliver_xhr(&mut self, xhr: ObjectId, response: HostResponse) -> Res<()> {
        let Some(generation) = self.page.state.xhrs.get(&xhr).map(|s| s.generation) else {
            return Ok(());
        };
        if response.error.is_some() {
            return xhr_request_error(self, xhr, "error");
        }
        let total = response.body.len();
        if let Some(state) = self.page.state.xhrs.get_mut(&xhr) {
            state.ready = XHR_HEADERS_RECEIVED;
            state.status = response.status;
            state.status_text = status_text(response.status, response.status_text);
            state.response_headers = response.headers;
            state.response_url = response.url;
        }
        let current =
            |vm: &Vm| vm.page.state.xhrs.get(&xhr).map(|s| s.generation) == Some(generation);
        fire(self, Target::Object(xhr), "readystatechange", false, false)?;
        if !current(self) {
            return Ok(());
        }
        if let Some(state) = self.page.state.xhrs.get_mut(&xhr) {
            state.ready = XHR_LOADING;
            state.response = response.body;
        }
        fire(self, Target::Object(xhr), "readystatechange", false, false)?;
        if !current(self) {
            return Ok(());
        }
        fire_progress(self, xhr, "progress", total, total)?;
        if !current(self) {
            return Ok(());
        }
        if let Some(state) = self.page.state.xhrs.get_mut(&xhr) {
            state.ready = XHR_DONE;
            state.sent = false;
        }
        fire(self, Target::Object(xhr), "readystatechange", false, false)?;
        if !current(self) {
            return Ok(());
        }
        fire_progress(self, xhr, "load", total, total)?;
        if current(self) {
            fire_progress(self, xhr, "loadend", total, total)?;
        }
        Ok(())
    }
}

/** The host's reason phrase, or Chromium's canonical one when the host passed none.
ponytail: the coordinator drops the server's reason phrase; carry it through WebResponse and
this fallback becomes the HTTP/2 path only. */
fn status_text(status: u16, given: String) -> String {
    if given.is_empty() && status != 0 {
        return reason_phrase_for(status).into();
    }
    given
}

/** Whether a request's payload fits the bounds every send enforces: a body within VM_REQUEST_BODY_BYTES
and Blob snapshots within the page's budget. A refused payload fails its request like a network error
(the request must not stop the page), so sync and async sends share this check. */
fn payload_admitted(state: &super::PageState, request: &HostRequest) -> bool {
    let body = request.body.as_ref().map_or(0, String::len) + request.body_bytes.len();
    body <= VM_REQUEST_BODY_BYTES && blob_fetch::admits(state, request)
}

/** The answer for this step's next synchronous send, when it answers this same request. Answers
are kept in the order the step reached them, so each send consumes the answer at its own turn: a
page that sends one URL twice gets two requests, never the first answer twice. */
fn sync_answer(vm: &mut Vm, request: &HostRequest) -> Option<HostResponse> {
    let turn = vm.page.effects.sync_turn;
    vm.page.effects.sync_turn += 1;
    let answer = vm.page.environment.sync_answers.get(turn)?;
    (answer.request == *request).then(|| answer.response.clone())
}

/** A synchronous XMLHttpRequest. When the step's answers hold this request, it completes now, as
a blocked send would have. Otherwise the script stops here (uncatchably) and the request goes to
the effects: the app answers it and runs the step again, so the script resumes with the answer in
hand. A step that runs out of answers reports the feature as unsupported, its earlier behavior. */
fn sync_send(vm: &mut Vm, id: ObjectId, request: HostRequest) -> Res<()> {
    if !payload_admitted(&vm.page.state, &request) {
        return xhr_request_error(vm, id, "error");
    }
    let Some(response) = sync_answer(vm, &request) else {
        vm.page.effects.sync_requests.push(request);
        return Err(vm.unsupported_error("synchronous XMLHttpRequest"));
    };
    if let Some(message) = &response.error {
        return Err(vm.dom_exception(
            "NetworkError",
            &format!("Failed to execute 'send' on 'XMLHttpRequest': {message}."),
        ));
    }
    deliver_sync(vm, id, response)
}

/** Delivers a synchronous answer's body, bounded as an async answer is, then its events. */
fn deliver_sync(vm: &mut Vm, id: ObjectId, mut response: HostResponse) -> Res<()> {
    if !response.body_bytes.is_empty() {
        response.body = response_body::text(&response.body_bytes);
    }
    if response.body.len() > VM_RESPONSE_BYTES {
        return Err(EngineError::limit("response body bytes", VM_RESPONSE_BYTES).into());
    }
    vm.deliver_xhr(id, response)
}

/** XHR's "request error steps": DONE, no response, readystatechange, then `kind` (error,
abort or timeout) and loadend as ProgressEvents of zero bytes. */
fn xhr_request_error(vm: &mut Vm, xhr: ObjectId, kind: &str) -> Res<()> {
    let Some(state) = vm.page.state.xhrs.get_mut(&xhr) else {
        return Ok(());
    };
    *state = XhrState {
        method: std::mem::take(&mut state.method),
        url: std::mem::take(&mut state.url),
        headers: std::mem::take(&mut state.headers),
        response_type: std::mem::take(&mut state.response_type),
        with_credentials: state.with_credentials,
        generation: state.generation,
        ready: XHR_DONE,
        ..XhrState::default()
    };
    let generation = state.generation;
    fire(vm, Target::Object(xhr), "readystatechange", false, false)?;
    if vm.page.state.xhrs.get(&xhr).map(|s| s.generation) != Some(generation) {
        return Ok(());
    }
    fire_progress(vm, xhr, kind, 0, 0)?;
    fire_progress(vm, xhr, "loadend", 0, 0)
}

/** Fire a ProgressEvent (an Event with ProgressEvent.prototype and loaded, total and
lengthComputable) at an XMLHttpRequest. */
fn fire_progress(vm: &mut Vm, xhr: ObjectId, kind: &str, loaded: usize, total: usize) -> Res<()> {
    let event = super::events::create_event(vm, kind, false, false)?;
    let Some(id) = event.object() else {
        return Ok(());
    };
    let global = JsValue::Object(crate::v9::vm::GLOBAL);
    let ctor = vm.get(&global, &Key::from("ProgressEvent"))?;
    if matches!(ctor, JsValue::Object(_))
        && let JsValue::Object(proto) = vm.get(&ctor, &Key::prototype())?
    {
        vm.set_proto(id, Some(proto))?;
    }
    vm.put(id, "lengthComputable", JsValue::Boolean(total > 0))?;
    vm.put(id, "loaded", JsValue::Number(loaded as f64))?;
    vm.put(id, "total", JsValue::Number(total as f64))?;
    super::events::dispatch(vm, Target::Object(xhr), id)?;
    Ok(())
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
            let asynchronous = call.args.len() <= 2 || call.arg(2).truthy();
            // Terminate any ongoing fetch, then fire readystatechange only when the state was
            // not OPENED already (XHR open() steps 13 and 14).
            vm.page.state.pending.retain(|pending| pending.owner != id);
            let mut was_opened = false;
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                was_opened = state.ready == 1;
                *state = XhrState {
                    method,
                    url,
                    ready: 1,
                    response_type: state.response_type.clone(),
                    with_credentials: state.with_credentials,
                    generation: state.generation.wrapping_add(1),
                    sync: !asynchronous,
                    ..XhrState::default()
                };
            }
            if !was_opened {
                fire(vm, Target::Object(id), "readystatechange", false, false)?;
            }
        }
        "setRequestHeader" => {
            let name = vm.to_text(&call.arg(0))?;
            let value = vm.to_text(&call.arg(1))?;
            xhr_set_header(vm, id, &name, &value)?;
        }
        "send" => {
            let Some(mut state) = vm.page.state.xhrs.get(&id).cloned() else {
                return Ok(JsValue::Undefined);
            };
            if state.ready != 1 || state.sent {
                return Err(vm.dom_exception(
                    "InvalidStateError",
                    "Failed to execute 'send' on 'XMLHttpRequest': The object's state must be OPENED.",
                ));
            }
            // XHR send() steps 3 and 4: GET and HEAD send no body; any other extracts one.
            let body = if matches!(state.method.as_str(), "GET" | "HEAD") {
                Body::default()
            } else {
                body::extract(vm, &call.arg(0))?
            };
            body::label(&mut state.headers, body.kind.as_deref());
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                state.sent = true;
            }
            let credentials = if state.with_credentials {
                "include"
            } else {
                "same-origin"
            };
            let sync = state.sync;
            let request = HostRequest {
                id: 0,
                kind: "xhr".into(),
                method: state.method,
                url: state.url,
                headers: state.headers,
                body: body.text,
                body_bytes: body.bytes,
                blob_source: None,
                credentials: credentials.into(),
                mode: "cors".into(),
            };
            if sync {
                return sync_send(vm, id, request).map(|()| JsValue::Undefined);
            }
            let generation = state.generation;
            if !record(vm, request, id, true, None)? {
                return xhr_request_error(vm, id, "error").map(|()| JsValue::Undefined);
            }
            fire_progress(vm, id, "loadstart", 0, 0)?;
            // A loadstart handler that reopened or aborted has already ended this request.
            let _ = generation;
        }
        "abort" => xhr_abort(vm, id)?,
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
        "overrideMimeType" => {
            // XHR overrideMimeType(): an InvalidStateError once LOADING or DONE; the override
            // itself only changes decoding, and Relay already hands over decoded text.
            let ready = vm.page.state.xhrs.get(&id).map_or(0, |s| s.ready);
            if ready == XHR_LOADING || ready == XHR_DONE {
                return Err(vm.dom_exception(
                    "InvalidStateError",
                    "Failed to execute 'overrideMimeType' on 'XMLHttpRequest': MimeType cannot be overridden when the state is LOADING or DONE.",
                ));
            }
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
/** XMLHttpRequest's LOADING state: the body is arriving and responseText is readable. */
const XHR_LOADING: u8 = 3;
/** XMLHttpRequest's DONE state. */
const XHR_DONE: u8 = 4;
/** XMLHttpResponseType's values; any other assignment is ignored. */
const XHR_RESPONSE_TYPES: [&str; 6] = ["", "arraybuffer", "blob", "document", "json", "text"];

/** Chromium's GetHttpReasonPhrase (net/http/http_status_code_list.h) for the codes pages
meet: the statusText a response gets when the host passes no reason phrase, as Chromium
synthesizes one for HTTP/2 and HTTP/3. */
fn reason_phrase_for(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        _ => "",
    }
}

/** `abort()` (XHR standard): an in-flight request (OPENED and sent, HEADERS_RECEIVED or
LOADING) runs the request error steps with `abort`; then a DONE request returns to UNSENT
without another readystatechange. An unsent or finished request fires nothing. */
fn xhr_abort(vm: &mut Vm, id: ObjectId) -> Res<()> {
    vm.page.state.pending.retain(|pending| pending.owner != id);
    let Some(state) = vm.page.state.xhrs.get_mut(&id) else {
        return Ok(());
    };
    let in_flight = (state.ready == 1 && state.sent)
        || state.ready == XHR_HEADERS_RECEIVED
        || state.ready == XHR_LOADING;
    state.generation = state.generation.wrapping_add(1);
    let generation = state.generation;
    if in_flight {
        xhr_request_error(vm, id, "abort")?;
    }
    if let Some(state) = vm.page.state.xhrs.get_mut(&id)
        && state.generation == generation
        && state.ready == XHR_DONE
    {
        state.ready = 0;
        state.status = 0;
        state.status_text.clear();
        state.response.clear();
        state.response_headers.clear();
    }
    Ok(())
}

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
        "responseText" => {
            if !matches!(state.response_type.as_str(), "" | "text") {
                return Err(vm.dom_exception(
                    "InvalidStateError",
                    "Failed to read the 'responseText' property from 'XMLHttpRequest': The value is only accessible if the object's 'responseType' is '' or 'text'.",
                ));
            }
            JsValue::string(&state.response)?
        }
        "responseURL" => JsValue::string(&state.response_url)?,
        "responseType" => JsValue::string(&state.response_type)?,
        "withCredentials" => JsValue::Boolean(state.with_credentials),
        "response" => {
            let text = matches!(state.response_type.as_str(), "" | "text");
            if text {
                return Ok(JsValue::string(&state.response)?);
            }
            if state.ready != XHR_DONE {
                return Ok(JsValue::Null);
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
            // XHR: an unknown value is ignored; LOADING or DONE is an InvalidStateError.
            let text = vm.to_text(&value)?;
            if !XHR_RESPONSE_TYPES.contains(&text.as_str()) {
                return Ok(JsValue::Undefined);
            }
            let ready = vm.page.state.xhrs.get(&id).map_or(0, |s| s.ready);
            if ready == XHR_LOADING || ready == XHR_DONE {
                return Err(vm.dom_exception(
                    "InvalidStateError",
                    "Failed to set the 'responseType' property on 'XMLHttpRequest': The response type cannot be set if the object's state is LOADING or DONE.",
                ));
            }
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
        return char::decode_utf16(kind.code_units())
            .map(|point| point.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
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
    let read = response_body_init(vm, body, status, &mut pairs, label)?;
    let data = ResponseData {
        status,
        status_text,
        url: String::new(),
        headers: pairs,
        body: read.text.unwrap_or_default(),
        used: false,
    };
    if data.body.len().saturating_add(read.bytes.len()) > VM_RESPONSE_BYTES {
        return Err(EngineError::limit("response body bytes", VM_RESPONSE_BYTES).into());
    }
    let response = built_response(vm, data, "default", true)?;
    response_body::store(vm, response.object().expect("Response object"), read.bytes)?;
    Ok(response)
}

/** Extract a constructed response without losing binary bytes or the body's MIME metadata. */
fn response_body_init(
    vm: &mut Vm,
    body: &JsValue,
    status: u16,
    pairs: &mut Vec<(String, String)>,
    label: &str,
) -> Res<Body> {
    if body.nullish() {
        return Ok(Body::default());
    }
    if NULL_BODY_STATUSES.contains(&status) {
        return Err(vm.type_error("Response with null body status cannot have body"));
    }
    let read = body::extract(vm, body)?;
    let kind = match body {
        JsValue::String(_) => Some(label),
        _ => read.kind.as_deref(),
    };
    body::label(pairs, kind);
    Ok(read)
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
        for key in [
            "responseType",
            "responseGuard",
            response_body::KEY,
            abort::SIGNAL,
        ] {
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
    if let Some(reason) = abort::body_reason(vm, id) {
        vm.settle(promise, PromiseState::Rejected(reason))?;
        return Ok(JsValue::Object(promise));
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
    let bytes = response_body::bytes(vm, id, &data)?;
    let outcome = match name {
        "text" => JsString::from_utf8(&response_body::text(&bytes))
            .map(JsValue::String)
            .map_err(Thrown::from),
        "json" => crate::v9::builtins::json_parse_text(vm, &response_body::text(&bytes)),
        _ => {
            let headers = live_headers(vm, &call.this).unwrap_or(data.headers.clone());
            body::consume(vm, name, bytes, &headers)
                .unwrap_or_else(|| Err(vm.unsupported_error(&format!("Response.{name}"))))
        }
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

/** URL statics that need blobs. */
pub(in crate::v9) fn url_object_url(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match native_name(&call) {
        "createObjectURL" => super::blob_urls::create(vm, call),
        "revokeObjectURL" => super::blob_urls::revoke(vm, call),
        _ => Err(vm.unsupported_error("Unknown object URL operation")),
    }
}
