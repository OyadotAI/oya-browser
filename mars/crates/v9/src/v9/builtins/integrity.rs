/** The [[GetPrototypeOf]], [[SetPrototypeOf]], [[IsExtensible]] and [[PreventExtensions]]
internal methods as Object and Reflect see them, ported from the spec's Proxy exotic object
(V8's builtins-proxy-gen and objects.cc): a proxy asks its handler's trap and the result is
checked against the target's invariants. vm/proxy.rs leaves these four to the target, so a trap
that threw, lied about extensibility or reported a foreign prototype went unnoticed. */
use super::*;
use crate::v9::vm::data::ProxyData;

/** The proxy's target and its named trap; a revoked proxy throws. */
fn trap(vm: &mut Vm, proxy: &ProxyData, name: &str) -> Res<(ObjectId, Option<JsValue>)> {
    let (Some(target), Some(handler)) = (proxy.target, proxy.handler) else {
        return Err(vm.type_error(&format!(
            "Cannot perform '{name}' on a proxy that has been revoked"
        )));
    };
    let method = vm.get(&JsValue::Object(handler), &key(name))?;
    if method.nullish() {
        return Ok((target, None));
    }
    if !vm.is_callable(&method) {
        return Err(vm.type_error(&format!("'{name}' on proxy: trap is not a function")));
    }
    Ok((target, Some(method)))
}

/** Call a trap with the handler as `this`. */
fn call_trap(vm: &mut Vm, proxy: &ProxyData, method: JsValue, args: Vec<JsValue>) -> Res<JsValue> {
    let handler = proxy.handler.map_or(JsValue::Undefined, JsValue::Object);
    vm.call(method, handler, args)
}

/** O.[[GetPrototypeOf]](). */
pub(super) fn get_prototype_of(vm: &mut Vm, id: ObjectId) -> Res<Option<ObjectId>> {
    let Some(proxy) = vm.proxy_of(id) else {
        return Ok(vm.proto_of(id));
    };
    let (target, Some(method)) = trap(vm, &proxy, "getPrototypeOf")? else {
        let target = proxy.target.unwrap_or(id);
        return get_prototype_of(vm, target);
    };
    let proto = match call_trap(vm, &proxy, method, vec![JsValue::Object(target)])? {
        JsValue::Object(proto) => Some(proto),
        JsValue::Null => None,
        _ => {
            return Err(
                vm.type_error("'getPrototypeOf' on proxy: trap returned neither object nor null")
            );
        }
    };
    if is_extensible(vm, target)? || get_prototype_of(vm, target)? == proto {
        return Ok(proto);
    }
    Err(vm.type_error("'getPrototypeOf' on proxy: proxy target is non-extensible but the trap did not return its actual prototype"))
}

/** O.[[SetPrototypeOf]](V). */
pub(super) fn set_prototype_of(vm: &mut Vm, id: ObjectId, proto: Option<ObjectId>) -> Res<bool> {
    let Some(proxy) = vm.proxy_of(id) else {
        return vm.set_proto(id, proto);
    };
    let (target, Some(method)) = trap(vm, &proxy, "setPrototypeOf")? else {
        let target = proxy.target.unwrap_or(id);
        return set_prototype_of(vm, target, proto);
    };
    let value = proto.map_or(JsValue::Null, JsValue::Object);
    if !call_trap(vm, &proxy, method, vec![JsValue::Object(target), value])?.truthy() {
        return Ok(false);
    }
    if is_extensible(vm, target)? || get_prototype_of(vm, target)? == proto {
        return Ok(true);
    }
    Err(vm.type_error("'setPrototypeOf' on proxy: trap returned truish for setting a new prototype on the non-extensible proxy target"))
}

/** IsExtensible(O). */
pub(super) fn is_extensible(vm: &mut Vm, id: ObjectId) -> Res<bool> {
    let Some(proxy) = vm.proxy_of(id) else {
        return Ok(vm.heap.get(id).is_none_or(|object| object.extensible));
    };
    let (target, Some(method)) = trap(vm, &proxy, "isExtensible")? else {
        let target = proxy.target.unwrap_or(id);
        return is_extensible(vm, target);
    };
    let result = call_trap(vm, &proxy, method, vec![JsValue::Object(target)])?.truthy();
    if result != is_extensible(vm, target)? {
        return Err(vm.type_error(
            "'isExtensible' on proxy: trap result does not reflect extensibility of proxy target",
        ));
    }
    Ok(result)
}

/** O.[[PreventExtensions]](). */
pub(super) fn prevent_extensions(vm: &mut Vm, id: ObjectId) -> Res<bool> {
    let Some(proxy) = vm.proxy_of(id) else {
        vm.materialize(id)?;
        if let Some(object) = vm.heap.get_mut(id) {
            object.extensible = false;
        }
        return Ok(true);
    };
    let (target, Some(method)) = trap(vm, &proxy, "preventExtensions")? else {
        let target = proxy.target.unwrap_or(id);
        return prevent_extensions(vm, target);
    };
    let result = call_trap(vm, &proxy, method, vec![JsValue::Object(target)])?.truthy();
    if result && is_extensible(vm, target)? {
        return Err(vm.type_error(
            "'preventExtensions' on proxy: trap returned truish but the proxy target is extensible",
        ));
    }
    Ok(result)
}
