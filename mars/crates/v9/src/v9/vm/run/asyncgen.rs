/** Async iteration ops: GetIterator with kind async, the steps of `for await`, AsyncIteratorClose,
and `yield*` in an async generator, which awaits each inner result before yielding its value. */
use super::Flow;
use crate::protocol::EngineError;
use crate::v9::compile::Op;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, IterState};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::{realm, symbol};
use crate::v9::vm::{Exit, Res, Resume, Vm};

/** What an async `yield*` step awaits: a next, throw or return result, the return() result of
closing an iterator without `throw`, or a return value when the iterator has no `return`. */
const STEP_NEXT: f64 = 0.0;
const STEP_THROW: f64 = 1.0;
const STEP_RETURN: f64 = 2.0;
const STEP_CLOSE: f64 = 3.0;
const STEP_RETURN_VALUE: f64 = 4.0;

impl Vm<'_> {
    /** Run one async iteration op. */
    pub(super) fn async_iteration_op(&mut self, op: Op) -> Res<Flow> {
        match op {
            Op::GetAsyncIterator => {
                let value = self.peek(0);
                let record = self.get_async_iterator(&value)?;
                self.pop();
                self.push(JsValue::Object(record));
            }
            Op::AsyncIterNext => {
                let (iterator, next) = self.top_record()?;
                let result = self.call(next, iterator, Vec::new())?;
                self.pop();
                self.push(result);
            }
            Op::AsyncIterValue(target) => {
                let result = self.peek(0);
                self.result_object(&result)?;
                if self.get(&result, &Key::from("done"))?.truthy() {
                    self.pop();
                    self.frame().pc = target as usize;
                } else {
                    let value = self.get(&result, &Key::from("value"))?;
                    self.pop();
                    self.push(value);
                }
            }
            Op::AsyncIterClose(target) => {
                let (iterator, _) = self.top_record()?;
                match self.get_method(&iterator, &Key::from("return"))? {
                    None => {
                        self.pop();
                        self.frame().pc = target as usize;
                    }
                    Some(method) => {
                        let result = self.call(method, iterator, Vec::new())?;
                        self.pop();
                        self.push(result);
                    }
                }
            }
            Op::AsyncCloseCheck => {
                let result = self.peek(0);
                self.result_object(&result)?;
                self.pop();
            }
            Op::AsyncYieldStar => return self.async_yield_star(),
            Op::AsyncYieldStarResult => return self.async_yield_star_result(),
            _ => return Err(EngineError::invalid("Invalid V9 async iteration op.").into()),
        }
        Ok(Flow::Next)
    }
    /** A TypeError unless an iterator result is an object. */
    fn result_object(&mut self, result: &JsValue) -> Res<()> {
        if matches!(result, JsValue::Object(_)) {
            return Ok(());
        }
        let text = self.callee_text(result);
        Err(self.type_error(&format!("Iterator result {text} is not an object")))
    }
    /** The iterator and next method of the iterator record on top of the stack. */
    fn top_record(&mut self) -> Res<(JsValue, JsValue)> {
        let record = self.peek(0);
        let state = record
            .object()
            .and_then(|id| self.heap.get(id))
            .map(|object| &object.class);
        match state {
            Some(Class::Iter(state)) => match state.as_ref() {
                IterState::Record { iterator, next, .. } => Ok((iterator.clone(), next.clone())),
                _ => Err(EngineError::invalid("Invalid V9 async iterator record.").into()),
            },
            _ => Err(EngineError::invalid("Invalid V9 async iterator record.").into()),
        }
    }
    /** An iterator record object over an iterator and its next method. */
    fn iterator_record(
        &mut self,
        iterator: JsValue,
        next: JsValue,
        proto: ObjectId,
    ) -> Res<ObjectId> {
        self.alloc(Object::new(
            Class::Iter(Box::new(IterState::Record {
                iterator,
                next,
                done: false,
            })),
            Some(proto),
        ))
    }
    /** Call an iterator-producing method and read the iterator's next method. */
    fn open_iterator(&mut self, method: JsValue, value: &JsValue) -> Res<(JsValue, JsValue)> {
        let iterator = self.call(method, value.clone(), Vec::new())?;
        if !matches!(iterator, JsValue::Object(_)) {
            return Err(
                self.type_error("Result of the Symbol.asyncIterator method is not an object")
            );
        }
        self.pin(iterator.clone());
        let next = self.get(&iterator, &Key::from("next"))?;
        Ok((iterator, next))
    }
    /** GetIterator(value, async): Symbol.asyncIterator, else CreateAsyncFromSyncIterator over the
    sync iterator. The AsyncFromSyncIterator holds the sync record as its own iterator state. */
    pub(in crate::v9) fn get_async_iterator(&mut self, value: &JsValue) -> Res<ObjectId> {
        let Some((iterator, next)) = self.async_iterator_parts(value)? else {
            return Err(self.type_error("object is not async iterable"));
        };
        self.iterator_record(iterator, next, realm().ids.iterator_prototype)
    }
    /** The iterator and next method GetIterator(value, async) opens, or None when the value has
    neither Symbol.asyncIterator nor Symbol.iterator (Array.fromAsync then reads an array-like). */
    pub(in crate::v9) fn async_iterator_parts(
        &mut self,
        value: &JsValue,
    ) -> Res<Option<(JsValue, JsValue)>> {
        if let Some(method) = self.get_method(value, &Key::Sym(symbol::ASYNC_ITERATOR))? {
            return self.open_iterator(method, value).map(Some);
        }
        let Some(method) = self.get_method(value, &Key::Sym(symbol::ITERATOR))? else {
            return Ok(None);
        };
        let (iterator, next) = self.open_iterator(method, value)?;
        let wrapper = realm().ids.async_from_sync_iterator_prototype;
        let sync = JsValue::Object(self.iterator_record(iterator, next, wrapper)?);
        self.pin(sync.clone());
        let next = self.get(&sync, &Key::from("next"))?;
        Ok(Some((sync, next)))
    }
    /** Forward a resumption to the inner iterator of an async `yield*` and await its result. */
    fn async_yield_star(&mut self) -> Res<Flow> {
        let mode = std::mem::take(&mut self.frame().resume);
        let received = self.pop();
        let (iterator, next) = self.top_record()?;
        let (step, method) = match mode {
            Resume::Next => (STEP_NEXT, Some(next)),
            Resume::Throw => match self.get_method(&iterator, &Key::from("throw"))? {
                Some(method) => (STEP_THROW, Some(method)),
                None => match self.get_method(&iterator, &Key::from("return"))? {
                    Some(method) => (STEP_CLOSE, Some(method)),
                    None => return Err(self.no_throw_method()),
                },
            },
            Resume::Return => match self.get_method(&iterator, &Key::from("return"))? {
                Some(method) => (STEP_RETURN, Some(method)),
                None => (STEP_RETURN_VALUE, None),
            },
        };
        let awaited = match method {
            Some(method) if step == STEP_CLOSE => self.call(method, iterator, Vec::new())?,
            Some(method) => self.call(method, iterator, vec![received])?,
            None => received,
        };
        self.push(JsValue::Number(step));
        Ok(self.suspend(Exit::Await(awaited)))
    }
    /** The TypeError for delegating a throw to an iterator without `throw`. */
    fn no_throw_method(&mut self) -> super::super::Thrown {
        self.type_error("The iterator does not provide a 'throw' method")
    }
    /** Finish an async `yield*` step with its awaited result: end the delegation or yield. */
    fn async_yield_star_result(&mut self) -> Res<Flow> {
        let result = self.pop();
        let step = match self.pop() {
            JsValue::Number(step) => step,
            _ => return Err(EngineError::invalid("Invalid V9 delegation step.").into()),
        };
        if step == STEP_RETURN_VALUE {
            self.pop();
            return self.do_return(result);
        }
        self.result_object(&result)?;
        if step == STEP_CLOSE {
            return Err(self.no_throw_method());
        }
        if self.get(&result, &Key::from("done"))?.truthy() {
            let value = self.get(&result, &Key::from("value"))?;
            self.pop();
            if step == STEP_RETURN {
                return self.do_return(value);
            }
            self.push(value);
            return Ok(Flow::Next);
        }
        let value = self.get(&result, &Key::from("value"))?;
        self.frame().pc -= 2;
        Ok(self.suspend(Exit::Yield(value)))
    }
}
