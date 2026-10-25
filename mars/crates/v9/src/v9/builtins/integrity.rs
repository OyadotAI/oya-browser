/** The [[GetPrototypeOf]], [[SetPrototypeOf]], [[IsExtensible]] and [[PreventExtensions]]
internal methods as Object and Reflect see them, ported from the spec's Proxy exotic object
(V8's builtins-proxy-gen and objects.cc): a proxy asks its handler's trap and the result is
checked against the target's invariants. vm/proxy.rs leaves these four to the target, so a trap
that threw, lied about extensibility or reported a foreign prototype went unnoticed.
Trap-less proxies are skipped in a loop (`Vm::proxy_trap`), and a trap's invariant check on its
target runs one native level deeper (`Vm::deeper`): recursing per proxy let a long chain overflow
the native stack and abort the process. */
use super::*;

/** O.[[GetPrototypeOf]](). */
pub(super) fn get_prototype_of(vm: &mut Vm, id: ObjectId) -> Res<Option<ObjectId>> {
    let Some(proxy) = vm.proxy_of(id) else {
        return Ok(vm.proto_of(id));
    };
    let (proxy, target, method) = vm.proxy_trap(&proxy, "getPrototypeOf")?;
    let Some(method) = method else {
        // The chain ends at an ordinary object, so this recursion is one level.
        return get_prototype_of(vm, target);
    };
    let proto = match vm.call_trap(&proxy, method, vec![JsValue::Object(target)])? {
        JsValue::Object(proto) => Some(proto),
        JsValue::Null => None,
        _ => {
            return Err(
                vm.type_error("'getPrototypeOf' on proxy: trap returned neither object nor null")
            );
        }
    };
    if vm.deeper(|vm| is_extensible(vm, target))?
        || vm.deeper(|vm| get_prototype_of(vm, target))? == proto
    {
        return Ok(proto);
    }
    Err(vm.type_error("'getPrototypeOf' on proxy: proxy target is non-extensible but the trap did not return its actual prototype"))
}

/** O.[[SetPrototypeOf]](V). */
pub(super) fn set_prototype_of(vm: &mut Vm, id: ObjectId, proto: Option<ObjectId>) -> Res<bool> {
    let Some(proxy) = vm.proxy_of(id) else {
        return vm.set_proto(id, proto);
    };
    let (proxy, target, method) = vm.proxy_trap(&proxy, "setPrototypeOf")?;
    let Some(method) = method else {
        // The chain ends at an ordinary object, so this recursion is one level.
        return set_prototype_of(vm, target, proto);
    };
    let value = proto.map_or(JsValue::Null, JsValue::Object);
    if !vm
        .call_trap(&proxy, method, vec![JsValue::Object(target), value])?
        .truthy()
    {
        return Ok(false);
    }
    if vm.deeper(|vm| is_extensible(vm, target))?
        || vm.deeper(|vm| get_prototype_of(vm, target))? == proto
    {
        return Ok(true);
    }
    Err(vm.type_error("'setPrototypeOf' on proxy: trap returned truish for setting a new prototype on the non-extensible proxy target"))
}

/** IsExtensible(O). */
pub(super) fn is_extensible(vm: &mut Vm, id: ObjectId) -> Res<bool> {
    let Some(proxy) = vm.proxy_of(id) else {
        return Ok(vm.heap.get(id).is_none_or(|object| object.extensible));
    };
    let (proxy, target, method) = vm.proxy_trap(&proxy, "isExtensible")?;
    let Some(method) = method else {
        // The chain ends at an ordinary object, so this recursion is one level.
        return is_extensible(vm, target);
    };
    let result = vm
        .call_trap(&proxy, method, vec![JsValue::Object(target)])?
        .truthy();
    if result != vm.deeper(|vm| is_extensible(vm, target))? {
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
    let (proxy, target, method) = vm.proxy_trap(&proxy, "preventExtensions")?;
    let Some(method) = method else {
        // The chain ends at an ordinary object, so this recursion is one level.
        return prevent_extensions(vm, target);
    };
    let result = vm
        .call_trap(&proxy, method, vec![JsValue::Object(target)])?
        .truthy();
    if result && vm.deeper(|vm| is_extensible(vm, target))? {
        return Err(vm.type_error(
            "'preventExtensions' on proxy: trap returned truish but the proxy target is extensible",
        ));
    }
    Ok(result)
}
