/** Proxy traps cannot contradict target property descriptors or its extensibility. */
use super::{JsValue, Key, ObjectId, Prop, Res, Vm};
use crate::v9::vm::heap::{CONFIGURABLE, ENUMERABLE, Slot, WRITABLE};
use crate::v9::vm::property::{Descriptor, compatible, merge};
use std::collections::HashSet;

impl Vm<'_> {
    /** Protect values held only in native locals while invariant checks can re-enter scripts. */
    pub(super) fn proxy_check<T>(
        &mut self,
        target: ObjectId,
        values: &[JsValue],
        check: impl FnOnce(&mut Self) -> Res<T>,
    ) -> Res<T> {
        let mark = self.pins.len();
        self.pins.push(JsValue::Object(target));
        self.pins.extend_from_slice(values);
        let result = check(self);
        self.pins.truncate(mark);
        result
    }
    /** Target methods are genuine exotic operations, guarded against recursive handler lookup. */
    fn target_own(&mut self, target: ObjectId, key: &Key) -> Res<Option<Prop>> {
        self.deeper(|vm| vm.own(target, key))
    }
    /** The target's real extensibility can itself be a Proxy operation. */
    fn target_extensible(&mut self, target: ObjectId) -> Res<bool> {
        self.deeper(|vm| crate::v9::builtins::object_is_extensible(vm, target))
    }
    /** Frozen data and getter-less fixed accessors constrain a successful get trap. */
    pub(super) fn check_get(&mut self, target: ObjectId, key: &Key, result: &JsValue) -> Res<()> {
        let Some(prop) = self.target_own(target, key)? else {
            return Ok(());
        };
        if prop.attrs & CONFIGURABLE != 0 {
            return Ok(());
        }
        match prop.slot {
            Slot::Data(value) if prop.attrs & WRITABLE == 0 && !result.same_value(&value) => {
                let name = self.proxy_invariant_key(key);
                let expected = self.proxy_invariant_value(&value);
                let actual = self.proxy_invariant_value(result);
                Err(self.type_error(&format!("'get' on proxy: property '{name}' is a read-only and non-configurable data property on the proxy target but the proxy did not return its actual value (expected '{expected}' but got '{actual}')")))
            }
            Slot::Accessor(None, _) if *result != JsValue::Undefined => {
                let name = self.proxy_invariant_key(key);
                let actual = self.proxy_invariant_value(result);
                Err(self.type_error(&format!("'get' on proxy: property '{name}' is a non-configurable accessor property on the proxy target and does not have a getter function, but the trap did not return 'undefined' (got '{actual}')")))
            }
            _ => Ok(()),
        }
    }
    /** Property names are rendered from inert key data, without calling script. */
    fn proxy_invariant_key(&self, key: &Key) -> String {
        match key {
            Key::Sym(id) => format!("Symbol({})", crate::v9::builtins::symbol_text(self, *id)),
            _ => crate::v9::vm::property::key_text(key),
        }
    }
    /** Describe values without invoking authored getters or conversion methods. */
    fn proxy_invariant_value(&self, value: &JsValue) -> String {
        if matches!(value, JsValue::Object(_)) { "#<Object>".into() } else { self.iterator_value_description(value) }
    }
    /** A truthy set trap still cannot claim a forbidden write to a fixed target property. */
    pub(super) fn check_set(&mut self, target: ObjectId, key: &Key, value: &JsValue) -> Res<()> {
        let Some(prop) = self.target_own(target, key)? else {
            return Ok(());
        };
        if prop.attrs & CONFIGURABLE != 0 {
            return Ok(());
        }
        let valid = match prop.slot {
            Slot::Data(current) => prop.attrs & WRITABLE != 0 || value.same_value(&current),
            Slot::Accessor(_, None) => false,
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(self.type_error("Proxy set trap contradicts a non-configurable target property"))
        }
    }
    /** Neither has nor delete traps can hide fixed or non-extensible target properties. */
    pub(super) fn check_hidden(&mut self, target: ObjectId, key: &Key) -> Res<()> {
        let Some(prop) = self.target_own(target, key)? else {
            return Ok(());
        };
        if prop.attrs & CONFIGURABLE == 0 || !self.target_extensible(target)? {
            return Err(self.type_error(
                "Proxy trap cannot hide a non-configurable or non-extensible target property",
            ));
        }
        Ok(())
    }
    /** A descriptor trap is completed only after reading target facts in specification order. */
    pub(super) fn checked_own(
        &mut self,
        target: ObjectId,
        key: &Key,
        result: &JsValue,
    ) -> Res<Option<Prop>> {
        if !matches!(result, JsValue::Object(_) | JsValue::Undefined) {
            return Err(self.type_error("Proxy descriptor trap must return an object or undefined"));
        }
        let current = self.target_own(target, key)?;
        if *result == JsValue::Undefined {
            if let Some(prop) = current {
                if prop.attrs & CONFIGURABLE == 0 || !self.target_extensible(target)? {
                    return Err(
                        self.type_error("Proxy descriptor trap cannot hide a target property")
                    );
                }
            }
            return Ok(None);
        }
        let extensible = self.target_extensible(target)?;
        let descriptor = crate::v9::builtins::to_descriptor(self, result)?;
        let prop = merge(None, descriptor);
        if !compatible(extensible, &complete(&prop), current.as_ref())
            || forbidden_fixed(&prop, current.as_ref())
        {
            return Err(self.type_error("Proxy descriptor trap contradicts the target descriptor"));
        }
        Ok(Some(prop))
    }
    /** A successful define trap must describe a change the target can really support. */
    pub(super) fn check_define(
        &mut self,
        target: ObjectId,
        key: &Key,
        descriptor: &Descriptor,
    ) -> Res<()> {
        let current = self.target_own(target, key)?;
        let extensible = self.target_extensible(target)?;
        let setting_fixed = descriptor.configurable == Some(false);
        let valid = compatible(extensible, descriptor, current.as_ref())
            && match current.as_ref() {
                None => !setting_fixed,
                Some(prop) => {
                    !(setting_fixed && prop.attrs & CONFIGURABLE != 0
                        || prop.attrs & CONFIGURABLE == 0
                            && prop.attrs & WRITABLE != 0
                            && matches!(prop.slot, Slot::Data(_))
                            && descriptor.writable == Some(false))
                }
            };
        if valid {
            Ok(())
        } else {
            Err(self.type_error("Proxy defineProperty trap contradicts the target descriptor"))
        }
    }
    /** ownKeys uses array-like reads, not iteration, and rejects duplicate names. */
    pub(super) fn checked_keys(&mut self, target: ObjectId, result: &JsValue) -> Res<Vec<Key>> {
        if !matches!(result, JsValue::Object(_)) {
            return Err(self.type_error("Proxy ownKeys trap must return an object"));
        }
        let length = self.length_of(result)?;
        if length > self.limits.array_length {
            return Err(self.range_error("Proxy ownKeys result is too large"));
        }
        let mut keys = Vec::with_capacity(length);
        let mut seen = HashSet::new();
        for index in 0..length {
            let value = self.get(result, &Key::from(&index.to_string()))?;
            if !matches!(value, JsValue::String(_) | JsValue::Symbol(_)) {
                return Err(self.type_error("Proxy ownKeys result contains a non-property key"));
            }
            let key = self.to_key(&value)?;
            if !seen.insert(key.clone()) {
                return Err(self.type_error("Proxy ownKeys result contains duplicate keys"));
            }
            keys.push(key);
        }
        self.check_key_set(target, &seen)?;
        Ok(keys)
    }
    /** A non-extensible target has an exact key set; every target retains its fixed keys. */
    fn check_key_set(&mut self, target: ObjectId, result: &HashSet<Key>) -> Res<()> {
        let extensible = self.target_extensible(target)?;
        let target_keys = self.deeper(|vm| vm.own_keys(target))?;
        for key in &target_keys {
            let fixed = self
                .target_own(target, key)?
                .is_some_and(|prop| prop.attrs & CONFIGURABLE == 0);
            if (fixed || !extensible) && !result.contains(key) {
                return Err(self.type_error("Proxy ownKeys trap omitted a required target key"));
            }
        }
        if !extensible && target_keys.len() != result.len() {
            return Err(
                self.type_error("Proxy ownKeys trap added a key to a non-extensible target")
            );
        }
        Ok(())
    }
}

/** CompletePropertyDescriptor gives every slot and attribute explicitly for compatibility. */
fn complete(prop: &Prop) -> Descriptor {
    let mut descriptor = Descriptor {
        enumerable: Some(prop.attrs & ENUMERABLE != 0),
        configurable: Some(prop.attrs & CONFIGURABLE != 0),
        ..Descriptor::default()
    };
    match &prop.slot {
        Slot::Data(value) => {
            descriptor.value = Some(value.clone());
            descriptor.writable = Some(prop.attrs & WRITABLE != 0);
        }
        Slot::Accessor(get, set) => {
            descriptor.get = Some(*get);
            descriptor.set = Some(*set);
        }
    }
    descriptor
}

/** A trap cannot newly report fixed attributes the target itself has not adopted. */
fn forbidden_fixed(result: &Prop, current: Option<&Prop>) -> bool {
    if result.attrs & CONFIGURABLE != 0 {
        return false;
    }
    current.is_none_or(|current| {
        current.attrs & CONFIGURABLE != 0
            || matches!(result.slot, Slot::Data(_))
                && result.attrs & WRITABLE == 0
                && matches!(current.slot, Slot::Data(_))
                && current.attrs & WRITABLE != 0
    })
}

/** FromPropertyDescriptor preserves absent fields passed to a defineProperty trap. */
pub(super) fn descriptor_object(vm: &mut Vm, desc: &Descriptor) -> Res<JsValue> {
    let id = vm.new_object()?;
    let mut fields = Vec::new();
    if let Some(value) = &desc.value {
        fields.push(("value", value.clone()));
    }
    if let Some(value) = desc.writable {
        fields.push(("writable", JsValue::Boolean(value)));
    }
    if let Some(value) = desc.get {
        fields.push(("get", value.map_or(JsValue::Undefined, JsValue::Object)));
    }
    if let Some(value) = desc.set {
        fields.push(("set", value.map_or(JsValue::Undefined, JsValue::Object)));
    }
    if let Some(value) = desc.enumerable {
        fields.push(("enumerable", JsValue::Boolean(value)));
    }
    if let Some(value) = desc.configurable {
        fields.push(("configurable", JsValue::Boolean(value)));
    }
    for (key, value) in fields {
        vm.put(id, key, value)?;
    }
    Ok(JsValue::Object(id))
}
