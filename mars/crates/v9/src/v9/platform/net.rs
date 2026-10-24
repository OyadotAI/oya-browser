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
use serde::{Deserialize, Serialize};

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
    if request
        .body
        .as_ref()
        .is_some_and(|body| body.len() > VM_REQUEST_BODY_BYTES)
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

/** A request body as text. */
fn body_text(vm: &mut Vm, value: &JsValue) -> Res<Option<String>> {
    if value.nullish() {
        return Ok(None);
    }
    if let Some(HostData::SearchParams(pairs)) = super::host_data(vm, value) {
        return Ok(Some(encode_pairs(&pairs)));
    }
    if matches!(value, JsValue::Object(_))
        && super::host_data(vm, value).is_none()
        && !matches!(
            vm.heap.get(value.object().unwrap_or(0)).map(|o| &o.class),
            Some(Class::String(_))
        )
    {
        return Err(vm.unsupported_error("request bodies other than strings and URLSearchParams"));
    }
    Ok(Some(vm.to_text(value)?))
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
    )?
    .to_ascii_uppercase();
    let headers = read(vm, "headers")?;
    let mut headers = if headers.nullish() {
        base.as_ref().map(|r| r.headers.clone()).unwrap_or_default()
    } else {
        header_list(vm, &headers)?
    };
    let body = read(vm, "body")?;
    let body = if body.nullish() {
        base.as_ref().and_then(|r| r.body.clone())
    } else {
        form_type(vm, &body, &mut headers);
        body_text(vm, &body)?
    };
    let credentials = read(vm, "credentials")?;
    let credentials = text(
        vm,
        credentials,
        base.as_ref()
            .map_or("same-origin".into(), |r| r.credentials.clone()),
    )?;
    let mode = read(vm, "mode")?;
    let mode = text(
        vm,
        mode,
        base.as_ref().map_or("cors".into(), |r| r.mode.clone()),
    )?;
    Ok(Ok(HostRequest {
        id: 0,
        kind: "fetch".into(),
        method,
        url,
        headers,
        body,
        credentials,
        mode,
    }))
}

/** `new Request(input, init)`: the request fetch would send, kept for `fetch(request)`. */
pub(in crate::v9) fn request_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
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
        "headers" => headers_object(vm, request.headers.clone()),
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
        "clone" => request_object(vm, *request),
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
    let body = body_text(vm, &call.arg(1))?;
    let request = HostRequest {
        id: 0,
        kind: "beacon".into(),
        method: "POST".into(),
        url,
        headers: Vec::new(),
        body,
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
            let method = vm.to_text(&call.arg(0))?.to_ascii_uppercase();
            let target = vm.to_text(&call.arg(1))?;
            if call.args.len() > 2 && !call.arg(2).truthy() {
                return Err(vm.unsupported_error("synchronous XMLHttpRequest"));
            }
            let Some(url) = resolve(vm, &target) else {
                return Err(
                    vm.syntax_error("Failed to execute 'open' on 'XMLHttpRequest': Invalid URL")
                );
            };
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
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let value = vm.to_text(&call.arg(1))?;
            if let Some(state) = vm.page.state.xhrs.get_mut(&id) {
                state.headers.push((name, value));
            }
        }
        "send" => {
            let body = body_text(vm, &call.arg(0))?;
            let Some(mut state) = vm.page.state.xhrs.get(&id).cloned() else {
                return Ok(JsValue::Undefined);
            };
            form_type(vm, &call.arg(0), &mut state.headers);
            if state.ready != 1 || state.sent {
                return Err(vm.throw(
                    crate::v9::vm::error::ErrorKind::Error,
                    "InvalidStateError: The object's state must be OPENED.",
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
                body,
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
            let values: Vec<String> = state
                .response_headers
                .iter()
                .filter(|(header, _)| *header == name)
                .map(|(_, value)| value.clone())
                .collect();
            return if values.is_empty() {
                Ok(JsValue::Null)
            } else {
                JsValue::string(&values.join(", ")).map_err(Into::into)
            };
        }
        "getAllResponseHeaders" => {
            let state = vm.page.state.xhrs.get(&id).cloned().unwrap_or_default();
            let text: String = state
                .response_headers
                .iter()
                .map(|(name, value)| format!("{name}: {value}\r\n"))
                .collect();
            return JsValue::string(&text).map_err(Into::into);
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
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
        "type" => JsValue::string(response_type(vm, &data))?,
        "bodyUsed" => JsValue::Boolean(data.used),
        "headers" => headers_object(vm, data.headers)?,
        _ => JsValue::Undefined,
    })
}

/// What kind of response the app handed back: an opaque one has status 0 and no URL, a CORS
/// one ended on another origin, and anything else is basic.
// ponytail: a CORS response redirected back to the page's own origin reads as "basic"; carry the
// tainting on HostResponse if a page ever depends on it.
fn response_type(vm: &Vm, data: &ResponseData) -> &'static str {
    let page = parse_url(&vm.page.state.href).map(|url| url.origin());
    let own = parse_url(&data.url).map(|url| url.origin());
    if data.status == 0 && data.url.is_empty() {
        "opaque"
    } else if own.is_some() && own != page {
        "cors"
    } else {
        "basic"
    }
}

/** Response body methods. */
pub(in crate::v9) fn response_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, data) = this_response(vm, &call)?;
    let promise = vm.new_promise()?;
    let name = native_name(&call);
    if name == "clone" {
        let copy = vm.alloc(Object::new(
            Class::Host(Box::new(HostData::Response(Box::new(data)))),
            Some(realm().ids.response_prototype),
        ))?;
        return Ok(JsValue::Object(copy));
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

/** A Headers object over pairs. */
fn headers_object(vm: &mut Vm, pairs: Vec<(String, String)>) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Headers(pairs))),
        Some(realm().ids.headers_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** `new Headers(init)`. */
pub(in crate::v9) fn headers_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let pairs = header_list(vm, &call.arg(0))?;
    headers_object(vm, pairs)
}

/** Headers methods. */
pub(in crate::v9) fn headers_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(HostData::Headers(mut pairs)), JsValue::Object(id)) =
        (super::host_data(vm, &call.this), call.this.clone())
    else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let name = if call.args.is_empty() {
        String::new()
    } else {
        vm.to_text(&call.arg(0))?.to_ascii_lowercase()
    };
    let result = match native_name(&call) {
        "get" => {
            let values: Vec<String> = pairs
                .iter()
                .filter(|(header, _)| *header == name)
                .map(|(_, value)| value.clone())
                .collect();
            return if values.is_empty() {
                Ok(JsValue::Null)
            } else {
                JsValue::string(&values.join(", ")).map_err(Into::into)
            };
        }
        "has" => {
            return Ok(JsValue::Boolean(
                pairs.iter().any(|(header, _)| *header == name),
            ));
        }
        "set" => {
            let value = vm.to_text(&call.arg(1))?;
            pairs.retain(|(header, _)| *header != name);
            pairs.push((name, value));
            JsValue::Undefined
        }
        "append" => {
            let value = vm.to_text(&call.arg(1))?;
            pairs.push((name, value));
            JsValue::Undefined
        }
        "delete" => {
            pairs.retain(|(header, _)| *header != name);
            JsValue::Undefined
        }
        "forEach" => {
            for (header, value) in pairs.clone() {
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
        kind => return pair_iterator(vm, &pairs, kind),
    };
    if let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|o| &mut o.class) {
        **host = HostData::Headers(pairs);
    }
    Ok(result)
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
        "host" => {
            let _ = url.set_host(Some(&value));
        }
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
