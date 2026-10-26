/** Tags retain canonical parameter contracts; exceptions carry opaque traced payloads and identity. */
use super::*;
use types::External;
pub(super) const TAGS: &str = "wasm.tags";
const TAG: &str = "wasm.tag.types";
const PAYLOAD: &str = "wasm.exception.payload";
const OWNER: &str = "wasm.exception.tag";

/** Install actual tags, exception payload accessors and the singleton JavaScript exception tag. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> (ObjectId, ObjectId, ObjectId) {
    let tag = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Tag", 1, tag_new, tag, None);
    b.alias(namespace, "Tag", ctor);
    let exception = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Exception", 2, exception_new, exception, None);
    b.alias(namespace, "Exception", ctor);
    b.method(exception, "getArg", 2, get_arg);
    b.method(exception, "is", 1, is);
    let js_tag = b.object(Some(tag));
    b.alias(namespace, "JSTag", js_tag);

    (tag, exception, js_tag)
}
/** A tag signature uses zero result values, including when defined with a nominal function subtype. */
pub(super) fn tag_types(m: &Module) -> Vec<u32> {
    m.imports
        .iter()
        .filter_map(|i| match i.kind {
            External::Tag(t) => Some(t),
            _ => None,
        })
        .chain(m.tags.iter().copied())
        .collect()
}
/** Every throw and catch checks the tag index against its validated signature. */
pub(super) fn signature(m: &Module, index: u32) -> Result<&types::Signature, Error> {
    tag_types(m)
        .get(index as usize)
        .and_then(|i| m.types.get(*i as usize))
        .ok_or_else(|| Error::invalid("unknown WebAssembly tag"))
}
/** Internal native tags retain source bytes and an exact type index for canonical linking. */
pub(super) fn create(vm: &mut Vm, module: ObjectId, index: u32) -> Res<JsValue> {
    let (_, m) = super::module(vm, &JsValue::Object(module))?;
    let signature = m
        .types
        .get(index as usize)
        .ok_or_else(|| vm.type_error("invalid Wasm tag type"))?;
    let encoded = signature
        .params
        .iter()
        .map(|t| global::type_value(*t))
        .collect::<crate::protocol::EngineResult<Vec<_>>>()?;
    let fields = vm.new_array(encoded)?;
    let tag = vm.alloc(Object::new(Class::Plain, Some(ids(vm).tag)))?;
    set(vm, tag, TAG, JsValue::Object(fields));
    set(vm, tag, "wasm.tag.module", JsValue::Object(module));
    set(vm, tag, "wasm.tag.index", JsValue::Number(index as f64));
    Ok(JsValue::Object(tag))
}
/** Imported tags require canonical equality, never function subtyping. */
pub(super) fn compatible(vm: &mut Vm, m: &Module, ty: u32, value: &JsValue) -> Res<bool> {
    let tag = tag_brand(vm, value)?;
    if let Some(module) = get(vm, tag, "wasm.tag.module") {
        let (_, actual) = super::module(vm, &module)?;
        let Some(JsValue::Number(index)) = get(vm, tag, "wasm.tag.index") else {
            return Err(vm.type_error("invalid Wasm tag type"));
        };
        return Ok(gc_types::equivalent(&actual, index as u32, m, ty));
    }
    let types = parameters(vm, value)?;
    Ok(m.types
        .get(ty as usize)
        .is_some_and(|sig| sig.params == types && sig.results.is_empty()))
}
/** JS-created tags retain only known value type descriptors, without executable callbacks. */
fn tag_new(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.as_ref() else {
        return Err(vm.type_error("WebAssembly.Tag must be invoked with 'new'"));
    };
    let descriptor = call.arg(0);
    if descriptor.object().is_none() {
        return Err(vm.type_error("WebAssembly.Tag descriptor must be an object"));
    }
    let params = vm.get(&descriptor, &key("parameters"))?;
    if params.object().is_none() {
        return Err(vm.type_error("WebAssembly.Tag parameters must be an object"));
    }
    let length = vm.length_of(&params)?;
    if length > MAX_LOCALS {
        return Err(vm.range_error("WebAssembly.Tag parameters exceed V9 limit"));
    }
    let mut types = Vec::new();
    for index in 0..length {
        vm.charge(1)?;
        let value = vm.get(&params, &key(&index.to_string()))?;
        let ty = global::value_type(vm, &value)?;
        types.push(global::type_value(ty)?)
    }
    let proto = vm.prototype_for(target, ids(vm).tag)?;
    let tag = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let params = vm.new_array(types)?;
    set(vm, tag, TAG, JsValue::Object(params));
    Ok(JsValue::Object(tag))
}
/** Parameter decoding rejects malformed state before converting a payload or returning arguments. */
fn parameters(vm: &mut Vm, value: &JsValue) -> Res<Vec<Type>> {
    let tag = tag_brand(vm, value)?;
    if is_js_tag(vm, &JsValue::Object(tag)) {
        return Ok(vec![Type::Extern]);
    }
    let array = get(vm, tag, TAG)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid Wasm tag record"))?;
    let values = match vm.heap.get(array).map(|o| &o.class) {
        Some(Class::Array(a)) => a.iter().cloned().collect::<Vec<_>>(),
        _ => return Err(vm.type_error("invalid Wasm tag parameters")),
    };
    values.iter().map(|v| global::record_type(vm, v)).collect()
}
/** Opaque exception records have no authored own properties, including native thrown exceptions. */
pub(super) fn create_exception(
    vm: &mut Vm,
    tag: JsValue,
    payload: Vec<JsValue>,
    proto: ObjectId,
) -> Res<JsValue> {
    let storage = vm.new_array(payload)?;
    let exception = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    set(vm, exception, OWNER, tag);
    set(vm, exception, PAYLOAD, JsValue::Object(storage));
    Ok(JsValue::Object(exception))
}
/** Constructor conversion and traceStack option getters execute in specification order. */
fn exception_new(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.as_ref() else {
        return Err(vm.type_error("WebAssembly.Exception must be invoked with 'new'"));
    };
    let tag = call.arg(0);
    if is_js_tag(vm, &tag) {
        return Err(vm.type_error("Argument 0 cannot be WebAssembly.JSTag"));
    }
    let types = parameters(vm, &tag)?;
    let args = call.arg(1);
    if args.object().is_none() {
        return Err(vm.type_error("WebAssembly.Exception payload must be an object"));
    }
    let length = vm.length_of(&args)?;
    if length != types.len() {
        return Err(vm.type_error("WebAssembly.Exception payload length mismatch"));
    }
    let mut payload = Vec::new();
    for (index, ty) in types.iter().enumerate() {
        let value = vm.get(&args, &key(&index.to_string()))?;
        if !ty.js_compatible() {
            return Err(vm.type_error("Invalid type v128"));
        }
        if let Some(module) = tag.object().and_then(|id| get(vm, id, "wasm.tag.module")) {
            let (_, m) = super::module(vm, &module)?;
            if ty.reference().is_some() && !gc::matches(vm, &m, *ty, &value)? {
                return Err(vm.type_error("object is not a subtype of expected type"));
            }
        }
        payload.push(global::convert(vm, *ty, &value)?)
    }
    let options = call.arg(2);
    let trace = if options.object().is_some() {
        vm.get(&options, &key("traceStack"))?.truthy()
    } else if matches!(options, JsValue::Undefined | JsValue::Null) {
        false
    } else {
        return Err(vm.type_error("WebAssembly.Exception options must be an object"));
    };
    let proto = vm.prototype_for(target, ids(vm).exception)?;
    let value = create_exception(vm, tag, payload, proto)?;
    if trace {
        vm.record_trace(value.object().unwrap())?
    }
    Ok(value)
}
/** Tag identity distinguishes native exceptions from arbitrary JS callback throws. */
pub(super) fn payload(vm: &mut Vm, value: &JsValue, tag: &JsValue) -> Res<Option<Vec<JsValue>>> {
    if value
        .object()
        .is_some_and(|id| get(vm, id, "wasm.trap").is_some())
    {
        return Ok(None);
    }
    let owner = value.object().and_then(|id| get(vm, id, OWNER));
    if owner.as_ref() != Some(tag) {
        return if owner.is_none() && is_js_tag(vm, &tag) {
            Ok(Some(vec![value.clone()]))
        } else {
            Ok(None)
        };
    }
    let storage = get(vm, value.object().unwrap(), PAYLOAD)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid Wasm exception payload"))?;
    match vm.heap.get(storage).map(|o| &o.class) {
        Some(Class::Array(a)) => Ok(Some(a.iter().cloned().collect())),
        _ => Err(vm.type_error("invalid Wasm exception payload")),
    }
}
/** Catch-all handlers catch JS and tagged exceptions while preserving native trap propagation. */
pub(super) fn catchable(vm: &Vm, value: &JsValue) -> bool {
    !value
        .object()
        .is_some_and(|id| get(vm, id, "wasm.trap").is_some())
}
/** Receiver branding happens before comparing the supplied tag. */
fn is(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let exception = brand(vm, &call.this, OWNER)?;
    let tag = tag_brand(vm, &call.arg(0))?;
    Ok(JsValue::Boolean(
        get(vm, exception, OWNER) == Some(JsValue::Object(tag)),
    ))
}
/** Argument access requires the exact tag identity and a valid uint32 argument index. */
fn get_arg(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let exception = brand(vm, &call.this, OWNER)?;
    let tag = call.arg(0);
    let _ = tag_brand(vm, &tag)?;
    if get(vm, exception, OWNER) != Some(tag.clone()) {
        return Err(vm.type_error("WebAssembly.Exception tag mismatch"));
    }
    let index = memory::index(vm, &call.arg(1))? as usize;
    if parameters(vm, &tag)?
        .get(index)
        .is_some_and(|ty| !ty.js_compatible())
    {
        return Err(vm.type_error("Invalid type v128"));
    }
    let values = payload(vm, &call.this, &tag)?.unwrap();
    values
        .get(index)
        .cloned()
        .ok_or_else(|| vm.range_error("WebAssembly.Exception argument index out of range"))
}

/** The static JavaScript tag is branded by its immutable realm identity. */
fn tag_brand(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    if is_js_tag(vm, value) {
        Ok(ids(vm).js_tag)
    } else {
        brand(vm, value, TAG)
    }
}

/** JavaScript's singleton tag throws its payload directly; ordinary tags create opaque packages. */
pub(super) fn throw(vm: &mut Vm, tag: JsValue, payload: Vec<JsValue>) -> Res<JsValue> {
    if is_js_tag(vm, &tag) {
        payload
            .first()
            .cloned()
            .ok_or_else(|| vm.type_error("invalid JavaScript exception payload"))
    } else {
        create_exception(vm, tag, payload, ids(vm).exception)
    }
}
/** Exception references wrap arbitrary JS values so their Wasm heap type remains distinct. */
pub(super) fn reference(vm: &mut Vm, value: &JsValue) -> Res<JsValue> {
    if is_reference(vm, value) {
        Ok(value.clone())
    } else {
        create_exception(
            vm,
            JsValue::Object(ids(vm).js_tag),
            vec![value.clone()],
            ids(vm).exception,
        )
    }
}
/** Dynamic exception tests inspect inaccessible package storage without executing authored getters. */
pub(super) fn is_reference(vm: &Vm, value: &JsValue) -> bool {
    value
        .object()
        .is_some_and(|id| get(vm, id, OWNER).is_some())
}
/** throw_ref recovers the original JS thrown value rather than exposing the internal wrapper. */
pub(super) fn dereference(vm: &mut Vm, value: &JsValue) -> Res<JsValue> {
    let tag = value
        .object()
        .and_then(|id| get(vm, id, OWNER))
        .ok_or_else(|| vm.type_error("invalid exception reference"))?;
    if is_js_tag(vm, &tag) {
        payload(vm, value, &tag)?
            .and_then(|v| v.first().cloned())
            .ok_or_else(|| vm.type_error("invalid JS exception reference"))
    } else {
        Ok(value.clone())
    }
}

/** Every realm's JSTag is an immutable intrinsic identity, including tags imported across realms. */
pub(super) fn is_js_tag(vm: &Vm, value: &JsValue) -> bool {
    let Some(id) = value.object() else {
        return false;
    };
    #[cfg(feature = "conformance")]
    {
        return id
            == IDS.get().unwrap().js_tag
                + vm.object_intrinsics(id).bank * crate::v9::vm::realm::banks::BANK_STRIDE;
    }
    #[cfg(not(feature = "conformance"))]
    {
        id == ids(vm).js_tag
    }
}
