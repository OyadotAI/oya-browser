/** The iterator protocol, with native stepping for arrays, strings, maps and sets. */
use super::data::{Class, IterKind, IterState};
use super::heap::{Key, Object};
use super::realm::{Static, realm, symbol};
use super::{Exit, Res, Resume, Thrown, Vm};
use crate::protocol::EngineError;
mod diagnostics;
mod enumeration;
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
        if !self.is_callable(&method) {
            // Name what was iterated, as V8's message does: a host collection a page expects
            // to be iterable shows up by name.
            let what = self.kind_name(value);
            return Err(self.type_error(&format!("{what} is not iterable")));
        }
        self.iterator_from_method(value, method)
    }
    /** GetIteratorFromMethod consumes an already observed method without repeating the source getter. */
    pub fn iterator_from_method(&mut self, value: &JsValue, method: JsValue) -> Res<ObjectId> {
        let iterator = self.call(method, value.clone(), Vec::new())?;
        let JsValue::Object(id) = iterator else {
            return Err(self.type_error("Result of the Symbol.iterator method is not an object"));
        };
        self.pin(iterator.clone());
        let next = self.get(&iterator, &Key::from("next"))?;
        if self.native_next_matches(id, &next) {
            return Ok(id);
        }
        self.new_iterator(IterState::Record {
            iterator,
            next,
            done: false,
        })
    }
    /** Optimization is valid only for the original next identity, never an arbitrary native function. */
    fn native_next_matches(&self, id: ObjectId, next: &JsValue) -> bool {
        let ids = &self.object_intrinsics(id).ids;
        let prototype = match self.heap.get(id).map(|object| &object.class) {
            Some(Class::Iter(state)) => match state.as_ref() {
                IterState::Array { .. } => ids.array_iterator_prototype,
                IterState::String { .. } => ids.string_iterator_prototype,
                IterState::Matches { .. } => ids.regexp_iterator_prototype,
                IterState::Table { table, .. }
                    if matches!(
                        self.heap.get(*table).map(|object| &object.class),
                        Some(Class::Map(_))
                    ) =>
                {
                    ids.map_iterator_prototype
                }
                IterState::Table { .. } => ids.set_iterator_prototype,
                _ => return false,
            },
            _ => return false,
        };
        matches!(self.object_intrinsics(id).object(prototype).and_then(|object| object.get(&Key::from("next"))), Some((_, Static::Value(original), _)) if original == next)
    }
    /** Advance an iterator; None when done. */
    pub fn iter_step(&mut self, id: ObjectId) -> Res<Option<JsValue>> {
        self.iter_step_value(id, true)
    }
    /** Elisions advance the cached next method and observe done, never the result's value property. */
    pub(super) fn iter_skip(&mut self, id: ObjectId) -> Res<()> {
        self.iter_step_value(id, false).map(|_| ())
    }
    /** Native next methods still perform their real work even when a destructuring hole discards the result. */
    fn iter_step_value(&mut self, id: ObjectId, read_value: bool) -> Res<Option<JsValue>> {
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
                    return Err(self.iterator_result_error(&result));
                }
                let roots = self.pins.len();
                self.pin(result.clone());
                let step = (|| {
                    if self.get(&result, &Key::from("done"))?.truthy() {
                        self.mark_done(id);
                        return Ok(None);
                    }
                    if read_value {
                        self.get(&result, &Key::from("value")).map(Some)
                    } else {
                        Ok(Some(JsValue::Undefined))
                    }
                })();
                self.pins.truncate(roots);
                step
            }
            _ => {
                let owner = self.object_realm(id);
                self.with_realm(owner, |vm| vm.native_step(id))
            }
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
                let typed = match object.object() {
                    Some(object) => crate::v9::builtins::typed_iterator_length(self, object)?,
                    None => None,
                };
                let length = match typed {
                    Some(length) => length,
                    None => self.length_of(&object)?,
                };
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
                if index as usize >= text.len() {
                    None
                } else {
                    let high = text.unit_at(index as usize).expect("checked string index");
                    let mut point = vec![high];
                    if let Some(low) = text.unit_at(index as usize + 1)
                        && (0xd800..=0xdbff).contains(&high)
                        && (0xdc00..=0xdfff).contains(&low)
                    {
                        point.push(low);
                    }
                    self.set_index(id, index + point.len() as u32);
                    #[cfg(feature = "conformance")]
                    if self.page.environment.conformance_d8 && point.len() == 1 && high <= 255 {
                        return Ok(Some(JsValue::String(self.character_root(high as u8)?)));
                    }
                    Some(JsValue::String(JsString::from_units(point)?))
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
    /** IteratorClose observes return on native iterators too and checks its result object. */
    pub fn iter_close(&mut self, id: ObjectId) -> Res<()> {
        let state = match self.heap.get(id).map(|object| &object.class) {
            Some(Class::Iter(state)) => (**state).clone(),
            _ => return Ok(()),
        };
        let iterator = match state {
            IterState::Record { done: true, .. }
            | IterState::Array { done: true, .. }
            | IterState::String { done: true, .. }
            | IterState::Table { done: true, .. }
            | IterState::Matches { done: true, .. } => return Ok(()),
            IterState::Record { iterator, .. } => {
                self.mark_done(id);
                iterator
            }
            _ => JsValue::Object(id),
        };
        self.return_iterator(iterator)
    }
    /** V8's numeric-iteration error path exhausts native backings before invoking a patched return. */
    pub fn finish_numeric_iterator(&mut self, mut id: ObjectId) {
        for _ in 0..self.limits.frames {
            match self.heap.get(id).map(|object| &object.class) {
                Some(Class::Iter(state)) => match state.as_ref() {
                    IterState::Record {
                        iterator: JsValue::Object(next),
                        ..
                    } => id = *next,
                    IterState::Record { .. } | IterState::ForIn { .. } => return,
                    _ => {
                        self.mark_done(id);
                        return;
                    }
                },
                _ => return,
            }
        }
    }
    /** Numeric rejection exhausts native backing first but still invokes its patched return exactly once. */
    pub(in crate::v9) fn close_numeric_iterator(&mut self, id: ObjectId) -> Res<()> {
        self.finish_numeric_iterator(id);
        let iterator = match self.heap.get(id).map(|object| &object.class) {
            Some(Class::Iter(state)) => match state.as_ref() {
                IterState::Record { iterator, .. } => iterator.clone(),
                _ => JsValue::Object(id),
            },
            _ => return Err(EngineError::invalid("Invalid V9 numeric iterator.").into()),
        };
        self.mark_done(id);
        self.return_iterator(iterator)
    }
    /** GetMethod and Call are observable even when internal stepping avoided a next lookup. */
    fn return_iterator(&mut self, iterator: JsValue) -> Res<()> {
        let method = self.get(&iterator, &Key::from("return"))?;
        if method.nullish() {
            return Ok(());
        }
        let result = self.call(method, iterator, Vec::new())?;
        if !matches!(result, JsValue::Object(_)) {
            return Err(self.type_error("Iterator return result is not an object"));
        }
        Ok(())
    }
    /** Delegation exposes a noncompleted iterator-result object without observing its value getter. */
    pub fn delegate_step(&mut self, id: ObjectId, mode: Resume, received: JsValue) -> Res<Exit> {
        let state = match self.heap.get(id).map(|object| &object.class) {
            Some(Class::Iter(state)) => (**state).clone(),
            _ => return Err(EngineError::invalid("Invalid V9 iterator.").into()),
        };
        let (iterator, next) = match state {
            IterState::Record { iterator, next, .. } => (iterator, next),
            _ if mode == Resume::Next => {
                let owner = self.object_realm(id);
                return self.with_realm(owner, |vm| match vm.native_step(id)? {
                    Some(value) => {
                        vm.pin(value.clone());
                        vm.iter_result(value, false).map(Exit::YieldRaw)
                    }
                    None => Ok(Exit::Return(JsValue::Undefined)),
                });
            }
            _ => (JsValue::Object(id), JsValue::Undefined),
        };
        let (method, name) = match mode {
            Resume::Next => (next, "next"),
            Resume::Throw => (self.get(&iterator, &Key::from("throw"))?, "throw"),
            Resume::Return => (self.get(&iterator, &Key::from("return"))?, "return"),
        };
        if method.nullish() {
            if mode == Resume::Return {
                return Ok(Exit::Return(received));
            }
            if mode == Resume::Throw {
                self.iter_close(id)?;
                return Err(
                    self.type_error(&format!("The iterator does not provide a '{name}' method"))
                );
            }
        }
        let result = self.call(method, iterator, vec![received])?;
        if !matches!(result, JsValue::Object(_)) {
            return Err(self.iterator_result_error(&result));
        }
        self.pin(result.clone());
        if !self.get(&result, &Key::from("done"))?.truthy() {
            return Ok(Exit::YieldRaw(result));
        }
        self.get(&result, &Key::from("value")).map(Exit::Return)
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
        self.collect_iterator(iterator)
    }
    /** Collect using the already observed iterator method without repeating its getter. */
    pub(in crate::v9) fn iterate_to_vec_with_method(&mut self, value: &JsValue, method: JsValue) -> Res<Vec<JsValue>> {
        let iterator = self.iterator_from_method(value, method)?;
        self.collect_iterator(iterator)
    }
    /** Preserve the existing bounds and rooting for either iterator entry path. */
    fn collect_iterator(&mut self, iterator: ObjectId) -> Res<Vec<JsValue>> {
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
