/** Compact pure hole-array iteration while preserving iterator state and call ordering. */
use super::super::array_elements::ArrayElements;
use super::super::data::{Class, IterKind, IterState};
use super::super::{Res, Vm};
use crate::v9::value::{JsValue, ObjectId};

impl Vm<'_> {
    /** Every index lookup must be a side-effect-free missing property throughout the chain. */
    fn inert_hole_chain(&self, source: ObjectId) -> bool {
        let mut current = Some(source);
        for _ in 0..self.limits.frames {
            let Some(id) = current else {
                return true;
            };
            if let Some(object) = self.heap.get(id) {
                #[cfg(feature = "conformance")]
                if object.global {
                    return false;
                }
                if object.props.iter().any(|(key, _)| key.index().is_some()) {
                    return false;
                }
                match &object.class {
                    Class::Array(elements) if elements.logical_present() == 0 => {}
                    Class::Plain => {}
                    _ => return false,
                }
                current = object.proto;
            } else {
                let Some(object) = self.object_intrinsics(id).object(id) else {
                    return false;
                };
                if object.props.iter().any(|(key, _, _)| key.index().is_some()) {
                    return false;
                }
                current = object.proto;
            }
        }
        false
    }

    /** Missing array values are undefined and can share one primitive backing value. */
    pub(super) fn compact_hole_spread(&mut self, array: &JsValue, iterator: ObjectId) -> Res<bool> {
        let Some(target) = array.object() else {
            return Ok(false);
        };
        if !matches!(self.heap.get(target).map(|object| &object.class), Some(Class::Array(elements)) if elements.is_empty())
        {
            return Ok(false);
        }
        let source = match self.heap.get(iterator).map(|object| &object.class) {
            Some(Class::Iter(state)) => match state.as_ref() {
                IterState::Array {
                    object: JsValue::Object(id),
                    index: 0,
                    kind: IterKind::Values,
                    done: false,
                } => *id,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        let length = match self.heap.get(source).map(|object| &object.class) {
            Some(Class::Array(elements))
                if elements.logical_present() == 0 && elements.len() > self.limits.array_length =>
            {
                elements.len()
            }
            _ => return Ok(false),
        };
        if source == target || !self.inert_hole_chain(source) {
            return Ok(false);
        }
        let seed = ArrayElements::from(vec![JsValue::Undefined]);
        let Some(elements) = ArrayElements::repeated_copy(&seed, length) else {
            return Ok(false);
        };
        if self
            .heap
            .counts
            .properties
            .saturating_add(elements.resident_len())
            > self.limits.properties
        {
            return Ok(false);
        }
        self.charge(length)?;
        let slots = elements.resident_len();
        let Some(Class::Array(destination)) =
            self.heap.get_mut(target).map(|object| &mut object.class)
        else {
            return Ok(false);
        };
        *destination = elements;
        self.heap.adjust(slots as isize, 0);
        if let Some(Class::Iter(state)) =
            self.heap.get_mut(iterator).map(|object| &mut object.class)
            && let IterState::Array { index, done, .. } = state.as_mut()
        {
            *index = length as u32;
            *done = true;
        }
        Ok(true)
    }
}
