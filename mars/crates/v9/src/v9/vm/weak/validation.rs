/** Saved weak edges cannot bypass source-independent object, token or generation validation. */
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::Vm;
use crate::v9::vm::data::Class;
use crate::v9::vm::realm::symbol;

impl Vm<'_> {
    /** Weak edges are checked too: they must never hide a forged dangling saved reference. */
    pub(in crate::v9::vm) fn validate_weak(&self) -> EngineResult<()> {
        for object in self.heap.objects.values() {
            if object.age > crate::v9::vm::gc::PROMOTION_AGE {
                return Err(EngineError::invalid(
                    "Invalid V9 state graph: object generation.",
                ));
            }
            let valid = match &object.class {
                Class::WeakRef(target) => self.valid_weak(target.as_ref()),
                Class::FinalizationRegistry(data) => {
                    self.resolves_weak_object(data.callback)
                        && data.cells.iter().all(|cell| {
                            self.valid_weak(cell.target.as_ref())
                                && self.valid_weak(cell.token.as_ref())
                                && cell.holdings != JsValue::Hole
                                && cell.target.as_ref() != Some(&cell.holdings)
                        })
                }
                Class::WeakMap(table) | Class::WeakSet(table) => {
                    table.entries.iter().flatten().all(|(key, value)| {
                        self.valid_weak(Some(key)) && self.valid_weak_value(value)
                    })
                }
                _ => true,
            };
            if !valid {
                return Err(EngineError::invalid(
                    "Invalid V9 state graph: weak reference.",
                ));
            }
        }
        Ok(())
    }

    /** Ephemeron values are not strong edges but still cannot smuggle dangling saved records. */
    fn valid_weak_value(&self, value: &JsValue) -> bool {
        match value {
            JsValue::Object(id) => self.resolves_weak_object(*id),
            JsValue::Symbol(id) => {
                (*id as usize) < symbol::WELL_KNOWN as usize + self.symbols.descriptions.len()
            }
            JsValue::Hole => false,
            _ => true,
        }
    }

    /** A live weak identity is an actual object or a valid unregistered symbol. */
    fn valid_weak(&self, value: Option<&JsValue>) -> bool {
        match value {
            None => true,
            Some(JsValue::Object(id)) => self.resolves_weak_object(*id),
            Some(JsValue::Symbol(id)) => {
                self.can_hold_weakly(&JsValue::Symbol(*id))
                    && (*id as usize)
                        < symbol::WELL_KNOWN as usize + self.symbols.descriptions.len()
            }
            _ => false,
        }
    }

    /** Intrinsics need no heap allocation; all other object identities must resolve exactly. */
    fn resolves_weak_object(&self, id: ObjectId) -> bool {
        self.heap.get(id).is_some()
            || self.realm_identity_valid(id)
                && (self.object_intrinsics(id).object(id).is_some()
                    || self.object_intrinsics(id).native(id).is_some())
    }
}
