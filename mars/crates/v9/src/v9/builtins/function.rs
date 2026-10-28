/** Function: call, apply, bind, toString, and dynamic code compiled by V9's own parser. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::data::{Class, Function, FunctionKind};
use crate::v9::vm::heap::{CONFIGURABLE, Object, Prop, Slot};
use crate::v9::vm::realm::{realm, symbol};

mod diagnostics;
mod dynamic;
pub(super) use diagnostics::receiver as diagnostic_receiver;

/** Indirect eval's global scope supplies vars, while the eval owns its lexical declarations. */
const INDIRECT_CONTEXT: &str = "n|g";

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
    let has_instance = b.symbol_method(
        proto,
        symbol::HAS_INSTANCE,
        "[Symbol.hasInstance]",
        1,
        has_instance,
    );
    b.put(
        proto,
        Key::Sym(symbol::HAS_INSTANCE),
        crate::v9::vm::realm::Static::Value(JsValue::Object(has_instance)),
        0,
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
    create_dynamic(vm, call, "function")
}

/** CreateDynamicFunction for Function, GeneratorFunction, AsyncFunction and
AsyncGeneratorFunction: `prefix` is the kind's keyword form. */
pub(super) fn create_dynamic(vm: &mut Vm, call: Call, prefix: &str) -> Res<JsValue> {
    vm.check_dynamic_function_access(call.callee)?;
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
    vm.check_string_codegen()?;
    let head = format!("({prefix}({}\n", params.join(","));
    let parameters_end = head.len();
    let source = format!("{head}) {{\n{body}\n}})");
    let native = vm.native_dynamic();
    if let Err(error) = crate::v9::syntax::validate_dynamic_function(
        &source,
        parameters_end,
        source.len() - 2,
        native,
    ) {
        if error.code == "script_syntax_error" {
            return Err(vm.syntax_error(&crate::v9::errors::syntax_reason(&error)));
        }
        return Err(error.into());
    }
    let function = dynamic::evaluate_function(vm, source)?;
    dynamic_metadata(vm, &call, function)
}

/** The display name is an own property, never a private lexical binding inside the body. */
fn dynamic_metadata(vm: &mut Vm, call: &Call, function: JsValue) -> Res<JsValue> {
    let id = function
        .object()
        .ok_or_else(|| EngineError::invalid("V9 dynamic code lost its function."))?;
    vm.pin(function.clone());
    vm.heap.put(
        id,
        Key::private("dynamic-function-source"),
        Prop::data(JsValue::Boolean(true)),
    );
    let fallback = vm.proto_of(id).unwrap_or(realm().ids.function_prototype);
    let new_target = call
        .new_target
        .clone()
        .unwrap_or(JsValue::Object(call.callee));
    let prototype = vm.prototype_for(&new_target, fallback)?;
    if let Some(object) = vm.heap.get_mut(id) {
        object.proto = Some(prototype);
    }
    vm.heap.put(
        id,
        key("name"),
        Prop {
            slot: Slot::Data(JsValue::string("anonymous")?),
            attrs: CONFIGURABLE,
        },
    );
    Ok(function)
}

/** Compile and run dynamic source at the top level, returning its completion value. */
pub(in crate::v9) fn evaluate(vm: &mut Vm, source: String) -> Res<JsValue> {
    let code = compile_indirect_eval(vm, source)?;
    vm.run_script(code)
}

/** Parse indirect evaluation in its caller context before a ShadowRealm activates the destination. */
pub(in crate::v9) fn compile_indirect_eval(
    vm: &mut Vm,
    source: String,
) -> Res<std::sync::Arc<crate::v9::compile::Code>> {
    vm.check_string_codegen()?;
    let origin = vm.capture_eval_origin()?;
    vm.prepare_dynamic(source.len().saturating_add(origin.bytes()))?;
    let native = vm.native_dynamic();
    let base = dynamic_base(&vm.dynamic);
    let code = match crate::v9::compile_dynamic(
        &mut vm.registry,
        &source,
        base,
        Some(INDIRECT_CONTEXT),
        native,
    ) {
        Ok(code) => code,
        Err(error) if error.code == "script_syntax_error" => {
            return Err(vm.syntax_error(&crate::v9::errors::syntax_reason(&error)));
        }
        Err(error) if error.code == "unsupported_feature" => {
            return Err(vm.unsupported_error("dynamic code with unsupported syntax"));
        }
        Err(error) => return Err(error.into()),
    };
    let origin = vm.finish_eval_origin(&source, origin)?;
    vm.dynamic.push(crate::v9::vm::DynamicSource {
        source,
        context: Some(INDIRECT_CONTEXT.into()),
        base: Some(base),
        origin: Some(origin),
    });
    Ok(code)
}

/** Where the next dynamic source's offsets start: past every page source and earlier dynamic code. */
pub(in crate::v9) fn dynamic_base(sources: &[crate::v9::vm::DynamicSource]) -> usize {
    sources
        .iter()
        .fold(crate::v9::DYNAMIC_OFFSET, |next, source| {
            source
                .base
                .unwrap_or(next)
                .saturating_add(source.source.len())
                .saturating_add(1)
        })
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
    vm.forward_call(call.this.clone(), call.arg(0), args)
}

/** Read an array-like into an argument list. */
pub(in crate::v9) fn list_from_array_like(vm: &mut Vm, value: &JsValue) -> Res<Vec<JsValue>> {
    if !matches!(value, JsValue::Object(_)) {
        return Err(vm.type_error("CreateListFromArrayLike called on non-object"));
    }
    if let JsValue::Object(id) = value
        && let Some(object) = vm.heap.get(*id).filter(|o| o.props.is_empty())
        && let Class::Array(elements) = &object.class
        && elements.resident_len() == elements.len()
        && elements.iter().all(|value| *value != JsValue::Hole)
    {
        if elements.len() > vm.limits.array_length {
            return Err(EngineError::limit("V9 call arguments", vm.limits.array_length).into());
        }
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
        let item = vm.get(value, &key(&index.to_string()))?;
        vm.pin(item.clone());
        list.push(item);
    }
    Ok(list)
}

/** Function.prototype.apply. */
fn apply(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Err(vm.type_error("Function.prototype.apply was called on a non-function"));
    }
    let args = if call.arg(1).nullish() {
        Vec::new()
    } else {
        list_from_array_like(vm, &call.arg(1))?
    };
    vm.forward_call(call.this.clone(), call.arg(0), args)
}

/** Pinned V8's uint16 call-input representation reserves ten slots and excludes its upper bound for bound captures. */
pub(super) const MAX_BOUND_ARGUMENTS: usize = 65_525;

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
    let proto = super::integrity::get_prototype_of(vm, target)?;
    let count = args.len();
    if count > MAX_BOUND_ARGUMENTS {
        return Err(vm.range_error(&format!(
            "Too many arguments in function call (only {MAX_BOUND_ARGUMENTS} allowed)"
        )));
    }
    let id = vm.alloc(Object::new(
        Class::Function(Box::new(Function {
            kind: FunctionKind::Bound {
                target,
                this: call.arg(0),
                args,
            },
            home: None,
            fields: None,
            name: None,
        })),
        proto,
    ))?;
    bound_metadata(vm, id, target, count)?;
    Ok(JsValue::Object(id))
}

/** Capture observable metadata during bind, so later target mutations do not change it. */
fn bound_metadata(vm: &mut Vm, id: ObjectId, target: ObjectId, count: usize) -> Res<()> {
    let length = bound_length(vm, target, count)?;
    let name = vm.bind_name_snapshot(target)?;
    vm.set_name_snapshot(id, name)?;
    for (name, value) in [("length", JsValue::Number(length))] {
        vm.heap.put(
            id,
            key(name),
            Prop {
                slot: Slot::Data(value),
                attrs: CONFIGURABLE,
            },
        );
    }
    Ok(())
}

/** Only an own numeric length contributes, converted to an integer before bound inputs count. */
fn bound_length(vm: &mut Vm, target: ObjectId, count: usize) -> Res<f64> {
    let length = key("length");
    if vm.own(target, &length)?.is_none() {
        return Ok(0.0);
    }
    match vm.get(&JsValue::Object(target), &length)?.number_value() {
        Some(number) if !number.is_nan() => Ok((number.trunc() - count as f64).max(0.0)),
        _ => Ok(0.0),
    }
}

/** Function.prototype.toString preserves retained source text and prints native functions with their installed name. */
pub(in crate::v9) fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Err(vm.type_error("Function.prototype.toString requires that 'this' be a Function"));
    }
    if let Some(source) = call
        .this
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| object.props.get(&Key::private("dynamic-function-source")))
        .and_then(|prop| match &prop.slot {
            Slot::Data(JsValue::String(source)) => Some(source.clone()),
            _ => None,
        })
    {
        return Ok(JsValue::String(source));
    }
    if let Some(source) = call
        .this
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| object.function())
        .and_then(|function| match function.kind {
            FunctionKind::Closure { site, .. } => vm.registry.get(site)?.original_source(),
            _ => None,
        })
        .map(str::to_owned)
    {
        let dynamic = call
            .this
            .object()
            .and_then(|id| vm.heap.get(id))
            .and_then(|object| object.props.get(&Key::private("dynamic-function-source")))
            .is_some_and(|prop| matches!(prop.slot, Slot::Data(JsValue::Boolean(true))));
        if dynamic {
            if let Some(index) = source.find('(') {
                return text(
                    vm,
                    &format!("{} anonymous{}", &source[..index], &source[index..]),
                );
            }
        }
        return text(vm, &source);
    }
    let anonymous = call
        .this
        .object()
        .and_then(|id| vm.heap.get(id))
        .is_some_and(|object| {
            matches!(&object.class, Class::Proxy(_))
                || object
                    .function()
                    .is_some_and(|function| matches!(function.kind, FunctionKind::Bound { .. }))
        });
    if anonymous {
        return text(vm, "function () { [native code] }");
    }
    let name = native_source_name(vm, &call.this);
    text(vm, &format!("function {name}() {{ [native code] }}"))
}

/** Native source uses the installation name and never invokes user name accessors. */
pub(super) fn native_source_name(vm: &Vm, value: &JsValue) -> String {
    let Some(id) = value.object() else {
        return String::new();
    };
    if let Some(Slot::Data(JsValue::String(name))) = vm
        .heap
        .get(id)
        .and_then(|object| {
            object
                .props
                .get(&Key::private("native-function-source-name"))
        })
        .map(|property| &property.slot)
    {
        return name.lossy();
    }
    if let Some(name) = vm
        .heap
        .get(id)
        .and_then(|object| object.function())
        .and_then(|function| match function.kind {
            FunctionKind::Closure { site, .. } => {
                vm.registry.get(site).map(|code| code.name.lossy())
            }
            _ => None,
        })
    {
        return name;
    }
    let intrinsics = vm.object_intrinsics(id);
    let index = vm
        .heap
        .get(id)
        .and_then(|object| object.function())
        .and_then(|function| match function.kind {
            FunctionKind::Native { id, .. } => Some(id),
            _ => None,
        })
        .or_else(|| intrinsics.object(id).and_then(|object| object.call))
        .or_else(|| {
            (id >= crate::v9::vm::heap::NATIVE_BASE).then(|| crate::v9::vm::realm::native_index(id))
        });
    index
        .and_then(|index| intrinsics.natives.get(index as usize))
        .map(|native| {
            native
                .display_name
                .as_deref()
                .unwrap_or(native.name)
                .to_string()
        })
        .unwrap_or_default()
}

/** Function.prototype[Symbol.hasInstance]. */
fn has_instance(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if !vm.is_callable(&call.this) {
        return Ok(JsValue::Boolean(false));
    }
    vm.ordinary_has_instance(&call.this.clone(), &call.arg(0))
        .map(JsValue::Boolean)
}
