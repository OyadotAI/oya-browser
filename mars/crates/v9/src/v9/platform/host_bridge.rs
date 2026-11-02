/**
 * The private bridge between the platform bootstrap (the `platform_apis` scripts) and the coordinator.
 * The bootstrap captures `__marsPlatformHost` and deletes it before any page script runs, so a
 * page never reaches it. `call(op, json)` queues one "platform" HostRequest and returns a promise
 * of the coordinator's JSON answer; the request carries the user-activation facts V9 alone
 * knows, so a page cannot claim an activation it never had.
 */
use super::dom::native_name;
use super::{HostRequest, HostResponse, PageState};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Bytes, Class, PromiseState};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** How long one trusted input keeps the page transiently activated, as HTML's "transient
activation duration" (Chrome uses five seconds). Clipboard writes and storage-access requests
need it. */
const TRANSIENT_ACTIVATION_MS: f64 = 5_000.0;
/** The global the bootstrap takes the bridge from and then deletes. */
pub(in crate::v9) const GLOBAL: &str = "__marsPlatformHost";
/** The request kind the coordinator dispatches to its platform handlers. */
pub(in crate::v9) const KIND: &str = "platform";
/** The most bytes of one bridge payload, so a page cannot fill the request queue through it. */
const PAYLOAD_BYTES: usize = crate::constants::VM_REQUEST_BODY_BYTES;

/** The bridge object's methods: call, activation, consume and context. */
pub(in crate::v9) fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match native_name(&call) {
        "call" => host_call(vm, &call),
        "activation" => activation(vm),
        "consume" => {
            vm.page.state.activation = None;
            Ok(JsValue::Undefined)
        }
        "context" => JsValue::string(context(vm)).map_err(Into::into),
        "lazy" => {
            vm.page.state.platform_installer = call.arg(0).object();
            Ok(JsValue::Undefined)
        }
        "state" => match vm.page.environment.service_worker_state.clone() {
            Some(state) => JsValue::string(&state).map_err(Into::into),
            None => Ok(JsValue::Null),
        },
        _ => Ok(JsValue::Undefined),
    }
}

/** Run the bootstrap's API installer once, the first time a page reaches one of the APIs; true
when it ran now. */
pub(in crate::v9) fn ensure_installed(vm: &mut Vm) -> Res<bool> {
    let Some(installer) = vm.page.state.platform_installer.take() else {
        return Ok(false);
    };
    vm.call(JsValue::Object(installer), JsValue::Undefined, Vec::new())?;
    Ok(true)
}

/** A native entry-point method (`ServiceWorkerContainer.prototype.register`, ...) that installs
the APIs, then calls the real method the installer defined under the same name. Once installed
it answers `undefined`, as the inert shape it replaced did, so a wrapper that kept it as its
fallback cannot loop. */
pub(in crate::v9) fn lazy_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !ensure_installed(vm)? {
        return Ok(JsValue::Undefined);
    }
    let name = native_name(&call).to_string();
    let method = vm.get(&call.this, &crate::v9::vm::heap::Key::from(name.as_str()))?;
    if method == JsValue::Object(call.callee) || !matches!(method, JsValue::Object(_)) {
        return Ok(JsValue::Undefined);
    }
    vm.call(method, call.this.clone(), call.args.to_vec())
}

/** A native entry-point getter (`ServiceWorkerContainer.prototype.ready`, ...): install, then
read the real accessor the installer defined. */
pub(in crate::v9) fn lazy_getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !ensure_installed(vm)? {
        return Ok(JsValue::Undefined);
    }
    let name = native_name(&call).to_string();
    vm.get(&call.this, &crate::v9::vm::heap::Key::from(name.as_str()))
}

/** The setter beside `lazy_getter` (`onmessage = ...` before anything else): install, then
assign through the real accessor. */
pub(in crate::v9) fn lazy_setter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if ensure_installed(vm)? {
        let name = native_name(&call).to_string();
        let key = crate::v9::vm::heap::Key::from(name.as_str());
        vm.set(&call.this, key, call.arg(0), true)?;
    }
    Ok(JsValue::Undefined)
}

/** navigator.userActivation's hasBeenActive and isActive. */
pub(in crate::v9) fn user_activation(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(match native_name(&call) {
        "hasBeenActive" => vm.page.state.sticky_activation,
        _ => is_active(vm),
    }))
}

/** Record a trusted input: transient activation starts now and sticky activation is set. */
pub(in crate::v9) fn activate(vm: &mut Vm) {
    let now = vm.page.script_now();
    vm.page.state.activation = Some(now);
    vm.page.state.sticky_activation = true;
}

/** Whether the page holds transient activation at this moment. */
fn is_active(vm: &Vm) -> bool {
    let now = vm.page.script_now();
    vm.page
        .state
        .activation
        .is_some_and(|at| now - at <= TRANSIENT_ACTIVATION_MS)
}

/** `[hasBeenActive, isActive]`, for navigator.userActivation. */
fn activation(vm: &mut Vm) -> Res<JsValue> {
    let sticky = JsValue::Boolean(vm.page.state.sticky_activation);
    let active = JsValue::Boolean(is_active(vm));
    let array = vm.new_array(vec![sticky, active])?;
    Ok(JsValue::Object(array))
}

/** Which global this is: "service" (a service worker), "worker", "frame" or "window". */
fn context(vm: &Vm) -> &'static str {
    let environment = &vm.page.environment;
    if environment.service_worker {
        "service"
    } else if environment.worker_name.is_some() {
        "worker"
    } else if environment.embedder_origin.is_some() {
        "frame"
    } else {
        "window"
    }
}

/** Queue one platform request and return the promise its answer settles. */
fn host_call(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let op = vm.to_text(&call.arg(0))?;
    let args = vm.to_text(&call.arg(1))?;
    let bytes = super::net::body_bytes(vm, &call.arg(2))?;
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    if args.len().saturating_add(bytes.len()) > PAYLOAD_BYTES {
        let error = vm.type_error("The platform request is too large.");
        return reject(vm, promise, error);
    }
    let body = serde_json::json!({ "args": args, "active": is_active(vm) }).to_string();
    let request = HostRequest {
        id: 0,
        kind: KIND.into(),
        method: op,
        url: vm.page.state.href.clone(),
        headers: Vec::new(),
        body: Some(body),
        body_bytes: bytes,
        blob_source: None,
        credentials: String::new(),
        mode: String::new(),
    };
    if !super::net::record_platform(vm, request, promise)? {
        let error = vm.type_error("Too many pending platform requests.");
        return reject(vm, promise, error);
    }
    Ok(JsValue::Object(promise))
}

/** Reject `promise` with the thrown value, returning the promise. */
fn reject(vm: &mut Vm, promise: ObjectId, error: Thrown) -> Res<JsValue> {
    let Thrown::Value(reason) = error else {
        return Err(error);
    };
    vm.settle(promise, PromiseState::Rejected(reason))?;
    Ok(JsValue::Object(promise))
}

/** Settle a platform promise with the coordinator's JSON text, or `[text, ArrayBuffer]` when the
answer carries bytes (a cached response body, clipboard image data); a lost request rejects
with a TypeError. */
pub(in crate::v9) fn deliver(vm: &mut Vm, promise: ObjectId, response: HostResponse) -> Res<()> {
    if let Some(message) = response.error {
        let error = vm.type_error(&message);
        return reject(vm, promise, error).map(|_| ());
    }
    let text = JsValue::string(&response.body)?;
    if response.body_bytes.is_empty() {
        return vm.resolve_promise(promise, text);
    }
    let buffer = vm.alloc(Object::new(
        Class::ArrayBuffer(Bytes(response.body_bytes)),
        Some(realm().ids.array_buffer_prototype),
    ))?;
    let pair = vm.new_array(vec![text, JsValue::Object(buffer)])?;
    vm.resolve_promise(promise, JsValue::Object(pair))
}

/** Restored activation times must be finite clock readings. */
pub(in crate::v9) fn validate(page: &PageState) -> bool {
    page.activation.is_none_or(f64::is_finite)
}
