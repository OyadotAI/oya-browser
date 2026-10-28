/** WebAssembly.Global wraps a typed value with validated mutability and JS numeric conversion. */
use super::*;

/** Install Global and its value/valueOf accessors. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> ObjectId {
    let proto = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Global", 1, construct, proto, None);
    b.alias(namespace, "Global", ctor);
    b.builtin_accessor(proto, "value", Some(value_get), Some(value_set));
    b.method(proto, "valueOf", 0, value_get);
    proto
}
/** JS descriptors name value types, while binary records use the same Type enum. */
pub(super) fn value_type(vm: &mut Vm, value: &JsValue) -> Res<Type> {
    Ok(match vm.to_text(value)?.as_str() {
        "i32" => Type::I32,
        "i64" => Type::I64,
        "f32" => Type::F32,
        "f64" => Type::F64,
        "v128" => Type::V128,
        "funcref" | "anyfunc" => Type::Func,
        "externref" => Type::Extern,
        "anyref" => Type::Ref(-18, true),
        "eqref" => Type::Ref(-19, true),
        "i31ref" => Type::Ref(-20, true),
        "structref" => Type::Ref(-21, true),
        "arrayref" => Type::Ref(-22, true),
        "exnref" => Type::Ref(-23, true),
        "stringref" => Type::Ref(-25, true),
        _ => return Err(vm.type_error("'value' must be a WebAssembly type")),
    })
}
/** Conversion happens exactly once at the JS/Wasm boundary. */
pub(super) fn convert(vm: &mut Vm, ty: Type, value: &JsValue) -> Res<JsValue> {
    Ok(match ty {
        Type::I32 => JsValue::Number(f64::from(crate::v9::value::to_int32(vm.to_number(value)?))),
        Type::I64 => JsValue::BigInt((super::super::bigint::to_bigint(vm, value)? as i64) as i128),
        Type::F32 => JsValue::Number((vm.to_number(value)? as f32) as f64),
        Type::F64 => JsValue::Number(vm.to_number(value)?),
        Type::V128 => {
            simd::vector(value)?;
            value.clone()
        }
        Type::Extern => value.clone(),
        Type::Ref(_, _) | Type::Exact(..) | Type::Shared(..) => gc::coerce(vm, ty, value)?,
        Type::Func => {
            if *value != JsValue::Null && !instance::is_function(vm, value) {
                return Err(vm.type_error("type incompatibility when transforming from/to JS"));
            }
            value.clone()
        }
    })
}
/** New locals and globals use Wasm defaults rather than JS's undefined-number conversion. */
pub(super) fn default(ty: Type) -> JsValue {
    match ty {
        Type::V128 => JsValue::String(
            JsString::from_units(vec![0; 8]).expect("one vector fits the string limit"),
        ),
        Type::I64 => JsValue::BigInt(0),
        Type::Func | Type::Extern | Type::Ref(_, _) | Type::Exact(..) | Type::Shared(..) => {
            JsValue::Null
        }
        _ => JsValue::Number(0.0),
    }
}
/** Internal globals retain their type and mutable flag in inaccessible inert properties. */
pub(super) fn create(
    vm: &mut Vm,
    ty: Type,
    mutable: bool,
    value: JsValue,
    proto: ObjectId,
) -> Res<ObjectId> {
    let value = convert(vm, ty, &value)?;
    let object = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set(vm, object, GLOBAL, value);
    set(vm, object, VALUE_TYPE, type_value(ty)?);
    set(vm, object, MUTABLE, JsValue::Boolean(mutable));
    Ok(object)
}
/** Descriptor mutable is read before value; a throwing accessor must win over later conversion. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("WebAssembly.Global must be invoked with 'new'"));
    }
    let descriptor = call.arg(0);
    if !matches!(descriptor, JsValue::Object(_)) {
        return Err(vm.type_error("WebAssembly.Global descriptor must be an object"));
    }
    let mutable = vm.get(&descriptor, &key("mutable"))?.truthy();
    let value = vm.get(&descriptor, &key("value"))?;
    let value = JsValue::String(vm.to_string(&value)?);
    let ty = match value_type(vm, &value) {
        Ok(ty) => ty,
        Err(Thrown::Value(_)) => return Err(vm.type_error("WebAssembly.Global(): Descriptor property 'value' must be a WebAssembly type")),
        Err(error) => return Err(error),
    };
    if ty == Type::Ref(-25, true) && !super::stringref_enabled(vm) {
        return Err(vm.type_error("WebAssembly.Global(): Descriptor property 'value' must be a WebAssembly type"));
    }
    if !ty.js_compatible() {
        return Err(vm.type_error("WebAssembly.Global value type is not a JavaScript value type"));
    }
    let value = if call.args.len() > 1 {
        let value = call.arg(1);
        if value == JsValue::Undefined
            && matches!(ty, Type::I32 | Type::I64 | Type::F32 | Type::F64)
        {
            default(ty)
        } else {
            value
        }
    } else if ty == Type::Extern {
        JsValue::Undefined
    } else {
        default(ty)
    };
    let proto = vm.prototype_for(call.new_target.as_ref().unwrap(), ids(vm).global)?;
    create(vm, ty, mutable, value, proto).map(JsValue::Object)
}
/** Restored globals must still carry a known scalar type. */
pub(super) fn ty(vm: &mut Vm, value: &JsValue) -> Res<(ObjectId, Type, bool)> {
    let object = brand(vm, value, GLOBAL)?;
    let ty = get(vm, object, VALUE_TYPE)
        .ok_or_else(|| vm.type_error("invalid WebAssembly global type"))?;
    let ty = record_type(vm, &ty)?;
    let mutable = match get(vm, object, MUTABLE) {
        Some(JsValue::Boolean(v)) => v,
        _ => return Err(vm.type_error("invalid WebAssembly global mutability")),
    };
    Ok((object, ty, mutable))
}

/** Concrete reference indices remain numeric inert metadata, never ambiguously named strings. */
pub(super) fn type_value(ty: Type) -> crate::protocol::EngineResult<JsValue> {
    match ty {
        Type::Ref(heap, nullable) => Ok(JsValue::Number((heap * 2 + i64::from(nullable)) as f64)),
        Type::Shared(heap, nullable) => {
            JsValue::string(&format!("shared:{heap}:{}", u8::from(nullable)))
        }
        Type::Exact(heap, nullable) => {
            JsValue::string(&format!("exact:{heap}:{}", u8::from(nullable)))
        }
        _ => JsValue::string(ty.name()),
    }
}
/** Decode only the bounded reference-index or known scalar-name forms. */
pub(super) fn record_type(vm: &mut Vm, value: &JsValue) -> Res<Type> {
    if let JsValue::String(text) = value
        && let Some(rest) = text.lossy().strip_prefix("shared:")
    {
        let fields: Vec<_> = rest.split(':').collect();
        if fields.len() == 2
            && matches!(fields[1], "0" | "1")
            && let Ok(heap) = fields[0].parse::<i64>()
            && matches!(heap, -37 | -36 | -22..=-17 | -15 | -14)
        {
            return Ok(Type::Shared(heap, fields[1] == "1"));
        }
        return Err(vm.type_error("invalid shared Wasm reference type record"));
    }
    if let JsValue::String(text) = value
        && let Some(rest) = text.lossy().strip_prefix("exact:")
    {
        let mut fields = rest.split(':');
        let heap = fields.next().and_then(|v| v.parse::<i64>().ok());
        let nullable = fields.next();
        if let (Some(heap), Some(nullable)) = (heap, nullable)
            && heap >= 0
            && heap < MAX_LOCALS as i64
            && matches!(nullable, "0" | "1")
            && fields.next().is_none()
        {
            return Ok(Type::Exact(heap, nullable == "1"));
        }
        return Err(vm.type_error("invalid exact Wasm reference type record"));
    }
    if let JsValue::Number(n) = value {
        if !n.is_finite()
            || n.fract() != 0.0
            || *n < -80.0
            || *n > constants::MAX_LOCALS as f64 * 2.0 + 1.0
        {
            return Err(vm.type_error("invalid Wasm reference type record"));
        }
        let n = *n as i64;
        return Ok(Type::Ref(n.div_euclid(2), n.rem_euclid(2) == 1));
    }
    value_type(vm, value)
}
/** Reading rechecks the stored type/value pair against the private state boundary. */
pub(super) fn read(vm: &mut Vm, value: &JsValue) -> Res<JsValue> {
    let (object, ty, _) = ty(vm, value)?;
    let value = get(vm, object, GLOBAL).unwrap();
    type_context::convert(vm, object, ty, &value)
}
/** Writes to immutable globals are TypeErrors, not silently ignored changes. */
pub(super) fn write(vm: &mut Vm, receiver: &JsValue, value: &JsValue) -> Res<()> {
    let (object, ty, mutable) = ty(vm, receiver)?;
    if !mutable {
        return Err(vm.type_error("cannot set an immutable WebAssembly.Global"));
    }
    let value = type_context::convert(vm, object, ty, value)?;
    set(vm, object, GLOBAL, value);
    Ok(())
}
/** value and valueOf share the same brand and typed read. */
fn value_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, ty, _) = ty(vm, &call.this)?;
    if !type_context::js_compatible(vm, object, ty)? {
        return Err(vm.type_error("invalid type for JavaScript WebAssembly global"));
    }
    read(vm, &call.this).map(|value| gc::expose(vm, value))
}
/** The setter returns undefined after the typed store. */
fn value_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, ty, _) = ty(vm, &call.this)?;
    if !type_context::js_compatible(vm, object, ty)? {
        return Err(vm.type_error("invalid type for JavaScript WebAssembly global"));
    }
    write(vm, &call.this, &call.arg(0))?;
    Ok(JsValue::Undefined)
}
