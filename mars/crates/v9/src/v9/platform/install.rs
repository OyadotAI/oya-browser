/** Builds the platform's intrinsic prototypes and singletons, and the global object's prototype. */
use super::attr_dom;
use super::dom::{self, REFLECTED};
use super::{console, events, interfaces, net, timers, url_bindings, window};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::Key;
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS};
use crate::v9::vm::realm::{Builder, NativeFn, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};

/** Frames of idle time an IdleDeadline reports: 48 ms, just under the 50 ms the specification caps it at. */
const IDLE_FRAMES: f64 = 3.0;

/** `Node.ELEMENT_NODE` and the other node type and position constants, on the prototype and
on the `Node` constructor itself, where DOM polyfills compare `nodeType` against them. */
const NODE_CONSTANTS: [(&str, f64); 18] = [
    ("ELEMENT_NODE", 1.0),
    ("ATTRIBUTE_NODE", 2.0),
    ("TEXT_NODE", 3.0),
    ("CDATA_SECTION_NODE", 4.0),
    ("ENTITY_REFERENCE_NODE", 5.0),
    ("ENTITY_NODE", 6.0),
    ("PROCESSING_INSTRUCTION_NODE", 7.0),
    ("COMMENT_NODE", 8.0),
    ("DOCUMENT_NODE", 9.0),
    ("DOCUMENT_TYPE_NODE", 10.0),
    ("DOCUMENT_FRAGMENT_NODE", 11.0),
    ("NOTATION_NODE", 12.0),
    ("DOCUMENT_POSITION_DISCONNECTED", 1.0),
    ("DOCUMENT_POSITION_PRECEDING", 2.0),
    ("DOCUMENT_POSITION_FOLLOWING", 4.0),
    ("DOCUMENT_POSITION_CONTAINS", 8.0),
    ("DOCUMENT_POSITION_CONTAINED_BY", 16.0),
    ("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC", 32.0),
];

/** Install the platform into the realm. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let noop = b.function("", 0, noop);
    b.ids.fields_dummy = noop;
    let target = b.object(Some(object_prototype));
    b.ids.event_target_prototype = target;
    methods(
        b,
        target,
        &["addEventListener", "removeEventListener", "dispatchEvent"],
        events::event_target,
    );
    let node = b.object(Some(target));
    b.ids.node_prototype = node;
    getters(
        b,
        node,
        &[
            "nodeType",
            "nodeName",
            "parentNode",
            "parentElement",
            "childNodes",
            "firstChild",
            "lastChild",
            "nextSibling",
            "previousSibling",
            "ownerDocument",
            "isConnected",
        ],
        dom::node_get,
    );
    for name in ["textContent", "nodeValue"] {
        b.accessor(node, name, Some(dom::node_get), Some(dom::node_set));
    }
    methods(
        b,
        node,
        &[
            "appendChild",
            "insertBefore",
            "removeChild",
            "replaceChild",
            "hasChildNodes",
            "contains",
            "cloneNode",
            "getRootNode",
            "compareDocumentPosition",
            "isEqualNode",
            "isSameNode",
            "lookupNamespaceURI",
            "isDefaultNamespace",
            "normalize",
        ],
        dom::node_method,
    );
    for (name, value) in NODE_CONSTANTS {
        b.value(node, name, JsValue::Number(value), 0);
    }
    let attributes = b.object(Some(b.ids.array_prototype));
    b.ids.named_node_map_prototype = attributes;
    methods(
        b,
        attributes,
        &["getNamedItem", "item", "removeNamedItem", "setNamedItem"],
        dom::named_node_map_method,
    );
    let element = b.object(Some(node));
    b.ids.element_prototype = element;
    shared_getters(
        b,
        element,
        &[
            "tagName",
            "localName",
            "namespaceURI",
            "prefix",
            "children",
            "childElementCount",
            "firstElementChild",
            "lastElementChild",
            "nextElementSibling",
            "previousElementSibling",
            "dataset",
            "sheet",
            "pathname",
            "host",
            "hostname",
            "protocol",
            "search",
            "hash",
            "origin",
            "port",
            "username",
            "password",
            "validity",
            "willValidate",
            "validationMessage",
            "attributes",
        ],
        dom::element_get,
    );
    for name in [
        "className",
        "innerHTML",
        "outerHTML",
        "style",
        "part",
        "classList",
    ] {
        b.accessor(
            element,
            name,
            Some(dom::element_get),
            Some(dom::element_set),
        );
    }
    methods(
        b,
        element,
        &[
            "getAttribute",
            "setAttribute",
            "removeAttribute",
            "hasAttribute",
            "getAttributeNode",
            "getAttributeNodeNS",
            "setAttributeNode",
            "setAttributeNodeNS",
            "removeAttributeNode",
            "getAttributeNS",
            "setAttributeNS",
            "removeAttributeNS",
            "hasAttributeNS",
            "hasAttributes",
            "getAttributeNames",
            "toggleAttribute",
            "querySelector",
            "querySelectorAll",
            "getElementsByTagName",
            "getElementsByTagNameNS",
            "getElementsByClassName",
            "matches",
            "webkitMatchesSelector",
            "msMatchesSelector",
            "closest",
            "remove",
            "append",
            "prepend",
            "before",
            "after",
            "replaceWith",
            "replaceChildren",
            "insertAdjacentHTML",
            "appendHTMLUnsafe",
            "prependHTMLUnsafe",
            "beforeHTMLUnsafe",
            "afterHTMLUnsafe",
            "insertAdjacentElement",
            "insertAdjacentText",
            "getBoundingClientRect",
            "getClientRects",
            "scrollIntoView",
            "scrollTo",
            "scrollBy",
            "scroll",
            "setPointerCapture",
            "releasePointerCapture",
            "attachShadow",
            "requestFullscreen",
        ],
        dom::element_method,
    );
    for name in REFLECTED
        .into_iter()
        .filter(|name| !interfaces::specific(name))
    {
        b.accessor(
            element,
            name,
            Some(dom::reflect_get),
            Some(dom::reflect_set),
        );
    }
    let html = b.object(Some(element));
    b.ids.html_element_prototype = html;
    b.accessor(html, "innerText", Some(dom::node_get), Some(dom::node_set));
    for name in ["scrollTop", "scrollLeft"] {
        b.accessor(html, name, Some(dom::element_get), Some(dom::element_set));
    }
    shared_getters(
        b,
        html,
        &[
            "offsetWidth",
            "offsetHeight",
            "clientWidth",
            "clientHeight",
            "scrollWidth",
            "scrollHeight",
            "offsetTop",
            "offsetLeft",
            "clientTop",
            "clientLeft",
            "offsetParent",
            "form",
            "options",
            "text",
        ],
        dom::element_get,
    );
    shared_methods(
        b,
        html,
        &[
            "click",
            "focus",
            "blur",
            "select",
            "checkValidity",
            "setCustomValidity",
            "load",
            "play",
            "pause",
            "canPlayType",
            "reportValidity",
            "submit",
            "requestSubmit",
            "reset",
        ],
        dom::element_method,
    );
    interfaces::install(b, html);
    let text = b.object(Some(node));
    b.ids.text_prototype = text;
    b.accessor(text, "data", Some(dom::node_get), Some(dom::node_set));
    getters(b, text, &["length"], dom::node_get);
    b.getter(text, "wholeText", dom::node_get);
    document(b, node);
    collections(b);
    event_types(b);
    network(b);
    singletons(b);
    global_object(b);
}

/** The document singleton and its prototype. */
fn document(b: &mut Builder, node: u32) {
    let proto = b.object(Some(node));
    b.ids.document_prototype = proto;
    b.ids.xml_document_prototype = b.object(Some(proto));
    getters(
        b,
        proto,
        &[
            "body",
            "head",
            "documentElement",
            "styleSheets",
            "readyState",
            "URL",
            "documentURI",
            "baseURI",
            "referrer",
            "domain",
            "activeElement",
            "scrollingElement",
            "currentScript",
            "forms",
            "images",
            "links",
            "scripts",
            "anchors",
            "embeds",
            "plugins",
            "childNodes",
            "children",
            "firstChild",
            "firstElementChild",
            "lastChild",
            "lastElementChild",
        ],
        dom::document_get,
    );
    for name in ["cookie", "title"] {
        b.accessor(
            proto,
            name,
            Some(dom::document_get),
            Some(dom::document_set),
        );
    }
    for name in ["characterSet", "charset", "inputEncoding"] {
        b.getter(proto, name, dom::document_get);
    }
    b.getter(proto, "contentType", dom::document_get);
    for (name, value) in [
        ("compatMode", "CSS1Compat"),
        ("visibilityState", "visible"),
        ("designMode", "off"),
        ("dir", ""),
        ("lastModified", "01/01/1970 00:00:00"),
    ] {
        b.value(proto, name, string(value), CONFIGURABLE);
    }
    b.value(proto, "hidden", JsValue::Boolean(false), CONFIGURABLE);
    b.getter(proto, "doctype", dom::document_get);
    methods(
        b,
        proto,
        &[
            "getElementById",
            "getElementsByName",
            "getElementsByTagName",
            "getElementsByTagNameNS",
            "getElementsByClassName",
            "querySelector",
            "querySelectorAll",
            "createElement",
            "createElementNS",
            "createAttribute",
            "createAttributeNS",
            "createTextNode",
            "createCDATASection",
            "createComment",
            "createDocumentFragment",
            "createEvent",
            "hasFocus",
            "elementFromPoint",
            "elementsFromPoint",
            "getSelection",
            "createRange",
            "open",
            "close",
            "write",
            "writeln",
            "importNode",
            "adoptNode",
        ],
        dom::document_method,
    );
    super::implementation::install_prototype(b);
    b.getter(
        proto,
        "implementation",
        super::implementation::implementation_get,
    );
    b.method(
        proto,
        "createProcessingInstruction",
        super::implementation::PI_LENGTH,
        dom::document_method,
    );
    font_faces(b, proto);
    ranges(b);
    sheets(b);
    let document = b.object(Some(proto));
    b.ids.document = document;
    let all = b.object(Some(b.ids.object_prototype));
    b.ids.document_all = all;
    b.getter(all, "length", dom::document_all);
    methods(b, all, &["item", "namedItem"], dom::document_all);
    b.getter(proto, "all", |_, _| {
        Ok(JsValue::Object(
            crate::v9::vm::realm::realm().ids.document_all,
        ))
    });
    b.getter(proto, "defaultView", window::default_view_get);
    b.getter(proto, "location", super::frames::location_get);
}

/** document.fonts: a FontFaceSet with nothing to load. V9 renders no text, so every font is
already "loaded": load() resolves to no faces, check() is true and `ready` is settled, which is
what pages that wait on fonts (Google's emoji preloads, layout code) need to carry on. */
fn font_faces(b: &mut Builder, document: u32) {
    let fonts = b.object(Some(b.ids.event_target_prototype));
    b.method(fonts, "load", 1, |vm, _| {
        let empty = vm.new_array(Vec::new())?;
        Ok(JsValue::Object(vm.promise_resolve(JsValue::Object(empty))?))
    });
    b.method(fonts, "check", 1, yes);
    b.getter(fonts, "ready", |vm, call| {
        Ok(JsValue::Object(vm.promise_resolve(call.this)?))
    });
    b.value(fonts, "status", string("loaded"), CONFIGURABLE);
    b.value(fonts, "size", JsValue::Number(0.0), CONFIGURABLE);
    methods(b, fonts, &["add", "clear", "forEach"], noop_method);
    for name in ["has", "delete"] {
        b.method(fonts, name, 1, |_, _| Ok(JsValue::Boolean(false)));
    }
    b.value(document, "fonts", JsValue::Object(fonts), CONFIGURABLE);
}

/** classList, style, dataset and node lists. */
fn collections(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let class_list = b.object(Some(object_prototype));
    b.ids.class_list_prototype = class_list;
    methods(
        b,
        class_list,
        &[
            "add", "remove", "toggle", "contains", "item", "replace", "toString", "keys",
            "entries", "forEach", "supports",
        ],
        dom::class_list,
    );
    let values = b.method(class_list, "values", 0, dom::class_list);
    b.put(
        class_list,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(values)),
        HIDDEN_ATTRS,
    );
    getters(b, class_list, &["length"], dom::class_list);
    b.accessor(
        class_list,
        "value",
        Some(dom::class_list),
        Some(dom::class_list_value_set),
    );
    let style = b.object(Some(object_prototype));
    b.ids.style_prototype = style;
    methods(
        b,
        style,
        &[
            "getPropertyValue",
            "setProperty",
            "removeProperty",
            "getPropertyPriority",
            "item",
        ],
        dom::style_method,
    );
    b.getter(style, "length", dom::style_method);
    let style_values = b.method(style, "values", 0, dom::style_method);
    b.put(
        style,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(style_values)),
        HIDDEN_ATTRS,
    );
    b.accessor(
        style,
        "cssText",
        Some(dom::style_method),
        Some(dom::style_text_set),
    );
    b.getter(style, "length", dom::style_method);
    let dataset = b.object(Some(object_prototype));
    b.ids.dataset_prototype = dataset;
    let list = b.object(Some(object_prototype));
    b.ids.node_list_prototype = list;
    methods(
        b,
        list,
        &["item", "namedItem", "forEach", "entries", "keys"],
        dom::list_method,
    );
    let values = b.method(list, "values", 0, dom::list_method);
    b.put(
        list,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(values)),
        HIDDEN_ATTRS,
    );
    let collection = b.object(Some(object_prototype));
    b.ids.html_collection_prototype = collection;
    methods(b, collection, &["item", "namedItem"], dom::list_method);
    // HTMLCollection has no forEach, entries or keys (only NodeList does), but it is iterable: its
    // @@iterator is the same list walk. The function is not installed under a visible name.
    let iterator = b.function("values", 0, dom::list_method);
    b.put(
        collection,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(iterator)),
        HIDDEN_ATTRS,
    );
    let storage = b.object(Some(object_prototype));
    b.ids.storage_prototype = storage;
    methods(
        b,
        storage,
        &["getItem", "setItem", "removeItem", "clear", "key"],
        window::storage_method,
    );
    b.getter(storage, "length", window::storage_method);
}

/** The phase constants, which Event and Event.prototype both carry. */
const PHASE_CONSTANTS: [(&str, f64); 4] = [
    ("NONE", 0.0),
    ("CAPTURING_PHASE", 1.0),
    ("AT_TARGET", 2.0),
    ("BUBBLING_PHASE", 3.0),
];

/** The animation and transition handler attributes, which start null on every event target. */
const EVENT_HANDLER_ATTRIBUTES: [&str; 8] = [
    "onanimationstart",
    "onanimationend",
    "onanimationiteration",
    "ontransitionend",
    "onwebkitanimationstart",
    "onwebkitanimationend",
    "onwebkitanimationiteration",
    "onwebkittransitionend",
];

/** The init method a subtype owns in the DOM standard, if it has one. */
fn legacy_init(name: &str) -> Option<&'static str> {
    match name {
        "UIEvent" => Some("initUIEvent"),
        "MouseEvent" => Some("initMouseEvent"),
        "KeyboardEvent" => Some("initKeyboardEvent"),
        _ => None,
    }
}

/** Event and its subtypes. */
fn event_types(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.event_prototype = proto;
    getters(
        b,
        proto,
        &[
            "type",
            "target",
            "srcElement",
            "currentTarget",
            "eventPhase",
            "bubbles",
            "cancelable",
            "defaultPrevented",
            "isTrusted",
            "composed",
            "timeStamp",
            "button",
            "buttons",
            "clientX",
            "clientY",
            "pageX",
            "pageY",
            "screenX",
            "screenY",
            "offsetX",
            "offsetY",
            "keyCode",
            "charCode",
            "which",
            "altKey",
            "ctrlKey",
            "metaKey",
            "shiftKey",
            "repeat",
            "isComposing",
            "key",
            "code",
            "relatedTarget",
            "view",
            "deltaX",
            "deltaY",
        ],
        events::event_get,
    );
    for name in ["returnValue", "cancelBubble"] {
        b.accessor(
            proto,
            name,
            Some(events::event_get),
            Some(events::event_set),
        );
    }
    methods(
        b,
        proto,
        &[
            "preventDefault",
            "stopPropagation",
            "stopImmediatePropagation",
            "composedPath",
            "initEvent",
            "initCustomEvent",
        ],
        events::event_method,
    );
    let event = b.constructor("Event", 1, events::construct_event, proto, None);
    for (name, value) in PHASE_CONSTANTS {
        b.value(proto, name, JsValue::Number(value), 0);
        b.value(event, name, JsValue::Number(value), 0);
    }
    b.global("Event", event);
    for name in [
        "CustomEvent",
        "UIEvent",
        "MouseEvent",
        "KeyboardEvent",
        "FocusEvent",
        "PointerEvent",
        "MessageEvent",
        "InputEvent",
        "WheelEvent",
        "TouchEvent",
        "ProgressEvent",
        "PopStateEvent",
        "HashChangeEvent",
        "StorageEvent",
        "ErrorEvent",
        "SubmitEvent",
        "AnimationEvent",
        "TransitionEvent",
    ] {
        let sub = b.object(Some(proto));
        if let Some(init) = legacy_init(name) {
            methods(b, sub, &[init], events::event_method);
        }
        if name == "KeyboardEvent" {
            events::keyboard_prototype(sub);
        }
        if name == "CustomEvent" {
            b.ids.custom_event_prototype = sub;
            // Only CustomEvent reads `detail`; a plain Event takes it as an own property, as
            // Polymer's `fire` assigns it in strict code.
            b.getter(sub, "detail", events::event_get);
        }
        let ctor = b.constructor(name, 1, events::construct_event, sub, Some(event));
        b.global(name, ctor);
    }
    let target = b.ids.event_target_prototype;
    for name in EVENT_HANDLER_ATTRIBUTES {
        let attrs = crate::v9::vm::heap::WRITABLE
            | crate::v9::vm::heap::CONFIGURABLE
            | crate::v9::vm::heap::ENUMERABLE;
        b.value(target, name, JsValue::Null, attrs);
    }
    let ctor = b.constructor("EventTarget", 0, construct_event_target, target, None);
    b.global("EventTarget", ctor);
}

/** XMLHttpRequest, Response, Headers, URL, URLSearchParams. */
fn network(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let xhr = b.object(Some(b.ids.event_target_prototype));
    b.ids.xhr_prototype = xhr;
    methods(
        b,
        xhr,
        &[
            "open",
            "setRequestHeader",
            "send",
            "abort",
            "getResponseHeader",
            "getAllResponseHeaders",
        ],
        net::xhr_method,
    );
    b.method(xhr, "overrideMimeType", 1, net::xhr_method);
    getters(
        b,
        xhr,
        &[
            "readyState",
            "status",
            "statusText",
            "responseText",
            "response",
            "responseURL",
            "responseXML",
            "upload",
            "timeout",
        ],
        net::xhr_get,
    );
    for name in ["responseType", "withCredentials"] {
        b.accessor(xhr, name, Some(net::xhr_get), Some(net::xhr_set));
    }
    let ctor = b.constructor("XMLHttpRequest", 0, net::xhr_construct, xhr, None);
    for (name, value) in [
        ("UNSENT", 0.0),
        ("OPENED", 1.0),
        ("HEADERS_RECEIVED", 2.0),
        ("LOADING", 3.0),
        ("DONE", 4.0),
    ] {
        b.value(ctor, name, JsValue::Number(value), 0);
        b.value(xhr, name, JsValue::Number(value), 0);
    }
    b.global("XMLHttpRequest", ctor);
    let request = b.object(Some(object_prototype));
    b.ids.request_prototype = request;
    getters(
        b,
        request,
        &[
            "url",
            "method",
            "headers",
            "credentials",
            "mode",
            "cache",
            "redirect",
            "referrer",
            "destination",
            "integrity",
            "keepalive",
            "bodyUsed",
            "signal",
        ],
        net::request_member,
    );
    methods(
        b,
        request,
        &[
            "clone",
            "text",
            "json",
            "arrayBuffer",
            "blob",
            "formData",
            "bytes",
        ],
        net::request_member,
    );
    let ctor = b.constructor("Request", 1, net::request_construct, request, None);
    b.global("Request", ctor);
    let response = b.object(Some(object_prototype));
    b.ids.response_prototype = response;
    getters(
        b,
        response,
        &[
            "ok",
            "status",
            "statusText",
            "url",
            "redirected",
            "type",
            "bodyUsed",
            "headers",
        ],
        net::response_get,
    );
    methods(
        b,
        response,
        &[
            "text",
            "json",
            "arrayBuffer",
            "blob",
            "formData",
            "bytes",
            "clone",
        ],
        net::response_method,
    );
    let ctor = b.constructor("Response", 0, net::response_construct, response, None);
    b.method(ctor, "error", 0, net::response_static);
    b.method(ctor, "redirect", 1, net::response_static);
    b.method(ctor, "json", 1, net::response_static);
    b.global("Response", ctor);
    let headers = b.object(Some(object_prototype));
    b.ids.headers_prototype = headers;
    methods(
        b,
        headers,
        &[
            "get",
            "has",
            "set",
            "append",
            "delete",
            "forEach",
            "keys",
            "values",
            "getSetCookie",
        ],
        net::headers_method,
    );
    let entries = b.method(headers, "entries", 0, net::headers_method);
    b.put(
        headers,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(entries)),
        HIDDEN_ATTRS,
    );
    let ctor = b.constructor("Headers", 0, net::headers_construct, headers, None);
    b.global("Headers", ctor);
    let url = b.object(Some(object_prototype));
    b.ids.url_prototype = url;
    for name in [
        "href", "protocol", "username", "password", "host", "hostname", "port", "pathname",
        "search", "hash",
    ] {
        b.accessor(
            url,
            name,
            Some(url_bindings::url_get),
            Some(url_bindings::url_set),
        );
    }
    getters(b, url, &["origin", "searchParams"], url_bindings::url_get);
    methods(b, url, &["toString", "toJSON"], url_bindings::url_get);
    let ctor = b.constructor("URL", 1, url_bindings::url_construct, url, None);
    methods(
        b,
        ctor,
        &["createObjectURL", "revokeObjectURL"],
        net::url_object_url,
    );
    methods(b, ctor, &["parse", "canParse"], url_bindings::url_static);
    b.global("URL", ctor);
    let params = b.object(Some(object_prototype));
    b.ids.search_params_prototype = params;
    methods(
        b,
        params,
        &[
            "get", "getAll", "has", "set", "append", "delete", "sort", "forEach", "toString",
            "keys", "values",
        ],
        url_bindings::params_method,
    );
    let entries = b.method(params, "entries", 0, url_bindings::params_method);
    b.put(
        params,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(entries)),
        HIDDEN_ATTRS,
    );
    b.getter(params, "size", url_bindings::params_method);
    url_bindings::install_iterator(b);
    let ctor = b.constructor(
        "URLSearchParams",
        0,
        url_bindings::params_construct,
        params,
        None,
    );
    b.global("URLSearchParams", ctor);
}

/** location, history, navigator, screen, console, performance, crypto. */
fn singletons(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let location = b.object(Some(object_prototype));
    b.ids.location = location;
    for name in [
        "href", "protocol", "host", "hostname", "port", "pathname", "search", "hash",
    ] {
        b.accessor(
            location,
            name,
            Some(url_bindings::url_get),
            Some(url_bindings::url_set),
        );
    }
    b.getter(location, "origin", url_bindings::url_get);
    methods(
        b,
        location,
        &["assign", "replace", "reload", "toString"],
        window::location_method,
    );
    let history = b.object(Some(object_prototype));
    getters(
        b,
        history,
        &["length", "state", "scrollRestoration"],
        window::history,
    );
    methods(
        b,
        history,
        &["pushState", "replaceState", "back", "forward", "go"],
        window::history,
    );
    b.global("history", history);
    let navigator = b.object(Some(object_prototype));
    getters(
        b,
        navigator,
        &[
            "userAgent",
            "appVersion",
            "platform",
            "language",
            "languages",
            "cookieEnabled",
            "onLine",
            "vendor",
            "appName",
            "appCodeName",
            "product",
            "productSub",
            "hardwareConcurrency",
            "deviceMemory",
            "maxTouchPoints",
            "webdriver",
            "pdfViewerEnabled",
            "doNotTrack",
            "plugins",
            "mimeTypes",
        ],
        window::navigator_get,
    );
    methods(
        b,
        navigator,
        &[
            "sendBeacon",
            "javaEnabled",
            "vibrate",
            "registerProtocolHandler",
            "getGamepads",
        ],
        window::navigator_method,
    );
    let ua_data = b.object(Some(object_prototype));
    getters(b, ua_data, &["brands", "mobile", "platform"], window::ua_data_get);
    methods(b, ua_data, &["getHighEntropyValues", "toJSON"], window::ua_data_method);
    b.alias(navigator, "userAgentData", ua_data);
    // As in Chrome, the members live on Navigator.prototype and `navigator` owns none.
    let navigator_proto = navigator;
    let navigator = b.object(Some(navigator_proto));
    super::chrome_env::navigator(b, navigator_proto);
    b.global("navigator", navigator);
    let screen = b.object(Some(object_prototype));
    getters(
        b,
        screen,
        &[
            "width",
            "height",
            "availWidth",
            "availHeight",
            "colorDepth",
            "pixelDepth",
            "availLeft",
            "availTop",
        ],
        window::screen_get,
    );
    let orientation = b.object(Some(object_prototype));
    b.value(
        orientation,
        "type",
        string("landscape-primary"),
        CONFIGURABLE,
    );
    b.value(orientation, "angle", JsValue::Number(0.0), CONFIGURABLE);
    b.value(
        screen,
        "orientation",
        JsValue::Object(orientation),
        CONFIGURABLE,
    );
    b.global("screen", screen);
    let console = b.object(Some(object_prototype));
    #[cfg(feature = "conformance")]
    let first_console_native = b.natives.len();
    methods(
        b,
        console,
        &[
            "log",
            "info",
            "warn",
            "error",
            "debug",
            "trace",
            "dir",
            "dirxml",
            "table",
            "assert",
            "group",
            "groupCollapsed",
            "groupEnd",
            "time",
            "timeEnd",
            "timeLog",
            "count",
            "countReset",
            "clear",
            "profile",
            "profileEnd",
            "timeStamp",
        ],
        console::console,
    );
    // d8's console is an embedder API object, so under conformance its frames follow the shell flag.
    #[cfg(feature = "conformance")]
    crate::v9::builtins::mark_host_api(&mut b.natives[first_console_native..]);
    b.global("console", console);
    let performance = b.object(Some(b.ids.event_target_prototype));
    b.put(
        performance,
        Key::private(super::performance::BRAND),
        Static::Value(JsValue::Boolean(true)),
        HIDDEN_ATTRS,
    );
    #[cfg(feature = "conformance")]
    b.method(
        performance,
        "measureMemory",
        0,
        crate::v9::builtins::measure_memory,
    );
    methods(
        b,
        performance,
        &[
            "now",
            "getEntries",
            "getEntriesByType",
            "getEntriesByName",
            "clearMarks",
            "clearMeasures",
        ],
        window::performance,
    );
    methods(
        b,
        performance,
        &["clearResourceTimings", "setResourceTimingBufferSize"],
        noop_method,
    );
    b.method(performance, "mark", 1, super::performance::mark);
    b.method(performance, "measure", 1, super::performance::measure);
    getters(
        b,
        performance,
        &["timeOrigin", "timing", "navigation"],
        window::performance,
    );
    super::chrome_env::performance(b, performance);
    b.global("performance", performance);
    let crypto = b.object(Some(object_prototype));
    methods(b, crypto, &["randomUUID"], window::window_method);
    // getRandomValues fills from the operating system's CSPRNG, like randomUUID.
    b.method(
        crypto,
        "getRandomValues",
        1,
        crate::v9::builtins::random_values,
    );
    b.global("crypto", crypto);
}

/** The global object's prototype: every global name. */
fn global_object(b: &mut Builder) {
    let proto = b.object(Some(b.ids.event_target_prototype));
    b.ids.window_prototype = proto;
    getters(
        b,
        proto,
        &[
            "window",
            "top",
            "document",
            "location",
            "closed",
            "isSecureContext",
            "frameElement",
            "localStorage",
            "sessionStorage",
        ],
        window::window_get,
    );
    // [Replaceable] attributes (and globalThis, a writable data property in a browser): an
    // assignment replaces the attribute with the script's own value, so `var length=0; length++`
    // and `globalThis=1` behave as in Chrome.
    for name in [
        "self",
        "globalThis",
        "frames",
        "parent",
        "innerWidth",
        "innerHeight",
        "outerWidth",
        "outerHeight",
        "devicePixelRatio",
        "scrollX",
        "scrollY",
        "pageXOffset",
        "pageYOffset",
        "screenX",
        "screenY",
        "screenLeft",
        "screenTop",
        "length",
        "origin",
        "opener",
        "event",
    ] {
        b.accessor(proto, name, Some(window::window_get), Some(window::replace));
    }
    b.accessor(
        proto,
        "name",
        Some(window::window_get),
        Some(window::set_name),
    );
    for (name, value) in [
        ("NaN", JsValue::Number(f64::NAN)),
        ("Infinity", JsValue::Number(f64::INFINITY)),
        ("undefined", JsValue::Undefined),
    ] {
        b.value(proto, name, value, 0);
    }
    let timer_names = [
        "setTimeout",
        "setInterval",
        "clearTimeout",
        "clearInterval",
        "requestAnimationFrame",
        "cancelAnimationFrame",
        "requestIdleCallback",
        "cancelIdleCallback",
        "queueMicrotask",
    ];
    methods(b, proto, &timer_names, timers::timer);
    b.method(proto, "fetch", 1, net::fetch);
    methods(
        b,
        proto,
        &[
            "atob",
            "btoa",
            "alert",
            "confirm",
            "prompt",
            "print",
            "open",
            "matchMedia",
            "getComputedStyle",
            "getSelection",
            "postMessage",
        ],
        window::window_method,
    );
    methods(
        b,
        proto,
        &[
            "close",
            "stop",
            "focus",
            "blur",
            "moveTo",
            "moveBy",
            "resizeTo",
            "resizeBy",
            "captureEvents",
            "releaseEvents",
        ],
        noop_method,
    );
    methods(
        b,
        proto,
        &["scroll", "scrollTo", "scrollBy"],
        super::scrolling::window_scroll,
    );
    let attr = b.object(Some(b.ids.node_prototype));
    b.ids.attr_prototype = attr;
    attr_members(b, attr);
    for name in [
        "Node",
        "Element",
        "Text",
        "Comment",
        "Document",
        "XMLDocument",
        "HTMLDocument",
        "Window",
        "NodeList",
        "HTMLCollection",
        "CSSStyleDeclaration",
        "DOMTokenList",
        "Storage",
        "HTMLDivElement",
        "HTMLSpanElement",
        "HTMLBodyElement",
        "HTMLHtmlElement",
        "HTMLTemplateElement",
        "SVGElement",
        "SVGGraphicsElement",
        "CharacterData",
        "Attr",
    ] {
        let prototype = match name {
            "Node" => b.ids.node_prototype,
            "Element" => b.ids.element_prototype,
            "Text" | "Comment" | "CharacterData" => b.ids.text_prototype,
            "Document" | "HTMLDocument" => b.ids.document_prototype,
            "XMLDocument" => b.ids.xml_document_prototype,
            "Window" => proto,
            "NodeList" => b.ids.node_list_prototype,
            "HTMLCollection" => b.ids.html_collection_prototype,
            "CSSStyleDeclaration" => b.ids.style_prototype,
            "DOMTokenList" => b.ids.class_list_prototype,
            "Storage" => b.ids.storage_prototype,
            "Attr" => b.ids.attr_prototype,
            _ => b.ids.html_element_prototype,
        };
        let ctor = match name {
            "Text" => b.constructor("Text", 1, construct_text, prototype, None),
            "Comment" => b.constructor("Comment", 1, construct_comment, prototype, None),
            _ => constructor_without_link(b, name, prototype),
        };
        if name == "Node" {
            for (name, value) in NODE_CONSTANTS {
                b.value(ctor, name, JsValue::Number(value), 0);
            }
        }
        b.global(name, ctor);
    }
    for name in [
        "IntersectionObserver",
        "ResizeObserver",
        "PerformanceObserver",
        "ReportingObserver",
    ] {
        let observer = b.object(Some(b.ids.object_prototype));
        methods(b, observer, &["unobserve", "disconnect"], noop_method);
        // Intersection and resize observers report each observed target once, asynchronously,
        // as a browser's first observation does; the others record nothing yet.
        let observe: NativeFn = match name {
            "IntersectionObserver" | "ResizeObserver" => window::observe,
            _ => noop_method,
        };
        b.method(observer, "observe", 1, observe);
        b.method(observer, "takeRecords", 0, |vm, _| {
            Ok(JsValue::Object(vm.new_array(Vec::new())?))
        });
        let ctor = b.constructor(name, 1, construct_observer, observer, None);
        b.global(name, ctor);
    }
    path2d(b);
    let image = b.object(Some(b.ids.html_element_prototype));
    let ctor = b.constructor("Image", 0, construct_image, image, None);
    b.global("Image", ctor);
    let audio = b.object(Some(b.ids.html_element_prototype));
    let ctor = b.constructor("Audio", 0, construct_audio, audio, None);
    b.global("Audio", ctor);
    let abort = b.object(Some(b.ids.object_prototype));
    b.method(abort, "abort", 0, abort_controller_abort);
    let ctor = b.constructor(
        "AbortController",
        0,
        construct_abort_controller,
        abort,
        None,
    );
    b.global("AbortController", ctor);
    abort_signal(b);
    css_namespace(b);
    let idle = b.object(Some(b.ids.object_prototype));
    b.ids.idle_deadline_prototype = idle;
    b.method(idle, "timeRemaining", 0, |_, _| {
        Ok(JsValue::Number(
            crate::constants::ANIMATION_FRAME_MS * IDLE_FRAMES,
        ))
    });
    super::animations::install(b);
    super::channel::install(b);
    super::dom_parser::install(b);
    super::xml_serializer::install(b);
    super::frames::install(b);
    super::implementation::install(b);
    super::scripts::install(b);
    super::components::install(b);
    // Absent APIs (FormData, Blob, typed arrays, WebSocket, Worker, Intl, Proxy...)
    // stay undefined, so libraries that feature-detect them take their fallback paths.
    // The list is kept, not taken: install_window_interfaces reads it after every install.
    let globals = b.globals.clone();
    for (name, value) in globals {
        b.put(proto, Key::from(name), Static::Value(value), HIDDEN_ATTRS);
    }
}

/** Path2D: a path object pages build for canvas drawing. V9 draws no canvas, so the path
methods record nothing; the interface exists because polyfill bundles extend
`Path2D.prototype` unguarded (Twitch's does, and its whole polyfill script failed without it). */
fn path2d(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    methods(
        b,
        proto,
        &[
            "addPath",
            "closePath",
            "moveTo",
            "lineTo",
            "bezierCurveTo",
            "quadraticCurveTo",
            "arc",
            "arcTo",
            "ellipse",
            "rect",
            "roundRect",
        ],
        noop_method,
    );
    let ctor = b.constructor("Path2D", 0, construct_plain, proto, None);
    b.global("Path2D", ctor);
}

/** `new X()` for an interface whose instances hold no state: a plain object over the prototype. */
fn construct_plain(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let Some(target) = &call.new_target else {
        return Err(vm.type_error("Constructor requires 'new'"));
    };
    let proto = vm.prototype_for(target, fallback)?;
    let id = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** Range and Selection: `document.createRange()`, `new Range()` and `getSelection()`. */
fn ranges(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let range = b.object(Some(object_prototype));
    b.ids.range_prototype = range;
    getters(
        b,
        range,
        &[
            "startContainer",
            "startOffset",
            "endContainer",
            "endOffset",
            "collapsed",
            "commonAncestorContainer",
        ],
        super::range::range_get,
    );
    methods(
        b,
        range,
        &[
            "setStart",
            "setEnd",
            "setStartBefore",
            "setStartAfter",
            "setEndBefore",
            "setEndAfter",
            "selectNode",
            "selectNodeContents",
            "collapse",
            "cloneRange",
            "detach",
            "toString",
            "compareBoundaryPoints",
            "comparePoint",
            "isPointInRange",
            "intersectsNode",
            "getBoundingClientRect",
            "getClientRects",
            "createContextualFragment",
            "insertNode",
            "deleteContents",
            "extractContents",
            "cloneContents",
            "surroundContents",
        ],
        super::range::range_method,
    );
    let ctor = b.constructor("Range", 0, super::range::construct_range, range, None);
    for (name, value) in super::range::COMPARE_MODES {
        b.value(range, name, JsValue::Number(value), 0);
        b.value(ctor, name, JsValue::Number(value), 0);
    }
    b.global("Range", ctor);
    let static_range = b.object(Some(object_prototype));
    b.ids.static_range_prototype = static_range;
    getters(
        b,
        static_range,
        &[
            "startContainer",
            "startOffset",
            "endContainer",
            "endOffset",
            "collapsed",
        ],
        super::range::range_get,
    );
    let static_ctor = b.constructor(
        "StaticRange",
        0,
        super::range::construct_static_range,
        static_range,
        None,
    );
    b.global("StaticRange", static_ctor);
    let selection = b.object(Some(object_prototype));
    b.ids.selection_prototype = selection;
    getters(
        b,
        selection,
        &[
            "anchorNode",
            "anchorOffset",
            "focusNode",
            "focusOffset",
            "baseNode",
            "baseOffset",
            "extentNode",
            "extentOffset",
            "isCollapsed",
            "rangeCount",
            "type",
            "direction",
        ],
        super::range::selection_get,
    );
    methods(
        b,
        selection,
        &[
            "getRangeAt",
            "addRange",
            "removeRange",
            "removeAllRanges",
            "empty",
            "collapse",
            "setPosition",
            "collapseToStart",
            "collapseToEnd",
            "extend",
            "setBaseAndExtent",
            "selectAllChildren",
            "deleteFromDocument",
            "containsNode",
            "toString",
        ],
        super::range::selection_method,
    );
    let ctor = b.constructor("Selection", 0, dom::illegal_constructor, selection, None);
    b.global("Selection", ctor);
}

/** CSSStyleSheet: what `document.styleSheets` and a style element's `sheet` hold. */
fn sheets(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.style_sheet_prototype = proto;
    getters(
        b,
        proto,
        &[
            "ownerNode",
            "href",
            "disabled",
            "type",
            "title",
            "media",
            "cssRules",
            "rules",
            "parentStyleSheet",
            "ownerRule",
        ],
        super::sheets::sheet_get,
    );
    methods(
        b,
        proto,
        &[
            "insertRule",
            "deleteRule",
            "addRule",
            "removeRule",
            "replace",
            "replaceSync",
        ],
        super::sheets::sheet_method,
    );
    let ctor = b.constructor("CSSStyleSheet", 0, dom::illegal_constructor, proto, None);
    b.global("CSSStyleSheet", ctor);
}

/** A constructor whose prototype already names its own constructor elsewhere. */
fn constructor_without_link(b: &mut Builder, name: &'static str, prototype: u32) -> u32 {
    // Document is the one interface scripts may construct; every other one is illegal.
    let function: NativeFn = if name == "Document" {
        super::implementation::construct_document
    } else {
        dom::illegal_constructor
    };
    b.constructor(name, 0, function, prototype, None)
}

/** The Attr members: name and owner reads, and value, nodeValue and textContent, which write back. */
fn attr_members(b: &mut Builder, attr: u32) {
    getters(
        b,
        attr,
        &[
            "name",
            "localName",
            "namespaceURI",
            "prefix",
            "ownerElement",
            "specified",
            "nodeName",
            "nodeType",
        ],
        attr_dom::attr_get,
    );
    for name in ["value", "nodeValue", "textContent"] {
        b.accessor(
            attr,
            name,
            Some(attr_dom::attr_get),
            Some(attr_dom::attr_set),
        );
    }
}

/** Getters on a shared prototype, leaving out the members one interface owns. */
fn shared_getters(b: &mut Builder, target: u32, names: &[&'static str], function: NativeFn) {
    for name in names.iter().filter(|name| !interfaces::specific(name)) {
        b.getter(target, name, function);
    }
}

/** Methods on a shared prototype, leaving out the members one interface owns. */
fn shared_methods(b: &mut Builder, target: u32, names: &[&'static str], function: NativeFn) {
    for name in names.iter().filter(|name| !interfaces::specific(name)) {
        b.method(target, name, 0, function);
    }
}

/** Register methods that share one native dispatching on its name. */
fn methods(b: &mut Builder, target: u32, names: &[&'static str], function: NativeFn) {
    for name in names {
        b.method(target, name, 0, function);
    }
}

/** Register getters that share one native dispatching on its name. */
fn getters(b: &mut Builder, target: u32, names: &[&'static str], function: NativeFn) {
    for name in names {
        b.getter(target, name, function);
    }
}

/** A function that does nothing. */
fn noop(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Undefined)
}

/** A method that does nothing. */
fn noop_method(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Undefined)
}

/** Always true. */
pub(super) fn yes(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(true))
}

/** `new EventTarget()`. */
fn construct_event_target(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(
            target,
            crate::v9::vm::realm::realm().ids.event_target_prototype,
        )?,
        None => {
            return Err(
                vm.type_error("Failed to construct 'EventTarget': Please use the 'new' operator.")
            );
        }
    };
    let id = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** Observers record nothing: V9 has no layout or mutation records to report. */
fn construct_observer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => fallback,
    };
    let id = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        let name = vm.get(
            &call.new_target.clone().unwrap_or(JsValue::Undefined),
            &Key::from("name"),
        )?;
        let name = vm.to_text(&name)?;
        return Err(vm.type_error(&format!(
            "Failed to construct '{name}': parameter 1 is not of type 'Function'."
        )));
    }
    window::keep_observer_callback(vm, id, callback)?;
    Ok(JsValue::Object(id))
}

/** `new Image(width, height)`. */
/** `new Text(data)`: a text node holding `data`, which is empty when the argument is omitted. */
fn construct_text(vm: &mut Vm, call: Call) -> Res<JsValue> {
    construct_character_node(vm, &call, "#text")
}

/** `new Comment(data)`: a comment node holding `data`, which is empty when the argument is omitted. */
fn construct_comment(vm: &mut Vm, call: Call) -> Res<JsValue> {
    construct_character_node(vm, &call, "#comment")
}

/** A character-data node made by its constructor. An omitted argument is the empty string, not
the text "undefined", as the DOM's `data = ""` default requires. */
fn construct_character_node(vm: &mut Vm, call: &Call, tag: &str) -> Res<JsValue> {
    let text = match call.arg(0) {
        JsValue::Undefined => String::new(),
        value => vm.to_text(&value)?,
    };
    let node = vm
        .page
        .create(tag, Some(text))
        .map_err(|error| dom::dom_error(vm, error))?;
    dom::wrap(vm, node)
}

fn construct_image(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = vm
        .page
        .create("img", None)
        .map_err(|error| dom::dom_error(vm, error))?;
    for (index, name) in ["width", "height"].iter().enumerate() {
        if index < call.args.len() {
            let value = vm.to_text(&call.arg(index))?;
            vm.page
                .set_attribute(node, name, Some(&value))
                .map_err(|error| dom::dom_error(vm, error))?;
        }
    }
    dom::wrap(vm, node)
}

/** `new Audio(src)`: an audio element, with its source when given; nothing plays. */
fn construct_audio(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = vm
        .page
        .create("audio", None)
        .map_err(|error| dom::dom_error(vm, error))?;
    if !call.args.is_empty() {
        let value = vm.to_text(&call.arg(0))?;
        vm.page
            .set_attribute(node, "src", Some(&value))
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    dom::wrap(vm, node)
}

/** The CSS namespace: escape() per CSSOM, and supports(), which answers true because Eye, not
V9, decides what a declaration does; pages use it to pick modern code paths. */
fn css_namespace(b: &mut Builder) {
    let css = b.object(Some(b.ids.object_prototype));
    b.method(css, "supports", 1, yes);
    b.method(css, "escape", 1, |vm, call| {
        let text = vm.to_string(&call.arg(0))?.lossy();
        Ok(JsValue::string(&css_escape(&text))?)
    });
    b.global("CSS", css);
}

/** CSS.escape: serialize an identifier (CSSOM 2.1.1). */
fn css_escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    for (index, c) in chars.iter().copied().enumerate() {
        let leading_digit = c.is_ascii_digit() && (index == 0 || index == 1 && chars[0] == '-');
        match c {
            '\0' => out.push('\u{fffd}'),
            '\u{1}'..='\u{1f}' | '\u{7f}' => out.push_str(&format!("\\{:x} ", c as u32)),
            _ if leading_digit => out.push_str(&format!("\\{:x} ", c as u32)),
            '-' if index == 0 && chars.len() == 1 => out.push_str("\\-"),
            c if c as u32 >= 0x80 || c == '-' || c == '_' || c.is_ascii_alphanumeric() => {
                out.push(c)
            }
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    out
}

/** AbortSignal: an EventTarget with aborted/reason, and the abort/timeout/any statics. */
fn abort_signal(b: &mut Builder) {
    let proto = b.object(Some(b.ids.event_target_prototype));
    b.ids.abort_signal_prototype = proto;
    let ctor = b.constructor("AbortSignal", 0, dom::illegal_constructor, proto, None);
    b.global("AbortSignal", ctor);
    b.method(proto, "throwIfAborted", 0, |vm, call| {
        if vm.get(&call.this, &Key::from("aborted"))?.truthy() {
            let reason = vm.get(&call.this, &Key::from("reason"))?;
            return Err(crate::v9::vm::Thrown::Value(reason));
        }
        Ok(JsValue::Undefined)
    });
    b.method(ctor, "abort", 0, |vm, call| {
        let signal = new_signal(vm)?;
        mark_aborted(vm, signal, call.arg(0))?;
        Ok(JsValue::Object(signal))
    });
    // V9 has no timer-backed signal source; a timeout signal simply never aborts.
    b.method(ctor, "timeout", 1, |vm, _| {
        Ok(JsValue::Object(new_signal(vm)?))
    });
    b.method(ctor, "any", 1, |vm, call| {
        let signal = new_signal(vm)?;
        for each in vm.iterate_to_vec(&call.arg(0))? {
            if vm.get(&each, &Key::from("aborted"))?.truthy() {
                let reason = vm.get(&each, &Key::from("reason"))?;
                mark_aborted(vm, signal, reason)?;
                break;
            }
        }
        Ok(JsValue::Object(signal))
    });
}

/** A fresh, unaborted AbortSignal. */
fn new_signal(vm: &mut Vm) -> Res<u32> {
    let signal = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(crate::v9::vm::realm::realm().ids.abort_signal_prototype),
    ))?;
    vm.put(signal, "aborted", JsValue::Boolean(false))?;
    vm.put(signal, "reason", JsValue::Undefined)?;
    vm.put(signal, "onabort", JsValue::Null)?;
    Ok(signal)
}

/** Abort a signal once: set aborted and reason (an AbortError by default), then fire `abort`. */
fn mark_aborted(vm: &mut Vm, signal: u32, reason: JsValue) -> Res<()> {
    if vm
        .get(&JsValue::Object(signal), &Key::from("aborted"))?
        .truthy()
    {
        return Ok(());
    }
    let reason = match reason {
        JsValue::Undefined => {
            match vm.dom_exception("AbortError", "signal is aborted without reason") {
                crate::v9::vm::Thrown::Value(value) => value,
                crate::v9::vm::Thrown::Fatal(error) => return Err(error.into()),
            }
        }
        other => other,
    };
    vm.put(signal, "aborted", JsValue::Boolean(true))?;
    vm.put(signal, "reason", reason)?;
    // Dispatch also runs the signal's onabort handler property.
    super::fire(vm, super::Target::Object(signal), "abort", false)?;
    Ok(())
}

/** `new AbortController()`: owns one AbortSignal. */
fn construct_abort_controller(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => fallback,
    };
    let controller = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    let signal = new_signal(vm)?;
    vm.put(controller, "signal", JsValue::Object(signal))?;
    Ok(JsValue::Object(controller))
}

/** AbortController.prototype.abort(reason). */
fn abort_controller_abort(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let signal = vm.get(&call.this, &Key::from("signal"))?;
    if let JsValue::Object(id) = signal {
        mark_aborted(vm, id, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}
