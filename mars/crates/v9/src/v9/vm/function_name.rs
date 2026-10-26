/** Proven ordinary function names preserve value snapshots without repeatedly flattening bound/wrapped chains. */
use super::data::{Class, FunctionKind, FunctionName};
use super::heap::{CONFIGURABLE, Key, Prop, Slot, WRITABLE};
use super::property::{Descriptor, compatible, merge};
use super::realm::{Static, realm_for};
use super::{Res, Vm};
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};

impl Vm<'_> {
    /** An ordinary unmodified own name has no observable getter or proxy trap to execute. */
    fn ordinary_name_snapshot(&self, id: ObjectId) -> Option<FunctionName> {
        let key = Key::from("name");
        if let Some(object) = self.heap.get(id) {
            if object.props.get(&key).is_some() || self.virtual_deleted(id, &key) {
                return None;
            }
            let function = object.function()?;
            if let Some(name) = &function.name {
                return Some(name.value_snapshot());
            }
            return match function.kind {
                FunctionKind::Closure { site, .. } => Some(FunctionName::flat(JsValue::String(
                    self.registry.get(site)?.name.clone(),
                ))),
                FunctionKind::Native { id: native, .. } => {
                    Some(FunctionName::flat(super::realm::string(
                        self.object_intrinsics(id)
                            .natives
                            .get(native as usize)?
                            .name,
                    )))
                }
                _ => None,
            };
        }
        let intrinsics = realm_for(id);
        if let Some(native) = intrinsics.native(id) {
            return Some(FunctionName::flat(super::realm::string(native.name)));
        }
        let object = intrinsics.object(id)?;
        object.call?;
        match object.get(&key) {
            Some((_, Static::Value(JsValue::String(name)), _)) => {
                Some(FunctionName::flat(JsValue::String(name.clone())))
            }
            _ => None,
        }
    }
    /** Slow names use the actual Get; only untouched wrapped snapshots acquire deferred resolution depth. */
    pub(in crate::v9) fn capture_name_snapshot(
        &mut self,
        target: ObjectId,
        wrapped: bool,
    ) -> Res<FunctionName> {
        if let Some(mut name) = self.ordinary_name_snapshot(target) {
            if wrapped {
                name.wrapped = name.wrapped.checked_add(1).ok_or_else(|| {
                    EngineError::limit("V9 function name depth", self.limits.functions)
                })?;
            }
            return Ok(name);
        }
        let name = self.get(&JsValue::Object(target), &Key::from("name"))?;
        Ok(FunctionName::flat(match name {
            JsValue::String(_) => name,
            _ => JsValue::String(JsString::empty()),
        }))
    }
    /** Binding adds one immutable prefix after the existing length-then-name Get sequence. */
    pub(in crate::v9) fn bind_name_snapshot(&mut self, target: ObjectId) -> Res<FunctionName> {
        let ordinary = self.ordinary_name_snapshot(target).is_some();
        let mut name = self.capture_name_snapshot(target, false)?;
        name.prefixes = name.prefixes.checked_add(1).ok_or_else(|| {
            EngineError::limit("V9 function name prefixes", self.limits.functions)
        })?;
        if !ordinary
            && name
                .length()
                .is_none_or(|length| length > crate::constants::JS_STRING_UNITS)
        {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                crate::constants::JS_STRING_UNITS,
            )
            .into());
        }
        Ok(name)
    }
    /** Actual string observation resolves logical wrapped boundaries under the existing call-depth bound. */
    pub(super) fn render_name_snapshot(&mut self, name: &FunctionName) -> Res<JsValue> {
        // V8 resolves a wrapped name by native recursion, so the chain gets the native bound.
        let available = crate::constants::VM_NESTING_DEPTH
            .saturating_sub(self.frames.len() + self.native_depth);
        if name.wrapped as usize >= available && name.wrapped != 0 {
            return Err(self.range_error("Maximum call stack size exceeded"));
        }
        self.charge(name.wrapped as usize)?;
        let length = name
            .length()
            .ok_or_else(|| EngineError::invalid("Invalid function name snapshot."))?;
        if length > crate::constants::JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                crate::constants::JS_STRING_UNITS,
            )
            .into());
        }
        let JsValue::String(tail) = &name.tail else {
            return Err(EngineError::invalid("Invalid function name tail.").into());
        };
        if name.prefixes == 0 {
            return Ok(JsValue::String(tail.clone()));
        }
        self.charge(length)?;
        let mut units = Vec::with_capacity(length);
        for _ in 0..name.prefixes {
            units.extend_from_slice(super::data::BOUND_PREFIX)
        }
        units.extend_from_slice(tail.as_units());
        Ok(JsValue::String(JsString::from_units(units)?))
    }
    /** Tail accounting follows the same existing unit budget as ordinary own string properties. */
    pub(in crate::v9) fn set_name_snapshot(&mut self, id: ObjectId, name: FunctionName) -> Res<()> {
        self.replace_name_snapshot(id, Some(name))
    }
    /** Replacement does not leave stale snapshots behind a later authored property or deletion mark. */
    fn replace_name_snapshot(&mut self, id: ObjectId, name: Option<FunctionName>) -> Res<()> {
        let Some(Class::Function(function)) = self.heap.get_mut(id).map(|object| &mut object.class)
        else {
            return Err(EngineError::invalid("Invalid function name receiver.").into());
        };
        let same =
            function.name.as_ref().map(|name| &name.tail) == name.as_ref().map(|name| &name.tail);
        let current = if same {
            0
        } else {
            name.as_ref().map_or(0, |name| name.tail.string_units())
        };
        function.name = name;
        self.heap.adjust(0, current as isize);
        Ok(())
    }
    /** Only still-virtual metadata uses this path; author properties remain ordinary property-table entries. */
    fn own_name_snapshot(&self, id: ObjectId, key: &Key) -> Option<FunctionName> {
        if !matches!(key,Key::Str(name) if name.is("name")) || self.virtual_deleted(id, key) {
            return None;
        }
        let object = self.heap.get(id)?;
        if object.props.get(key).is_some() {
            return None;
        }
        object.function()?.name.clone()
    }
    /** Set/Delete inspect data attributes without forcing a lazy name's value. */
    pub(super) fn own_for_write(&mut self, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
        if let Some(name) = self.own_name_snapshot(id, key) {
            return Ok(Some(Prop {
                slot: Slot::Data(JsValue::Undefined),
                attrs: name.attrs,
            }));
        }
        self.own(id, key)
    }
    /** Assignment to a writable virtual name installs an ordinary data value, preserving its attributes. */
    pub(super) fn write_name_snapshot(
        &mut self,
        id: ObjectId,
        key: &Key,
        value: &JsValue,
    ) -> Res<Option<bool>> {
        let Some(name) = self.own_name_snapshot(id, key) else {
            return Ok(None);
        };
        if name.attrs & WRITABLE == 0 {
            return Ok(Some(false));
        }
        self.replace_name_snapshot(id, None)?;
        self.heap.put(
            id,
            key.clone(),
            Prop {
                slot: Slot::Data(value.clone()),
                attrs: name.attrs,
            },
        );
        Ok(Some(true))
    }
    /** Configurable deletion need not evaluate the deleted data value. */
    pub(super) fn clear_name_snapshot(&mut self, id: ObjectId, key: &Key) -> Res<()> {
        if matches!(key,Key::Str(name) if name.is("name"))
            && self
                .heap
                .get(id)
                .and_then(|object| object.function())
                .is_some_and(|function| function.name.is_some())
        {
            self.replace_name_snapshot(id, None)?;
        }
        Ok(())
    }
    /** DefineProperty preserves immutable snapshots for attribute-only changes and handles replacements without old-value Gets. */
    pub(super) fn define_name_snapshot(
        &mut self,
        id: ObjectId,
        key: &Key,
        desc: &Descriptor,
    ) -> Res<Option<bool>> {
        let Some(mut name) = self.own_name_snapshot(id, key) else {
            return Ok(None);
        };
        if name.attrs & (CONFIGURABLE | WRITABLE) == 0 && desc.value.is_some() {
            return Ok(None);
        }
        let existing = Prop {
            slot: Slot::Data(JsValue::Undefined),
            attrs: name.attrs,
        };
        if !compatible(true, desc, Some(&existing)) {
            return Ok(Some(false));
        }
        let property = merge(Some(existing), desc.clone());
        if desc.value.is_none() && !desc.accessor() {
            name.attrs = property.attrs;
            self.replace_name_snapshot(id, Some(name))?;
        } else {
            self.replace_name_snapshot(id, None)?;
            self.heap.put(id, key.clone(), property);
        }
        Ok(Some(true))
    }
}
