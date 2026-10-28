/** Property semantics: own lookup, prototype chains, assignment, definition, deletion and keys. */
use super::data::{Class, FunctionKind};
use super::heap::{
    CONFIGURABLE, DEFAULT_ATTRS, ENUMERABLE, INTRINSIC_BASE, Key, NATIVE_BASE, Object, Prop, Slot,
    WRITABLE,
};
use super::realm::{Static, native_index, realm, realm_for};
use super::{Res, Thrown, Vm};
use crate::constants::VM_PROPERTY_KEY_UNITS;
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};

mod descriptor;
pub(super) use descriptor::compatible;

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
        Class::View(view) => {
            (view.kind == super::data::ElementKind::DataView
                || crate::v9::builtins::typed_numeric_index(key).is_none())
                && !key_is(key, "length")
        }
        Class::Array(_) | Class::String(_) => key.index().is_none() && !key_is(key, "length"),
        Class::MappedArguments(_) => key.index().is_none(),
        _ => true,
    }
}

/** Longest prototype chain walked before assuming a cycle. */
pub(in crate::v9) const CHAIN_STEPS: usize = 4096;

impl Vm<'_> {
    /** Native StringData belongs to real wrappers and the canonical empty intrinsic, never a public prototype/tag/name. */
    pub(in crate::v9) fn string_data(&self, id: ObjectId) -> Option<JsString> {
        match self.heap.get(id).map(|object| &object.class) {
            Some(Class::String(value)) => Some(value.clone()),
            None | Some(Class::Plain) if id == realm_for(id).ids.string_prototype => {
                Some(JsString::empty())
            }
            _ => None,
        }
    }
    /** V8's interesting-property lookup skips records that cannot own either well-known hook. */
    pub(in crate::v9) fn interesting_holder(&self, start: ObjectId) -> Option<ObjectId> {
        let tag = Key::Sym(super::realm::symbol::TO_STRING_TAG);
        let primitive = Key::Sym(super::realm::symbol::TO_PRIMITIVE);
        let mut current = Some(start);
        for _ in 0..CHAIN_STEPS {
            let id = current?;
            if let Some(object) = self.heap.get(id) {
                #[cfg(feature = "conformance")]
                if object.global {
                    return Some(id);
                }
                if matches!(object.class, Class::Proxy(_) | Class::Host(_))
                    || object.props.get(&tag).is_some()
                    || object.props.get(&primitive).is_some()
                {
                    return Some(id);
                }
            } else if let Some(def) = realm_for(id).object(id) {
                if def.get(&tag).is_some() || def.get(&primitive).is_some() {
                    return Some(id);
                }
            }
            current = self.proto_of(id);
        }
        Some(start)
    }
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
            return Some(realm_for(id).ids.function_prototype);
        }
        realm_for(id).object(id).and_then(|def| def.proto)
    }
    /** Copy an intrinsic into the heap so scripts can change it. */
    pub fn materialize(&mut self, id: ObjectId) -> Res<()> {
        if id < INTRINSIC_BASE || self.in_heap(id) {
            return Ok(());
        }
        let mut object = if id >= NATIVE_BASE {
            let index = native_index(id);
            Object::new(
                Class::Function(Box::new(super::data::Function {
                    kind: FunctionKind::Native {
                        id: index,
                        slots: Vec::new(),
                    },
                    home: None,
                    fields: None,
                    name: None,
                })),
                Some(realm_for(id).ids.function_prototype),
            )
        } else {
            let def = realm_for(id)
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
                    name: None,
                })),
                None if id == realm_for(id).ids.array_prototype => Class::Array(Vec::new().into()),
                None if id == realm_for(id).ids.string_prototype => {
                    Class::String(JsString::empty())
                }
                None => Class::Plain,
            };
            let mut object = Object::new(class, def.proto);
            for (key, value, attrs) in &def.props {
                if id == realm_for(id).ids.array_prototype && key_is(key, "length") {
                    continue;
                }
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
        #[cfg(feature = "conformance")]
        {
            object.realm = super::realm::banks::owner(id);
        }
        object.extensible = id != realm_for(id).ids.throw_type_error;
        self.heap.insert_at(id, object);
        Ok(())
    }
    /** An own property, if present. */
    pub fn own(&mut self, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
        self.guard_realm_global(id)?;
        #[cfg(feature = "conformance")]
        if let Some(value) = crate::v9::builtins::read_shared_container(self, id, key)? {
            let attrs = WRITABLE | ENUMERABLE;
            return Ok(Some(Prop {
                slot: Slot::Data(value),
                attrs,
            }));
        }
        let Some(object) = self.heap.get(id) else {
            return Ok(self.intrinsic_own(id, key));
        };
        match &object.class {
            Class::MappedArguments(_) => {
                if let Some(value) = self.argument_value(id, key)?
                    && let Some(mut prop) = object.props.get(key).cloned()
                {
                    prop.slot = Slot::Data(value);
                    return Ok(Some(prop));
                }
            }
            // Private keys live on the proxy object itself and never reach a trap.
            Class::Proxy(proxy) if !key.hidden() => {
                let proxy = (**proxy).clone();
                return self.proxy_own(&proxy, key);
            }
            Class::Array(elements) => {
                if let Some(index) = key.index() {
                    // An element with its own attributes or an accessor lives in the property
                    // list (V8's dictionary elements), its dense slot a hole.
                    let property = elements
                        .get(index as usize)
                        .filter(|value| **value != JsValue::Hole)
                        .map(|value| Prop::data(value.clone()))
                        .or_else(|| object.props.get(key).cloned());
                    #[cfg(feature = "conformance")]
                    if crate::v9::builtins::is_shared_object(self, id) {
                        return Ok(property.map(|mut property| {
                            property.attrs = WRITABLE | ENUMERABLE;
                            property
                        }));
                    }
                    return Ok(property);
                }
                if key_is(key, "length") {
                    let fixed = object.props.get(&fixed_length_mark()).is_some();
                    return Ok(Some(Prop {
                        slot: Slot::Data(JsValue::Number(elements.len() as f64)),
                        attrs: if fixed { 0 } else { WRITABLE },
                    }));
                }
            }
            Class::String(text) => {
                if let Some(index) = key.index() {
                    if let Some(unit) = text.unit_at(index as usize) {
                        return Ok(Some(Prop {
                            slot: Slot::Data(JsValue::String(JsString::from_slice(&[unit])?)),
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
                let bank = self.object_realm(id);
                if let Some(prop) = self.with_realm(bank, |vm| {
                    crate::v9::modules::namespace_own(vm, &module, key)
                })? {
                    return Ok(Some(prop));
                }
            }
            Class::View(_) => {
                if let Some(index) = crate::v9::builtins::typed_index(self, id, key) {
                    return Ok(index
                        .and_then(|index| crate::v9::builtins::typed_load(self, id, index))
                        .map(|value| {
                            let mut prop = Prop::data(value);
                            if crate::v9::builtins::view_of(self, id).is_some_and(|view| {
                                crate::v9::builtins::buffer_immutable(self, view.buffer)
                            }) {
                                prop.attrs = ENUMERABLE;
                            }
                            prop
                        }));
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
            let def = realm_for(id).native(id)?;
            return if key_is(key, "name") {
                Some(Prop {
                    slot: Slot::Data(super::realm::string(
                        def.display_name.as_deref().unwrap_or(def.name),
                    )),
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
        let (_, value, attrs) = realm_for(id).object(id)?.get(key)?;
        Some(Prop {
            slot: match value {
                Static::Value(value) => Slot::Data(value.clone()),
                Static::Accessor(get, set) => Slot::Accessor(*get, *set),
            },
            attrs: *attrs,
        })
    }
    /** Whether a function's virtual `name` or `length` was deleted. */
    pub(super) fn virtual_deleted(&self, id: ObjectId, key: &Key) -> bool {
        (key_is(key, "name") || key_is(key, "length"))
            && self
                .heap
                .get(id)
                .is_some_and(|o| o.props.get(&deleted_mark(key)).is_some())
    }
    /** `name`, `length` and a lazily created `prototype` of functions. */
    fn function_virtual(&mut self, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
        if key_is(key, "name")
            && let Some(name) = self
                .heap
                .get(id)
                .and_then(|object| object.function())
                .and_then(|function| function.name.clone())
        {
            let value = self.render_name_snapshot(&name)?;
            return Ok(Some(Prop {
                slot: Slot::Data(value),
                attrs: name.attrs,
            }));
        }
        let intrinsics = self.object_intrinsics(id);
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
                    #[cfg(feature = "conformance")]
                    let name = self.isolate_code_constant(site, u32::MAX, &code.name);
                    #[cfg(not(feature = "conformance"))]
                    let name = code.name.clone();
                    return Ok(Some(readonly(JsValue::String(name))));
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
                    let value = if key_is(key, "arguments") {
                        self.active_arguments(id)?
                    } else {
                        self.active_caller(id)
                    };
                    return Ok(Some(Prop {
                        slot: Slot::Data(value),
                        attrs: 0,
                    }));
                }
                if key_is(key, "prototype") && self.has_function_prototype(id) {
                    let proto = if flags.generator && flags.asynchronous {
                        intrinsics.ids.async_generator_prototype
                    } else if flags.generator {
                        intrinsics.ids.generator_prototype
                    } else {
                        intrinsics.ids.object_prototype
                    };
                    let object = Object::new(Class::Plain, Some(proto));
                    #[cfg(feature = "conformance")]
                    let object = {
                        let mut object = object;
                        object.realm = self.object_realm(id);
                        object
                    };
                    let prototype = self.alloc(object)?;
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
                let def = &self.object_intrinsics(id).natives[native as usize];
                if key_is(key, "name") {
                    return Ok(Some(readonly(super::realm::string(
                        def.display_name.as_deref().unwrap_or(def.name),
                    ))));
                }
                if key_is(key, "length") {
                    return Ok(Some(readonly(JsValue::Number(f64::from(def.length)))));
                }
            }
            // bind captured these own data properties before returning the function.
            FunctionKind::Bound { .. } => {}
        }
        Ok(None)
    }
    /** [[Get]] on any value; primitives read through their prototypes. */
    pub fn get(&mut self, base: &JsValue, key: &Key) -> Res<JsValue> {
        let start = match base {
            JsValue::Object(id) => *id,
            JsValue::String(text) => {
                if let Some(index) = key.index() {
                    if let Some(unit) = text.unit_at(index as usize) {
                        return Ok(JsValue::String(JsString::from_slice(&[unit])?));
                    }
                } else if key_is(key, "length") {
                    return Ok(JsValue::Number(text.len() as f64));
                }
                realm().ids.string_prototype
            }
            #[cfg(feature = "conformance")]
            JsValue::Smi(_) => realm().ids.number_prototype,
            JsValue::Number(_) => realm().ids.number_prototype,
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(_) | JsValue::LocalNumber(_) => realm().ids.number_prototype,
            JsValue::Boolean(_) => realm().ids.boolean_prototype,
            JsValue::Symbol(_) => realm().ids.symbol_prototype,
            JsValue::BigInt(_) | JsValue::BigIntWide(_) => realm().ids.bigint_prototype,
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
            // V8's GetPropertyWithFailedAccessCheck returns undefined for well-known symbols.
            if matches!(key, Key::Sym(symbol) if *symbol < super::realm::symbol::WELL_KNOWN)
                && !self.realm_global_accessible(id)
            {
                return Ok(JsValue::Undefined);
            }
            self.guard_realm_global(id)?;
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
            if let Some(index) = crate::v9::builtins::typed_index(self, id, key) {
                return Ok(index
                    .and_then(|index| crate::v9::builtins::typed_load(self, id, index))
                    .unwrap_or(JsValue::Undefined));
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
            JsValue::String(text) => {
                // ToObject(string) owns its readonly UTF-16 indices and length before its prototype.
                if key_is(&key, "length")
                    || key
                        .index()
                        .is_some_and(|index| (index as usize) < text.len())
                {
                    if strict {
                        return Err(self.type_error(&format!(
                            "Cannot assign to read only property '{}' of object",
                            key_text(&key)
                        )));
                    }
                    return Ok(());
                }
                realm().ids.string_prototype
            }
            #[cfg(feature = "conformance")]
            JsValue::Smi(_) => realm().ids.number_prototype,
            JsValue::Number(_) => realm().ids.number_prototype,
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(_) | JsValue::LocalNumber(_) => realm().ids.number_prototype,
            JsValue::Boolean(_) => realm().ids.boolean_prototype,
            JsValue::Symbol(_) => realm().ids.symbol_prototype,
            JsValue::BigInt(_) | JsValue::BigIntWide(_) => realm().ids.bigint_prototype,
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
        self.set_from_context(start, key, value, receiver, false)
    }
    /** Contextual global writes use the same traversal without running a separate proxy trap. */
    pub(super) fn set_global_context(&mut self, key: Key, value: JsValue) -> Res<()> {
        let start = self.global_object();
        let receiver = JsValue::Object(start);
        if !self.set_from_context(start, key.clone(), value, &receiver, true)? {
            return Err(self.type_error(&format!(
                "Cannot assign to read only property '{}' of object",
                key_text(&key)
            )));
        }
        Ok(())
    }
    /** Traverse once so contextual assignment observes setters and proxies in native order. */
    fn set_from_context(
        &mut self,
        start: ObjectId,
        key: Key,
        value: JsValue,
        receiver: &JsValue,
        contextual: bool,
    ) -> Res<bool> {
        self.guard_realm_global(start)?;
        if *receiver == JsValue::Object(start) && self.fast_write(start, &key, &value) {
            return Ok(true);
        }
        let mut id = start;
        let mut writable = false;
        for _ in 0..CHAIN_STEPS {
            if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
                return self.proxy_set(id, &proxy, key, value, receiver);
            }
            if let Some(index) = crate::v9::builtins::typed_index(self, id, &key) {
                if *receiver == JsValue::Object(id) {
                    return crate::v9::builtins::typed_set_integer(self, id, &key, &value);
                }
                if index.is_none() {
                    return Ok(true);
                }
            }
            if let Some(prop) = self.own_for_write(id, &key)? {
                match prop.slot {
                    Slot::Accessor(_, Some(setter)) => {
                        self.call(JsValue::Object(setter), receiver.clone(), vec![value])?;
                        return Ok(true);
                    }
                    Slot::Accessor(_, None) => return Ok(false),
                    Slot::Data(_) if prop.attrs & WRITABLE == 0 => return Ok(false),
                    Slot::Data(_) => {
                        let window = realm().ids.window_prototype;
                        let intrinsic = id == window
                            && self.proto_of(start) == Some(window)
                            && realm()
                                .object(window)
                                .is_some_and(|o| o.get(&key).is_some());
                        if contextual && id != start && !intrinsic {
                            return Err(
                                self.reference_error(&format!("{} is not defined", key_text(&key)))
                            );
                        }
                        writable = true;
                        break;
                    }
                }
            }
            match self.proto_of(id) {
                Some(proto) => id = proto,
                None if contextual => {
                    return Err(self.reference_error(&format!("{} is not defined", key_text(&key))));
                }
                None => break,
            }
        }
        if contextual && !writable {
            return Err(EngineError::limit("V9 prototype chain", CHAIN_STEPS).into());
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
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::shared_container_active(self, id) {
            return Fast::Slow;
        }
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
        self.heap.track_value(value);
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::is_shared_object(self, id) {
            return false;
        }
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
        #[cfg(feature = "conformance")]
        if let Some(result) = crate::v9::builtins::check_shared_store(self, id, &key, &value)? { return Ok(result); }
        #[cfg(feature = "conformance")]
        let value = if crate::v9::builtins::is_shared_object(self, id) {
            let number = match &value {
                JsValue::LocalNumber(number) => Some(number.value),
                JsValue::Number(number) if !number.is_finite() || number.fract() != 0.0 || *number < -(1_i64 << 30) as f64 || *number >= (1_i64 << 30) as f64 || (*number == 0.0 && number.is_sign_negative()) => Some(*number),
                _ => None,
            };
            if let Some(number) = number {
                let mark = self.pins.len();
                self.pin(JsValue::Object(id));
                let result = crate::v9::builtins::new_shared_number(self, number);
                self.pins.truncate(mark);
                result?
            } else { value }
        } else { value };
        #[cfg(feature = "conformance")]
        if matches!(value, JsValue::SharedNumber(_)) {
            let mark = self.pins.len();
            self.pin(value.clone());
            let result = self.write_own_value(id, key, value);
            self.pins.truncate(mark);
            return result;
        }
        self.write_own_value(id, key, value)
    }
    /** Perform a pinned ordinary property write after shared-value publication. */
    fn write_own_value(&mut self, id: ObjectId, key: Key, value: JsValue) -> Res<bool> {
        if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
            let desc = match self.own(id, &key)? {
                Some(prop)
                    if matches!(prop.slot, Slot::Accessor(..)) || prop.attrs & WRITABLE == 0 =>
                {
                    return Ok(false);
                }
                Some(_) => Descriptor {
                    value: Some(value),
                    ..Descriptor::default()
                },
                None => Descriptor::data(value),
            };
            return self.proxy_define(&proxy, key, desc);
        }
        self.materialize(id)?;
        if let Some(done) = self.write_name_snapshot(id, &key, &value)? {
            return Ok(done);
        }
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
                let value = self.argument_write(id, &key, value);
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
        #[cfg(feature = "conformance")]
        if let Some(result) = crate::v9::builtins::check_shared_store(self, id, key, value)? {
            return Ok(Some(result));
        }
        #[cfg(feature = "conformance")]
        if let Some(result) = crate::v9::builtins::write_shared_container(self, id, key, value)? {
            return Ok(Some(result));
        }
        let limit = self.limits.array_length;
        let Some(object) = self.heap.get(id) else {
            return Ok(None);
        };
        match &object.class {
            Class::Array(elements) => {
                if let Some(index) = key.index() {
                    let index = index as usize;
                    let resident = elements
                        .get(index)
                        .is_some_and(|value| *value != JsValue::Hole);
                    if !object.extensible && !resident && object.props.get(key).is_none() {
                        return Ok(Some(false));
                    }
                    let fixed = object.props.get(&fixed_length_mark()).is_some();
                    if index >= elements.len() && (!object.extensible || fixed) {
                        return Ok(Some(false));
                    }
                    if object.props.get(key).is_some() {
                        return Ok(None);
                    }
                    if elements.resident_len() >= limit && !resident {
                        return Err(EngineError::limit("V9 resident array elements", limit).into());
                    }
                    self.array_store(id, index, value.clone());
                    return Ok(Some(true));
                }
                if key_is(key, "length") {
                    let length = self.array_length_value(value)?;
                    let Some(object) = self.heap.get(id) else {
                        return Ok(Some(false));
                    };
                    let Class::Array(elements) = &object.class else {
                        return Ok(Some(false));
                    };
                    let current = elements.len();
                    let fixed = object.props.get(&fixed_length_mark()).is_some();
                    if fixed {
                        return Ok(Some(length == current));
                    }
                    return self.set_array_length(id, length).map(Some);
                }
                Ok(None)
            }
            Class::String(text)
                if key_is(key, "length")
                    || key
                        .index()
                        .is_some_and(|index| (index as usize) < text.len()) =>
            {
                Ok(Some(false))
            }
            Class::Host(_) => super::super::platform::host_write(self, id, key, value),
            // A namespace's exports are read-only to their importers.
            Class::Namespace(_) if !key.hidden() => Ok(Some(false)),
            // Integer-indexed: in-range writes store into the buffer, others are dropped.
            Class::View(_) if crate::v9::builtins::typed_index(self, id, key).is_some() => {
                crate::v9::builtins::typed_set_integer(self, id, key, value).map(Some)
            }
            _ => Ok(None),
        }
    }
    /** Store one resident array element without allocating gaps, keeping exact live counts. */
    pub fn array_store(&mut self, id: ObjectId, index: usize, value: JsValue) {
        self.heap.track_value(&value);
        let Some(Class::Array(elements)) = self.heap.get_mut(id).map(|object| &mut object.class)
        else {
            return;
        };
        let (mut properties, mut units) = (0isize, 0isize);
        let old = elements.set(index, value);
        let new = &elements[index];
        if old == JsValue::Hole && *new != JsValue::Hole {
            properties += 1;
        } else if old != JsValue::Hole && *new == JsValue::Hole {
            properties -= 1;
        }
        units += new.string_units() as isize - old.string_units() as isize;
        self.heap.adjust(properties, units);
    }
    /** ArraySetLength observes both ToUint32 and ToNumber conversions before validating equality. */
    fn array_length_value(&mut self, value: &JsValue) -> Res<usize> {
        let number = self.to_number(value)?;
        let length = crate::v9::value::to_uint32(number);
        let checked = self.to_number(value)?;
        if f64::from(length) != checked {
            return Err(self.range_error("Invalid array length"));
        }
        Ok(length as usize)
    }
    /** Truncate or extend an array (spec ArraySetLength): deleting stops above the highest
    non-configurable element, and false reports that the length could not shrink that far. */
    pub fn set_array_length(&mut self, id: ObjectId, length: usize) -> Res<bool> {
        if length > u32::MAX as usize {
            return Err(self.range_error("Invalid array length"));
        }
        let (length, done) = self.trim_dictionary_elements(id, length);
        let Some(Class::Array(elements)) = self.heap.get_mut(id).map(|object| &mut object.class)
        else {
            return Ok(done);
        };
        let removed: Vec<JsValue> = if length < elements.len() {
            elements.truncate_collect(length)
        } else {
            elements.resize_holes(length);
            Vec::new()
        };
        let properties = removed
            .iter()
            .filter(|value| **value != JsValue::Hole)
            .count() as isize;
        let units = removed.iter().map(JsValue::string_units).sum::<usize>() as isize;
        self.heap.adjust(-properties, -units);
        Ok(done)
    }
    /** Remove an array's property-list elements at or above `length`, highest first, stopping
    at a non-configurable one; returns the length reached and whether it is `length`. */
    fn trim_dictionary_elements(&mut self, id: ObjectId, length: usize) -> (usize, bool) {
        let Some(object) = self.heap.get(id) else {
            return (length, true);
        };
        let mut above: Vec<(usize, Key)> = object
            .props
            .ordered_keys()
            .into_iter()
            .filter_map(|key| key.index().map(|index| (index as usize, key)))
            .filter(|(index, _)| *index >= length)
            .collect();
        above.sort_by(|a, b| b.0.cmp(&a.0));
        for (index, key) in above {
            let fixed = self
                .heap
                .get(id)
                .and_then(|object| object.props.get(&key))
                .is_some_and(|prop| prop.attrs & CONFIGURABLE == 0);
            if fixed {
                return (index + 1, false);
            }
            self.heap.remove(id, &key);
        }
        (length, true)
    }
    /** [[DefineOwnProperty]] with a partial descriptor. Returns false when refused. */
    pub fn define(&mut self, id: ObjectId, key: Key, desc: Descriptor) -> Res<bool> {
        if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
            return self.proxy_define(&proxy, key, desc);
        }
        if let Some(result) = crate::v9::builtins::typed_define_integer(self, id, &key, &desc) {
            return result;
        }
        if let Some(done) = crate::v9::modules::namespace_define(self, id, &key, &desc)? {
            return Ok(done);
        }
        #[cfg(feature = "conformance")]
        crate::v9::builtins::check_shared_define(self, id, &key, &desc)?;
        #[cfg(feature = "conformance")]
        if let Some(result) = crate::v9::builtins::define_shared_container(self, id, &key, &desc)? {
            return Ok(result);
        }
        self.materialize(id)?;
        if let Some(done) = self.define_name_snapshot(id, &key, &desc)? {
            return Ok(done);
        }
        let existing = self.own(id, &key)?;
        let is_array = matches!(self.heap.get(id).map(|o| &o.class), Some(Class::Array(_)));
        if is_array && key_is(&key, "length") {
            return self.define_array_length(id, &desc);
        }
        if is_array && let Some(index) = key.index() {
            if let Some(object) = self.heap.get(id)
                && let Class::Array(elements) = &object.class
                && index as usize >= elements.len()
                && object.props.get(&fixed_length_mark()).is_some()
            {
                return Ok(false);
            }
            if self.dense_element_define(id, &key, &desc, existing.is_some()) {
                let value = desc.value.clone().unwrap_or(JsValue::Undefined);
                if existing.is_some() && desc.value.is_none() {
                    return Ok(true);
                }
                return self
                    .exotic_write(id, &key, &value)
                    .map(|done| done.unwrap_or(true));
            }
        }
        let extensible = self.heap.get(id).is_some_and(|object| object.extensible);
        if !compatible(extensible, &desc, existing.as_ref()) {
            return Ok(false);
        }
        if existing.is_none() {
            check_key(&key)?;
        }
        let mut prop = merge(existing, desc);
        if is_array && let Some(index) = key.index() {
            // The element moves to the property list; its indexed slot becomes a hole that still
            // counts toward the length.
            self.array_store(id, index as usize, JsValue::Hole);
        }
        if let Some(Class::Host(_)) = self.heap.get(id).map(|object| &object.class)
            && let Slot::Data(value) = &prop.slot
            && let Some(done) = super::super::platform::host_write(self, id, &key, value)?
        {
            return Ok(done);
        }
        if self.argument_slot(id, &key).is_some() {
            if let Slot::Data(value) = &prop.slot {
                self.argument_write(id, &key, value.clone());
            }
            if matches!(prop.slot, Slot::Accessor(..)) || prop.attrs & WRITABLE == 0 {
                self.detach_argument(id, &key);
            } else {
                prop.slot = Slot::Data(JsValue::Undefined);
            }
        }
        self.clear_name_snapshot(id, &key)?;
        self.heap.put(id, key, prop);
        Ok(true)
    }
    /** [[DefineOwnProperty]] for an array's `length` (spec 10.4.2.1 ArraySetLength): it stays
    a non-enumerable, non-configurable data property, and once non-writable it never changes. */
    fn define_array_length(&mut self, id: ObjectId, desc: &Descriptor) -> Res<bool> {
        // The new length converts (and throws RangeError) before any validation.
        let length = match &desc.value {
            Some(value) => Some(self.array_length_value(value)?),
            None => None,
        };
        let fixed = self
            .heap
            .get(id)
            .is_some_and(|object| object.props.get(&fixed_length_mark()).is_some());
        if desc.accessor()
            || desc.configurable == Some(true)
            || desc.enumerable == Some(true)
            || fixed && desc.writable == Some(true)
        {
            return Ok(false);
        }
        let mut done = true;
        if let Some(length) = length {
            done = self
                .exotic_write(id, &Key::from("length"), &JsValue::Number(length as f64))?
                .unwrap_or(true);
        }
        if desc.writable == Some(false) && !fixed {
            let mark = Prop {
                slot: Slot::Data(JsValue::Undefined),
                attrs: 0,
            };
            self.heap.put(id, fixed_length_mark(), mark);
        }
        Ok(done)
    }
    /** Whether an array element definition can stay in the dense elements: a plain data
    element with every attribute (given, or kept from a dense element) and no property-list
    entry already. Anything else takes the ordinary validation and moves to the property list. */
    fn dense_element_define(
        &self,
        id: ObjectId,
        key: &Key,
        desc: &Descriptor,
        exists: bool,
    ) -> bool {
        let in_list = self
            .heap
            .get(id)
            .is_some_and(|object| object.props.get(key).is_some());
        let attrs = [desc.writable, desc.enumerable, desc.configurable];
        let plain = !desc.accessor() && attrs.iter().all(|a| *a != Some(false));
        !in_list && plain && (exists || attrs.iter().all(|a| *a == Some(true)))
    }
    /** [[Delete]]; non-configurable properties stay. */
    pub fn delete(&mut self, id: ObjectId, key: &Key) -> Res<bool> {
        if let Some(proxy) = self.proxy_of(id).filter(|_| !key.hidden()) {
            return self.proxy_delete(&proxy, key);
        }
        if let Some(exported) = crate::v9::modules::namespace_has(self, id, key) {
            return Ok(!exported);
        }
        if let Some(index) = crate::v9::builtins::typed_index(self, id, key) {
            return Ok(index.is_none());
        }
        let Some(prop) = self.own_for_write(id, key)? else {
            return Ok(true);
        };
        if prop.attrs & CONFIGURABLE == 0 {
            return Ok(false);
        }
        if let Some(Class::Array(elements)) = self.heap.get(id).map(|object| &object.class)
            && let Some(index) = key.index().map(|index| index as usize)
            && elements
                .get(index)
                .is_some_and(|value| *value != JsValue::Hole)
        {
            self.array_store(id, index, JsValue::Hole);
            return Ok(true);
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
        self.detach_argument(id, key);
        self.clear_name_snapshot(id, key)?;
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
            if let Some(index) = crate::v9::builtins::typed_index(self, current, key) {
                return Ok(index.is_some());
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
        self.guard_realm_global(id)?;
        let Some(object) = self.heap.get(id) else {
            if id >= NATIVE_BASE {
                return Ok(vec![Key::from("length"), Key::from("name")]);
            }
            return Ok(realm_for(id)
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
                keys.extend(elements.resident_entries().map(|(index, _)| {
                    Key::Str(JsString::from_utf8(&index.to_string()).expect("index"))
                }));
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
                keys.extend(
                    (0..crate::v9::builtins::typed_keys_length(self, id)).map(|index| {
                        Key::Str(JsString::from_utf8(&index.to_string()).expect("index"))
                    }),
                );
            }
            _ => {}
        }
        let object = self.heap.get(id).expect("checked above");
        let is_function = object.function().is_some();
        let own: Vec<Key> = object.props.ordered_keys();
        // Deletion marks are private keys, which `ordered_keys` leaves out, so ask the map.
        let deleted = |name: &str| object.props.get(&deleted_mark(&Key::from(name))).is_some();
        let virtual_names: Vec<&str> = ["length", "name", "prototype"]
            .into_iter()
            .filter(|name| {
                is_function
                    && !deleted(name)
                    && (*name != "prototype" || self.has_function_prototype(id))
            })
            .collect();
        let mut indexes: Vec<Key> = own
            .iter()
            .filter(|key| key.index().is_some())
            .cloned()
            .collect();
        indexes.sort_by_key(|key| key.index());
        keys.splice(0..0, indexes);
        // An array's property-list elements interleave with its dense ones.
        let leading = keys.iter().take_while(|key| key.index().is_some()).count();
        keys[..leading].sort_by_key(|key| key.index());
        for name in &virtual_names {
            let key = Key::from(*name);
            // Replacing an existing virtual property keeps its original insertion position.
            keys.push(key);
        }
        keys.extend(own.into_iter().filter(|key| {
            key.index().is_none()
                && !key.hidden()
                && !virtual_names.iter().any(|name| key_is(key, name))
        }));
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
    /** Ordinary prototype mutation stops its cycle scan at exotic GetPrototypeOf implementations. */
    pub fn set_proto(&mut self, id: ObjectId, proto: Option<ObjectId>) -> Res<bool> {
        if self.proto_of(id) == proto {
            return Ok(true);
        }
        self.materialize(id)?;
        if self.heap.get(id).is_some_and(|object| !object.extensible)
            || id == self.object_intrinsics(id).ids.object_prototype
        {
            return Ok(false);
        }
        let mut cursor = proto;
        let mut steps = 0;
        while let Some(current) = cursor {
            if current == id {
                return Ok(false);
            }
            if self.proxy_of(current).is_some() {
                break;
            }
            steps += 1;
            if steps > CHAIN_STEPS {
                return Ok(false);
            }
            cursor = self.proto_of(current);
        }
        let object = self
            .heap
            .get_mut(id)
            .ok_or_else(|| EngineError::invalid("Invalid V9 object reference."))?;
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
    let accessor = desc.accessor()
        || desc.value.is_none()
            && desc.writable.is_none()
            && existing
                .as_ref()
                .is_some_and(|prop| matches!(prop.slot, Slot::Accessor(..)));
    let slot = if accessor {
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
/** The hidden mark an array carries once its `length` is non-writable. Private names compile
to `site:name` with a numeric site, so no script name can spell it. */
pub(super) fn fixed_length_mark() -> Key {
    Key::Private(JsString::from_utf8("fixed:length").expect("short key"))
}

fn deleted_mark(key: &Key) -> Key {
    let mut units: Vec<u16> = "deleted:".encode_utf16().collect();
    if let Key::Str(text) = key {
        units.extend(text.code_units());
    }
    Key::Private(JsString::from_slice(&units).expect("short key"))
}
