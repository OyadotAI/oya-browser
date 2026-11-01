/** Context attributes and loss state: Mars fixes one opaque sRGB context that never loses its backing. */
use super::*;

/** Installs the two reporting members; neither changes pixels or state. */
pub(super) fn install(b: &mut Builder, proto: u32) {
    b.host_method(proto, "getContextAttributes", 0, attributes);
    b.host_method(proto, "isContextLost", 0, lost);
}

/** A fresh object each call, carrying the attributes Chrome reports for a default 2D context. */
fn attributes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    let object = vm.new_object()?;
    vm.put(object, "alpha", JsValue::Boolean(true))?;
    vm.put(object, "colorSpace", JsValue::string("srgb")?)?;
    vm.put(object, "desynchronized", JsValue::Boolean(false))?;
    vm.put(object, "willReadFrequently", JsValue::Boolean(false))?;
    Ok(JsValue::Object(object))
}

/** Mars never drops a context's backing, so the context is never lost. */
fn lost(vm: &mut Vm, call: Call) -> Res<JsValue> {
    context_node(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))?;
    Ok(JsValue::Boolean(false))
}
