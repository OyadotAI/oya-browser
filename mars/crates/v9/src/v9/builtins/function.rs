/** Function: call, apply, bind, toString, and dynamic code compiled by V9's own parser. */
use super::*;
use crate::constants::{VM_DYNAMIC_BYTES, VM_DYNAMIC_SOURCES};
use crate::protocol::EngineError;
use crate::v9::vm::data::{Class, Function, FunctionKind};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{realm, symbol};

/** Function.prototype and the Function constructor. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.ids.function_prototype;
    let empty = b.native("", 0, empty);
    b.objects[(proto - crate::v9::vm::heap::INTRINSIC_BASE) as usize].call = Some(empty);
    b.value(
        proto,
        "name",
        crate::v9::vm::realm::string(""),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    b.value(
        proto,
        "length",
        JsValue::Number(0.0),
        crate::v9::vm::heap::CONFIGURABLE,
    );
    install_thrower(b, proto);
    let ctor = b.constructor("Function", 1, construct, proto, None);
    b.global("Function", ctor);
    b.method(proto, "call", 1, call_method);
    b.method(proto, "apply", 2, apply);
    b.method(proto, "bind", 1, bind);
    b.method(proto, "toString", 0, to_string);
    b.symbol_method(
        proto,
        symbol::HAS_INSTANCE,
        "[Symbol.hasInstance]",
        1,
        has_instance,
    );
}

/** %ThrowTypeError% (spec 10.2.4.1) and AddRestrictedFunctionProperties (10.2.4): one frozen
function whose `name` and `length` are fixed, used as both halves of Function.prototype's
`caller` and `arguments` accessors and of a strict arguments object's `callee`. */
fn install_thrower(b: &mut Builder, proto: ObjectId) {
    let thrower = b.object(Some(proto));
    let native = b.native("", 0, throw_type_error);
    b.objects[(thrower - crate::v9::vm::heap::INTRINSIC_BASE) as usize].call = Some(native);
    b.value(thrower, "length", JsValue::Number(0.0), 0);
    b.value(thrower, "name", crate::v9::vm::realm::string(""), 0);
    b.ids.throw_type_error = thrower;
    for name in ["caller", "arguments"] {
        b.put(
            proto,
            crate::v9::vm::heap::Key::from(name),
            crate::v9::vm::realm::Static::Accessor(Some(thrower), Some(thrower)),
            crate::v9::vm::heap::CONFIGURABLE,
        );
    }
}

/** The body of %ThrowTypeError%. */
fn throw_type_error(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Err(vm.type_error(
        "'caller', 'callee', and 'arguments' properties may not be accessed on strict mode functions or the arguments objects for calls to them",
    ))
}

/** Function.prototype is callable and returns undefined. */
fn empty(_: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Undefined)
}

/** `new Function(params..., body)` compiles with V9's parser; it grants no new authority. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut params = Vec::new();
    let count = call.args.len();
    for index in 0..count.saturating_sub(1) {
        params.push(arg_string(vm, &call, index)?.lossy());
    }
    let body = if count == 0 {
        String::new()
    } else {
        arg_string(vm, &call, count - 1)?.lossy()
    };
    let source = format!(
        "(function anonymous({}\n) {{\n{}\n}})",
        params.join(","),
        body
    );
    evaluate(vm, source)
}

/** Compile and run dynamic source at the top level, returning its completion value. */
pub(in crate::v9) fn evaluate(vm: &mut Vm, source: String) -> Res<JsValue> {
    let used: usize = vm.dynamic.iter().map(|d| d.source.len()).sum();
    if vm.dynamic.len() >= VM_DYNAMIC_SOURCES || used + source.len() > VM_DYNAMIC_BYTES {
        return Err(EngineError::limit("V9 dynamic code", VM_DYNAMIC_BYTES).into());
    }
    let base = dynamic_base(&vm.dynamic);
    let code = match crate::v9::compile_dynamic(&mut vm.registry, &source, base, None) {
        Ok(code) => code,
        Err(error) if error.code == "script_syntax_error" => {
            return Err(vm.syntax_error(&error.message));
        }
        Err(error) if error.code == "unsupported_feature" => {
            return Err(vm.unsupported_error("dynamic code with unsupported syntax"));
        }
        Err(error) => return Err(error.into()),
    };
    vm.dynamic.push(crate::v9::vm::DynamicSource {
        source,
        context: None,
    });
    vm.run_script(code)
}

/** Where the next dynamic source's offsets start: past every page source and earlier dynamic code. */
pub(in crate::v9) fn dynamic_base(sources: &[crate::v9::vm::DynamicSource]) -> usize {
    crate::v9::DYNAMIC_OFFSET
        + sources
            .iter()
            .map(|dynamic| dynamic.source.len() + 1)
            .sum::<usize>()
}

/** Function.prototype.call. */
fn call_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Err(vm.type_error("Function.prototype.call called on a non-function"));
    }
    let args = call
        .args
        .get(1..)
        .map(<[JsValue]>::to_vec)
        .unwrap_or_default();
    vm.call(call.this.clone(), call.arg(0), args)
}

/** Read an array-like into an argument list. */
pub(in crate::v9) fn list_from_array_like(vm: &mut Vm, value: &JsValue) -> Res<Vec<JsValue>> {
    if value.nullish() {
        return Ok(Vec::new());
    }
    if !matches!(value, JsValue::Object(_)) {
        return Err(vm.type_error("CreateListFromArrayLike called on non-object"));
    }
    if let JsValue::Object(id) = value
        && let Some(Class::Array(elements)) = vm.heap.get(*id).map(|o| &o.class)
    {
        return Ok(elements
            .iter()
            .map(|value| {
                if *value == JsValue::Hole {
                    JsValue::Undefined
                } else {
                    value.clone()
                }
            })
            .collect());
    }
    let length = vm.length_of(value)?;
    if length > vm.limits.array_length {
        return Err(EngineError::limit("V9 call arguments", vm.limits.array_length).into());
    }
    let mut list = Vec::with_capacity(length);
    for index in 0..length {
        list.push(vm.get(value, &key(&index.to_string()))?);
    }
    Ok(list)
}

/** Function.prototype.apply. */
fn apply(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Err(vm.type_error("Function.prototype.apply was called on a non-function"));
    }
    let args = list_from_array_like(vm, &call.arg(1))?;
    vm.call(call.this.clone(), call.arg(0), args)
}

/** Function.prototype.bind. */
fn bind(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Object(target) = call.this else {
        return Err(vm.type_error("Bind must be called on a function"));
    };
    if !vm.is_callable(&call.this) {
        return Err(vm.type_error("Bind must be called on a function"));
    }
    let args = call
        .args
        .get(1..)
        .map(<[JsValue]>::to_vec)
        .unwrap_or_default();
    let proto = vm.proto_of(target);
    let id = vm.alloc(Object::new(
        Class::Function(Box::new(Function {
            kind: FunctionKind::Bound {
                target,
                this: call.arg(0),
                args,
            },
            home: None,
            fields: None,
        })),
        proto,
    ))?;
    Ok(JsValue::Object(id))
}

/** Function.prototype.toString: V9 keeps no source text, so every function prints as native. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Err(vm.type_error("Function.prototype.toString requires that 'this' be a Function"));
    }
    let name = vm.get(&call.this, &key("name"))?;
    let name = match name {
        JsValue::String(name) => name.lossy(),
        _ => String::new(),
    };
    let _ = realm();
    text(vm, &format!("function {name}() {{ [native code] }}"))
}

/** Function.prototype[Symbol.hasInstance]. */
fn has_instance(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Ok(JsValue::Boolean(false));
    }
    vm.ordinary_has_instance(&call.this.clone(), &call.arg(0))
        .map(JsValue::Boolean)
}
