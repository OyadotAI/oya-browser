/** The iterator protocol, with native stepping for arrays, strings, maps and sets. */
use super::data::{Class, IterKind, IterState};
use super::heap::{ENUMERABLE, Key, Object};
use super::realm::{realm, symbol};
use super::{Res, Resume, Thrown, Vm};
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};

impl Vm<'_> {
    /** A native iterator object over internal state. */
    pub fn new_iterator(&mut self, state: IterState) -> Res<ObjectId> {
        let ids = &realm().ids;
        let proto = match &state {
            IterState::Array { .. } => ids.array_iterator_prototype,
            IterState::String { .. } => ids.string_iterator_prototype,
            IterState::Table { table, .. } => {
                if matches!(self.heap.get(*table).map(|o| &o.class), Some(Class::Map(_))) {
                    ids.map_iterator_prototype
                } else {
                    ids.set_iterator_prototype
                }
            }
            IterState::Matches { .. } => ids.regexp_iterator_prototype,
            IterState::ForIn { .. } | IterState::Record { .. } => ids.iterator_prototype,
        };
        self.alloc(Object::new(Class::Iter(Box::new(state)), Some(proto)))
    }
    /** GetIterator: native iterators for unmodified builtins, records for everything else. */
    pub fn get_iterator(&mut self, value: &JsValue) -> Res<ObjectId> {
        let method = self.get(value, &Key::Sym(symbol::ITERATOR))?;
        let builtin = method == JsValue::Object(realm().ids.array_values);
        if builtin
            && let JsValue::Object(id) = value
            && matches!(self.heap.get(*id).map(|o| &o.class), Some(Class::Array(_)))
        {
            return self.new_iterator(IterState::Array {
                object: value.clone(),
                index: 0,
                kind: IterKind::Values,
                done: false,
            });
        }
        if !self.is_callable(&method) {
            return Err(self.type_error("object is not iterable"));
        }
        let iterator = self.call(method, value.clone(), Vec::new())?;
        let JsValue::Object(id) = iterator else {
            return Err(self.type_error("Result of the Symbol.iterator method is not an object"));
        };
        if matches!(self.heap.get(id).map(|o| &o.class), Some(Class::Iter(state)) if !matches!(**state, IterState::ForIn { .. }))
            && self.uses_native_next(id)?
        {
            return Ok(id);
        }
        self.pin(iterator.clone());
        let next = self.get(&iterator, &Key::from("next"))?;
        self.new_iterator(IterState::Record {
            iterator,
            next,
            done: false,
        })
    }
    /** Whether an iterator object still uses its builtin `next`. */
    fn uses_native_next(&mut self, id: ObjectId) -> Res<bool> {
        let next = self.get(&JsValue::Object(id), &Key::from("next"))?;
        Ok(matches!(next, JsValue::Object(next) if next >= super::heap::NATIVE_BASE))
    }
    /** Advance an iterator; None when done. */
    pub fn iter_step(&mut self, id: ObjectId) -> Res<Option<JsValue>> {
        let state = match self.heap.get(id).map(|o| &o.class) {
            Some(Class::Iter(state)) => (**state).clone(),
            _ => return Err(EngineError::invalid("Invalid V9 iterator.").into()),
        };
        match state {
            IterState::Record {
                iterator,
                next,
                done,
            } => {
                if done {
                    return Ok(None);
                }
                let result = self.call(next, iterator, Vec::new())?;
                if !matches!(result, JsValue::Object(_)) {
                    return Err(self.type_error("Iterator result is not an object"));
                }
                let finished = self.get(&result, &Key::from("done"))?.truthy();
                if finished {
                    self.mark_done(id);
                    return Ok(None);
                }
                self.get(&result, &Key::from("value")).map(Some)
            }
            _ => self.native_step(id),
        }
    }
    /** Mark an iterator record finished. */
    fn mark_done(&mut self, id: ObjectId) {
        if let Some(Class::Iter(state)) = self.heap.get_mut(id).map(|o| &mut o.class) {
            match state.as_mut() {
                IterState::Record { done, .. }
                | IterState::Array { done, .. }
                | IterState::String { done, .. }
                | IterState::Table { done, .. }
                | IterState::Matches { done, .. } => *done = true,
                IterState::ForIn { .. } => {}
            }
        }
    }
    /** Step a native iterator without calling script code. */
    pub fn native_step(&mut self, id: ObjectId) -> Res<Option<JsValue>> {
        let Some(Class::Iter(state)) = self.heap.get(id).map(|o| &o.class) else {
            return Err(EngineError::invalid("Invalid V9 iterator.").into());
        };
        let state = (**state).clone();
        let result = match state {
            IterState::Array {
                object,
                index,
                kind,
                done,
            } => {
                if done {
                    return Ok(None);
                }
                let length = self.length_of(&object)?;
                if index as usize >= length {
                    None
                } else {
                    let key = Key::Str(JsString::from_utf8(&index.to_string())?);
                    let value = match kind {
                        IterKind::Keys => JsValue::Number(f64::from(index)),
                        IterKind::Values => self.get(&object, &key)?,
                        IterKind::Entries => {
                            let value = self.get(&object, &key)?;
                            JsValue::Object(
                                self.new_array(vec![JsValue::Number(f64::from(index)), value])?,
                            )
                        }
                    };
                    self.set_index(id, index + 1);
                    Some(value)
                }
            }
            IterState::String { text, index, done } => {
                if done {
                    return Ok(None);
                }
                let units = text.as_units();
                if index as usize >= units.len() {
                    None
                } else {
                    let point = text.code_points_from(index as usize);
                    self.set_index(id, index + point.len() as u32);
                    Some(JsValue::String(JsString::from_slice(point)?))
                }
            }
            IterState::Table {
                table,
                index,
                kind,
                done,
            } => {
                if done {
                    return Ok(None);
                }
                self.table_step(id, table, index, kind)?
            }
            IterState::Matches { .. } => return self.matches_step(id),
            IterState::ForIn { .. } | IterState::Record { .. } => {
                return Err(EngineError::invalid("Invalid V9 native iterator.").into());
            }
        };
        if result.is_none() {
            self.mark_done(id);
        }
        Ok(result)
    }
    /** The next live entry of a map or set. */
    fn table_step(
        &mut self,
        id: ObjectId,
        table: ObjectId,
        index: u32,
        kind: IterKind,
    ) -> Res<Option<JsValue>> {
        let mut position = index as usize;
        let entry = loop {
            let Some(Class::Map(data) | Class::Set(data)) = self.heap.get(table).map(|o| &o.class)
            else {
                return Ok(None);
            };
            match data.entries.get(position) {
                None => return Ok(None),
                Some(None) => position += 1,
                Some(Some(entry)) => break entry.clone(),
            }
        };
        let is_set = matches!(self.heap.get(table).map(|o| &o.class), Some(Class::Set(_)));
        self.set_index(id, position as u32 + 1);
        let (key, value) = entry;
        Ok(Some(match kind {
            IterKind::Keys => key,
            IterKind::Values => {
                if is_set {
                    key
                } else {
                    value
                }
            }
            IterKind::Entries => {
                let value = if is_set { key.clone() } else { value };
                JsValue::Object(self.new_array(vec![key, value])?)
            }
        }))
    }
    /** Update a native iterator's position. */
    fn set_index(&mut self, id: ObjectId, next: u32) {
        if let Some(Class::Iter(state)) = self.heap.get_mut(id).map(|o| &mut o.class) {
            match state.as_mut() {
                IterState::Array { index, .. }
                | IterState::String { index, .. }
                | IterState::Table { index, .. }
                | IterState::ForIn { index, .. } => *index = next,
                _ => {}
            }
        }
    }
    /** Close an iterator early, calling its `return` method. */
    pub fn iter_close(&mut self, id: ObjectId) -> Res<()> {
        let state = match self.heap.get(id).map(|o| &o.class) {
            Some(Class::Iter(state)) => (**state).clone(),
            _ => return Ok(()),
        };
        if let IterState::Record {
            iterator,
            done: false,
            ..
        } = state
        {
            self.mark_done(id);
            let method = self.get(&iterator, &Key::from("return"))?;
            if self.is_callable(&method) {
                self.call(method, iterator, Vec::new())?;
            }
        } else {
            self.mark_done(id);
        }
        Ok(())
    }
    /** One `yield*` step: forward the resumption to the inner iterator. */
    pub fn delegate_step(
        &mut self,
        id: ObjectId,
        mode: Resume,
        received: JsValue,
    ) -> Res<(JsValue, bool)> {
        let state = match self.heap.get(id).map(|o| &o.class) {
            Some(Class::Iter(state)) => (**state).clone(),
            _ => return Err(EngineError::invalid("Invalid V9 iterator.").into()),
        };
        let IterState::Record { iterator, next, .. } = state else {
            return match mode {
                Resume::Next => Ok(match self.native_step(id)? {
                    Some(value) => (value, false),
                    None => (JsValue::Undefined, true),
                }),
                Resume::Return => Ok((received, true)),
                Resume::Throw => Err(Thrown::Value(received)),
            };
        };
        let (method, name) = match mode {
            Resume::Next => (next, "next"),
            Resume::Throw => (self.get(&iterator, &Key::from("throw"))?, "throw"),
            Resume::Return => (self.get(&iterator, &Key::from("return"))?, "return"),
        };
        if !self.is_callable(&method) {
            return match mode {
                Resume::Return => Ok((received, true)),
                _ => {
                    self.iter_close(id)?;
                    Err(self
                        .type_error(&format!("The iterator does not provide a '{name}' method")))
                }
            };
        }
        let result = self.call(method, iterator, vec![received])?;
        if !matches!(result, JsValue::Object(_)) {
            return Err(self.type_error("Iterator result is not an object"));
        }
        let done = self.get(&result, &Key::from("done"))?.truthy();
        let value = self.get(&result, &Key::from("value"))?;
        Ok((value, done))
    }
    /** for-in: enumerable string keys along the prototype chain, shadowed names once. */
    pub fn for_in_iterator(&mut self, value: &JsValue) -> Res<ObjectId> {
        let mut keys: Vec<Key> = Vec::new();
        if !value.nullish() {
            let mut seen = std::collections::BTreeSet::new();
            let mut current = Some(self.to_object(value)?);
            let mut depth = 0;
            while let Some(id) = current {
                for key in self.own_keys(id)? {
                    if !matches!(key, Key::Str(_)) || !seen.insert(key.clone()) {
                        continue;
                    }
                    if let Some(prop) = self.own(id, &key)?
                        && prop.attrs & ENUMERABLE != 0
                    {
                        keys.push(key);
                    }
                }
                current = self.proto_of(id);
                depth += 1;
                if depth > FOR_IN_DEPTH {
                    break;
                }
            }
        }
        self.charge(keys.len())?;
        let object = if value.nullish() {
            JsValue::Undefined
        } else {
            JsValue::Object(self.to_object(value)?)
        };
        self.new_iterator(IterState::ForIn {
            object,
            keys,
            index: 0,
        })
    }
    /** The next for-in key still present on the object. */
    pub fn for_in_step(&mut self, id: ObjectId) -> Res<Option<JsValue>> {
        loop {
            let (object, key, index) = match self.heap.get(id).map(|o| &o.class) {
                Some(Class::Iter(state)) => match state.as_ref() {
                    IterState::ForIn {
                        object,
                        keys,
                        index,
                    } => match keys.get(*index as usize) {
                        Some(key) => (object.clone(), key.clone(), *index),
                        None => return Ok(None),
                    },
                    _ => return Err(EngineError::invalid("Invalid V9 for-in iterator.").into()),
                },
                _ => return Err(EngineError::invalid("Invalid V9 for-in iterator.").into()),
            };
            self.set_index(id, index + 1);
            if let JsValue::Object(target) = object
                && self.has_property(target, &key)?
            {
                return Ok(Some(key.value()));
            }
        }
    }
    /** `length` of an array-like as an integer. */
    pub fn length_of(&mut self, value: &JsValue) -> Res<usize> {
        if let JsValue::Object(id) = value
            && let Some(Class::Array(elements)) = self.heap.get(*id).map(|o| &o.class)
        {
            return Ok(elements.len());
        }
        let length = self.get(value, &Key::from("length"))?;
        let length = self.to_integer(&length)?;
        Ok(length.clamp(0.0, crate::constants::VM_MAX_SAFE_LENGTH) as usize)
    }
    /** An iterator result object `{value, done}`. */
    pub fn iter_result(&mut self, value: JsValue, done: bool) -> Res<JsValue> {
        let id = self.new_object()?;
        self.put(id, "value", value)?;
        self.put(id, "done", JsValue::Boolean(done))?;
        Ok(JsValue::Object(id))
    }
    /** Collect an iterable's values. */
    pub fn iterate_to_vec(&mut self, value: &JsValue) -> Res<Vec<JsValue>> {
        let iterator = self.get_iterator(value)?;
        self.pin(JsValue::Object(iterator));
        let mut out = Vec::new();
        while let Some(item) = self.iter_step(iterator)? {
            if out.len() >= self.limits.array_length {
                return Err(EngineError::limit("V9 array length", self.limits.array_length).into());
            }
            self.pin(item.clone());
            out.push(item);
        }
        Ok(out)
    }
}

/** Prototype levels for-in walks. */
const FOR_IN_DEPTH: usize = 64;

impl JsString {
    /** The code point starting at a unit offset (one or two units). */
    pub fn code_points_from(&self, offset: usize) -> &[u16] {
        let units = self.as_units();
        let end = if (0xd800..=0xdbff).contains(&units[offset])
            && units
                .get(offset + 1)
                .is_some_and(|unit| (0xdc00..=0xdfff).contains(unit))
        {
            offset + 2
        } else {
            offset + 1
        };
        &units[offset..end]
    }
}
