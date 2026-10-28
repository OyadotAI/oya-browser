/**
 * Platform objects Chrome has that pages feature-detect: the Navigator interface (members on its
 * prototype, as in Chrome), the connection and permissions objects, `performance.memory`,
 * `window.chrome`, and interface objects scripts test for before using an API. Values are those
 * of Chrome 152 on a Mac on a fast network; none carries behavior beyond what a page observes.
 */
use super::dom::{illegal_constructor, native_name};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{CONFIGURABLE, ENUMERABLE, Key};
use crate::v9::vm::realm::{Builder, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};

/** Interfaces detection code checks only for existence. */
const PLAIN_INTERFACES: [&str; 2] = ["SVGFETurbulenceElement", "CSSPositionTryDescriptors"];
/** navigator.connection, as Chrome reports a fast connection. */
const CONNECTION: [(&str, f64); 2] = [("rtt", 50.0), ("downlink", 10.0)];
/** performance.memory, as Chrome reports a fresh tab on a 64-bit Mac. */
const MEMORY: [(&str, f64); 3] = [
    ("jsHeapSizeLimit", 4_294_705_152.0),
    ("totalJSHeapSize", 26_000_000.0),
    ("usedJSHeapSize", 19_000_000.0),
];
/** chrome.csi's transition type for an ordinary navigation. */
const CSI_TRANSITION: f64 = 15.0;
/** Milliseconds per second, for chrome.loadTimes, which reports seconds. */
const MS_PER_SECOND: f64 = 1000.0;

/** `Symbol.toStringTag`, so `Object.prototype.toString` names the interface as Chrome does. */
fn tag(b: &mut Builder, target: u32, name: &'static str) {
    b.put(target, Key::Sym(symbol::TO_STRING_TAG), Static::Value(string(name)), CONFIGURABLE);
}

/** An interface object whose constructor throws, with its prototype. */
fn interface(b: &mut Builder, name: &'static str, proto: u32) {
    tag(b, proto, name);
    let ctor = b.constructor(name, 0, illegal_constructor, proto, None);
    b.global(name, ctor);
}

/** Navigator, NetworkInformation, Permissions and the plain interfaces. */
pub(super) fn navigator(b: &mut Builder, navigator_proto: u32) {
    interface(b, "Navigator", navigator_proto);
    for name in PLAIN_INTERFACES {
        let proto = b.object(Some(b.ids.object_prototype));
        interface(b, name, proto);
    }
    let connection_proto = b.object(Some(b.ids.event_target_prototype));
    interface(b, "NetworkInformation", connection_proto);
    let connection = b.object(Some(connection_proto));
    b.value(connection, "effectiveType", string("4g"), ENUMERABLE);
    for (name, value) in CONNECTION {
        b.value(connection, name, JsValue::Number(value), ENUMERABLE);
    }
    b.value(connection, "saveData", JsValue::Boolean(false), ENUMERABLE);
    b.alias(navigator_proto, "connection", connection);
    let permissions_proto = b.object(Some(b.ids.object_prototype));
    interface(b, "Permissions", permissions_proto);
    b.method(permissions_proto, "query", 1, query);
    let permissions = b.object(Some(permissions_proto));
    b.alias(navigator_proto, "permissions", permissions);
    chrome(b);
}

/** performance.memory. */
pub(super) fn performance(b: &mut Builder, performance: u32) {
    let memory = b.object(Some(b.ids.object_prototype));
    for (name, value) in MEMORY {
        b.value(memory, name, JsValue::Number(value), ENUMERABLE);
    }
    b.alias(performance, "memory", memory);
}

/** window.chrome: the app object, csi() and loadTimes(), as an ordinary page sees them. */
fn chrome(b: &mut Builder) {
    let chrome = b.object(Some(b.ids.object_prototype));
    let app = b.object(Some(b.ids.object_prototype));
    b.value(app, "isInstalled", JsValue::Boolean(false), ENUMERABLE);
    let install = b.object(Some(b.ids.object_prototype));
    for (key, value) in [("DISABLED", "disabled"), ("INSTALLED", "installed"), ("NOT_INSTALLED", "not_installed")] {
        b.value(install, key, string(value), ENUMERABLE);
    }
    b.alias(app, "InstallState", install);
    let running = b.object(Some(b.ids.object_prototype));
    for (key, value) in [("CANNOT_RUN", "cannot_run"), ("READY_TO_RUN", "ready_to_run"), ("RUNNING", "running")] {
        b.value(running, key, string(value), ENUMERABLE);
    }
    b.alias(app, "RunningState", running);
    for name in ["getDetails", "getIsInstalled", "installState", "runningState"] {
        b.method(app, name, 0, chrome_method);
    }
    b.alias(chrome, "app", app);
    for name in ["csi", "loadTimes"] {
        b.method(chrome, name, 0, chrome_method);
    }
    b.global("chrome", chrome);
}

/** navigator.permissions.query: every permission a fresh profile has not decided is "prompt". */
fn query(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let descriptor = call.arg(0);
    if !matches!(descriptor, JsValue::Object(_)) {
        return Err(vm.type_error("Failed to execute 'query' on 'Permissions': parameter 1 is not of type 'object'."));
    }
    let name = vm.get(&descriptor, &Key::from("name"))?;
    let status = record(vm, &[("name", name), ("state", JsValue::string("prompt")?), ("onchange", JsValue::Null)])?;
    Ok(JsValue::Object(vm.promise_resolve(status)?))
}

/** chrome.app's methods, chrome.csi() and chrome.loadTimes(). */
fn chrome_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let start = vm.page.environment.time_origin_ms;
    match native_name(&call) {
        "getDetails" => Ok(JsValue::Null),
        "getIsInstalled" => Ok(JsValue::Boolean(false)),
        "runningState" => Ok(JsValue::string("cannot_run")?),
        "csi" => record(
            vm,
            &[
                ("startE", JsValue::Number(start)),
                ("onloadT", JsValue::Number(start)),
                ("pageT", JsValue::Number(0.0)),
                ("tran", JsValue::Number(CSI_TRANSITION)),
            ],
        ),
        "loadTimes" => load_times(vm, start / MS_PER_SECOND),
        _ => Ok(JsValue::Undefined),
    }
}

/** chrome.loadTimes(): the page's timing in seconds and how it was fetched (HTTP/2). */
fn load_times(vm: &mut Vm, start: f64) -> Res<JsValue> {
    let mut fields = Vec::new();
    for name in ["requestTime", "startLoadTime", "commitLoadTime", "finishDocumentLoadTime", "finishLoadTime", "firstPaintTime"] {
        fields.push((name, JsValue::Number(start)));
    }
    fields.push(("firstPaintAfterLoadTime", JsValue::Number(0.0)));
    fields.push(("navigationType", JsValue::string("Other")?));
    fields.push(("wasFetchedViaSpdy", JsValue::Boolean(true)));
    fields.push(("wasNpnNegotiated", JsValue::Boolean(true)));
    fields.push(("npnNegotiatedProtocol", JsValue::string("h2")?));
    fields.push(("wasAlternateProtocolAvailable", JsValue::Boolean(false)));
    fields.push(("connectionInfo", JsValue::string("h2")?));
    record(vm, &fields)
}

/** A plain object with these data properties, in order. */
fn record(vm: &mut Vm, fields: &[(&str, JsValue)]) -> Res<JsValue> {
    let object = JsValue::Object(vm.new_object()?);
    for (name, value) in fields {
        vm.set(&object, Key::from(name), value.clone(), true)?;
    }
    Ok(object)
}
