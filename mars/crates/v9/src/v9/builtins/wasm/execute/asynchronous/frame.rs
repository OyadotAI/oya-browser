use super::program::{Code, Program};
/** A resumable frame stores immutable module ownership and the exact next instruction without replaying callbacks. */
use super::*;
/** Control records contain only source-derived scope identities and checked operand heights. */
pub(super) struct Control {
    pub scope: usize,
    pub height: usize,
    pub caught: Option<JsValue>,
}
/** Explicit transfers let a pending imported Promise unwind Rust while retaining the real Wasm state. */
pub(super) enum Step {
    Next,
    Call(ObjectId, Arc<Module>, u32, Vec<JsValue>),
    Tail(ObjectId, Arc<Module>, u32, Vec<JsValue>),
    Done(Vec<JsValue>),
    Pending(ObjectId),
    Effect(ObjectId, Vec<JsValue>),
    Switch(ObjectId, JsValue, Vec<JsValue>, u32),
    ResumeSaved(ObjectId, JsValue, Vec<JsValue>),
    ResumeException(ObjectId, JsValue, JsValue),
}
pub(super) struct Frame {
    /// Active native trace identity is rebuilt from these real frames after each resumption, never serialized.
    pub trace: Option<usize>,
    pub instance: ObjectId,
    pub module: Arc<Module>,
    pub index: u32,
    pub pc: usize,
    pub program: Arc<Program>,
    pub controls: Vec<Control>,
    pub locals: Vec<JsValue>,
    pub stack: Vec<JsValue>,
    pub roots: super::super::super::roots::Roots,
}
impl Frame {
    pub fn new(
        vm: &mut Vm,
        instance: ObjectId,
        module: Arc<Module>,
        index: u32,
        args: Vec<JsValue>,
    ) -> Res<Self> {
        vm.charge(1)?;
        let sig = module
            .signature(index)
            .map_err(|e| compile_failure(vm, e))?;
        if args.len() != sig.params.len() {
            return Err(vm.type_error("invalid Wasm continuation arguments"));
        }
        let mut locals = args;
        if index as usize >= module.imported_functions() {
            locals.extend(
                module.bodies[index as usize - module.imported_functions()]
                    .locals
                    .iter()
                    .map(|t| global::default(*t)),
            );
        }
        let program = if (index as usize) < module.imported_functions() {
            Program::compile(&[])?
        } else {
            Program::compile(&module.bodies[index as usize - module.imported_functions()].code)?
        };
        Ok(Self {
            trace: None,
            program: Arc::new(program),
            controls: vec![],
            instance,
            module,
            index,
            pc: 0,
            locals,
            stack: vec![],
            roots: Default::default(),
        })
    }
    /** Local functions, unlike imported JS entry records, have genuine running Wasm code. */
    pub fn enter_trace(&mut self, vm: &mut Vm) {
        if self.index as usize >= self.module.imported_functions() {
            let position = self
                .program
                .ops
                .get(self.pc.saturating_sub(1))
                .map_or(0, |op| op.position);
            self.trace = Some(vm.begin_wasm_activation(self.instance, self.index, position));
        }
    }
    /** A completed or tail-replaced frame relinquishes only its own transient scope and precise roots. */
    pub fn release(&mut self, vm: &mut Vm) {
        self.roots.release(vm);
        if let Some(trace) = self.trace.take() {
            vm.end_native_activation(trace);
        }
    }
    /** Imported calls suspend only for genuine Promise values returned through a Suspending wrapper. */
    fn import(&mut self, vm: &mut Vm, promising: bool) -> Res<Step> {
        if self.pc == 1 {
            return Ok(Step::Done(std::mem::take(&mut self.stack)));
        }
        let function = instance::item(vm, self.instance, "wasm.imports", self.index)?;
        if instance::is_function(vm, &function) {
            let (instance, module, index) = instance::target(vm, &function)?;
            return Ok(Step::Tail(
                instance,
                Arc::new(module),
                index,
                self.locals.clone(),
            ));
        }
        let sig = self
            .module
            .signature(self.index)
            .map_err(|e| compile_failure(vm, e))?;
        if sig
            .params
            .iter()
            .chain(&sig.results)
            .any(|t| !gc_types::js_compatible(&self.module, *t))
        {
            return Err(vm.type_error(
                "type incompatibility when transforming from/to JS: unsupported value type",
            ));
        }
        let suspending = jspi::is_suspending(vm, &function);
        let previous = get(vm, self.instance, jspi::CONTEXT).unwrap_or(JsValue::Undefined);
        if suspending && !promising {
            let message = if matches!(previous, JsValue::Number(_)) {
                "trying to suspend JS frames"
            } else {
                "trying to suspend without WebAssembly.promising"
            };
            return Err(failure(vm, ids(vm).suspend_error, message));
        }
        set(
            vm,
            self.instance,
            jspi::CONTEXT,
            JsValue::Number(vm.frames.len() as f64),
        );
        let result = if suspending {
            jspi::callback(vm, &function)
                .and_then(|callback| vm.call(callback, JsValue::Undefined, self.locals.clone()))
        } else {
            vm.call(function, JsValue::Undefined, self.locals.clone())
        };
        set(vm, self.instance, jspi::CONTEXT, previous);
        let value = result?;
        vm.pin(value.clone());
        self.pc = 1;
        if suspending
            && let Some(promise) = value.object()
            && matches!(
                vm.heap.get(promise).map(|o| &o.class),
                Some(Class::Promise(_))
            )
        {
            return Ok(Step::Pending(promise));
        }
        Ok(Step::Done(self.convert_return(vm, value)?))
    }
    /** Multi-value coercion observes the actual returned iterable exactly once after settlement. */
    pub fn convert_return(&self, vm: &mut Vm, value: JsValue) -> Res<Vec<JsValue>> {
        let sig = self
            .module
            .signature(self.index)
            .map_err(|e| compile_failure(vm, e))?;
        let values = match sig.results.len() {
            0 => vec![],
            1 => vec![value],
            _ => vm.iterate_to_vec(&value)?,
        };
        if values.len() != sig.results.len() {
            return Err(vm.type_error("multi-return length mismatch"));
        }
        sig.results
            .iter()
            .zip(values)
            .map(|(ty, value)| {
                let value = global::convert(vm, *ty, &value)?;
                if ty.reference().is_some() && !gc::matches(vm, &self.module, *ty, &value)? {
                    return Err(
                        vm.type_error("WebAssembly imported function reference type mismatch")
                    );
                }
                Ok(value)
            })
            .collect()
    }
    /** Roots remain traced while an explicit control scope or an awaited child is active. */
    pub fn step(&mut self, vm: &mut Vm, promising: bool) -> Res<Step> {
        self.roots.sync(
            vm,
            self.locals
                .iter()
                .chain(&self.stack)
                .chain(
                    self.controls
                        .iter()
                        .filter_map(|control| control.caught.as_ref()),
                )
                .chain(std::iter::once(&JsValue::Object(self.instance))),
        )?;
        if (self.index as usize) < self.module.imported_functions() {
            return self.import(vm, promising);
        }
        if self.pc >= self.program.ops.len() {
            let count = self
                .module
                .signature(self.index)
                .map_err(|e| compile_failure(vm, e))?
                .results
                .len();
            if self.stack.len() < count {
                return Err(vm.type_error("invalid Wasm continuation result stack"));
            }
            return Ok(Step::Done(self.stack.split_off(self.stack.len() - count)));
        }
        let op = self.program.ops[self.pc].clone();
        if let Some(trace) = self.trace {
            vm.set_wasm_position(trace, op.position)?;
        }
        self.pc += 1;
        if self
            .controls
            .iter()
            .map(|c| c.scope)
            .ne(op.scopes.iter().copied())
        {
            return Err(vm.type_error("invalid Wasm continuation control scope"));
        }
        let module = self.module.clone();
        let results = module
            .signature(self.index)
            .map_err(|e| compile_failure(vm, e))?
            .results
            .clone();
        let returns = results.len();
        let mut labels = vec![results];
        labels.extend(self.controls.iter().map(|control| {
            let scope = &self.program.scopes[control.scope];
            if scope.kind == 3 {
                scope.signature.params.clone()
            } else {
                scope.signature.results.clone()
            }
        }));
        let mut activation = Activation {
            trace: self.trace,
            index: self.index,
            instance: self.instance,
            module: &module,
            locals: std::mem::take(&mut self.locals),
            stack: std::mem::take(&mut self.stack),
            labels,
            caught: self
                .controls
                .iter()
                .enumerate()
                .filter_map(|(index, control)| {
                    control.caught.clone().map(|value| (index + 1, value))
                })
                .collect(),
            returns,
            roots: std::mem::take(&mut self.roots),
        };
        let result = self
            .operation(vm, &module, &mut activation, &op.code)
            .map_err(|error| trace(vm, error, self.index, op.position));
        self.locals = activation.locals;
        self.stack = activation.stack;
        self.roots = activation.roots;
        result
    }
    /** Enter/end steps preserve each block's lower operands and avoid a native Rust control stack. */
    fn operation(
        &mut self,
        vm: &mut Vm,
        module: &Arc<Module>,
        a: &mut Activation,
        code: &Code,
    ) -> Res<Step> {
        match code {
            Code::Enter(id) => {
                vm.charge(2)?;
                let scope = &self.program.scopes[*id];
                let condition = if scope.kind == 4 {
                    numeric::i32_value(&numeric::pop(&mut a.stack)?)? != 0
                } else {
                    true
                };
                let height = a
                    .stack
                    .len()
                    .checked_sub(scope.signature.params.len())
                    .ok_or_else(|| vm.type_error("invalid Wasm block parameter stack"))?;
                self.controls.push(Control {
                    scope: *id,
                    height,
                    caught: None,
                });
                if !condition {
                    self.pc = scope.otherwise;
                }
                Ok(Step::Next)
            }
            Code::Else(id) => {
                self.pc = self.program.scopes[*id].end;
                Ok(Step::Next)
            }
            Code::End(id) => {
                let scope = &self.program.scopes[*id];
                let values = a.take(scope.signature.results.len())?;
                let control = self
                    .controls
                    .pop()
                    .ok_or_else(|| vm.type_error("missing Wasm control scope"))?;
                a.stack.truncate(control.height);
                a.stack.extend(values);
                Ok(Step::Next)
            }
            Code::Plain(instruction) => self.plain(vm, module, a, instruction),
        }
    }
    /** Direct, reference and table calls enter the same continuation-capable owned frame stack. */
    fn plain(
        &mut self,
        vm: &mut Vm,
        module: &Arc<Module>,
        a: &mut Activation,
        instruction: &Instruction,
    ) -> Res<Step> {
        let mut inner = instruction;
        while let Instruction::Located(_, next) = inner {
            inner = next;
        }
        match inner {
            Instruction::Resume(index, _) => {
                let value = numeric::pop(&mut a.stack)?;
                let id = value
                    .object()
                    .ok_or_else(|| trap(vm, "null continuation reference"))?;
                if get(vm, id, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
                    return Err(trap(vm, "resuming an invalid continuation"));
                }
                let function = get(vm, id, "wasm.cont.function")
                    .ok_or_else(|| trap(vm, "invalid continuation"))?;
                let saved = get(vm, id, "wasm.cont.saved").filter(|v| *v != JsValue::Undefined);
                let ty = match module.definitions[*index as usize].kind {
                    gc_types::Kind::Continuation(ty) => ty,
                    _ => unreachable!(),
                };
                let mut args = Activation::bound_arguments(vm, id);
                args.extend(a.take(module.types[ty as usize].params.len())?);
                set(vm, id, "wasm.cont.consumed", JsValue::Boolean(true));
                if let Some(saved) = saved {
                    return Ok(Step::ResumeSaved(id, saved, args));
                }
                let (instance, module, index) = instance::target(vm, &function)?;
                Ok(Step::Call(instance, Arc::new(module), index, args))
            }
            Instruction::ResumeThrow(_, tag, _) => {
                let value = numeric::pop(&mut a.stack)?;
                let id = value
                    .object()
                    .ok_or_else(|| trap(vm, "null continuation reference"))?;
                if get(vm, id, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
                    return Err(trap(vm, "resuming an invalid continuation"));
                }
                let thrown = if let Some(tag) = tag {
                    let signature = exceptions::exception_signature(module, *tag)
                        .map_err(|e| compile_failure(vm, e))?;
                    let payload = a.take(signature.params.len())?;
                    let tag = instance::item(vm, self.instance, exceptions::TAGS, *tag)?;
                    exceptions::throw(vm, tag, payload)?
                } else {
                    let reference = numeric::pop(&mut a.stack)?;
                    if reference == JsValue::Null {
                        return Err(trap(vm, "rethrowing null value"));
                    }
                    exceptions::dereference(vm, &reference)?
                };
                set(vm, id, "wasm.cont.consumed", JsValue::Boolean(true));
                if let Some(saved) =
                    get(vm, id, "wasm.cont.saved").filter(|v| *v != JsValue::Undefined)
                {
                    return Ok(Step::ResumeException(id, saved, thrown));
                }
                Err(Thrown::Value(thrown))
            }
            Instruction::Suspend(index) => {
                vm.charge(2)?;
                let signature =
                    exceptions::signature(module, *index).map_err(|e| compile_failure(vm, e))?;
                let tag = instance::item(vm, self.instance, exceptions::TAGS, *index)?
                    .object()
                    .ok_or_else(|| vm.type_error("invalid effect tag"))?;
                Ok(Step::Effect(tag, a.take(signature.params.len())?))
            }
            Instruction::Switch(index, tag) => {
                vm.charge(2)?;
                let target = numeric::pop(&mut a.stack)?;
                let function = match module.definitions[*index as usize].kind {
                    gc_types::Kind::Continuation(function) => function,
                    _ => unreachable!(),
                };
                let params = &module.types[function as usize].params;
                let return_type = params
                    .last()
                    .and_then(|ty| ty.reference())
                    .map(|(heap, _)| heap as u32)
                    .ok_or_else(|| vm.type_error("invalid switch continuation type"))?;
                let tag = instance::item(vm, self.instance, exceptions::TAGS, *tag)?
                    .object()
                    .ok_or_else(|| vm.type_error("invalid switch tag"))?;
                Ok(Step::Switch(
                    tag,
                    target,
                    a.take(params.len() - 1)?,
                    return_type,
                ))
            }
            Instruction::Call(index) => {
                vm.charge(2)?;
                let count = module
                    .signature(*index)
                    .map_err(|e| compile_failure(vm, e))?
                    .params
                    .len();
                Ok(Step::Call(
                    self.instance,
                    module.clone(),
                    *index,
                    a.take(count)?,
                ))
            }
            Instruction::CallRef(ty) | Instruction::Indirect(ty, _) => {
                vm.charge(2)?;
                let function = if let Instruction::Indirect(_, table) = inner {
                    a.indirect_target(vm, *ty, *table)?
                } else {
                    let function = numeric::pop(&mut a.stack)?;
                    if function == JsValue::Null {
                        return Err(trap(vm, "dereferencing a null pointer"));
                    }
                    if !instance::function_matches(vm, module, *ty, &function)? {
                        return Err(trap(vm, "function signature mismatch"));
                    }
                    function
                };
                let args = a.take(module.types[*ty as usize].params.len())?;
                let (instance, module, index) = instance::target(vm, &function)?;
                Ok(Step::Call(instance, Arc::new(module), index, args))
            }
            _ => match a.sequence(vm, std::slice::from_ref(instruction))? {
                Flow::Next => Ok(Step::Next),
                Flow::Return(values) => Ok(Step::Done(values)),
                Flow::Branch(depth, values) => self.branch(vm, a, depth, values),
                Flow::TailLocal(index, args) => {
                    Ok(Step::Tail(self.instance, module.clone(), index, args))
                }
                Flow::TailReference(function, args) => {
                    let (instance, module, index) = instance::target(vm, &function)?;
                    Ok(Step::Tail(instance, Arc::new(module), index, args))
                }
                Flow::Delegated(value, _) => Err(Thrown::Value(value)),
            },
        }
    }
    /** A suspended caller retains the handlers attached to its current resume instruction. */
    pub fn resume_handlers(&self) -> Option<Vec<types::EffectHandler>> {
        let op = self
            .pc
            .checked_sub(1)
            .and_then(|pc| self.program.ops.get(pc))?;
        let Code::Plain(instruction) = &op.code else {
            return None;
        };
        let mut instruction = instruction;
        while let Instruction::Located(_, next) = instruction {
            instruction = next;
        }
        match instruction {
            Instruction::Resume(_, handlers) | Instruction::ResumeThrow(_, _, handlers) => {
                Some(handlers.clone())
            }
            _ => None,
        }
    }
    /** A handler branches with its exact typed continuation into an outer structured label. */
    pub fn effect_target(&self, depth: u32) -> Option<u32> {
        let depth = depth as usize;
        let ty = if depth == self.controls.len() {
            self.module.signature(self.index).ok()?.results.last()?
        } else {
            let index = self.controls.len().checked_sub(depth + 1)?;
            let scope = &self.program.scopes[self.controls[index].scope];
            if scope.kind == 3 {
                scope.signature.params.last()?
            } else {
                scope.signature.results.last()?
            }
        };
        let (heap, _) = ty.reference()?;
        (heap >= 0).then_some(heap as u32)
    }
    /** Complete a matched effect branch without retaining exited control scopes. */
    pub fn effect_branch(&mut self, vm: &mut Vm, depth: u32, values: Vec<JsValue>) -> Res<Step> {
        let depth = depth as usize;
        if depth == self.controls.len() {
            return Ok(Step::Done(values));
        }
        let target = self
            .controls
            .len()
            .checked_sub(depth + 1)
            .ok_or_else(|| vm.type_error("invalid effect handler depth"))?;
        let scope = &self.program.scopes[self.controls[target].scope];
        self.stack.truncate(self.controls[target].height);
        self.stack.extend(values);
        if scope.kind == 3 {
            self.pc = scope.body;
            self.controls.truncate(target + 1);
        } else {
            self.pc = scope.end + 1;
            self.controls.truncate(target);
        }
        Ok(Step::Next)
    }
    /** Route a catchable exception through the nearest active try_table without replaying frames. */
    pub fn handle_exception(&mut self, vm: &mut Vm, value: &JsValue) -> Res<Option<Step>> {
        if !exceptions::catchable(vm, value) {
            return Ok(None);
        }
        let mut index = self.controls.len();
        while index > 0 {
            index -= 1;
            let scope = &self.program.scopes[self.controls[index].scope];
            if let Some(depth) = scope.delegate {
                index = index.saturating_sub(depth as usize);
                continue;
            }
            if self.controls[index].caught.is_none() {
                for (tag, start) in &scope.legacy {
                    let payload = match tag {
                        Some(tag) => {
                            let tag = instance::item(vm, self.instance, exceptions::TAGS, *tag)?;
                            exceptions::payload(vm, value, &tag)?
                        }
                        None => Some(vec![]),
                    };
                    if let Some(payload) = payload {
                        let start = *start;
                        let height = self.controls[index].height;
                        self.controls.truncate(index + 1);
                        self.controls[index].caught = Some(value.clone());
                        self.stack.truncate(height);
                        self.stack.extend(payload);
                        self.pc = start;
                        return Ok(Some(Step::Next));
                    }
                }
            }
            for arm in &scope.handlers {
                let payload = match arm.tag {
                    Some(tag) => {
                        let tag = instance::item(vm, self.instance, exceptions::TAGS, tag)?;
                        exceptions::payload(vm, value, &tag)?
                    }
                    None => Some(vec![]),
                };
                let Some(mut payload) = payload else {
                    continue;
                };
                if arm.reference {
                    payload.push(exceptions::reference(vm, value)?);
                }
                let depth = arm.depth as usize;
                let height = self.controls[index].height;
                self.controls.truncate(index);
                self.stack.truncate(height);
                if depth == self.controls.len() {
                    return Ok(Some(Step::Done(payload)));
                }
                let target = self
                    .controls
                    .len()
                    .checked_sub(depth + 1)
                    .ok_or_else(|| vm.type_error("invalid exception handler depth"))?;
                let target_scope = &self.program.scopes[self.controls[target].scope];
                self.stack.truncate(self.controls[target].height);
                self.stack.extend(payload);
                if target_scope.kind == 3 {
                    self.pc = target_scope.body;
                    self.controls.truncate(target + 1);
                } else {
                    self.pc = target_scope.end + 1;
                    self.controls.truncate(target);
                }
                return Ok(Some(Step::Next));
            }
        }
        Ok(None)
    }
    /** Loop branches retain their target scope; other branches finish it with only its declared values. */
    fn branch(
        &mut self,
        vm: &mut Vm,
        a: &mut Activation,
        depth: u32,
        values: Vec<JsValue>,
    ) -> Res<Step> {
        if depth as usize == self.controls.len() {
            return Ok(Step::Done(values));
        }
        let index = self
            .controls
            .len()
            .checked_sub(depth as usize + 1)
            .ok_or_else(|| vm.type_error("invalid Wasm continuation branch"))?;
        let control = &self.controls[index];
        let scope = &self.program.scopes[control.scope];
        a.stack.truncate(control.height);
        a.stack.extend(values);
        if scope.kind == 3 {
            self.pc = scope.body;
            self.controls.truncate(index + 1);
        } else {
            self.pc = scope.end + 1;
            self.controls.truncate(index);
        }
        Ok(Step::Next)
    }
}
