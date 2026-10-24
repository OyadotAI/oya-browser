/** The Proxy constructor and Proxy.revocable; the internal methods live in vm/proxy.rs. */
use super::*;
use crate::v9::vm::data::{Class, ProxyData};
use crate::v9::vm::heap::Object;

/** The revoke function's native index, set once at install. */
static REVOKE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/** Proxy and Proxy.revocable. ponytail: V9's Proxy carries an unused `prototype` object (the
specification gives it none); drop it if a page ever depends on its absence. */
pub(super) fn install(b: &mut Builder) {
    let unused = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Proxy", 2, construct, unused, None);
    b.method(ctor, "revocable", 2, revocable);
    b.global("Proxy", ctor);
    let _ = REVOKE.set(b.native("", 0, revoke));
}

/** A new proxy over `target` with `handler`; both must be objects. */
fn new_proxy(vm: &mut Vm, target: &JsValue, handler: &JsValue) -> Res<ObjectId> {
    let (JsValue::Object(target), JsValue::Object(handler)) = (target, handler) else {
        return Err(vm.type_error("Cannot create proxy with a non-object as target or handler"));
    };
    let data = ProxyData {
        target: Some(*target),
        handler: Some(*handler),
    };
    vm.proxies = true;
    vm.alloc(Object::new(Class::Proxy(Box::new(data)), None))
}

/** `new Proxy(target, handler)`; calling without new throws. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Constructor Proxy requires 'new'"));
    }
    Ok(JsValue::Object(new_proxy(vm, &call.arg(0), &call.arg(1))?))
}

/** Proxy.revocable(target, handler): `{proxy, revoke}`. */
fn revocable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let proxy = new_proxy(vm, &call.arg(0), &call.arg(1))?;
    let index = *REVOKE.get().expect("revoke native installed");
    let revoke = vm.native_closure(index, vec![JsValue::Object(proxy)])?;
    let result = vm.new_object()?;
    vm.put(result, "proxy", JsValue::Object(proxy))?;
    vm.put(result, "revoke", JsValue::Object(revoke))?;
    Ok(JsValue::Object(result))
}

/** A revoke function: drop the proxy's target and handler. */
fn revoke(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(JsValue::Object(proxy)) = vm.slots(call.callee).first()
        && let Some(Class::Proxy(data)) = vm.heap.get_mut(*proxy).map(|object| &mut object.class)
    {
        data.target = None;
        data.handler = None;
    }
    Ok(JsValue::Undefined)
}
