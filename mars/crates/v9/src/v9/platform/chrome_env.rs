/**
 * Platform objects Chrome has that pages feature-detect: the Navigator interface (members on its
 * prototype, as in Chrome), the connection and permissions objects, `performance.memory`,
 * `window.chrome`, and interface objects scripts test for before using an API. Values are those
 * of Chrome 152 on a Mac on a fast network; none carries behavior beyond what a page observes.
 */
use super::chrome_shapes::Known;
use super::dom::{illegal_constructor, native_name};
use crate::v9::value::JsValue;
use crate::v9::vm::data::PromiseState;
use crate::v9::vm::heap::{CONFIGURABLE, DEFAULT_ATTRS, ENUMERABLE, INTRINSIC_BASE, Key};
use crate::v9::vm::realm::{Builder, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};

/** Interfaces detection code checks only for existence; XRPlane is WebXR's, present on every
Chrome desktop. */
const PLAIN_INTERFACES: [&str; 5] = [
    "SVGFETurbulenceElement",
    "CSSPositionTryDescriptors",
    "XRPlane",
    "ScreenOrientation",
    "Scheduler",
];
/** Notification.maxActions on Chrome desktop. */
const NOTIFICATION_ACTIONS: f64 = 2.0;
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
/** Sub-millisecond offset so `chrome.csi().pageT` is a fractional elapsed time like Blink's. */
const CSI_PAGE_TIME_MS: f64 = 12.375;
/** Milliseconds per second, for chrome.loadTimes, which reports seconds. */
const MS_PER_SECOND: f64 = 1000.0;

/** `Symbol.toStringTag`, so `Object.prototype.toString` names the interface as Chrome does. */
fn tag(b: &mut Builder, target: u32, name: &'static str) {
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        CONFIGURABLE,
    );
}

/** An interface object whose constructor throws, with its prototype; returns the constructor. */
pub(super) fn interface(b: &mut Builder, name: &'static str, proto: u32) -> u32 {
    tag(b, proto, name);
    let ctor = b.constructor(name, 0, illegal_constructor, proto, None);
    b.global(name, ctor);
    ctor
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
    b.host_method(permissions_proto, "query", 1, query);
    let permissions = b.object(Some(permissions_proto));
    b.alias(navigator_proto, "permissions", permissions);
    let label_proto = b.object(Some(b.ids.object_prototype));
    interface(b, "CookieDeprecationLabel", label_proto);
    b.host_method(label_proto, "getValue", 0, empty_label);
    let label = b.object(Some(label_proto));
    b.alias(navigator_proto, "cookieDeprecationLabel", label);
    super::plugins::install(b, navigator_proto);
    super::trusted_types::install(b);
    notification(b);
    chrome(b);
}

/** navigator.cookieDeprecationLabel.getValue(): a profile outside the experiment has no label. */
/** Adapter discovery completes asynchronously even when no native GPU backend is available. */
fn no_adapter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let valid = match call.this.object() {
        Some(id) => vm.own(id, &Key::private("gpu.brand"))?.is_some(),
        None => false,
    };
    if !valid {
        return Err(vm.type_error("Illegal invocation"));
    }
    Ok(JsValue::Object(vm.promise_resolve(JsValue::Null)?))
}

/** navigator.cookieDeprecationLabel.getValue(): a profile outside the experiment has no label. */
fn empty_label(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Object(vm.promise_resolve(string(""))?))
}

/** Notification, as a page sees it before asking: permission "default". Constructing one is
not supported, so the constructor throws as the other interfaces here do. */
fn notification(b: &mut Builder) {
    let proto = b.object(Some(b.ids.event_target_prototype));
    let ctor = interface(b, "Notification", proto);
    b.value(ctor, "permission", string("default"), ENUMERABLE);
    b.value(
        ctor,
        "maxActions",
        JsValue::Number(NOTIFICATION_ACTIONS),
        ENUMERABLE,
    );
    b.host_method(ctor, "requestPermission", 0, request_permission);
}

/** Notification.requestPermission(): nobody answers, so the permission stays "default". */
fn request_permission(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let state = string("default");
    let callback = call.arg(0);
    if vm.is_callable(&callback) {
        vm.call(callback, JsValue::Undefined, vec![state.clone()])?;
    }
    Ok(JsValue::Object(vm.promise_resolve(state)?))
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
    for (key, value) in [
        ("DISABLED", "disabled"),
        ("INSTALLED", "installed"),
        ("NOT_INSTALLED", "not_installed"),
    ] {
        b.value(install, key, string(value), ENUMERABLE);
    }
    b.alias(app, "InstallState", install);
    let running = b.object(Some(b.ids.object_prototype));
    for (key, value) in [
        ("CANNOT_RUN", "cannot_run"),
        ("READY_TO_RUN", "ready_to_run"),
        ("RUNNING", "running"),
    ] {
        b.value(running, key, string(value), ENUMERABLE);
    }
    b.alias(app, "RunningState", running);
    for name in [
        "getDetails",
        "getIsInstalled",
        "installState",
        "runningState",
    ] {
        b.host_method(app, name, 0, chrome_method);
    }
    b.value(chrome, "app", JsValue::Object(app), DEFAULT_ATTRS);
    for name in ["csi", "loadTimes"] {
        let method = b.function(name, 0, chrome_method);
        b.value(chrome, name, JsValue::Object(method), DEFAULT_ATTRS);
    }
    b.global("chrome", chrome);
}

/** navigator.permissions.query: every permission a fresh profile has not decided is "prompt". */
fn query(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let descriptor = call.arg(0);
    if !matches!(descriptor, JsValue::Object(_)) {
        return Err(vm.type_error(
            "Failed to execute 'query' on 'Permissions': parameter 1 is not of type 'object'.",
        ));
    }
    let name = vm.get(&descriptor, &Key::from("name"))?;
    let status = record(
        vm,
        &[
            ("name", name),
            ("state", JsValue::string("prompt")?),
            ("onchange", JsValue::Null),
        ],
    )?;
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
                (
                    "pageT",
                    JsValue::Number(super::performance::now(vm) + CSI_PAGE_TIME_MS),
                ),
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
    for name in [
        "requestTime",
        "startLoadTime",
        "commitLoadTime",
        "finishDocumentLoadTime",
        "finishLoadTime",
        "firstPaintTime",
    ] {
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

/** Navigator members that are objects of another interface, and that interface: a page reads
`navigator.mediaDevices.enumerateDevices` and `navigator.storage.estimate` before anything
else, and a member that is `undefined` where Chrome has an object ends the page. The
interfaces come from the shape table; the members below replace their placeholders with
answers a Mac with no camera, no microphone and a full battery would give. */
const NAVIGATOR_OBJECTS: [(&str, &str); 27] = [
    ("bluetooth", "Bluetooth"),
    ("clipboard", "Clipboard"),
    ("credentials", "CredentialsContainer"),
    ("devicePosture", "DevicePosture"),
    ("geolocation", "Geolocation"),
    ("gpu", "GPU"),
    ("hid", "HID"),
    ("ink", "Ink"),
    ("keyboard", "Keyboard"),
    ("locks", "LockManager"),
    ("login", "NavigatorLogin"),
    ("managed", "NavigatorManagedData"),
    ("mediaCapabilities", "MediaCapabilities"),
    ("mediaDevices", "MediaDevices"),
    ("mediaSession", "MediaSession"),
    ("presentation", "Presentation"),
    ("protectedAudience", "ProtectedAudience"),
    ("scheduling", "Scheduling"),
    ("serial", "Serial"),
    ("serviceWorker", "ServiceWorkerContainer"),
    ("storage", "StorageManager"),
    ("storageBuckets", "StorageBucketManager"),
    ("usb", "USB"),
    ("userActivation", "UserActivation"),
    ("virtualKeyboard", "VirtualKeyboard"),
    ("wakeLock", "WakeLock"),
    ("windowControlsOverlay", "WindowControlsOverlay"),
];
/** Constraints `getSupportedConstraints` reports, every one true as in Chrome. */
const SUPPORTED_CONSTRAINTS: [&str; 17] = [
    "aspectRatio",
    "autoGainControl",
    "channelCount",
    "deviceId",
    "displaySurface",
    "echoCancellation",
    "facingMode",
    "frameRate",
    "groupId",
    "height",
    "latency",
    "noiseSuppression",
    "resizeMode",
    "sampleRate",
    "sampleSize",
    "suppressLocalAudioPlayback",
    "width",
];
/** The storage quota Chrome reports to an origin on a Mac with a roomy disk, in bytes. */
const STORAGE_QUOTA: f64 = 296_630_877_388.0;
/** A full battery on mains power: level 1, no time to charge, never discharging. */
const BATTERY: [(&str, JsValue); 4] = [
    ("charging", JsValue::Boolean(true)),
    ("chargingTime", JsValue::Number(0.0)),
    ("dischargingTime", JsValue::Number(f64::INFINITY)),
    ("level", JsValue::Number(1.0)),
];

/** Methods whose first call installs the coordinator-backed APIs (`host_bridge::lazy_method`),
by interface. Native entry points cost a page nothing until it uses one: patching these
prototypes from the bootstrap would copy them into every page's serialized heap. */
const LAZY_METHODS: [(&str, &[&str]); 8] = [
    ("ServiceWorkerContainer", &["register", "getRegistration", "getRegistrations", "startMessages", "addEventListener"]),
    ("Clipboard", &["read", "readText", "write", "writeText"]),
    ("LockManager", &["request", "query"]),
    ("CredentialsContainer", &["get", "create", "store", "preventSilentAccess"]),
    ("WakeLock", &["request"]),
    ("Navigator", &["getGamepads", "requestMIDIAccess", "requestMediaKeySystemAccess", "getInstalledRelatedApps"]),
    ("Document", &["hasStorageAccess", "hasUnpartitionedCookieAccess", "requestStorageAccess", "requestStorageAccessFor", "execCommand", "queryCommandSupported", "queryCommandEnabled"]),
    ("HTMLMediaElement", &["setMediaKeys"]),
];
/** Accessors whose first read installs the APIs (`host_bridge::lazy_getter`). */
const LAZY_GETTERS: [(&str, &[&str]); 2] = [
    ("ServiceWorkerContainer", &["ready", "controller", "oncontrollerchange", "onmessage"]),
    ("HTMLMediaElement", &["mediaKeys"]),
];

/** Install the lazy entry points on their interfaces' prototypes. */
fn lazy_entry_points(b: &mut Builder, known: &Known) {
    for (interface, names) in LAZY_METHODS {
        if let Some((_, proto)) = known.get(interface) {
            for name in names {
                b.host_method(*proto, name, 0, super::host_bridge::lazy_method);
            }
        }
    }
    for (interface, names) in LAZY_GETTERS {
        if let Some((_, proto)) = known.get(interface) {
            for name in names {
                b.accessor(
                    *proto,
                    name,
                    Some(super::host_bridge::lazy_getter),
                    Some(super::host_bridge::lazy_setter),
                );
            }
        }
    }
}

/** Navigator's member objects, after the shape table has given their interfaces prototypes. */
pub(super) fn navigator_objects(b: &mut Builder, known: &Known) {
    let Some(navigator_proto) = b
        .globals
        .iter()
        .find(|(name, _)| *name == "navigator")
        .and_then(|(_, value)| match value {
            JsValue::Object(id) => b.objects[(*id - INTRINSIC_BASE) as usize].proto,
            _ => None,
        })
    else {
        return;
    };
    for (member, interface) in NAVIGATOR_OBJECTS {
        if let Some((_, proto)) = known.get(interface) {
            let object = b.object(Some(*proto));
            if member == "gpu" {
                b.put(
                    object,
                    Key::private("gpu.brand"),
                    Static::Value(JsValue::Boolean(true)),
                    crate::v9::vm::heap::HIDDEN_ATTRS,
                );
            }
            b.alias(navigator_proto, member, object);
        }
    }
    if let Some((_, proto)) = known.get("MediaDevices") {
        b.host_method(*proto, "enumerateDevices", 0, no_devices);
        b.host_method(*proto, "getUserMedia", 0, device_not_found);
        b.host_method(*proto, "getDisplayMedia", 0, device_not_found);
        b.host_method(*proto, "getSupportedConstraints", 0, supported_constraints);
    }
    if let Some((_, proto)) = known.get("GPU") {
        b.host_method(*proto, "requestAdapter", 0, no_adapter);
    }
    if let Some((_, proto)) = known.get("StorageManager") {
        b.host_method(*proto, "estimate", 0, storage_estimate);
        b.host_method(*proto, "persisted", 0, not_persisted);
        b.host_method(*proto, "persist", 0, not_persisted);
    }
    if let Some((_, proto)) = known.get("UserActivation") {
        b.accessor(*proto, "hasBeenActive", Some(super::host_bridge::user_activation), None);
        b.accessor(*proto, "isActive", Some(super::host_bridge::user_activation), None);
    }
    lazy_entry_points(b, known);
    if let Some((_, proto)) = known.get("BatteryManager") {
        for (name, _) in BATTERY {
            b.accessor(*proto, name, Some(battery_get), None);
        }
        b.host_method(navigator_proto, "getBattery", 0, get_battery);
    }
}

/** enumerateDevices(): no camera or microphone, so an empty list. */
fn no_devices(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let list = JsValue::Object(vm.new_array(Vec::new())?);
    Ok(JsValue::Object(vm.promise_resolve(list)?))
}

/** getUserMedia() and getDisplayMedia() with no device to capture: NotFoundError. */
fn device_not_found(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let error = match vm.dom_exception("NotFoundError", "Requested device not found") {
        crate::v9::vm::Thrown::Value(value) => value,
        fatal => return Err(fatal),
    };
    let promise = vm.new_promise()?;
    vm.settle(promise, PromiseState::Rejected(error))?;
    Ok(JsValue::Object(promise))
}

/** getSupportedConstraints(): every constraint Chrome supports. */
fn supported_constraints(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let fields: Vec<(&str, JsValue)> = SUPPORTED_CONSTRAINTS
        .iter()
        .map(|name| (*name, JsValue::Boolean(true)))
        .collect();
    record(vm, &fields)
}

/** storage.estimate(): nothing stored yet, Chrome's quota. */
fn storage_estimate(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let estimate = record(
        vm,
        &[
            ("quota", JsValue::Number(STORAGE_QUOTA)),
            ("usage", JsValue::Number(0.0)),
        ],
    )?;
    Ok(JsValue::Object(vm.promise_resolve(estimate)?))
}

/** storage.persisted() and persist(): nothing is persisted. */
fn not_persisted(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Object(
        vm.promise_resolve(JsValue::Boolean(false))?,
    ))
}

/** A BatteryManager getter, by name. */
fn battery_get(_: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    Ok(BATTERY
        .iter()
        .find(|(field, _)| *field == name)
        .map_or(JsValue::Undefined, |(_, value)| value.clone()))
}

/** navigator.getBattery(): a promise of the one BatteryManager. */
fn get_battery(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let Some(ctor) = vm
        .get(
            &JsValue::Object(crate::v9::vm::GLOBAL),
            &Key::from("BatteryManager"),
        )?
        .object()
    else {
        return Ok(JsValue::Undefined);
    };
    let proto = vm.get(&JsValue::Object(ctor), &Key::from("prototype"))?;
    let manager = vm.new_object()?;
    if let JsValue::Object(proto) = proto {
        vm.set_proto(manager, Some(proto))?;
    }
    Ok(JsValue::Object(
        vm.promise_resolve(JsValue::Object(manager))?,
    ))
}
