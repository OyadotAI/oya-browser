/**
 * The `with` statement's object environment record (spec 9.1.1.2, V8's with context). The
 * object lives in a hidden env slot; names the body looks up across it ask HasBinding first,
 * which consults Symbol.unscopables, then read, write or delete the object's property.
 */
use super::Vm;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::Res;
use crate::v9::vm::heap::Key;
use crate::v9::vm::realm::symbol;

impl Vm<'_> {
    /** The `with` object in env slot (hops, slot). */
    fn with_object(&mut self, hops: u16, slot: u16) -> Res<Option<ObjectId>> {
        let env = self.env_at(hops)?;
        Ok(match self.env_slot(env, slot)? {
            JsValue::Object(id) => Some(id),
            _ => None,
        })
    }
    /** HasBinding (spec 9.1.1.2.1): the object has the property and unscopables do not hide it. */
    fn with_binding(&mut self, hops: u16, slot: u16, name: u32) -> Res<Option<(ObjectId, Key)>> {
        let key = Key::Str(self.constant(name));
        let Some(id) = self.with_object(hops, slot)? else {
            return Ok(None);
        };
        if !self.has_property(id, &key)? {
            return Ok(None);
        }
        let unscopables = self.get(&JsValue::Object(id), &Key::Sym(symbol::UNSCOPABLES))?;
        if let JsValue::Object(_) = unscopables {
            let blocked = self.get(&unscopables, &key)?;
            if blocked.truthy() {
                return Ok(None);
            }
        }
        Ok(Some((id, key)))
    }
    /** GetBindingValue (spec 9.1.1.2.6): a property that vanished reads undefined in sloppy
    code and throws in strict code. */
    pub(super) fn get_with(&mut self, name: u32, hops: u16, slot: u16, call: bool) -> Res<()> {
        let Some((id, key)) = self.with_binding(hops, slot, name)? else {
            self.push(JsValue::Boolean(false));
            return Ok(());
        };
        let value = if self.has_property(id, &key)? {
            self.get(&JsValue::Object(id), &key)?
        } else if self.frame().code.strict {
            let name = self.constant(name).lossy();
            return Err(self.reference_error(&format!("{name} is not defined")));
        } else {
            JsValue::Undefined
        };
        self.push(value);
        if call {
            self.push(JsValue::Object(id));
        }
        self.push(JsValue::Boolean(true));
        Ok(())
    }
    /** SetMutableBinding (spec 9.1.1.2.5) for the value on the stack, which stays. */
    pub(super) fn set_with(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let Some((id, key)) = self.with_binding(hops, slot, name)? else {
            self.push(JsValue::Boolean(false));
            return Ok(());
        };
        let strict = self.frame().code.strict;
        if !self.has_property(id, &key)? && strict {
            let name = self.constant(name).lossy();
            return Err(self.reference_error(&format!("{name} is not defined")));
        }
        let value = self.peek(0);
        self.set(&JsValue::Object(id), key, value, strict)?;
        self.push(JsValue::Boolean(true));
        Ok(())
    }
    /** DeleteBinding (spec 9.1.1.2.7): `delete name` deletes the object's property. */
    pub(super) fn delete_with(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let Some((id, key)) = self.with_binding(hops, slot, name)? else {
            self.push(JsValue::Boolean(false));
            return Ok(());
        };
        let deleted = self.delete(id, &key)?;
        self.push(JsValue::Boolean(deleted));
        self.push(JsValue::Boolean(true));
        Ok(())
    }
}
