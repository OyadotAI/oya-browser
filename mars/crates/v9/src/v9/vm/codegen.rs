/** Host string-code policy is checked before compilation without granting or consulting author-defined constructors. */
use super::{Res, Thrown, Vm};
use crate::v9::value::JsValue;

impl Vm<'_> {
    /** Eval and dynamic Function constructors share the same genuine host admission policy. */
    pub(in crate::v9) fn check_string_codegen(&mut self) -> Res<()> {
        if self.page.environment.disallow_code_generation_from_strings {
            let error = self.make_error(
                super::error::ErrorKind::Eval,
                "Code generation from strings disallowed for this context",
            )?;
            return Err(Thrown::Value(JsValue::Object(error)));
        }
        Ok(())
    }
}
