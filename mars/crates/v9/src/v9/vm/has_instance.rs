/** OrdinaryHasInstance follows actual GetPrototypeOf operations, retaining temporary trap results across collection. */
use super::heap::Key;
use super::{Res, Vm};
use crate::v9::value::{JsValue, ObjectId};

impl Vm<'_> {
    /** Constructor and input operands remain native roots while getters and proxy traps execute user code. */
    pub fn ordinary_has_instance(&mut self, constructor: &JsValue, value: &JsValue) -> Res<bool> {
        let roots = self.pins.len();
        self.pin(constructor.clone());
        self.pin(value.clone());
        let result = self.has_instance_target(constructor, value);
        self.pins.truncate(roots);
        result
    }

    /** Bound constructors delegate to the target's full instanceof operation before checking primitive inputs. */
    fn has_instance_target(&mut self, constructor: &JsValue, value: &JsValue) -> Res<bool> {
        if !self.is_callable(constructor) {
            return Ok(false);
        }
        if let Some(target) = self.bound_target(constructor) {
            return self.instance_of(value, &JsValue::Object(target));
        }
        let &JsValue::Object(id) = value else {
            return Ok(false);
        };
        let prototype = self.get(constructor, &Key::prototype())?;
        let JsValue::Object(prototype) = prototype else {
            return Err(self.type_error("Function has non-object prototype in instanceof check"));
        };
        self.pin(JsValue::Object(prototype));
        self.prototype_chain_contains(id, prototype)
    }

    /** Both standard prototype-membership operations retain their operands while actual proxy traps run. */
    pub(in crate::v9) fn prototype_chain_contains(
        &mut self,
        current: ObjectId,
        prototype: ObjectId,
    ) -> Res<bool> {
        let roots = self.pins.len();
        self.pin(JsValue::Object(prototype));
        let cursor = self.pins.len();
        self.pin(JsValue::Object(current));
        let result = self.walk_prototypes(current, prototype, cursor);
        self.pins.truncate(roots);
        result
    }

    /** Exhausting the existing traversal budget is an explicit language exception, never a fabricated absent prototype. */
    fn walk_prototypes(
        &mut self,
        mut current: ObjectId,
        prototype: ObjectId,
        cursor: usize,
    ) -> Res<bool> {
        for _ in 0..super::property::CHAIN_STEPS {
            match crate::v9::builtins::object_get_prototype(self, current)? {
                Some(next) if next == prototype => return Ok(true),
                Some(next) => {
                    current = next;
                    self.pins[cursor] = JsValue::Object(next);
                    self.pins.truncate(cursor + 1);
                }
                None => return Ok(false),
            }
        }
        Err(self.range_error(&format!(
            "Prototype traversal exceeds the limit of {}",
            super::property::CHAIN_STEPS
        )))
    }
}
