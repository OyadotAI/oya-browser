/** Async generators (spec 27.6): AsyncGeneratorStart, Enqueue, Resume, CompleteStep, AwaitReturn
and DrainQueue, after V8's async generator builtins. The body runs as a boundary frame; an await
parks it in the generator object, which is also the promise reaction's holder. */
use super::data::{
    AsyncGeneratorData, AsyncRequest, Class, GeneratorState, PromiseState, SavedFrame,
};
use super::heap::{Key, Object};
use super::realm::realm;
use super::{Exit, Res, Resume, Thrown, Vm};
use crate::v9::value::{JsValue, ObjectId};
use std::collections::VecDeque;

/** A request's resumption mode as stored. */
fn mode_code(mode: Resume) -> u8 {
    match mode {
        Resume::Next => 0,
        Resume::Throw => 1,
        Resume::Return => 2,
    }
}

/** A stored request mode back as a resumption. */
fn mode_of(code: u8) -> Resume {
    match code {
        1 => Resume::Throw,
        2 => Resume::Return,
        _ => Resume::Next,
    }
}

impl Vm<'_> {
    /** AsyncGeneratorStart: an async generator object over a body suspended at its start. */
    pub fn new_async_generator(&mut self, frame: SavedFrame, callee: &JsValue) -> Res<ObjectId> {
        let proto = self.prototype_for(callee, realm().ids.async_generator_prototype)?;
        self.alloc(Object::new(
            Class::AsyncGenerator(Box::new(AsyncGeneratorData {
                state: GeneratorState::Start,
                frame: Some(frame),
                queue: VecDeque::new(),
                returning: false,
            })),
            Some(proto),
        ))
    }
    /** The async generator data of an object, if it is one. */
    fn async_gen(&mut self, id: ObjectId) -> Option<&mut AsyncGeneratorData> {
        match self.heap.get_mut(id).map(|object| &mut object.class) {
            Some(Class::AsyncGenerator(data)) => Some(data),
            _ => None,
        }
    }
    /** The state of an async generator (Done for anything else). */
    fn async_gen_state(&mut self, id: ObjectId) -> GeneratorState {
        self.async_gen(id)
            .map_or(GeneratorState::Done, |data| data.state)
    }
    /** Set an async generator's state. */
    fn set_async_gen_state(&mut self, id: ObjectId, state: GeneratorState) {
        if let Some(data) = self.async_gen(id) {
            data.state = state;
        }
    }
    /** AsyncGenerator.prototype.next / throw / return: validate, enqueue and maybe resume. */
    pub fn async_generator_enqueue(
        &mut self,
        this: JsValue,
        mode: Resume,
        value: JsValue,
    ) -> Res<JsValue> {
        let promise = self.new_promise()?;
        self.pin(JsValue::Object(promise));
        let generator = match this {
            JsValue::Object(id) if self.async_gen(id).is_some() => id,
            _ => {
                let error =
                    self.type_error("AsyncGenerator method called on incompatible receiver");
                self.reject_with(promise, error)?;
                return Ok(JsValue::Object(promise));
            }
        };
        let mut state = self.async_gen_state(generator);
        if mode == Resume::Throw && state == GeneratorState::Start {
            self.finish_async_gen(generator);
            state = GeneratorState::Done;
        }
        if state == GeneratorState::Done && mode != Resume::Return {
            if mode == Resume::Throw {
                self.settle(promise, PromiseState::Rejected(value))?;
            } else {
                let result = self.iter_result(JsValue::Undefined, true)?;
                self.resolve_promise(promise, result)?;
            }
            return Ok(JsValue::Object(promise));
        }
        if let Some(data) = self.async_gen(generator) {
            data.queue.push_back(AsyncRequest {
                mode: mode_code(mode),
                value: value.clone(),
                promise,
            });
        }
        match state {
            GeneratorState::Start | GeneratorState::Done if mode == Resume::Return => {
                self.finish_async_gen(generator);
                self.drain_async_gen(generator)?;
            }
            GeneratorState::Start => self.run_async_gen(generator, None, false)?,
            GeneratorState::Yielded => self.run_async_gen(generator, Some((mode, value)), true)?,
            _ => {}
        }
        Ok(JsValue::Object(promise))
    }
    /** Reject a promise with a thrown error, or pass a fatal one on. */
    fn reject_with(&mut self, promise: ObjectId, error: Thrown) -> Res<()> {
        match error {
            Thrown::Value(reason) => self.settle(promise, PromiseState::Rejected(reason)),
            fatal => Err(fatal),
        }
    }
    /** Mark an async generator completed and drop its body. */
    fn finish_async_gen(&mut self, generator: ObjectId) {
        if let Some(data) = self.async_gen(generator) {
            data.state = GeneratorState::Done;
            data.frame = None;
            data.returning = false;
        }
    }
    /** A promise the generator awaited settled: resume its body or finish an awaited return. */
    pub fn async_generator_settled(
        &mut self,
        generator: ObjectId,
        mode: Resume,
        value: JsValue,
    ) -> Res<()> {
        match self.async_gen_state(generator) {
            GeneratorState::AwaitingReturn => {
                self.set_async_gen_state(generator, GeneratorState::Done);
                let result = if mode == Resume::Throw {
                    Err(value)
                } else {
                    Ok(value)
                };
                self.complete_async_step(generator, result, true)?;
                self.drain_async_gen(generator)
            }
            GeneratorState::Running => {
                let returning = self
                    .async_gen(generator)
                    .is_some_and(|data| std::mem::take(&mut data.returning));
                let mode = if returning && mode == Resume::Next {
                    Resume::Return
                } else {
                    mode
                };
                self.run_async_gen(generator, Some((mode, value)), false)
            }
            _ => Ok(()),
        }
    }
    /** Await a value inside the body; a PromiseResolve throw resumes the body with it instead. */
    fn async_gen_await(
        &mut self,
        generator: ObjectId,
        value: JsValue,
        returning: bool,
    ) -> Res<Option<(Resume, JsValue)>> {
        match self.promise_resolve(value) {
            Ok(promise) => {
                if let Some(data) = self.async_gen(generator) {
                    data.returning = returning;
                }
                self.await_promise(promise, generator)?;
                Ok(None)
            }
            Err(Thrown::Value(error)) => Ok(Some((Resume::Throw, error))),
            Err(fatal) => Err(fatal),
        }
    }
    /** AsyncGeneratorResume: run the body until it suspends with no request left, or finishes.
    `at_yield` says the body is parked at a yield, where a return request first awaits its value
    (AsyncGeneratorUnwrapYieldResumption). */
    fn run_async_gen(
        &mut self,
        generator: ObjectId,
        mut resume: Option<(Resume, JsValue)>,
        mut at_yield: bool,
    ) -> Res<()> {
        loop {
            self.set_async_gen_state(generator, GeneratorState::Running);
            if at_yield && let Some((Resume::Return, value)) = &resume {
                match self.async_gen_await(generator, value.clone(), true)? {
                    None => return Ok(()),
                    Some(next) => {
                        resume = Some(next);
                        at_yield = false;
                        continue;
                    }
                }
            }
            let Some(frame) = self.async_gen(generator).and_then(|data| data.frame.take()) else {
                return Ok(());
            };
            self.restore_frame(frame, true)?;
            if let Some((mode, value)) = resume.take() {
                self.resume_with(mode, value)?;
            }
            let pinning = std::mem::replace(&mut self.pinning, false);
            self.native_depth += 1;
            let result = self.run();
            self.native_depth -= 1;
            self.pinning = pinning;
            let parked = matches!(result, Ok(Exit::Yield(_) | Exit::Await(_)));
            if parked {
                let frame = self.suspended.take();
                if let Some(data) = self.async_gen(generator) {
                    data.frame = frame;
                }
            }
            match result {
                Ok(Exit::Yield(value)) => {
                    self.complete_async_step(generator, Ok(value), false)?;
                    let front = self
                        .async_gen(generator)
                        .and_then(|data| data.queue.front().cloned());
                    let Some(request) = front else {
                        self.set_async_gen_state(generator, GeneratorState::Yielded);
                        return Ok(());
                    };
                    resume = Some((mode_of(request.mode), request.value));
                    at_yield = true;
                }
                Ok(Exit::Await(value)) => match self.async_gen_await(generator, value, false)? {
                    None => return Ok(()),
                    Some(next) => {
                        resume = Some(next);
                        at_yield = false;
                    }
                },
                Ok(Exit::Return(value)) => {
                    self.finish_async_gen(generator);
                    self.complete_async_step(generator, Ok(value), true)?;
                    return self.drain_async_gen(generator);
                }
                Err(Thrown::Value(error)) => {
                    self.finish_async_gen(generator);
                    self.complete_async_step(generator, Err(error), true)?;
                    return self.drain_async_gen(generator);
                }
                Err(fatal) => {
                    self.finish_async_gen(generator);
                    return Err(fatal);
                }
            }
        }
    }
    /** AsyncGeneratorCompleteStep: settle the oldest request with a result or an error. */
    fn complete_async_step(
        &mut self,
        generator: ObjectId,
        result: Result<JsValue, JsValue>,
        done: bool,
    ) -> Res<()> {
        let Some(request) = self
            .async_gen(generator)
            .and_then(|data| data.queue.pop_front())
        else {
            return Ok(());
        };
        self.pin(JsValue::Object(request.promise));
        match result {
            Ok(value) => {
                let result = self.iter_result(value, done)?;
                self.resolve_promise(request.promise, result)
            }
            Err(error) => self.settle(request.promise, PromiseState::Rejected(error)),
        }
    }
    /** AsyncGeneratorDrainQueue (with AsyncGeneratorAwaitReturn): settle requests made after the
    body completed; a return request awaits its value first. */
    fn drain_async_gen(&mut self, generator: ObjectId) -> Res<()> {
        loop {
            let Some(request) = self
                .async_gen(generator)
                .and_then(|data| data.queue.front().cloned())
            else {
                return Ok(());
            };
            match mode_of(request.mode) {
                Resume::Return => {
                    self.set_async_gen_state(generator, GeneratorState::AwaitingReturn);
                    match self.promise_resolve(request.value) {
                        Ok(promise) => return self.await_promise(promise, generator),
                        Err(Thrown::Value(error)) => {
                            self.set_async_gen_state(generator, GeneratorState::Done);
                            self.complete_async_step(generator, Err(error), true)?;
                        }
                        Err(fatal) => return Err(fatal),
                    }
                }
                Resume::Throw => self.complete_async_step(generator, Err(request.value), true)?,
                Resume::Next => {
                    self.complete_async_step(generator, Ok(JsValue::Undefined), true)?
                }
            }
        }
    }
    /** GetMethod: undefined for undefined or null, a TypeError for anything not callable. */
    pub fn get_method(&mut self, value: &JsValue, key: &Key) -> Res<Option<JsValue>> {
        let method = self.get(value, key)?;
        if method.nullish() {
            return Ok(None);
        }
        if !self.is_callable(&method) {
            return Err(self.type_error("Iterator method is not callable"));
        }
        Ok(Some(method))
    }
}
