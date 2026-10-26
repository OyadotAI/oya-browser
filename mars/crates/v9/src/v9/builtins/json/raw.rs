/** Raw JSON stores a validated scalar source in a frozen, unforgeable object with a null prototype. */
use super::*;
use crate::v9::vm::heap::{ENUMERABLE, Object, Prop, Slot};

/** A hidden brand cannot be manufactured by any JavaScript property key. */
fn brand() -> Key {
    Key::private("raw-json-brand")
}

/** Install raw source construction and brand testing beside parse and stringify. */
pub(super) fn install(builder: &mut Builder, json: ObjectId) {
    builder.method(json, "rawJSON", 1, create);
    builder.method(json, "isRawJSON", 1, is_raw);
}

/** Validation accepts exactly one JSON primitive and preserves its original spelling. */
fn validate(vm: &mut Vm, source: &JsString) -> Res<()> {
    let whitespace = |unit: &u16| matches!(unit, 0x20 | 0x09 | 0x0a | 0x0d);
    if source.as_units().first().is_none_or(whitespace)
        || source.as_units().last().is_some_and(whitespace)
    {
        return Err(vm.syntax_error("Invalid raw JSON text"));
    }
    if matches!(json_parse_units(vm, source.as_units())?, JsValue::Object(_)) {
        return Err(vm.syntax_error("Raw JSON must contain a primitive value"));
    }
    Ok(())
}

/** Immutable own storage prevents normal script from changing validated source after branding. */
fn create(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let source = vm.to_string(&call.arg(0))?;
    validate(vm, &source)?;
    let id = vm.alloc(Object::new(Class::Plain, None))?;
    vm.heap.put(
        id,
        Key::from("rawJSON"),
        Prop {
            slot: Slot::Data(JsValue::String(source)),
            attrs: ENUMERABLE,
        },
    );
    vm.heap.put(
        id,
        brand(),
        Prop {
            slot: Slot::Data(JsValue::Boolean(true)),
            attrs: 0,
        },
    );
    if let Some(object) = vm.heap.get_mut(id) {
        object.extensible = false;
    }
    Ok(JsValue::Object(id))
}

/** Proxies never inherit an internal slot from their target and are not unwrapped here. */
fn source(vm: &Vm, value: &JsValue) -> Option<JsString> {
    let object = vm.heap.get(value.object()?)?;
    if !matches!(object.class, Class::Plain)
        || !matches!(
            object.props.get(&brand()).map(|prop| &prop.slot),
            Some(Slot::Data(JsValue::Boolean(true)))
        )
    {
        return None;
    }
    match &object.props.get(&Key::from("rawJSON"))?.slot {
        Slot::Data(JsValue::String(source)) => Some(source.clone()),
        _ => None,
    }
}

/** The structured-clone boundary must reject this extra internal-slot object kind. */
pub(in crate::v9) fn branded(vm: &Vm, value: &JsValue) -> bool {
    source(vm, value).is_some()
}

/** Brand testing performs no author property accesses, including on revoked proxies. */
fn is_raw(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(source(vm, &call.arg(0)).is_some()))
}

/** Restored heap records are revalidated before raw source may enter the serialized output. */
pub(super) fn text(vm: &mut Vm, value: &JsValue) -> Res<Option<JsString>> {
    let Some(source) = source(vm, value) else {
        return Ok(None);
    };
    validate(vm, &source)?;
    Ok(Some(source))
}
