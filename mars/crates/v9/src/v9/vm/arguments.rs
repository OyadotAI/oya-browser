/** Mapped arguments share parameter cells while deletion or descriptor changes detach indices. */
use super::data::{ArgumentsData, Class, FunctionKind};
use super::heap::{EnvId, HIDDEN_ATTRS, Key, Prop, Slot};
use super::{Res, Vm};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};

impl Vm<'_> {
    /** The newest active invocation exposes its immediate sloppy caller, never a native callback. */
    pub(super) fn active_caller(&self, callee: ObjectId) -> JsValue {
        let Some(position) = self.frames.iter().rposition(|frame| {
            self.real_activation(frame) && frame.callee == JsValue::Object(callee)
        }) else {
            return JsValue::Null;
        };
        if self.frames[position].caller_hidden {
            return JsValue::Null;
        }
        self.frames[..position]
            .iter()
            .rev()
            .find(|frame| self.real_activation(frame))
            .filter(|frame| {
                !frame.code.strict
                    && frame
                        .callee
                        .object()
                        .is_some_and(|id| self.caller_realm_visible(id))
            })
            .map_or(JsValue::Null, |frame| frame.callee.clone())
    }
    /** Eval/script frames are not calls, even when a direct eval carries the caller's callee value. */
    fn real_activation(&self, frame: &super::Frame) -> bool {
        frame.callee.object().and_then(|id| self.heap.get(id)).and_then(|object| object.function())
            .is_some_and(|function| matches!(function.kind, FunctionKind::Closure { site, .. } if site == frame.code.site))
    }
    /** V8's legacy function.arguments exposes a fresh snapshot of the active call's inputs. */
    pub(super) fn active_arguments(&mut self, callee: ObjectId) -> Res<JsValue> {
        let args = self
            .frames
            .iter()
            .rev()
            .find(|frame| frame.callee == JsValue::Object(callee))
            .map(|frame| frame.args.clone());
        match args {
            Some(args) => {
                let id = self.arguments_list(&args)?;
                self.heap.put(
                    id,
                    Key::from("callee"),
                    Prop {
                        slot: Slot::Data(JsValue::Object(callee)),
                        attrs: HIDDEN_ATTRS,
                    },
                );
                Ok(JsValue::Object(id))
            }
            None => Ok(JsValue::Null),
        }
    }
    /** Attach compiler-selected parameter cells without retaining stale copies of their values. */
    pub(super) fn map_arguments(&mut self, id: ObjectId, count: usize) {
        let Some(frame) = self.frames.last() else {
            return;
        };
        let (Some(slots), Some(env)) = (&frame.code.arguments, frame.env) else {
            return;
        };
        let data = ArgumentsData {
            site: frame.code.site,
            env,
            slots: slots.iter().take(count).copied().collect(),
        };
        if data.slots.iter().all(Option::is_none) {
            return;
        }
        for (index, slot) in data.slots.iter().enumerate() {
            if slot.is_some() {
                let key = Key::from(&index.to_string());
                if let Some(mut prop) = self.heap.get(id).and_then(|o| o.props.get(&key)).cloned() {
                    prop.slot = Slot::Data(JsValue::Undefined);
                    self.heap.put(id, key, prop);
                }
            }
        }
        if let Some(object) = self.heap.get_mut(id) {
            object.class = Class::MappedArguments(Box::new(data));
        }
    }
    /** The live cell an indexed property aliases, if its mapping is still attached. */
    pub(super) fn argument_slot(&self, id: ObjectId, key: &Key) -> Option<(EnvId, u16)> {
        let Class::MappedArguments(data) = &self.heap.get(id)?.class else {
            return None;
        };
        let slot = data.slots.get(key.index()? as usize).copied().flatten()?;
        Some((data.env, slot))
    }
    /** Read the parameter's current value instead of the property's placeholder. */
    pub(super) fn argument_value(&self, id: ObjectId, key: &Key) -> Res<Option<JsValue>> {
        let Some((env, slot)) = self.argument_slot(id, key) else {
            return Ok(None);
        };
        self.heap
            .env(env)
            .and_then(|e| e.slots.get(slot as usize))
            .cloned()
            .map(Some)
            .ok_or_else(|| EngineError::invalid("Invalid V9 arguments parameter cell.").into())
    }
    /** A detached index becomes an ordinary property and never maps again. */
    pub(super) fn detach_argument(&mut self, id: ObjectId, key: &Key) {
        let Some(index) = key.index() else { return };
        if let Some(Class::MappedArguments(data)) = self.heap.get_mut(id).map(|o| &mut o.class)
            && let Some(slot) = data.slots.get_mut(index as usize)
        {
            *slot = None;
            if data.slots.iter().all(Option::is_none)
                && let Some(object) = self.heap.get_mut(id)
            {
                object.class = Class::Arguments;
            }
        }
    }
    /** A mapped property stores only its attributes; the environment owns the live value. */
    pub(super) fn argument_write(&mut self, id: ObjectId, key: &Key, value: JsValue) -> JsValue {
        if let Some((env, slot)) = self.argument_slot(id, key) {
            self.heap.set_slot(env, slot as usize, value);
            JsValue::Undefined
        } else {
            value
        }
    }
}
