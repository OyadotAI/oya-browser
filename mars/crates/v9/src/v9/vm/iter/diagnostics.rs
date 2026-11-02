/** Iterator protocol diagnostics describe rejected values without calling authored conversion hooks. */
use super::*;

impl Vm<'_> {
    /** Bad next/return results retain the actual primitive in the TypeError message. */
    pub(in crate::v9) fn iterator_result_error(&mut self, value: &JsValue) -> Thrown {
        let description = self.iterator_value_description(value);
        self.type_error(&format!("Iterator result {description} is not an object"))
    }

    /** Opening a noniterable async source fails synchronously before the first await. */
    pub(in crate::v9) fn async_iterable_error(&mut self, value: &JsValue) -> Thrown {
        let description = self.iterator_value_description(value);
        self.type_error(&format!("{description} is not async iterable"))
    }

    /** Only inert primitive data and native brands are inspected; user getters and toString are never run. */
    pub(in crate::v9) fn iterator_value_description(&self, value: &JsValue) -> String {
        match value {
            JsValue::Undefined | JsValue::Hole => "undefined".into(),
            JsValue::Null => "null".into(),
            JsValue::String(value) => value.lossy(),
            JsValue::Boolean(value) => value.to_string(),
            #[cfg(feature = "conformance")]
            JsValue::Smi(value) => crate::v9::value::number_to_string(*value),
            JsValue::Number(value) => crate::v9::value::number_to_string(*value),
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(value) | JsValue::LocalNumber(value) => {
                crate::v9::value::number_to_string(value.value)
            }
            JsValue::BigInt(value) => value.to_string(),
            JsValue::BigIntWide(value) => value.diagnostic(),
            JsValue::Symbol(id) => {
                format!("Symbol({})", crate::v9::builtins::symbol_text(self, *id))
            }
            JsValue::Object(id)
                if self
                    .heap
                    .get(*id)
                    .is_some_and(|object| matches!(object.class, Class::Plain)) =>
            {
                "{}".into()
            }
            JsValue::Object(_) => "object".into(),
        }
    }
}
