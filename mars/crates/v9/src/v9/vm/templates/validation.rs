/** Restored literal identities must describe the actual frozen arrays emitted by their freshly recompiled template slot. */
use super::super::{
    Vm,
    data::Class,
    heap::{ENUMERABLE, Key, Slot},
    property::fixed_length_mark,
};
use crate::{
    protocol::{EngineError, EngineResult},
    v9::value::{JsString, JsValue, ObjectId},
};
use std::collections::HashSet;

impl Vm<'_> {
    /** Validate every entry, including ones not reached by the current script, before any script can consume it. */
    pub(in crate::v9::vm) fn validate_template_cache(&self) -> EngineResult<()> {
        let mut objects = HashSet::new();
        for (&(realm, base, site, slot), &id) in &self.templates.0 {
            let code = self
                .registry
                .get(site)
                .filter(|code| {
                    code.source
                        .as_ref()
                        .is_some_and(|(_, start)| *start == base)
                })
                .ok_or_else(invalid)?;
            let chunks = code.templates.get(slot as usize).ok_or_else(invalid)?;
            let object = self.heap.get(id).ok_or_else(invalid)?;
            let raw = match object.props.get(&Key::from("raw")) {
                Some(prop) if prop.attrs == 0 => match prop.slot {
                    Slot::Data(JsValue::Object(raw)) => raw,
                    _ => return Err(invalid()),
                },
                _ => return Err(invalid()),
            };
            if !objects.insert(id) || !objects.insert(raw) {
                return Err(invalid());
            }
            self.validate_template_array(
                id,
                realm,
                chunks.iter().map(|(cooked, _)| cooked.clone()),
                true,
            )?;
            self.validate_template_array(
                raw,
                realm,
                chunks.iter().map(|(_, raw)| Some(raw.clone())),
                false,
            )?;
        }
        Ok(())
    }

    /** Literal arrays contain only canonical readonly element, length and optional raw descriptors. */
    fn validate_template_array(
        &self,
        id: ObjectId,
        realm: u32,
        values: impl ExactSizeIterator<Item = Option<JsString>>,
        cooked: bool,
    ) -> EngineResult<()> {
        let object = self.heap.get(id).ok_or_else(invalid)?;
        let Class::Array(elements) = &object.class else {
            return Err(invalid());
        };
        if !self.realm_identity_valid(id)
            || self.object_realm(id) != realm
            || object.extensible
            || object.proto != Some(self.object_intrinsics(id).ids.array_prototype)
            || elements.len() != values.len()
            || elements.resident_len() != 0
        {
            return Err(invalid());
        }
        if object.props.len() != values.len() + 1 + usize::from(cooked)
            || !object.props.get(&fixed_length_mark()).is_some_and(|prop| {
                prop.attrs == 0 && matches!(prop.slot, Slot::Data(JsValue::Undefined))
            })
        {
            return Err(invalid());
        }
        for (index, value) in values.enumerate() {
            let expected = value.map_or(JsValue::Undefined, JsValue::String);
            if !object
                .props
                .get(&Key::from(index.to_string().as_str()))
                .is_some_and(|prop| {
                    prop.attrs == ENUMERABLE
                        && matches!(&prop.slot,Slot::Data(value)if *value==expected)
                })
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

/** Cache metadata never supplies executable behavior or foreign object identities. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid V9 template cache graph.")
}
