/**
 * Proxy objects: each internal method asks the handler's trap, or forwards to the target when the
 * handler has none. Trap-less proxies are followed in a loop (`proxy_trap`), so a chain of any
 * length costs no native stack; each trap call and each forward to an ordinary target is one
 * level of native recursion (`deeper`), so chains whose traps or prototypes lead back into
 * proxies meet the call-depth bound as a RangeError instead of overflowing. V9 does not check the
 * specification's invariants against non-configurable target properties, and getPrototypeOf
 * reads the target's prototype without a trap.
 */
use super::data::{Class, ProxyData};
use super::heap::{Key, Prop};
use super::{Res, Vm};
use crate::v9::value::{JsValue, ObjectId};

impl Vm<'_> {
    /** The proxy data of an object, if it is a proxy. */
    pub fn proxy_of(&self, id: ObjectId) -> Option<ProxyData> {
        if !self.proxies {
            return None;
        }
        match self.heap.get(id).map(|object| &object.class) {
            Some(Class::Proxy(proxy)) => Some((**proxy).clone()),
            _ => None,
        }
    }
    /** The target and the named trap (None when the handler has none); revoked proxies throw. */
    fn trap(&mut self, proxy: &ProxyData, name: &str) -> Res<(ObjectId, Option<JsValue>)> {
        let (Some(target), Some(handler)) = (proxy.target, proxy.handler) else {
            return Err(self.type_error(&format!(
                "Cannot perform '{name}' on a proxy that has been revoked"
            )));
        };
        let trap = self.get(&JsValue::Object(handler), &Key::from(name))?;
        if trap.nullish() {
            return Ok((target, None));
        }
        if !self.is_callable(&trap) {
            return Err(self.type_error(&format!("'{name}' on proxy: trap is not a function")));
        }
        Ok((target, Some(trap)))
    }
    /** The named trap along a proxy chain. Proxies without it forward to their targets, followed
    in a loop because recursing once per hop let a million trap-less proxies overflow the native
    stack. Returns the proxy whose handler has the trap, its target and the trap; or, with no
    trap, the last proxy and the ordinary object the chain ends at. Revoked proxies throw. */
    pub(in crate::v9) fn proxy_trap(
        &mut self,
        proxy: &ProxyData,
        name: &str,
    ) -> Res<(ProxyData, ObjectId, Option<JsValue>)> {
        let mut proxy = proxy.clone();
        loop {
            let (target, trap) = self.trap(&proxy, name)?;
            match (trap, self.proxy_of(target)) {
                (None, Some(next)) => {
                    // Each hop is work, so even a forged cyclic chain meets the operation budget.
                    self.charge(1)?;
                    proxy = next;
                }
                (trap, _) => return Ok((proxy, target, trap)),
            }
        }
    }
    /** Run one level of native recursion: past the call-depth bound it throws a RangeError, as
    script recursion does, instead of overflowing the native stack. */
    pub(in crate::v9) fn deeper<T>(&mut self, run: impl FnOnce(&mut Self) -> Res<T>) -> Res<T> {
        if self.frames.len() + self.native_depth >= self.limits.frames {
            return Err(self.range_error("Maximum call stack size exceeded"));
        }
        self.native_depth += 1;
        let result = run(self);
        self.native_depth -= 1;
        result
    }
    /** Call a trap with the handler as `this`, one native level deeper. */
    pub(in crate::v9) fn call_trap(
        &mut self,
        proxy: &ProxyData,
        trap: JsValue,
        args: Vec<JsValue>,
    ) -> Res<JsValue> {
        let handler = proxy.handler.map_or(JsValue::Undefined, JsValue::Object);
        self.deeper(|vm| vm.call(trap, handler, args))
    }
    /** [[Get]]. */
    pub fn proxy_get(&mut self, proxy: &ProxyData, key: &Key, receiver: &JsValue) -> Res<JsValue> {
        match self.proxy_trap(proxy, "get")? {
            (proxy, target, Some(trap)) => {
                let args = vec![JsValue::Object(target), key.value(), receiver.clone()];
                self.call_trap(&proxy, trap, args)
            }
            (_, target, None) => self.deeper(|vm| vm.get_from(target, key, receiver)),
        }
    }
    /** [[Set]]; the receiver becomes the target when it is the proxy itself. */
    pub fn proxy_set(
        &mut self,
        id: ObjectId,
        proxy: &ProxyData,
        key: Key,
        value: JsValue,
        receiver: &JsValue,
    ) -> Res<bool> {
        match self.proxy_trap(proxy, "set")? {
            (proxy, target, Some(trap)) => {
                let args = vec![
                    JsValue::Object(target),
                    key.value(),
                    value,
                    receiver.clone(),
                ];
                Ok(self.call_trap(&proxy, trap, args)?.truthy())
            }
            (_, target, None) => {
                let receiver = if *receiver == JsValue::Object(id) {
                    JsValue::Object(target)
                } else {
                    receiver.clone()
                };
                self.deeper(|vm| vm.set_from(target, key, value, &receiver))
            }
        }
    }
    /** [[HasProperty]]. */
    pub fn proxy_has(&mut self, proxy: &ProxyData, key: &Key) -> Res<bool> {
        match self.proxy_trap(proxy, "has")? {
            (proxy, target, Some(trap)) => Ok(self
                .call_trap(&proxy, trap, vec![JsValue::Object(target), key.value()])?
                .truthy()),
            (_, target, None) => self.deeper(|vm| vm.has_property(target, key)),
        }
    }
    /** [[Delete]]. */
    pub fn proxy_delete(&mut self, proxy: &ProxyData, key: &Key) -> Res<bool> {
        match self.proxy_trap(proxy, "deleteProperty")? {
            (proxy, target, Some(trap)) => Ok(self
                .call_trap(&proxy, trap, vec![JsValue::Object(target), key.value()])?
                .truthy()),
            (_, target, None) => self.deeper(|vm| vm.delete(target, key)),
        }
    }
    /** [[GetOwnProperty]]. */
    pub fn proxy_own(&mut self, proxy: &ProxyData, key: &Key) -> Res<Option<Prop>> {
        match self.proxy_trap(proxy, "getOwnPropertyDescriptor")? {
            (proxy, target, Some(trap)) => {
                let result =
                    self.call_trap(&proxy, trap, vec![JsValue::Object(target), key.value()])?;
                if result.nullish() {
                    return Ok(None);
                }
                let desc = crate::v9::builtins::to_descriptor(self, &result)?;
                Ok(Some(super::property::merge(None, desc)))
            }
            (_, target, None) => self.deeper(|vm| vm.own(target, key)),
        }
    }
    /** [[DefineOwnProperty]]. */
    pub fn proxy_define(
        &mut self,
        proxy: &ProxyData,
        key: Key,
        desc: super::property::Descriptor,
    ) -> Res<bool> {
        match self.proxy_trap(proxy, "defineProperty")? {
            (proxy, target, Some(trap)) => {
                let probe = self.new_object()?;
                self.define(probe, key.clone(), desc)?;
                let attributes = crate::v9::builtins::from_descriptor(self, probe, &key)?;
                Ok(self
                    .call_trap(
                        &proxy,
                        trap,
                        vec![JsValue::Object(target), key.value(), attributes],
                    )?
                    .truthy())
            }
            (_, target, None) => self.deeper(|vm| vm.define(target, key, desc)),
        }
    }
    /** [[OwnPropertyKeys]]. */
    pub fn proxy_own_keys(&mut self, proxy: &ProxyData) -> Res<Vec<Key>> {
        match self.proxy_trap(proxy, "ownKeys")? {
            (proxy, target, Some(trap)) => {
                let list = self.call_trap(&proxy, trap, vec![JsValue::Object(target)])?;
                let mut keys = Vec::new();
                for value in self.iterate_to_vec(&list)? {
                    if !matches!(value, JsValue::String(_) | JsValue::Symbol(_)) {
                        return Err(self.type_error("proxy [[OwnPropertyKeys]] must return an array with only string and symbol elements"));
                    }
                    keys.push(self.to_key(&value)?);
                }
                Ok(keys)
            }
            (_, target, None) => self.deeper(|vm| vm.own_keys(target)),
        }
    }
    /** [[Call]] and [[Construct]]: the apply and construct traps, or the target itself. */
    pub fn proxy_invoke(
        &mut self,
        proxy: &ProxyData,
        this: JsValue,
        args: Vec<JsValue>,
        construct: Option<JsValue>,
    ) -> Res<JsValue> {
        let name = if construct.is_some() {
            "construct"
        } else {
            "apply"
        };
        match self.proxy_trap(proxy, name)? {
            (proxy, target, Some(trap)) => {
                let list = JsValue::Object(self.new_array(args)?);
                let args = match construct {
                    Some(new_target) => vec![JsValue::Object(target), list, new_target],
                    None => vec![JsValue::Object(target), this, list],
                };
                let result = self.call_trap(&proxy, trap, args)?;
                if name == "construct" && !matches!(result, JsValue::Object(_)) {
                    return Err(self.type_error("proxy [[Construct]] must return an object"));
                }
                Ok(result)
            }
            (_, target, None) => match construct {
                Some(new_target) => self.construct(JsValue::Object(target), args, Some(new_target)),
                None => self.call(JsValue::Object(target), this, args),
            },
        }
    }
}
