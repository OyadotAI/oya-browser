/** A captured super reference is ordinary rooted stack values, never a synthetic object or a mutable runtime mode. */
use super::*;

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
        let key = self.to_key(&value)?;
        let start = base
            .object()
            .ok_or_else(|| self.type_error("Cannot read properties of null"))?;
        let value = self.get_from(start, &key, &receiver)?;
        self.push(value);
        Ok(())
    }

    /** PutValue uses the captured base and receiver even after getters or numeric conversions change the home prototype. */
    pub(super) fn write_super_reference(&mut self) -> Res<()> {
        let receiver = self.peek(3);
        let base = self.peek(2);
        let key_value = self.peek(1);
        let key = self.to_key(&key_value)?;
        let value = self.peek(0);
        let start = base
            .object()
            .ok_or_else(|| self.type_error("Cannot set properties of null"))?;
        if !self.set_from(start, key.clone(), value.clone(), &receiver)? && self.reference_strict()
        {
            return Err(self.type_error(&format!(
                "Cannot assign to read only property '{}' of object",
                crate::v9::vm::property::key_text(&key)
            )));
        }
        self.drop_n(4);
        self.push(value);
        Ok(())
    }
}
