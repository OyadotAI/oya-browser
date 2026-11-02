/**
 * The `with` statement's object environment record (spec 9.1.1.2, V8's with context). The
 * object lives in a hidden env slot; names the body looks up across it ask HasBinding first,
 * which consults Symbol.unscopables, then read, write or delete the object's property.
 * Assignments and updates resolve the name to its object before their right-hand side runs
 * (spec 6.2.5 Reference Records) and read or write through that object afterwards, so a
 * property the right-hand side deletes or adds cannot move the store.
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
    /** HasBinding for a reference: the `with` object's check, or the eval-var object's own
    property (its prototype is null, so nothing inherited can answer). */
    fn ref_binding(
        &mut self,
        hops: u16,
        slot: u16,
        name: u32,
        with: bool,
    ) -> Res<Option<ObjectId>> {
        if with {
            return Ok(self.with_binding(hops, slot, name)?.map(|(id, _)| id));
        }
        let key = Key::Str(self.constant(name));
        let Some(id) = self.with_object(hops, slot)? else {
            return Ok(None);
        };
        Ok(self.own(id, &key)?.map(|_| id))
    }
    /** GetBindingValue (spec 9.1.1.2.6) on a bound object: a property that vanished reads
    undefined in sloppy code and throws in strict code. */
    fn read_binding(&mut self, id: ObjectId, name: u32) -> Res<JsValue> {
        let key = Key::Str(self.constant(name));
        if self.has_property(id, &key)? {
            return self.get(&JsValue::Object(id), &key);
        }
        if self.reference_strict() {
            let name = self.constant(name).lossy();
            return Err(self.reference_error(&format!("{name} is not defined")));
        }
        Ok(JsValue::Undefined)
    }
    /** SetMutableBinding (spec 9.1.1.2.5) on a bound object: HasProperty always runs (a
    Proxy observes it), then Set whether or not the property survived; strict code throws
    first when it vanished. */
    fn write_binding(&mut self, id: ObjectId, name: u32, value: JsValue) -> Res<()> {
        let key = Key::Str(self.constant(name));
        let strict = self.reference_strict();
        let still_exists = self.has_property(id, &key)?;
        if strict && !still_exists {
            let name = self.constant(name).lossy();
            return Err(self.reference_error(&format!("{name} is not defined")));
        }
        self.set(&JsValue::Object(id), key, value, strict)
    }
    /** Push the `with` binding's value and true (and the object as `this` for a call), or false. */
    pub(super) fn get_with(&mut self, name: u32, hops: u16, slot: u16, call: bool) -> Res<()> {
        let Some((id, _)) = self.with_binding(hops, slot, name)? else {
            self.push(JsValue::Boolean(false));
            return Ok(());
        };
        let value = self.read_binding(id, name)?;
        self.push(value);
        if call {
            self.push(JsValue::Object(id));
        }
        self.push(JsValue::Boolean(true));
        Ok(())
    }
    /** Store the top of stack into the `with` binding, keeping it; push whether it was bound. */
    pub(super) fn set_with(&mut self, name: u32, hops: u16, slot: u16) -> Res<()> {
        let Some((id, _)) = self.with_binding(hops, slot, name)? else {
            self.push(JsValue::Boolean(false));
            return Ok(());
        };
        let value = self.peek(0);
        self.write_binding(id, name, value)?;
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
    /** The reference step of an assignment or update (spec 13.15.2 step 1): push the object
    that holds the name and true, or false, before the right-hand side can change it. */
    pub(super) fn resolve_ref(&mut self, name: u32, hops: u16, slot: u16, with: bool) -> Res<()> {
        let found = self.ref_binding(hops, slot, name, with)?;
        if let Some(id) = found {
            self.push(JsValue::Object(id));
        }
        self.push(JsValue::Boolean(found.is_some()));
        Ok(())
    }
    /** Read through a resolved reference: `base -> base value true`, or `base false` when no
    object was resolved and the static binding answers. */
    pub(super) fn get_ref(&mut self, name: u32) -> Res<()> {
        let JsValue::Object(id) = self.peek(0) else {
            self.push(JsValue::Boolean(false));
            return Ok(());
        };
        let value = self.read_binding(id, name)?;
        self.push(value);
        self.push(JsValue::Boolean(true));
        Ok(())
    }
    /** Write through a resolved reference: `base value -> value true`, or `value false` when no
    object was resolved and the static binding takes the store. */
    pub(super) fn set_ref(&mut self, name: u32) -> Res<()> {
        let value = self.pop();
        let base = self.pop();
        if let JsValue::Object(id) = base {
            self.write_binding(id, name, value.clone())?;
        }
        self.push(value);
        self.push(JsValue::Boolean(matches!(base, JsValue::Object(_))));
        Ok(())
    }
}
