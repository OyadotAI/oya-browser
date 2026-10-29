/** Reflect.apply rejects noncallables with inert target/type diagnostics before reading the argument list. */
use super::*;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Slot;

/** V8 caps fallback object tags independently of language-level toString conversion. */
const TAG_UNITS: usize = 1000;

/** The shared array-like call builtin identifies itself as Function.prototype.apply in this diagnostic. */
pub(super) fn reject_apply(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    let kind = match &value {
        JsValue::Undefined | JsValue::Hole => "undefined",
        JsValue::Null => "null",
        JsValue::Boolean(_) => "a boolean",
        #[cfg(feature = "conformance")]
        JsValue::Smi(_) => "a number",
        JsValue::Number(_) => "a number",
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(_) | JsValue::LocalNumber(_) => "a number",
        JsValue::String(_) => "a string",
        JsValue::Symbol(_) => "a symbol",
        JsValue::BigInt(_) | JsValue::BigIntWide(_) => "a bigint",
        JsValue::Object(_) => "an object",
    };
    let display = display(vm, value)?;
    Err(vm.type_error(&format!(
        "Function.prototype.apply was called on {display}, which is {kind} and not a function"
    )))
}

/** Internal target traversal cannot invoke proxy traps and still consumes the existing operation budget. */
fn display(vm: &mut Vm, mut value: JsValue) -> Res<String> {
    while let JsValue::Object(id) = value {
        let Some(proxy) = vm.proxy_of(id) else { break };
        vm.charge(1)?;
        value = proxy.target.map_or(JsValue::Null, JsValue::Object);
    }
    match value {
        JsValue::Object(id) => object(vm, id),
        // The existing inert primitive renderer has the same conversion rules without authored hooks.
        other => super::super::function::diagnostic_receiver(vm, &other),
    }
}

/** Only actual Error stack data or the original native error toString selects safe name/message formatting. */
fn object(vm: &mut Vm, id: ObjectId) -> Res<String> {
    let to_string = data(vm, id, &key("toString"))?;
    let intrinsics = vm.object_intrinsics(id);
    let error_method = intrinsic(vm, intrinsics.ids.error_prototype, &key("toString"));
    if error_record(vm, id) || to_string == error_method && to_string != JsValue::Undefined {
        return error(vm, id);
    }
    let object_method = intrinsic(vm, intrinsics.ids.object_prototype, &key("toString"));
    if to_string == object_method
        && to_string != JsValue::Undefined
        && let Some(ctor) = data(vm, id, &key("constructor"))?.object()
        && vm.proxy_of(ctor).is_none()
        && vm.is_callable(&JsValue::Object(ctor))
        && let JsValue::String(name) = data(vm, ctor, &key("name"))?
        && !name.is_empty()
    {
        return Ok(format!("#<{}>", name.lossy()));
    }
    let tag = match data(vm, id, &Key::Sym(symbol::TO_STRING_TAG))? {
        JsValue::String(tag) => {
            String::from_utf16_lossy(&tag.copy_range(0, tag.len().min(TAG_UNITS)))
        }
        _ => brand(vm, id).to_owned(),
    };
    Ok(format!("[object {tag}]"))
}

/** Nonstring names/messages become empty; self references never recurse or invoke conversion code. */
fn error(vm: &mut Vm, id: ObjectId) -> Res<String> {
    let name = match data(vm, id, &key("name"))? {
        JsValue::String(name) => name.lossy(),
        _ => String::new(),
    };
    let message = match data(vm, id, &key("message"))? {
        JsValue::String(message) => message.lossy(),
        _ => String::new(),
    };
    Ok(match (name.is_empty(), message.is_empty()) {
        (true, _) => message,
        (_, true) => name,
        _ => format!("{name}: {message}"),
    })
}

/** Genuine captured plain objects share ErrorStackData branding with native Error objects. */
fn error_record(vm: &Vm, id: ObjectId) -> bool {
    vm.heap.get(id).is_some_and(|object| {
        matches!(object.class, Class::Error)
            || [
                crate::v9::vm::error::FRAMES_KEY,
                crate::v9::vm::error::TRACE_KEY,
            ]
            .iter()
            .any(|name| object.props.get(&Key::private(name)).is_some())
    })
}

/** Read only data-valued own/prototype slots, stopping at accessors and proxy boundaries without executing code. */
fn data(vm: &mut Vm, mut id: ObjectId, property: &Key) -> Res<JsValue> {
    for _ in 0..vm.limits.objects {
        vm.charge(1)?;
        if vm.proxy_of(id).is_some() {
            return Ok(JsValue::Undefined);
        }
        if let Some(object) = vm.heap.get(id) {
            if let Some(prop) = object.props.get(property) {
                return Ok(match &prop.slot {
                    Slot::Data(value) => value.clone(),
                    _ => JsValue::Undefined,
                });
            }
            if *property == key("name") && object.function().is_some() {
                return Ok(JsValue::string(
                    &super::super::function::native_source_name(vm, &JsValue::Object(id)),
                )?);
            }
        } else {
            if let Some((_, value, _)) = vm
                .object_intrinsics(id)
                .object(id)
                .and_then(|definition| definition.get(property))
            {
                return Ok(match value {
                    Static::Value(value) => value.clone(),
                    _ => JsValue::Undefined,
                });
            }
            if *property == key("name")
                && let Some(native) = vm.object_intrinsics(id).native(id)
            {
                return Ok(JsValue::string(
                    native.display_name.as_deref().unwrap_or(native.name),
                )?);
            }
        }
        match vm.proto_of(id) {
            Some(proto) => id = proto,
            None => return Ok(JsValue::Undefined),
        }
    }
    Ok(JsValue::Undefined)
}

/** Original builtin identities do not follow mutable public method properties or authored accessors. */
fn intrinsic(vm: &Vm, id: ObjectId, property: &Key) -> JsValue {
    vm.object_intrinsics(id)
        .object(id)
        .and_then(|definition| definition.get(property))
        .and_then(|(_, value, _)| match value {
            Static::Value(value) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or(JsValue::Undefined)
}

/** Native storage supplies fallback brands when toString and Symbol.toStringTag cannot be read as data. */
fn brand(vm: &Vm, id: ObjectId) -> &'static str {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::Array(_)) => "Array",
        Some(Class::Number(_)) => "Number",
        Some(Class::Boolean(_)) => "Boolean",
        Some(Class::String(_)) => "String",
        Some(Class::Date(_)) => "Date",
        Some(Class::RegExp(_)) => "RegExp",
        Some(Class::ArrayBuffer(_)) => "ArrayBuffer",
        Some(Class::SharedArrayBuffer(_)) => "SharedArrayBuffer",
        _ => "Object",
    }
}
