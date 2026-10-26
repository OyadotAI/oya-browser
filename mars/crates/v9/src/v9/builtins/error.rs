/** Error and its subclasses, with the V8-style stack property scripts commonly read. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Object};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{NativeFn, realm, string};
mod stack_entry;
mod trace;
#[cfg(feature = "conformance")]
pub(in crate::v9) use trace::stack as formatted_stack;

/** Error, the native error types, and AggregateError. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.error_prototype = proto;
    let error = b.constructor("Error", 1, construct, proto, None);
    b.ids.error = error;
    trace::install(b);
    b.global("Error", error);
    b.value(proto, "name", string("Error"), HIDDEN_ATTRS);
    b.value(proto, "message", string(""), HIDDEN_ATTRS);
    b.method(proto, "toString", 0, to_string);
    let stack_getter = b.function("get stack", 0, stack_get);
    let stack_setter = b.function("set stack", 1, stack_set);
    b.put(
        proto,
        Key::private("errorStackAccessor"),
        crate::v9::vm::realm::Static::Accessor(Some(stack_getter), Some(stack_setter)),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    b.method(error, "captureStackTrace", 1, capture_stack_trace);
    b.method(error, "isError", 1, is_error);
    b.value(
        error,
        "stackTraceLimit",
        JsValue::Number(STACK_TRACE_LIMIT),
        crate::v9::vm::heap::DEFAULT_ATTRS,
    );
    let kinds: [(&'static str, NativeFn); 7] = [
        ("TypeError", construct),
        ("RangeError", construct),
        ("ReferenceError", construct),
        ("SyntaxError", construct),
        ("EvalError", construct),
        ("URIError", construct),
        ("AggregateError", aggregate),
    ];
    for (name, function) in kinds {
        let sub = b.object(Some(proto));
        let length = if name == "AggregateError" { 2 } else { 1 };
        let ctor = b.constructor(name, length, function, sub, Some(error));
        b.value(sub, "name", string(name), HIDDEN_ATTRS);
        b.value(sub, "message", string(""), HIDDEN_ATTRS);
        b.global(name, ctor);
        let ids = &mut b.ids;
        match name {
            "TypeError" => ids.type_error_prototype = sub,
            "RangeError" => ids.range_error_prototype = sub,
            "ReferenceError" => ids.reference_error_prototype = sub,
            "SyntaxError" => ids.syntax_error_prototype = sub,
            "EvalError" => ids.eval_error_prototype = sub,
            "URIError" => ids.uri_error_prototype = sub,
            _ => ids.aggregate_error_prototype = sub,
        }
    }
    install_dom_exception(b, proto);
}

/** WebIDL's `DOMException`: an interface whose prototype inherits `Error.prototype`, so
 * `new DOMException(message, name)` and `instanceof DOMException` work as in Blink.
 * LinkedIn's app bundle builds `new DOMException("cancelled by new navigation", "AbortError")`
 * at module top level, and the ReferenceError stopped every module of its sign-in page. */
fn install_dom_exception(b: &mut Builder, error_prototype: ObjectId) {
    let proto = b.object(Some(error_prototype));
    let ctor = b.constructor("DOMException", 0, dom_exception, proto, None);
    b.value(proto, "name", string("Error"), HIDDEN_ATTRS);
    b.value(proto, "message", string(""), HIDDEN_ATTRS);
    b.value(proto, "code", JsValue::Number(0.0), HIDDEN_ATTRS);
    for (name, code) in LEGACY_CODES {
        b.value(
            ctor,
            legacy_constant(name),
            JsValue::Number(f64::from(*code)),
            0,
        );
    }
    b.global("DOMException", ctor);
    b.ids.dom_exception_prototype = proto;
}

/** The legacy `code` of each DOMException name that has one (WebIDL's error names table). */
const LEGACY_CODES: &[(&str, u8)] = &[
    ("IndexSizeError", 1),
    ("HierarchyRequestError", 3),
    ("WrongDocumentError", 4),
    ("InvalidCharacterError", 5),
    ("NoModificationAllowedError", 7),
    ("NotFoundError", 8),
    ("NotSupportedError", 9),
    ("InvalidStateError", 11),
    ("SyntaxError", 12),
    ("InvalidModificationError", 13),
    ("NamespaceError", 14),
    ("InvalidAccessError", 15),
    ("TypeMismatchError", 17),
    ("SecurityError", 18),
    ("NetworkError", 19),
    ("AbortError", 20),
    ("URLMismatchError", 21),
    ("QuotaExceededError", 22),
    ("TimeoutError", 23),
    ("InvalidNodeTypeError", 24),
    ("DataCloneError", 25),
];

/** The constant WebIDL names after a legacy code: `AbortError` is `ABORT_ERR`. */
fn legacy_constant(name: &str) -> &'static str {
    match name {
        "IndexSizeError" => "INDEX_SIZE_ERR",
        "HierarchyRequestError" => "HIERARCHY_REQUEST_ERR",
        "WrongDocumentError" => "WRONG_DOCUMENT_ERR",
        "InvalidCharacterError" => "INVALID_CHARACTER_ERR",
        "NoModificationAllowedError" => "NO_MODIFICATION_ALLOWED_ERR",
        "NotFoundError" => "NOT_FOUND_ERR",
        "NotSupportedError" => "NOT_SUPPORTED_ERR",
        "InvalidStateError" => "INVALID_STATE_ERR",
        "SyntaxError" => "SYNTAX_ERR",
        "InvalidModificationError" => "INVALID_MODIFICATION_ERR",
        "NamespaceError" => "NAMESPACE_ERR",
        "InvalidAccessError" => "INVALID_ACCESS_ERR",
        "TypeMismatchError" => "TYPE_MISMATCH_ERR",
        "SecurityError" => "SECURITY_ERR",
        "NetworkError" => "NETWORK_ERR",
        "AbortError" => "ABORT_ERR",
        "URLMismatchError" => "URL_MISMATCH_ERR",
        "QuotaExceededError" => "QUOTA_EXCEEDED_ERR",
        "TimeoutError" => "TIMEOUT_ERR",
        "InvalidNodeTypeError" => "INVALID_NODE_TYPE_ERR",
        _ => "DATA_CLONE_ERR",
    }
}

/** The legacy code of a DOMException name, 0 for names without one. */
pub(in crate::v9) fn legacy_code(name: &str) -> u8 {
    LEGACY_CODES
        .iter()
        .find(|(known, _)| *known == name)
        .map_or(0, |(_, code)| *code)
}

/** `new DOMException(message = "", name = "Error")`; WebIDL constructors need `new`. */
fn dom_exception(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(
            vm.type_error("Failed to construct 'DOMException': Please use the 'new' operator.")
        );
    }
    let id = make(vm, &call, call.arg(0), JsValue::Undefined)?;
    let name = match call.arg(1) {
        JsValue::Undefined => "Error".to_string(),
        name => vm.to_text(&name)?,
    };
    let code = legacy_code(&name);
    for (property, value) in [
        ("name", text(vm, &name)?),
        ("code", JsValue::Number(f64::from(code))),
    ] {
        vm.define(
            id,
            key(property),
            Descriptor {
                value: Some(value),
                writable: Some(false),
                enumerable: Some(false),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
    }
    vm.record_trace(id)?;
    Ok(JsValue::Object(id))
}

/** What V8 reports for Error.stackTraceLimit. */
const STACK_TRACE_LIMIT: f64 = 10.0;

/** The prototype a native error constructor makes instances of. */
fn default_proto(vm: &mut Vm, callee: ObjectId) -> Res<ObjectId> {
    match vm.get(&JsValue::Object(callee), &key("prototype"))? {
        JsValue::Object(id) => Ok(id),
        _ => Ok(realm().ids.error_prototype),
    }
}

/** Shared construction: message and cause. */
fn make(vm: &mut Vm, call: &Call, message: JsValue, options: JsValue) -> Res<ObjectId> {
    let fallback = default_proto(vm, call.callee)?;
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => fallback,
    };
    let id = vm.alloc(Object::new(Class::Error, Some(proto)))?;
    if !matches!(message, JsValue::Undefined) {
        let message = vm.to_string(&message)?;
        vm.define(
            id,
            key("message"),
            Descriptor {
                value: Some(JsValue::String(message)),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
    }
    if let JsValue::Object(options) = options
        && vm.has_property(options, &key("cause"))?
    {
        let cause = vm.get(&JsValue::Object(options), &key("cause"))?;
        vm.define(
            id,
            key("cause"),
            Descriptor {
                value: Some(cause),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
    }
    Ok(id)
}

/** `Error(message, options)` with or without `new`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = make(vm, &call, call.arg(0), call.arg(1))?;
    vm.record_trace(id)?;
    Ok(JsValue::Object(id))
}

/** `AggregateError(errors, message, options)`. */
fn aggregate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = make(vm, &call, call.arg(1), call.arg(2))?;
    let errors = vm.iterate_to_vec(&call.arg(0))?;
    let list = vm.new_array(errors)?;
    vm.define(
        id,
        key("errors"),
        Descriptor {
            value: Some(JsValue::Object(list)),
            writable: Some(true),
            enumerable: Some(false),
            configurable: Some(true),
            ..Descriptor::default()
        },
    )?;
    vm.record_trace(id)?;
    Ok(JsValue::Object(id))
}

/** Error.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Err(vm.type_error("Error.prototype.toString called on non-object"));
    }
    let name = vm.get(&call.this, &key("name"))?;
    let name = if name == JsValue::Undefined {
        "Error".to_string()
    } else {
        vm.to_text(&name)?
    };
    let message = vm.get(&call.this, &key("message"))?;
    let message = if message == JsValue::Undefined {
        String::new()
    } else {
        vm.to_text(&message)?
    };
    let result = match (name.is_empty(), message.is_empty()) {
        (true, _) => message,
        (_, true) => name,
        _ => format!("{name}: {message}"),
    };
    text(vm, &result)
}

/** Borrowed stack accessors inspect the receiver's own trace record after native receiver conversion. */
fn stack_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match stack_entry::receiver(vm, &call.this)? {
        Some(id) => trace::stack(vm, id),
        None => Ok(JsValue::Undefined),
    }
}

/** Assigning `stack` stores an own value. */
fn stack_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(id) = stack_entry::receiver(vm, &call.this)? {
        vm.define(
            id,
            key("stack"),
            Descriptor {
                value: Some(call.arg(0)),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
    }
    Ok(JsValue::Undefined)
}

/** Error.captureStackTrace(object): gives the object a `stack` string. */
fn capture_stack_trace(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = stack_entry::prepare_capture(vm, &call.arg(0))?;
    vm.record_trace(id)?;
    trace::skip_constructor(vm, id, &call.arg(1));
    Ok(JsValue::Undefined)
}

/** Error.isError tests native ErrorData identity without traversing prototypes or proxy targets. */
fn is_error(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(matches!(
        call.arg(0)
            .object()
            .and_then(|id| vm.heap.get(id))
            .map(|object| &object.class),
        Some(Class::Error)
    )))
}
