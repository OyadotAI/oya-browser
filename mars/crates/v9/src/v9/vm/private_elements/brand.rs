/** Private access checks use own brands and immutable lexical class metadata for diagnostics. */
use super::*;

impl Vm<'_> {
    /** Class names come from immutable closure metadata rather than an overridable name getter. */
    pub(super) fn class_name(&self, home: ObjectId) -> String {
        let constructor = self
            .heap
            .get(home)
            .and_then(|object| object.props.get(&Key::constructor()))
            .and_then(|prop| match prop.slot {
                Slot::Data(JsValue::Object(id)) => Some(id),
                _ => None,
            })
            .unwrap_or(home);
        self.heap
            .get(constructor)
            .and_then(|object| object.function())
            .and_then(|function| match function.kind {
                crate::v9::vm::data::FunctionKind::Closure { site, .. } => self.registry.get(site),
                _ => None,
            })
            .map(|code| code.name.lossy())
            .unwrap_or_default()
    }

    /** Static methods refer to the constructor; instance methods refer to its prototype. */
    fn private_method_home(&self, home: ObjectId, key: &Key) -> Option<ObjectId> {
        let object = self.heap.get(home)?;
        if object.function().is_some() && object.props.get(key).is_some() {
            return Some(home);
        }
        let template_home = if self.method_template(home).is_some() {
            home
        } else {
            match object
                .props
                .get(&Key::prototype())
                .map(|prop| &prop.slot)
            {
                Some(Slot::Data(JsValue::Object(id))) => *id,
                _ => return None,
            }
        };
        self.method_template(template_home)
            .and_then(|id| self.heap.get(id))
            .filter(|object| object.props.get(key).is_some())
            .map(|_| template_home)
    }
    /** The active lexical class identifies a missing method brand for its receiver diagnostic. */
    fn method_owner(&self, key: &Key) -> Option<ObjectId> {
        self.frames
            .iter()
            .rev()
            .filter_map(|frame| frame.callee.object())
            .filter_map(|id| self.heap.get(id)?.function()?.home)
            .find_map(|home| self.private_method_home(home, key))
    }

    /** PrivateElementFind uses own private slots: inheriting an instance never inherits its brand. */
    pub(in crate::v9) fn private_brand(
        &mut self,
        value: &JsValue,
        key: &Key,
        write: bool,
    ) -> Res<()> {
        let Some(id) = value.object() else {
            return Err(self.type_error("Cannot access a private member of a non-object"));
        };
        self.guard_realm_global(id)?;
        if let Some(prop) = self
            .heap
            .get(id)
            .and_then(|object| object.props.get(key))
            .cloned()
        {
            if write && matches!(prop.slot, Slot::Data(_)) && prop.attrs & WRITABLE == 0 {
                return Err(self.type_error(&format!(
                    "Private method '#{}' is not writable",
                    private_name(key)
                )));
            }
            if write && matches!(prop.slot, Slot::Accessor(_, None)) {
                return Err(self.type_error(&format!(
                    "'#{}' was defined without a setter",
                    private_name(key)
                )));
            }
            if !write && matches!(prop.slot, Slot::Accessor(None, _)) {
                return Err(self.type_error(&format!(
                    "'#{}' was defined without a getter",
                    private_name(key)
                )));
            }
            return Ok(());
        }
        if let Some(home) = self.method_owner(key) {
            return Err(self.type_error(&format!(
                "Receiver must be an instance of class {}",
                self.class_name(home)
            )));
        }
        let (operation, direction) = if write {
            ("write", "to")
        } else {
            ("read", "from")
        };
        Err(self.type_error(&format!("Cannot {operation} private member #{} {direction} an object whose class did not declare it", private_name(key))))
    }
}
