use super::heap::{Key, Prop, Slot};
/** Realm- and parsed-source-owned template objects, with bounded inert cache state and source-sensitive tracing. */
use super::{Res, Vm};
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};
mod cache;
mod tracing;
mod validation;
pub(in crate::v9) use cache::Cache;

impl Vm<'_> {
    /** One parsed template slot in one realm returns the same frozen object for that source's lifetime. */
    pub(super) fn cached_template(&mut self, index: u32) -> Res<ObjectId> {
        let code = self.frames.last().expect("template frame").code.clone();
        let base = code
            .source
            .as_ref()
            .map(|(_, base)| *base)
            .ok_or_else(|| EngineError::invalid("V9 template source is missing."))?;
        let key = (self.active_realm, base, code.site, index);
        if let Some(id) = self.templates.get(&key) {
            return Ok(id);
        }
        let chunks = code
            .templates
            .get(index as usize)
            .ok_or_else(|| EngineError::invalid("V9 template slot is missing."))?;
        let roots = self.pins.len();
        let result = self.frozen_template(chunks);
        if let Ok(id) = result {
            self.templates.insert(key, id);
        }
        self.pins.truncate(roots);
        result
    }

    /** New arrays are rooted before another allocation or descriptor operation can collect them. */
    fn frozen_template(&mut self, chunks: &[(Option<JsString>, JsString)]) -> Res<ObjectId> {
        let raw = self.new_array(
            chunks
                .iter()
                .map(|(_, raw)| JsValue::String(raw.clone()))
                .collect(),
        )?;
        self.pin(JsValue::Object(raw));
        let cooked = self.new_array(
            chunks
                .iter()
                .map(|(value, _)| value.clone().map_or(JsValue::Undefined, JsValue::String))
                .collect(),
        )?;
        self.pin(JsValue::Object(cooked));
        self.heap.put(
            cooked,
            Key::from("raw"),
            Prop {
                slot: Slot::Data(JsValue::Object(raw)),
                attrs: 0,
            },
        );
        crate::v9::builtins::freeze_object(self, raw)?;
        crate::v9::builtins::freeze_object(self, cooked)?;
        Ok(cooked)
    }
}
