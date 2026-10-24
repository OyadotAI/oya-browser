/**
 * Proxy objects: each internal method asks the handler's trap, or forwards to the target when the
 * handler has none. Every trap call is an ordinary script call, so call depth, the operation
 * budget and the deadline bound proxy chains like any other recursion. V9 does not check the
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
    /** Call a trap with the handler as `this`. */
    fn call_trap(&mut self, proxy: &ProxyData, trap: JsValue, args: Vec<JsValue>) -> Res<JsValue> {
        let handler = proxy.handler.map_or(JsValue::Undefined, JsValue::Object);
        self.call(trap, handler, args)
    }
    /** [[Get]]. */
    pub fn proxy_get(&mut self, proxy: &ProxyData, key: &Key, receiver: &JsValue) -> Res<JsValue> {
        match self.trap(proxy, "get")? {
            (target, Some(trap)) => {
                let args = vec![JsValue::Object(target), key.value(), receiver.clone()];
                self.call_trap(proxy, trap, args)
            }
            (target, None) => self.get_from(target, key, receiver),
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
        match self.trap(proxy, "set")? {
            (target, Some(trap)) => {
                let args = vec![
                    JsValue::Object(target),
                    key.value(),
                    value,
                    receiver.clone(),
                ];
                Ok(self.call_trap(proxy, trap, args)?.truthy())
            }
            (target, None) => {
                let receiver = if *receiver == JsValue::Object(id) {
                    JsValue::Object(target)
                } else {
                    receiver.clone()
                };
                self.set_from(target, key, value, &receiver)
            }
        }
    }
    /** [[HasProperty]]. */
    pub fn proxy_has(&mut self, proxy: &ProxyData, key: &Key) -> Res<bool> {
        match self.trap(proxy, "has")? {
            (target, Some(trap)) => Ok(self
                .call_trap(proxy, trap, vec![JsValue::Object(target), key.value()])?
                .truthy()),
            (target, None) => self.has_property(target, key),
        }
    }
    /** [[Delete]]. */
    pub fn proxy_delete(&mut self, proxy: &ProxyData, key: &Key) -> Res<bool> {
        match self.trap(proxy, "deleteProperty")? {
            (target, Some(trap)) => Ok(self
                .call_trap(proxy, trap, vec![JsValue::Object(target), key.value()])?
                .truthy()),
            (target, None) => self.delete(target, key),
        }
    }
    /** [[GetOwnProperty]]. */
    pub fn proxy_own(&mut self, proxy: &ProxyData, key: &Key) -> Res<Option<Prop>> {
        match self.trap(proxy, "getOwnPropertyDescriptor")? {
            (target, Some(trap)) => {
                let result =
                    self.call_trap(proxy, trap, vec![JsValue::Object(target), key.value()])?;
                if result.nullish() {
                    return Ok(None);
                }
                let desc = crate::v9::builtins::to_descriptor(self, &result)?;
                Ok(Some(super::property::merge(None, desc)))
            }
            (target, None) => self.own(target, key),
        }
    }
    /** [[DefineOwnProperty]]. */
    pub fn proxy_define(
        &mut self,
        proxy: &ProxyData,
        key: Key,
        desc: super::property::Descriptor,
    ) -> Res<bool> {
        match self.trap(proxy, "defineProperty")? {
            (target, Some(trap)) => {
                let probe = self.new_object()?;
                self.define(probe, key.clone(), desc)?;
                let attributes = crate::v9::builtins::from_descriptor(self, probe, &key)?;
                Ok(self
                    .call_trap(
                        proxy,
                        trap,
                        vec![JsValue::Object(target), key.value(), attributes],
                    )?
                    .truthy())
            }
            (target, None) => self.define(target, key, desc),
        }
    }
    /** [[OwnPropertyKeys]]. */
    pub fn proxy_own_keys(&mut self, proxy: &ProxyData) -> Res<Vec<Key>> {
        match self.trap(proxy, "ownKeys")? {
            (target, Some(trap)) => {
                let list = self.call_trap(proxy, trap, vec![JsValue::Object(target)])?;
                let mut keys = Vec::new();
                for value in self.iterate_to_vec(&list)? {
                    if !matches!(value, JsValue::String(_) | JsValue::Symbol(_)) {
                        return Err(self.type_error("proxy [[OwnPropertyKeys]] must return an array with only string and symbol elements"));
                    }
                    keys.push(self.to_key(&value)?);
                }
                Ok(keys)
            }
            (target, None) => self.own_keys(target),
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
        match self.trap(proxy, name)? {
            (target, Some(trap)) => {
                let list = JsValue::Object(self.new_array(args)?);
                let args = match construct {
                    Some(new_target) => vec![JsValue::Object(target), list, new_target],
                    None => vec![JsValue::Object(target), this, list],
                };
                let result = self.call_trap(proxy, trap, args)?;
                if name == "construct" && !matches!(result, JsValue::Object(_)) {
                    return Err(self.type_error("proxy [[Construct]] must return an object"));
                }
                Ok(result)
            }
            (target, None) => match construct {
                Some(new_target) => self.construct(JsValue::Object(target), args, Some(new_target)),
                None => self.call(JsValue::Object(target), this, args),
            },
        }
    }
}
