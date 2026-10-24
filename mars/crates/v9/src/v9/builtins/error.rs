/** Error and its subclasses, with the V8-style stack property scripts commonly read. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Object};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::{NativeFn, realm, string};

/** Error, the native error types, and AggregateError. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.error_prototype = proto;
    let error = b.constructor("Error", 1, construct, proto, None);
    b.global("Error", error);
    b.value(proto, "name", string("Error"), HIDDEN_ATTRS);
    b.value(proto, "message", string(""), HIDDEN_ATTRS);
    b.method(proto, "toString", 0, to_string);
    b.builtin_accessor(proto, "stack", Some(stack_get), Some(stack_set));
    b.method(error, "captureStackTrace", 1, capture_stack_trace);
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

/** `stack`: the error's summary line; V9 records no call sites. */
fn stack_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Ok(JsValue::Undefined);
    }
    let summary = to_string(
        vm,
        Call {
            this: call.this.clone(),
            args: &[],
            new_target: None,
            callee: call.callee,
        },
    )?;
    let summary = vm.to_text(&summary)?;
    let trace = vm.get(&call.this, &key(crate::v9::vm::error::TRACE_KEY))?;
    let trace = match trace {
        JsValue::String(trace) if !trace.as_units().is_empty() => trace
            .lossy()
            .lines()
            .map(|line| format!("\n    at {line}"))
            .collect::<String>(),
        _ => "\n    at <anonymous>".into(),
    };
    text(vm, &format!("{summary}{trace}"))
}

/** Assigning `stack` stores an own value. */
fn stack_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let JsValue::Object(id) = call.this {
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
    let target = call.arg(0);
    if let JsValue::Object(id) = target {
        let value = stack_get(
            vm,
            Call {
                this: target.clone(),
                args: &[],
                new_target: None,
                callee: call.callee,
            },
        )?;
        vm.define(
            id,
            key("stack"),
            Descriptor {
                value: Some(value),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Descriptor::default()
            },
        )?;
    }
    Ok(JsValue::Undefined)
}
