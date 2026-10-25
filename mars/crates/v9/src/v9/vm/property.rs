/** Property semantics: own lookup, prototype chains, assignment, definition, deletion and keys. */
use super::data::{Class, FunctionKind};
use super::heap::{
    CONFIGURABLE, DEFAULT_ATTRS, ENUMERABLE, INTRINSIC_BASE, Key, NATIVE_BASE, Object, Prop, Slot,
    WRITABLE,
};
use super::realm::{Static, realm};
use super::{Res, Thrown, Vm};
use crate::constants::VM_PROPERTY_KEY_UNITS;
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};

/** A property descriptor as defineProperty takes it; absent fields stay unchanged. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct Descriptor {
    pub value: Option<JsValue>,
    pub get: Option<Option<ObjectId>>,
    pub set: Option<Option<ObjectId>>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

impl Descriptor {
    /** A data descriptor with every attribute. */
    pub fn data(value: JsValue) -> Self {
        Self {
            value: Some(value),
            writable: Some(true),
            enumerable: Some(true),
            configurable: Some(true),
            ..Self::default()
        }
    }
    /** Whether it describes an accessor. */
    pub fn accessor(&self) -> bool {
        self.get.is_some() || self.set.is_some()
    }
}

/** What the fast property path found on one object. */
enum Fast {
    /** An own data property's value. */
    Value(JsValue),
    /** No own property: the walk continues at this prototype. */
    Missing(Option<ObjectId>),
    /** Exotic object or accessor: the full path decides. */
    Slow,
}

/** Whether `own` reads `key` on this class straight from the property list with no exotic
behaviour (proxies, host objects, namespaces, and the index and length keys of arrays, strings
and typed views all have their own). */
fn ordinary_slot(class: &Class, key: &Key) -> bool {
    match class {
        Class::Proxy(_) | Class::Host(_) | Class::Namespace(_) => false,
        Class::Array(_) | Class::String(_) | Class::View(_) => {
            key.index().is_none() && !key_is(key, "length")
        }
        _ => true,
    }
}

/** Longest prototype chain walked before assuming a cycle. */
pub(in crate::v9) const CHAIN_STEPS: usize = 4096;

impl Vm<'_> {
    /** Whether the identity has a heap record (including materialized intrinsics). */
    pub fn in_heap(&self, id: ObjectId) -> bool {
        self.heap.get(id).is_some()
    }
    /** The prototype of an object. */
    pub fn proto_of(&self, id: ObjectId) -> Option<ObjectId> {
        let mut id = id;
        // A proxy reports its target's prototype (no getPrototypeOf trap); chains are bounded.
        for _ in 0..CHAIN_STEPS {
            match self.heap.get(id).map(|object| &object.class) {
                Some(Class::Proxy(proxy)) => id = proxy.target?,
                _ => break,
            }
        }
        if let Some(object) = self.heap.get(id) {
            return object.proto;
        }
        if id >= NATIVE_BASE {
            return Some(realm().ids.function_prototype);
        }
        realm().object(id).and_then(|def| def.proto)
    }
    /** Copy an intrinsic into the heap so scripts can change it. */
    pub fn materialize(&mut self, id: ObjectId) -> Res<()> {
        if id < INTRINSIC_BASE || self.in_heap(id) {
            return Ok(());
        }
        let mut object = if id >= NATIVE_BASE {
            let index = id - NATIVE_BASE;
            Object::new(
                Class::Function(Box::new(super::data::Function {
                    kind: FunctionKind::Native {
                        id: index,
                        slots: Vec::new(),
                    },
                    home: None,
                    fields: None,
                })),
                Some(realm().ids.function_prototype),
            )
        } else {
            let def = realm()
                .object(id)
                .ok_or_else(|| EngineError::invalid("Invalid V9 intrinsic identity."))?;
            let class = match def.call {
                Some(native) => Class::Function(Box::new(super::data::Function {
                    kind: FunctionKind::Native {
                        id: native,
                        slots: Vec::new(),
                    },
                    home: None,
                    fields: None,
                })),
                None => Class::Plain,
            };
            let mut object = Object::new(class, def.proto);
            for (key, value, attrs) in &def.props {
                let slot = match value {
                    Static::Value(value) => Slot::Data(value.clone()),
                    Static::Accessor(get, set) => Slot::Accessor(*get, *set),
                };
                object.props.insert(
                    key.clone(),
                    Prop {
                        slot,
                        attrs: *attrs,
                    },
                );
            }
            object
        };
        object.extensible = id != realm().ids.throw_type_error;
        self.heap.insert_at(id, object);
        Ok(())
    }
    /** An own property, if present. */
    pub fn own(&mut self, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
        let Some(object) = self.heap.get(id) else {
            return Ok(self.intrinsic_own(id, key));
        };
        match &object.class {
            // Private keys live on the proxy object itself and never reach a trap.
            Class::Proxy(proxy) if !key.hidden() => {
                let proxy = (**proxy).clone();
                return self.proxy_own(&proxy, key);
            }
            Class::Array(elements) => {
                if let Some(index) = key.index() {
                    return Ok(elements
                        .get(index as usize)
                        .filter(|value| **value != JsValue::Hole)
                        .map(|value| Prop::data(value.clone())));
                }
                if key_is(key, "length") {
                    return Ok(Some(Prop {
                        slot: Slot::Data(JsValue::Number(elements.len() as f64)),
                        attrs: WRITABLE,
                    }));
                }
            }
            Class::String(text) => {
                if let Some(index) = key.index() {
                    if let Some(unit) = text.as_units().get(index as usize) {
                        return Ok(Some(Prop {
                            slot: Slot::Data(JsValue::String(JsString::from_slice(&[*unit])?)),
                            attrs: ENUMERABLE,
                        }));
                    }
                } else if key_is(key, "length") {
                    return Ok(Some(Prop {
                        slot: Slot::Data(JsValue::Number(text.len() as f64)),
                        attrs: 0,
                    }));
                }
            }
            Class::Host(_) => {
                if let Some(prop) = super::super::platform::host_own(self, id, key)? {
                    return Ok(Some(prop));
                }
            }
            Class::Namespace(module) => {
                let module = module.clone();
                if let Some(prop) = crate::v9::modules::namespace_own(self, &module, key)? {
                    return Ok(Some(prop));
                }
            }
            Class::View(_) => {
                if let Some(index) = key.index() {
                    return Ok(
                        crate::v9::builtins::typed_load(self, id, index as usize).map(|value| {
                            Prop {
                                slot: Slot::Data(value),
                                attrs: WRITABLE | ENUMERABLE | CONFIGURABLE,
                            }
                        }),
                    );
                }
            }
            _ => {}
        }
        let object = self.heap.get(id).expect("checked above");
        if let Some(prop) = object.props.get(key) {
            return Ok(Some(prop.clone()));
        }
        if object.function().is_some() && !self.virtual_deleted(id, key) {
            return self.function_virtual(id, key);
        }
        Ok(None)
    }
    /** Static properties of an intrinsic not yet copied to the heap. */
    fn intrinsic_own(&self, id: ObjectId, key: &Key) -> Option<Prop> {
        if id >= NATIVE_BASE {
            let def = realm().native(id)?;
            return if key_is(key, "name") {
                Some(Prop {
                    slot: Slot::Data(super::realm::string(def.name)),
                    attrs: CONFIGURABLE,
                })
            } else if key_is(key, "length") {
                Some(Prop {
                    slot: Slot::Data(JsValue::Number(f64::from(def.length))),
                    attrs: CONFIGURABLE,
                })
            } else {
                None
            };
        }
        let (_, value, attrs) = realm().object(id)?.get(key)?;
        Some(Prop {
            slot: match value {
                Static::Value(value) => Slot::Data(value.clone()),
                Static::Accessor(get, set) => Slot::Accessor(*get, *set),
            },
            attrs: *attrs,
        })
    }
    /** Whether a function's virtual `name` or `length` was deleted. */
    fn virtual_deleted(&self, id: ObjectId, key: &Key) -> bool {
        (key_is(key, "name") || key_is(key, "length"))
            && self
                .heap
                .get(id)
                .is_some_and(|o| o.props.get(&deleted_mark(key)).is_some())
    }
    /** `name`, `length` and a lazily created `prototype` of functions. */
    fn function_virtual(&mut self, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
        let readonly = |value: JsValue| Prop {
            slot: Slot::Data(value),
            attrs: CONFIGURABLE,
        };
        let kind = self
            .heap
            .get(id)
            .and_then(|object| object.function())
            .map(|function| function.kind.clone());
        let Some(kind) = kind else {
            return Ok(None);
        };
        match kind {
            FunctionKind::Closure { site, .. } => {
                let Some(code) = self.registry.get(site).cloned() else {
                    return Ok(None);
                };
                if key_is(key, "name") {
                    return Ok(Some(readonly(JsValue::String(code.name.clone()))));
                }
                if key_is(key, "length") {
                    return Ok(Some(readonly(JsValue::Number(f64::from(code.length)))));
                }
                let flags = code.flags;
                let plain = !flags.arrow && !flags.method && !flags.asynchronous;
                if plain
                    && !flags.generator
                    && !code.strict
                    && (key_is(key, "caller") || key_is(key, "arguments"))
                {
                    // V8 gives sloppy functions their own null `caller` and `arguments`, so
                    // pages reading `f.caller` never reach Function.prototype's thrower.
                    return Ok(Some(Prop {
                        slot: Slot::Data(JsValue::Null),
                        attrs: 0,
                    }));
                }
                if key_is(key, "prototype")
                    && (flags.generator
                        || !flags.arrow && !flags.method && !flags.asynchronous && !flags.fields)
                {
                    let proto = if flags.generator && flags.asynchronous {
                        realm().ids.async_generator_prototype
                    } else if flags.generator {
                        realm().ids.generator_prototype
                    } else {
                        realm().ids.object_prototype
                    };
                    let prototype = self.alloc(Object::new(Class::Plain, Some(proto)))?;
                    if !flags.generator {
                        self.heap.put(
                            prototype,
                            Key::from("constructor"),
                            Prop {
                                slot: Slot::Data(JsValue::Object(id)),
                                attrs: WRITABLE | CONFIGURABLE,
                            },
                        );
                    }
                    let prop = Prop {
                        slot: Slot::Data(JsValue::Object(prototype)),
                        attrs: WRITABLE,
                    };
                    self.heap.put(id, Key::from("prototype"), prop.clone());
                    return Ok(Some(prop));
                }
            }
            FunctionKind::Native { id: native, .. } => {
                let def = &realm().natives[native as usize];
                if key_is(key, "name") {
                    return Ok(Some(readonly(super::realm::string(def.name))));
                }
                if key_is(key, "length") {
                    return Ok(Some(readonly(JsValue::Number(f64::from(def.length)))));
                }
            }
            FunctionKind::Bound { target, args, .. } => {
                if key_is(key, "name") {
                    let name = self.get(&JsValue::Object(target), &Key::from("name"))?;
                    let name = match name {
                        JsValue::String(name) => name,
                        _ => JsString::empty(),
                    };
                    let bound = JsString::from_utf8("bound ")?.concat(&name)?;
                    return Ok(Some(readonly(JsValue::String(bound))));
                }
                if key_is(key, "length") {
                    // BoundFunctionCreate reads `length` only when the target owns it.
                    let length_key = Key::from("length");
                    let length = if self.own(target, &length_key)?.is_some() {
                        self.get(&JsValue::Object(target), &length_key)?
                    } else {
                        JsValue::Undefined
                    };
                    let length = match length {
                        JsValue::Number(length) => (length - args.len() as f64).max(0.0),
                        _ => 0.0,
                    };
                    return Ok(Some(readonly(JsValue::Number(length))));
                }
            }
        }
        Ok(None)
    }
    /** [[Get]] on any value; primitives read through their prototypes. */
    pub fn get(&mut self, base: &JsValue, key: &Key) -> Res<JsValue> {
        let start = match base {
            JsValue::Object(id) => *id,
            JsValue::String(text) => {
                if let Some(index) = key.index() {
                    if let Some(unit) = text.as_units().get(index as usize) {
                        return Ok(JsValue::String(JsString::from_slice(&[*unit])?));
                    }
                } else if key_is(key, "length") {
                    return Ok(JsValue::Number(text.len() as f64));
                }
                realm().ids.string_prototype
            }
            JsValue::Number(_) => realm().ids.number_prototype,
            JsValue::Boolean(_) => realm().ids.boolean_prototype,
            JsValue::Symbol(_) => realm().ids.symbol_prototype,
            JsValue::BigInt(_) => realm().ids.bigint_prototype,
            JsValue::Undefined | JsValue::Null | JsValue::Hole => {
                let what = if matches!(base, JsValue::Null) {
                    "null"
                } else {
                    "undefined"
                };
                return Err(self.type_error(&format!(
                    "Cannot read properties of {what} (reading '{}')",
                    key_text(key)
                )));
            }
        };
        self.get_from(start, key, base)
    }
    /** [[Get]] starting at an object with an explicit receiver for getters. */
    pub fn get_from(&mut self, start: ObjectId, key: &Key, receiver: &JsValue) -> Res<JsValue> {
        let mut id = start;
        for _ in 0..CHAIN_STEPS {
            match self.fast_own(id, key) {
                Fast::Value(value) => return Ok(value),
                Fast::Missing(Some(proto)) => {
                    id = proto;
                    continue;
                }
                Fast::Missing(None) => return Ok(JsValue::Undefined),
                Fast::Slow => {}
            }
            if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
                return self.proxy_get(&proxy, key, receiver);
            }
            if let Some(prop) = self.own(id, key)? {
                return match prop.slot {
                    Slot::Data(value) => Ok(value),
                    Slot::Accessor(Some(getter), _) => {
                        self.call(JsValue::Object(getter), receiver.clone(), Vec::new())
                    }
                    Slot::Accessor(None, _) => Ok(JsValue::Undefined),
                };
            }
            match self.proto_of(id) {
                Some(proto) => id = proto,
                None => return Ok(JsValue::Undefined),
            }
        }
        Err(EngineError::limit("V9 prototype chain", CHAIN_STEPS).into())
    }
    /** [[Set]] on any value; failures throw in strict code. */
    pub fn set(&mut self, base: &JsValue, key: Key, value: JsValue, strict: bool) -> Res<()> {
        let start = match base {
            JsValue::Object(id) => *id,
            JsValue::Undefined | JsValue::Null | JsValue::Hole => {
                let what = if matches!(base, JsValue::Null) {
                    "null"
                } else {
                    "undefined"
                };
                return Err(self.type_error(&format!(
                    "Cannot set properties of {what} (setting '{}')",
                    key_text(&key)
                )));
            }
            JsValue::String(_) => realm().ids.string_prototype,
            JsValue::Number(_) => realm().ids.number_prototype,
            JsValue::Boolean(_) => realm().ids.boolean_prototype,
            JsValue::Symbol(_) => realm().ids.symbol_prototype,
            JsValue::BigInt(_) => realm().ids.bigint_prototype,
        };
        let done = self.set_from(start, key.clone(), value, base)?;
        if !done && strict {
            return Err(self.type_error(&format!(
                "Cannot assign to read only property '{}' of object",
                key_text(&key)
            )));
        }
        Ok(())
    }
    /** The ordinary [[Set]] walk: setters and read-only data stop it; otherwise write on the receiver. */
    pub fn set_from(
        &mut self,
        start: ObjectId,
        key: Key,
        value: JsValue,
        receiver: &JsValue,
    ) -> Res<bool> {
        if *receiver == JsValue::Object(start) && self.fast_write(start, &key, &value) {
            return Ok(true);
        }
        let mut id = start;
        for _ in 0..CHAIN_STEPS {
            if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
                return self.proxy_set(id, &proxy, key, value, receiver);
            }
            if let Some(prop) = self.own(id, &key)? {
                match prop.slot {
                    Slot::Accessor(_, Some(setter)) => {
                        self.call(JsValue::Object(setter), receiver.clone(), vec![value])?;
                        return Ok(true);
                    }
                    Slot::Accessor(_, None) => return Ok(false),
                    Slot::Data(_) if prop.attrs & WRITABLE == 0 => return Ok(false),
                    Slot::Data(_) => break,
                }
            }
            match self.proto_of(id) {
                Some(proto) => id = proto,
                None => break,
            }
        }
        let JsValue::Object(target) = receiver else {
            return Ok(false);
        };
        // A namespace receiver reached through `super` or Reflect.set: [[GetOwnProperty]] (which
        // throws for an export in its TDZ), then [[DefineOwnProperty]] (spec 10.1.9.2).
        if *target != start {
            let desc = Descriptor {
                value: Some(value.clone()),
                ..Descriptor::default()
            };
            if let Some(done) = crate::v9::modules::namespace_define(self, *target, &key, &desc)? {
                return Ok(done);
            }
        }
        self.write_own(*target, key, value)
    }
    /** One step of the ordinary [[Get]] walk answered straight from a heap object's property
    list, as V8's fast-properties load does; anything exotic, an accessor, or a function's
    virtual names says Slow and takes the full path, so behaviour is unchanged. */
    fn fast_own(&self, id: ObjectId, key: &Key) -> Fast {
        let Some(object) = self.heap.get(id) else {
            return Fast::Slow;
        };
        if !ordinary_slot(&object.class, key) {
            return Fast::Slow;
        }
        match object.props.get(key) {
            Some(Prop {
                slot: Slot::Data(value),
                ..
            }) => Fast::Value(value.clone()),
            Some(_) => Fast::Slow,
            None if object.function().is_some() => Fast::Slow,
            None => Fast::Missing(object.proto),
        }
    }
    /** Overwrite an own writable data property of an ordinary object in place, the common
    `o.x = v` on an existing field. False leaves every other case to the full [[Set]]. */
    fn fast_write(&mut self, id: ObjectId, key: &Key, value: &JsValue) -> bool {
        let Some(object) = self.heap.get_mut(id) else {
            return false;
        };
        if !ordinary_slot(&object.class, key) {
            return false;
        }
        let Some(prop) = object.props.get_mut(key) else {
            return false;
        };
        let Slot::Data(old) = &mut prop.slot else {
            return false;
        };
        if prop.attrs & WRITABLE == 0 {
            return false;
        }
        let delta = value.string_units() as isize - old.string_units() as isize;
        *old = value.clone();
        self.heap.adjust(0, delta);
        true
    }
    /** Create or update an own data property as assignment does. */
    pub fn write_own(&mut self, id: ObjectId, key: Key, value: JsValue) -> Res<bool> {
        if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
            let desc = Descriptor {
                value: Some(value),
                writable: Some(true),
                enumerable: Some(true),
                configurable: Some(true),
                ..Descriptor::default()
            };
            return self.proxy_define(&proxy, key, desc);
        }
        self.materialize(id)?;
        if let Some(handled) = self.exotic_write(id, &key, &value)? {
            return Ok(handled);
        }
        let object = self
            .heap
            .get(id)
            .ok_or_else(|| EngineError::invalid("Invalid V9 object reference."))?;
        match object.props.get(&key) {
            Some(prop) => {
                if prop.attrs & WRITABLE == 0 || matches!(prop.slot, Slot::Accessor(..)) {
                    return Ok(false);
                }
                let attrs = prop.attrs;
                self.heap.put(
                    id,
                    key,
                    Prop {
                        slot: Slot::Data(value),
                        attrs,
                    },
                );
            }
            None => {
                if !object.extensible {
                    return Ok(false);
                }
                check_key(&key)?;
                self.heap.put(id, key, Prop::data(value));
            }
        }
        Ok(true)
    }
    /** Arrays and host objects handle some keys themselves; None means ordinary handling. */
    fn exotic_write(&mut self, id: ObjectId, key: &Key, value: &JsValue) -> Res<Option<bool>> {
        let limit = self.limits.array_length;
        let Some(object) = self.heap.get(id) else {
            return Ok(None);
        };
        match &object.class {
            Class::Array(elements) => {
                if let Some(index) = key.index() {
                    let index = index as usize;
                    if index >= limit {
                        return Err(EngineError::limit("V9 array length", limit).into());
                    }
                    if index >= elements.len() && !object.extensible {
                        return Ok(Some(false));
                    }
                    self.array_store(id, index, value.clone());
                    return Ok(Some(true));
                }
                if key_is(key, "length") {
                    let length = self.array_length_value(value)?;
                    self.set_array_length(id, length)?;
                    return Ok(Some(true));
                }
                Ok(None)
            }
            Class::String(_) if key.index().is_some() || key_is(key, "length") => Ok(Some(false)),
            Class::Host(_) => super::super::platform::host_write(self, id, key, value),
            // A namespace's exports are read-only to their importers.
            Class::Namespace(_) => Ok(Some(false)),
            // Integer-indexed: in-range writes store into the buffer, others are dropped.
            Class::View(_) if key.index().is_some() => {
                let index = key.index().unwrap_or(0) as usize;
                crate::v9::builtins::typed_store(self, id, index, value)?;
                Ok(Some(true))
            }
            _ => Ok(None),
        }
    }
    /** Store one array element, growing with holes and keeping counts. */
    pub fn array_store(&mut self, id: ObjectId, index: usize, value: JsValue) {
        let Some(Class::Array(elements)) = self.heap.get_mut(id).map(|object| &mut object.class)
        else {
            return;
        };
        let (mut properties, mut units) = (0isize, 0isize);
        if index >= elements.len() {
            elements.resize(index + 1, JsValue::Hole);
        }
        let old = std::mem::replace(&mut elements[index], value);
        let new = &elements[index];
        if old == JsValue::Hole && *new != JsValue::Hole {
            properties += 1;
        } else if old != JsValue::Hole && *new == JsValue::Hole {
            properties -= 1;
        }
        units += new.string_units() as isize - old.string_units() as isize;
        self.heap.adjust(properties, units);
    }
    /** ToUint32 for array lengths, rejecting non-integers. */
    fn array_length_value(&mut self, value: &JsValue) -> Res<usize> {
        let number = self.to_number(value)?;
        if number < 0.0 || number.fract() != 0.0 || number > f64::from(u32::MAX) {
            return Err(self.range_error("Invalid array length"));
        }
        Ok(number as usize)
    }
    /** Truncate or extend an array. */
    pub fn set_array_length(&mut self, id: ObjectId, length: usize) -> Res<()> {
        if length > self.limits.array_length {
            return Err(EngineError::limit("V9 array length", self.limits.array_length).into());
        }
        let Some(Class::Array(elements)) = self.heap.get_mut(id).map(|object| &mut object.class)
        else {
            return Ok(());
        };
        let removed: Vec<JsValue> = if length < elements.len() {
            elements.drain(length..).collect()
        } else {
            elements.resize(length, JsValue::Hole);
            Vec::new()
        };
        let properties = removed
            .iter()
            .filter(|value| **value != JsValue::Hole)
            .count() as isize;
        let units = removed.iter().map(JsValue::string_units).sum::<usize>() as isize;
        self.heap.adjust(-properties, -units);
        Ok(())
    }
    /** [[DefineOwnProperty]] with a partial descriptor. Returns false when refused. */
    pub fn define(&mut self, id: ObjectId, key: Key, desc: Descriptor) -> Res<bool> {
        if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
            return self.proxy_define(&proxy, key, desc);
        }
        if let Some(done) = crate::v9::modules::namespace_define(self, id, &key, &desc)? {
            return Ok(done);
        }
        self.materialize(id)?;
        let existing = self.own(id, &key)?;
        let is_array = matches!(self.heap.get(id).map(|o| &o.class), Some(Class::Array(_)));
        if is_array && (key.index().is_some() || key_is(&key, "length")) {
            if let Some(value) = desc.value.clone() {
                return self
                    .exotic_write(id, &key, &value)
                    .map(|done| done.unwrap_or(true));
            }
            if desc.accessor() && key.index().is_some() {
                return Err(self.unsupported_error("accessor properties on array elements"));
            }
            return Ok(true);
        }
        if let Some(existing) = &existing
            && existing.attrs & CONFIGURABLE == 0
        {
            // A non-configurable property only accepts a new value while writable, or the same
            // value again (a polyfill redefining `Number.MAX_SAFE_INTEGER` is allowed to).
            let same_value = match (&desc.value, &existing.slot) {
                (None, _) => true,
                (Some(value), Slot::Data(current)) => value.same_value(current),
                _ => false,
            };
            let compatible = !desc.accessor()
                && desc.configurable != Some(true)
                && desc
                    .enumerable
                    .is_none_or(|e| e == (existing.attrs & ENUMERABLE != 0))
                && (existing.attrs & WRITABLE != 0 || same_value && desc.writable != Some(true));
            if !compatible {
                return Ok(false);
            }
        }
        let extensible = self.heap.get(id).is_some_and(|object| object.extensible);
        if existing.is_none() && !extensible {
            return Ok(false);
        }
        if existing.is_none() {
            check_key(&key)?;
        }
        let prop = merge(existing, desc);
        if let Some(Class::Host(_)) = self.heap.get(id).map(|object| &object.class)
            && let Slot::Data(value) = &prop.slot
            && let Some(done) = super::super::platform::host_write(self, id, &key, value)?
        {
            return Ok(done);
        }
        self.heap.put(id, key, prop);
        Ok(true)
    }
    /** [[Delete]]; non-configurable properties stay. */
    pub fn delete(&mut self, id: ObjectId, key: &Key) -> Res<bool> {
        if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
            return self.proxy_delete(&proxy, key);
        }
        if let Some(exported) = crate::v9::modules::namespace_has(self, id, key) {
            return Ok(!exported);
        }
        let Some(prop) = self.own(id, key)? else {
            return Ok(true);
        };
        if prop.attrs & CONFIGURABLE == 0 {
            if matches!(self.heap.get(id).map(|o| &o.class), Some(Class::Array(_)))
                && key.index().is_some()
            {
                self.array_store(id, key.index().unwrap_or(0) as usize, JsValue::Hole);
                return Ok(true);
            }
            return Ok(false);
        }
        self.materialize(id)?;
        if let Some(Class::Host(_)) = self.heap.get(id).map(|object| &object.class)
            && super::super::platform::host_delete(self, id, key)?
        {
            return Ok(true);
        }
        let virtual_slot = self.heap.get(id).is_some_and(|o| o.function().is_some());
        if virtual_slot && (key_is(key, "name") || key_is(key, "length")) {
            // A function's `name` and `length` live outside its property table until written
            // (a built-in's may also sit in it, over the virtual one); deleting one leaves a hidden mark so the virtual value stops answering (spec:
            // SetFunctionName and SetFunctionLength define ordinary configurable properties).
            let mark = Prop {
                slot: Slot::Data(JsValue::Undefined),
                attrs: 0,
            };
            self.heap.put(id, deleted_mark(key), mark);
        }
        self.heap.remove(id, key);
        Ok(true)
    }
    /** [[HasProperty]] along the prototype chain. */
    pub fn has_property(&mut self, id: ObjectId, key: &Key) -> Res<bool> {
        let mut current = id;
        for _ in 0..CHAIN_STEPS {
            if let Some(proxy) = self.proxy_of(current).filter(|_| !key.hidden()) {
                return self.proxy_has(&proxy, key);
            }
            if crate::v9::modules::namespace_has(self, current, key) == Some(true) {
                return Ok(true);
            }
            if self.own(current, key)?.is_some() {
                return Ok(true);
            }
            match self.proto_of(current) {
                Some(proto) => current = proto,
                None => return Ok(false),
            }
        }
        Err(EngineError::limit("V9 prototype chain", CHAIN_STEPS).into())
    }
    /** Own keys in spec order, without private names. */
    pub fn own_keys(&mut self, id: ObjectId) -> Res<Vec<Key>> {
        let Some(object) = self.heap.get(id) else {
            if id >= NATIVE_BASE {
                return Ok(vec![Key::from("length"), Key::from("name")]);
            }
            return Ok(realm()
                .object(id)
                .map(|def| def.props.iter().map(|(key, ..)| key.clone()).collect())
                .unwrap_or_default());
        };
        if let Class::Proxy(proxy) = &object.class {
            let proxy = (**proxy).clone();
            return self.proxy_own_keys(&proxy);
        }
        let mut keys = Vec::new();
        match &object.class {
            Class::Array(elements) => {
                keys.extend(
                    elements
                        .iter()
                        .enumerate()
                        .filter(|(_, value)| **value != JsValue::Hole)
                        .map(|(index, _)| {
                            Key::Str(JsString::from_utf8(&index.to_string()).expect("index"))
                        }),
                );
                keys.push(Key::from("length"));
            }
            Class::String(text) => {
                keys.extend((0..text.len()).map(|index| {
                    Key::Str(JsString::from_utf8(&index.to_string()).expect("index"))
                }));
                keys.push(Key::from("length"));
            }
            Class::Host(_) => keys.extend(super::super::platform::host_keys(self, id)?),
            Class::Namespace(module) => {
                let module = module.clone();
                keys.extend(crate::v9::modules::namespace_keys(self, &module)?);
            }
            Class::View(view) if view.kind != super::data::ElementKind::DataView => {
                keys.extend((0..view.length).map(|index| {
                    Key::Str(JsString::from_utf8(&index.to_string()).expect("index"))
                }));
            }
            _ => {}
        }
        let object = self.heap.get(id).expect("checked above");
        let is_function = object.function().is_some();
        let own: Vec<Key> = object.props.ordered_keys();
        // Deletion marks are private keys, which `ordered_keys` leaves out, so ask the map.
        let deleted = |name: &str| object.props.get(&deleted_mark(&Key::from(name))).is_some();
        let virtual_names: Vec<&str> = ["length", "name"]
            .into_iter()
            .filter(|name| is_function && !deleted(name))
            .collect();
        let mut indexes: Vec<Key> = own
            .iter()
            .filter(|key| key.index().is_some())
            .cloned()
            .collect();
        indexes.sort_by_key(|key| key.index());
        keys.splice(0..0, indexes);
        for name in virtual_names {
            let key = Key::from(name);
            if !own.contains(&key) {
                keys.push(key);
            }
        }
        keys.extend(
            own.into_iter()
                .filter(|key| key.index().is_none() && !key.hidden()),
        );
        Ok(keys)
    }
    /** Enumerable own string keys, as Object.keys and for-in list them. */
    pub fn enumerable_keys(&mut self, id: ObjectId) -> Res<Vec<Key>> {
        let mut out = Vec::new();
        for key in self.own_keys(id)? {
            if !matches!(key, Key::Str(_)) {
                continue;
            }
            if let Some(prop) = self.own(id, &key)?
                && prop.attrs & ENUMERABLE != 0
            {
                out.push(key);
            }
        }
        Ok(out)
    }
    /** Change an object's prototype, refusing cycles. */
    pub fn set_proto(&mut self, id: ObjectId, proto: Option<ObjectId>) -> Res<bool> {
        let mut cursor = proto;
        let mut steps = 0;
        while let Some(current) = cursor {
            if current == id {
                return Ok(false);
            }
            steps += 1;
            if steps > CHAIN_STEPS {
                return Ok(false);
            }
            cursor = self.proto_of(current);
        }
        if self.proto_of(id) == proto {
            return Ok(true);
        }
        self.materialize(id)?;
        let object = self
            .heap
            .get_mut(id)
            .ok_or_else(|| EngineError::invalid("Invalid V9 object reference."))?;
        if !object.extensible {
            return Ok(false);
        }
        object.proto = proto;
        Ok(true)
    }
    /** Write a data property, throwing on failure; for building result objects. */
    pub fn put(&mut self, id: ObjectId, name: &str, value: JsValue) -> Res<()> {
        self.write_own(id, Key::from(name), value)?;
        Ok(())
    }
    /** Write a property under any key. */
    pub fn put_key(&mut self, id: ObjectId, key: Key, value: JsValue) -> Res<()> {
        if !self.write_own(id, key.clone(), value)? {
            return Err(self.type_error(&format!(
                "Cannot assign to read only property '{}'",
                key_text(&key)
            )));
        }
        Ok(())
    }
}

/** Merge a partial descriptor over an existing property. */
pub(super) fn merge(existing: Option<Prop>, desc: Descriptor) -> Prop {
    let base_attrs = existing.as_ref().map_or(0, |prop| prop.attrs);
    let mut attrs = base_attrs;
    let set_bit = |attrs: &mut u8, bit: u8, value: Option<bool>| {
        if let Some(value) = value {
            if value {
                *attrs |= bit;
            } else {
                *attrs &= !bit;
            }
        }
    };
    set_bit(&mut attrs, ENUMERABLE, desc.enumerable);
    set_bit(&mut attrs, CONFIGURABLE, desc.configurable);
    let slot = if desc.accessor() {
        attrs &= !WRITABLE;
        let (old_get, old_set) = match existing.as_ref().map(|prop| &prop.slot) {
            Some(Slot::Accessor(get, set)) => (*get, *set),
            _ => (None, None),
        };
        Slot::Accessor(desc.get.unwrap_or(old_get), desc.set.unwrap_or(old_set))
    } else {
        set_bit(&mut attrs, WRITABLE, desc.writable);
        let old = match existing.map(|prop| prop.slot) {
            Some(Slot::Data(value)) => value,
            _ => JsValue::Undefined,
        };
        Slot::Data(desc.value.unwrap_or(old))
    };
    let _ = DEFAULT_ATTRS;
    Prop { slot, attrs }
}

/** New property keys must fit the per-key bound. */
fn check_key(key: &Key) -> Result<(), Thrown> {
    if let Key::Str(text) = key
        && text.len() > VM_PROPERTY_KEY_UNITS
    {
        return Err(
            EngineError::limit("V9 property key UTF-16 units", VM_PROPERTY_KEY_UNITS).into(),
        );
    }
    Ok(())
}

/** Whether a key is this ASCII name. */
pub(in crate::v9) fn key_is(key: &Key, name: &str) -> bool {
    matches!(key, Key::Str(text) if text.is(name))
}

/** A key as text for messages. */
pub(in crate::v9) fn key_text(key: &Key) -> String {
    match key {
        Key::Str(text) => text.lossy(),
        // Well-known symbols print by name; user symbols' descriptions need the VM, so they
        // print without one rather than as an internal id.
        Key::Sym(id) => match super::realm::symbol::NAMES.get(*id as usize) {
            Some(name) => format!("Symbol({name})"),
            None => "Symbol()".into(),
        },
        Key::Private(name) => format!("#{}", name.lossy().rsplit(':').next().unwrap_or("")),
    }
}

/** The hidden key that records a deleted virtual `name` or `length` of a function. */
fn deleted_mark(key: &Key) -> Key {
    let mut units: Vec<u16> = "deleted:".encode_utf16().collect();
    if let Key::Str(text) = key {
        units.extend_from_slice(text.as_units());
    }
    Key::Private(JsString::from_slice(&units).expect("short key"))
}
