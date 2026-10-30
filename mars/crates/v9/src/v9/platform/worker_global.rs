/** Dedicated worker globals share language intrinsics without inheriting any Window capability. */
use super::{dom, window};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key};
use crate::v9::vm::realm::{Builder, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
pub(in crate::v9) mod imports;

/** Only implemented worker-compatible platform constructors and singletons cross this seam. */
const GLOBALS: &[&str] = &[
    "Event",
    "EventTarget",
    "CustomEvent",
    "MessageEvent",
    "ErrorEvent",
    "DOMException",
    "AbortController",
    "AbortSignal",
    "URL",
    "URLSearchParams",
    "Headers",
    "Request",
    "Response",
    "Blob",
    "TextEncoder",
    "TextDecoder",
    "console",
    "performance",
    "crypto",
    "structuredClone",
    "MessageChannel",
    "MessagePort",
    "Worker",
];
/** Window exposes these implemented global functions with worker-compatible behavior. */
const FUNCTIONS: &[&str] = &[
    "setTimeout",
    "setInterval",
    "clearTimeout",
    "clearInterval",
    "queueMicrotask",
    "atob",
    "btoa",
    "fetch",
];
/** Read-only worker navigator facts reuse the same browser-supplied values as pages. */
const NAVIGATOR: &[&str] = &[
    "userAgent",
    "appVersion",
    "platform",
    "language",
    "languages",
    "onLine",
    "appName",
    "appCodeName",
    "product",
    "hardwareConcurrency",
    "deviceMemory",
];
/** Location has no navigation setters or Window back-reference. */
const LOCATION: &[&str] = &[
    "href", "origin", "protocol", "host", "hostname", "port", "pathname", "search", "hash",
];

/** Build an explicit global profile after platform installation, leaving Window unchanged. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let globals = b.ids.worker_interfaces;
    for (name, value) in b.globals.clone() {
        if GLOBALS.contains(&name) {
            b.value(globals, name, value, HIDDEN_ATTRS);
        }
    }
    let base = b.object(Some(b.ids.event_target_prototype));
    let proto = b.object(Some(base));
    b.ids.worker_prototype = proto;
    interface(b, globals, "WorkerGlobalScope", base);
    interface(b, globals, "DedicatedWorkerGlobalScope", proto);
    for name in ["self", "name", "origin", "isSecureContext"] {
        b.getter(proto, name, global_get);
    }
    copy_functions(b, proto);
    b.host_method(proto, "importScripts", 0, imports::run);
    b.host_method(proto, "postMessage", 1, super::workers::post_to_owner);
    b.host_method(proto, "close", 0, super::workers::close);
    for name in ["onmessage", "onmessageerror", "onerror"] {
        b.value(proto, name, JsValue::Null, HIDDEN_ATTRS);
    }
    navigator(b, globals, proto);
    location(b, globals, proto);
}

/** Brand a nonconstructible platform prototype without exposing unrelated interfaces. */
fn interface(b: &mut Builder, globals: u32, name: &'static str, proto: u32) {
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        HIDDEN_ATTRS,
    );
    let constructor = b.constructor(name, 0, dom::illegal_constructor, proto, None);
    b.value(globals, name, JsValue::Object(constructor), HIDDEN_ATTRS);
}

/** Copy only the selected function slots, not the Window prototype or its properties. */
fn copy_functions(b: &mut Builder, proto: u32) {
    for name in FUNCTIONS {
        let index = (b.ids.window_prototype - crate::v9::vm::heap::INTRINSIC_BASE) as usize;
        if let Some((key, value, attrs)) = b.objects[index].get(&Key::from(name)).cloned() {
            b.put(proto, key, value, attrs);
        }
    }
}

/** Separate Navigator and WorkerNavigator objects prevent accidental document-only access. */
fn navigator(b: &mut Builder, globals: u32, scope: u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    for name in NAVIGATOR {
        b.getter(proto, name, window::navigator_get);
    }
    interface(b, globals, "WorkerNavigator", proto);
    let object = b.object(Some(proto));
    b.value(scope, "navigator", JsValue::Object(object), HIDDEN_ATTRS);
}

/** WorkerLocation reads the worker's immutable script URL and cannot navigate a document. */
fn location(b: &mut Builder, globals: u32, scope: u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    for name in LOCATION {
        b.getter(proto, name, location_get);
    }
    b.host_method(proto, "toString", 0, location_get);
    interface(b, globals, "WorkerLocation", proto);
    let object = b.object(Some(proto));
    b.value(scope, "location", JsValue::Object(object), HIDDEN_ATTRS);
}

/** Worker-specific metadata stays in its environment, never in a page's browsing context. */
fn global_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if dom::native_name(&call) == "name" {
        return JsValue::string(
            vm.page
                .environment
                .worker_name
                .as_deref()
                .unwrap_or_default(),
        )
        .map_err(Into::into);
    }
    window::window_get(vm, call)
}

/** Resolve readonly URL components from this worker's source URL. */
fn location_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let url = url::Url::parse(&vm.page.environment.url)
        .map_err(|_| vm.type_error("Invalid worker URL"))?;
    let text = match dom::native_name(&call) {
        "origin" => url.origin().ascii_serialization(),
        "protocol" => format!("{}:", url.scheme()),
        "hostname" => url.host_str().unwrap_or_default().into(),
        "host" => url[url::Position::BeforeHost..url::Position::AfterPort].into(),
        "port" => url.port().map(|port| port.to_string()).unwrap_or_default(),
        "pathname" => url.path().into(),
        "search" => url
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default(),
        "hash" => url
            .fragment()
            .map(|hash| format!("#{hash}"))
            .unwrap_or_default(),
        _ => url.to_string(),
    };
    JsValue::string(&text).map_err(Into::into)
}
