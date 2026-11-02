/** A captured super reference is ordinary rooted stack values, never a synthetic object or a mutable runtime mode. */
use super::*;
use crate::v9::value::ObjectId;

impl Vm<'_> {
    /** Capture the home object's actual prototype before ToPropertyKey can mutate that prototype. */
    pub(super) fn capture_super_base(&mut self) -> Res<()> {
        let home = self
            .peek(0)
            .object()
            .ok_or_else(|| EngineError::invalid("Invalid V9 super reference home object."))?;
        let base = crate::v9::builtins::object_get_prototype(self, home)?
            .map_or(JsValue::Null, JsValue::Object);
        self.pop();
        self.push(base);
        Ok(())
    }

    /** GetValue keeps receiver, base and canonical key alive for the later PutValue. */
    pub(super) fn read_super_reference(&mut self) -> Res<()> {
        let receiver = self.peek(2);
        let base = self.peek(1);
        let value = self.peek(0);
        let start = base
            .object()
            .ok_or_else(|| self.type_error("Cannot read properties of null"))?;
        let key = self.to_key(&value)?;
        self.pop();
        self.push(key.clone().value());
        let value = self.get_from(start, &key, &receiver)?;
        self.push(value);
        Ok(())
    }

    /** PutValue uses the captured base and receiver even after getters or numeric conversions change the home prototype. */
    pub(super) fn write_super_reference(&mut self) -> Res<()> {
        let receiver = self.peek(3);
        let base = self.peek(2);
        let key_value = self.peek(1);
        let value = self.peek(0);
        let start = base
            .object()
            .ok_or_else(|| self.type_error("Cannot set properties of null"))?;
        let key = self.to_key(&key_value)?;
        if !self.set_from(start, key.clone(), value.clone(), &receiver)? && self.reference_strict()
        {
            if self.inert_super_receiver_redefinition(start, &key, &receiver) {
                return Err(self.type_error(&format!(
                    "Cannot redefine property: {}",
                    crate::v9::vm::property::key_text(&key)
                )));
            }
            let object = receiver
                .object()
                .and_then(|id| self.inert_constructor_name(id))
                .map_or_else(String::new, |name| format!(" '#<{name}>'"));
            return Err(self.type_error(&format!(
                "Cannot assign to read only property '{}' of object{object}",
                crate::v9::vm::property::key_text(&key),
            )));
        }
        self.drop_n(4);
        self.push(value);
        Ok(())
    }

    /** Recognize receiver descriptor conflicts only when the base chain is inert and inspectable. */
    fn inert_super_receiver_redefinition(
        &self,
        start: ObjectId,
        key: &Key,
        receiver: &JsValue,
    ) -> bool {
        use crate::v9::vm::heap::{Slot, WRITABLE};
        let Some(object) = receiver.object().and_then(|id| self.heap.get(id)) else {
            return false;
        };
        if !matches!(object.class, Class::Plain)
            || !object
                .props
                .get(key)
                .is_some_and(|prop| matches!(prop.slot, Slot::Accessor(..)))
        {
            return false;
        }
        let mut current = Some(start);
        for _ in 0..crate::v9::vm::property::CHAIN_STEPS {
            let Some(id) = current else {
                return true;
            };
            if let Some(object) = self.heap.get(id) {
                if !matches!(object.class, Class::Plain) {
                    return false;
                }
                if let Some(prop) = object.props.get(key) {
                    return matches!(prop.slot, Slot::Data(_)) && prop.attrs & WRITABLE != 0;
                }
            } else if super::super::realm::realm()
                .object(id)
                .and_then(|object| object.get(key))
                .is_some()
            {
                return false;
            }
            current = self.proto_of(id);
        }
        false
    }
}
