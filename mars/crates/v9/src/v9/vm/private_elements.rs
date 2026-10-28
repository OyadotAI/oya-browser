/** Class-private fields and methods belong to the receiver, independent of its prototype. */
use super::data::Class;
use super::heap::{Key, Object, Prop, Slot, WRITABLE};
use super::property::Descriptor;
use super::{Res, Vm};
use crate::v9::value::{JsValue, ObjectId};

mod brand;

/** Unspellable storage for the private methods installed before instance fields. */
fn methods_key() -> Key {
    Key::private("class-private-methods")
}

/** User-facing private spelling excludes evaluation and environment identities. */
fn private_name(key: &Key) -> String {
    match key {
        Key::Private(name) => name.lossy().rsplit(':').next().unwrap_or("").to_string(),
        _ => String::new(),
    }
}

impl Vm<'_> {
    /** PrivateFieldAdd ignores extensibility but refuses a second initialization. */
    pub(super) fn add_private_field(&mut self, id: ObjectId, key: Key, value: JsValue) -> Res<()> {
        self.guard_realm_global(id)?;
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::is_shared_object(self, id) {
            return Err(self.type_error("Cannot add private fields to shared objects"));
        }
        if self
            .heap
            .get(id)
            .is_some_and(|object| object.props.get(&key).is_some())
        {
            return Err(self.type_error(&format!(
                "Cannot initialize #{} twice on the same object",
                private_name(&key)
            )));
        }
        self.heap.put(id, key, Prop::data(value));
        self.check_heap_bounds()
    }

    /** Class definitions retain a private method template without branding their prototype. */
    pub(super) fn define_private_method(
        &mut self,
        home: ObjectId,
        key: Key,
        mut desc: Descriptor,
    ) -> Res<()> {
        if !desc.accessor() {
            desc.writable = Some(false);
        }
        let target = if self
            .heap
            .get(home)
            .is_some_and(|object| matches!(object.class, Class::Function(_)))
        {
            home
        } else {
            self.private_methods(home)?
        };
        self.define(target, key, desc)?;
        Ok(())
    }

    /** A private template is ordinary inert heap state and is traced with the class prototype. */
    fn private_methods(&mut self, home: ObjectId) -> Res<ObjectId> {
        if let Some(id) = self.method_template(home) {
            return Ok(id);
        }
        let id = self.alloc(Object::new(Class::Plain, None))?;
        self.heap
            .put(home, methods_key(), Prop::data(JsValue::Object(id)));
        Ok(id)
    }

    /** Locate the template without property access, proxy traps or inherited class brands. */
    fn method_template(&self, home: ObjectId) -> Option<ObjectId> {
        match &self.heap.get(home)?.props.get(&methods_key())?.slot {
            Slot::Data(JsValue::Object(id)) => Some(*id),
            _ => None,
        }
    }

    /** InitializeInstanceElements stamps private methods even when super returns a foreign object. */
    pub(super) fn install_private_methods(
        &mut self,
        function: &JsValue,
        this: &JsValue,
    ) -> Res<()> {
        let Some(home) = function
            .object()
            .and_then(|id| self.heap.get(id))
            .and_then(|object| object.function())
            .and_then(|function| function.home)
        else {
            return Ok(());
        };
        let Some(template) = self.method_template(home) else {
            return Ok(());
        };
        let Some(id) = this.object() else {
            return Err(self.type_error("Cannot initialize private methods on a non-object"));
        };
        self.guard_realm_global(id)?;
        #[cfg(feature = "conformance")]
        if crate::v9::builtins::is_shared_object(self, id) {
            return Err(self.type_error("Cannot add private methods to shared objects"));
        }
        let methods: Vec<_> = self
            .heap
            .get(template)
            .map(|object| {
                object
                    .props
                    .iter()
                    .map(|(key, prop)| (key.clone(), prop.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if methods.iter().any(|(key, _)| {
            self.heap
                .get(id)
                .is_some_and(|object| object.props.get(key).is_some())
        }) {
            return Err(self.type_error(&format!(
                "Cannot initialize private methods of class {} twice on the same object",
                self.class_name(home)
            )));
        }
        for (key, prop) in methods {
            self.heap.put(id, key, prop);
        }
        self.check_heap_bounds()
    }
}
