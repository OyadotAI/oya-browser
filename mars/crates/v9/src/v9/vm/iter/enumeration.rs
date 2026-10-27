/* For-in collects proxy keys before filtering descriptors, then tests each candidate from the original receiver. */
use super::{Class, EngineError, FOR_IN_DEPTH, IterState, JsValue, Key, ObjectId, Res, Vm};
use crate::v9::vm::heap::ENUMERABLE;
use std::collections::BTreeSet;

impl Vm<'_> {
    /** Snapshot string candidates while retaining every temporary prototype produced by an author trap. */
    pub fn for_in_iterator(&mut self, value: &JsValue) -> Res<ObjectId> {
        if value.nullish() {
            return self.new_iterator(IterState::ForIn {
                object: JsValue::Undefined,
                keys: Vec::new(),
                index: 0,
            });
        }
        let roots = self.pins.len();
        let result = (|| {
            let receiver = self.to_object(value)?;
            self.pin(JsValue::Object(receiver));
            let keys = self.for_in_keys(receiver)?;
            self.new_iterator(IterState::ForIn {
                object: JsValue::Object(receiver),
                keys,
                index: 0,
            })
        })();
        self.pins.truncate(roots);
        result
    }
    /** Ordinary nonenumerable properties shadow prototypes; proxies defer their descriptor observations until iteration. */
    fn for_in_keys(&mut self, receiver: ObjectId) -> Res<Vec<Key>> {
        let (mut keys, mut seen, mut chain) = (Vec::new(), BTreeSet::new(), BTreeSet::new());
        let mut current = Some(receiver);
        while let Some(id) = current {
            self.charge(1)?;
            if !chain.insert(id) {
                return Err(self.range_error("Cyclic prototype chain in for-in"));
            }
            if chain.len() > FOR_IN_DEPTH {
                return Err(EngineError::limit("V9 for-in prototype chain", FOR_IN_DEPTH).into());
            }
            self.pin(JsValue::Object(id));
            let proxy = self.proxy_of(id).is_some();
            for key in self.own_keys(id)? {
                self.charge(1)?;
                if !matches!(key, Key::Str(_)) || !seen.insert(key.clone()) {
                    continue;
                }
                if proxy
                    || self
                        .own(id, &key)?
                        .is_some_and(|property| property.attrs & ENUMERABLE != 0)
                {
                    keys.push(key)
                }
            }
            current = crate::v9::builtins::object_get_prototype(self, id)?;
        }
        // Conformance only: V8 cannot enumerate more keys than a fixed array holds, so an oversized
        // typed array throws RangeError here. Browser builds keep the unbounded snapshot.
        #[cfg(feature = "conformance")]
        if keys.len() > crate::constants::VM_ARRAY_LENGTH {
            return Err(self.range_error("Invalid array length"));
        }
        Ok(keys)
    }
    /** Candidate presence checks use descriptors and virtual prototypes, never the proxy has trap. */
    pub fn for_in_step(&mut self, id: ObjectId) -> Res<Option<JsValue>> {
        loop {
            let (object, key, index) = match self.heap.get(id).map(|object| &object.class) {
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
            if let JsValue::Object(receiver) = object {
                let roots = self.pins.len();
                let visible = self.for_in_visible(receiver, &key);
                self.pins.truncate(roots);
                if visible? {
                    return Ok(Some(key.value()));
                }
            }
        }
    }
    /** Proxy nonenumerability stops a lookup; ordinary candidates retain their snapshot enumeration treatment. */
    fn for_in_visible(&mut self, receiver: ObjectId, key: &Key) -> Res<bool> {
        let mut current = Some(receiver);
        let mut chain = BTreeSet::new();
        while let Some(id) = current {
            self.charge(1)?;
            if !chain.insert(id) {
                return Err(self.range_error("Cyclic prototype chain in for-in"));
            }
            if chain.len() > FOR_IN_DEPTH {
                return Err(EngineError::limit("V9 for-in prototype chain", FOR_IN_DEPTH).into());
            }
            self.pin(JsValue::Object(id));
            if crate::v9::builtins::typed_index(self, id, key) == Some(None) {
                return Ok(false);
            }
            let proxy = self.proxy_of(id).is_some();
            if let Some(property) = self.own(id, key)? {
                return Ok(!proxy || property.attrs & ENUMERABLE != 0);
            }
            current = crate::v9::builtins::object_get_prototype(self, id)?;
        }
        Ok(false)
    }
}
