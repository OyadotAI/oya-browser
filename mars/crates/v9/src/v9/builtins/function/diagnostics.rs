/** Incompatible receivers use inert function storage and bounded source previews, never authored conversion hooks. */
use super::*;

/** V8 abbreviates diagnostic function source longer than this many UTF16 units. */
const SOURCE_UNITS: usize = 128;
/** The original prefix remains recognizable while leaving room for the omission marker and suffix. */
const SOURCE_PREFIX: usize = 111;
/** The last source units distinguish the function's actual closing text. */
const SOURCE_SUFFIX: usize = 2;
/** This marker is diagnostic formatting; language-level Function.toString keeps the complete source. */
const OMITTED: &str = "...<omitted>...";

/** Proxy targets may be inspected internally without invoking get, apply or conversion traps. */
pub(in crate::v9::builtins) fn receiver(vm: &mut Vm, value: &JsValue) -> Res<String> {
    let mut value = value.clone();
    while let JsValue::Object(id) = value {
        let Some(proxy) = vm.proxy_of(id) else { break };
        vm.charge(1)?;
        value = proxy.target.map_or(JsValue::Null, JsValue::Object);
    }
    if let Some(id) = value.object()
        && vm.is_callable(&value)
    {
        return Ok(function(vm, id));
    }
    Ok(primitive(vm, &value))
}

/** Function source comes from compiled code or native installation metadata, so overridden names and toString stay inert. */
fn function(vm: &Vm, id: ObjectId) -> String {
    let object = vm.heap.get(id);
    let dynamic =
        object.and_then(|object| object.props.get(&Key::private("dynamic-function-source")));
    if let Some(Prop {
        slot: Slot::Data(JsValue::String(source)),
        ..
    }) = dynamic
    {
        let suffix = source.copy_range(source.len().saturating_sub(SOURCE_SUFFIX), source.len());
        return shorten(source.code_units(), &suffix);
    }
    if let Some(source) = object
        .and_then(|object| object.function())
        .and_then(|function| match function.kind {
            FunctionKind::Closure { site, .. } => vm.registry.get(site)?.original_source(),
            _ => None,
        })
    {
        let named = dynamic
            .is_some_and(|property| matches!(property.slot, Slot::Data(JsValue::Boolean(true))));
        return source_preview(source, named);
    }
    if object
        .and_then(|object| object.function())
        .is_some_and(|function| matches!(function.kind, FunctionKind::Bound { .. }))
    {
        return "function () { [native code] }".into();
    }
    let name = native_source_name(vm, &JsValue::Object(id));
    let units = "function "
        .encode_utf16()
        .chain(name.encode_utf16())
        .chain("() { [native code] }".encode_utf16());
    shorten(units, &[u16::from(b' '), u16::from(b'}')])
}

/** Dynamic functions insert their display name exactly as Function.prototype.toString does. */
fn source_preview(source: &str, dynamic: bool) -> String {
    let (prefix, suffix, inserted) =
        if let Some(index) = dynamic.then(|| source.find('(')).flatten() {
            (&source[..index], &source[index..], " anonymous")
        } else {
            (source, "", "")
        };
    let units = prefix
        .encode_utf16()
        .chain(inserted.encode_utf16())
        .chain(suffix.encode_utf16());
    let last_chars = source.chars().rev().take(SOURCE_SUFFIX).collect::<Vec<_>>();
    let last = last_chars
        .iter()
        .rev()
        .collect::<String>()
        .encode_utf16()
        .collect::<Vec<_>>();
    shorten(units, &last[last.len().saturating_sub(SOURCE_SUFFIX)..])
}

/** Only the first threshold-plus-one units are inspected, avoiding a complete temporary copy of a large function. */
fn shorten(units: impl Iterator<Item = u16>, suffix: &[u16]) -> String {
    let mut prefix = units.take(SOURCE_UNITS + 1).collect::<Vec<_>>();
    if prefix.len() > SOURCE_UNITS {
        prefix.truncate(SOURCE_PREFIX);
        prefix.extend(OMITTED.encode_utf16());
        prefix.extend(suffix);
    }
    String::from_utf16_lossy(&prefix)
}

/** Primitive data and ordinary native brands need no JavaScript property reads to describe a rejected receiver. */
fn primitive(vm: &Vm, value: &JsValue) -> String {
    match value {
        JsValue::Undefined | JsValue::Hole => "undefined".into(),
        JsValue::Null => "null".into(),
        JsValue::String(value) => value.lossy(),
        JsValue::Boolean(value) => value.to_string(),
        #[cfg(feature = "conformance")]
        JsValue::Smi(value) => crate::v9::value::number_to_string(*value),
        JsValue::Number(value) => crate::v9::value::number_to_string(*value),
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(value) | JsValue::LocalNumber(value) => crate::v9::value::number_to_string(value.value),
        JsValue::BigInt(value) => value.to_string(),
        JsValue::BigIntWide(value) => value.diagnostic(),
        JsValue::Symbol(id) => format!("Symbol({})", super::super::symbol_text(vm, *id)),
        JsValue::Object(id) => match vm.heap.get(*id).map(|object| &object.class) {
            Some(Class::Array(_)) => "#<Array>".into(),
            Some(Class::Number(_)) => "#<Number>".into(),
            Some(Class::Boolean(_)) => "#<Boolean>".into(),
            Some(Class::String(_)) => "#<String>".into(),
            _ => "#<Object>".into(),
        },
    }
}
