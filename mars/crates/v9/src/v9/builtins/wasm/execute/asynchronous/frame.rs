use super::program::{Code, Program};
/** A resumable frame stores immutable module ownership and the exact next instruction without replaying callbacks. */
use super::*;
/** Control records contain only source-derived scope identities and checked operand heights. */
pub(super) struct Control {
    pub scope: usize,
    pub height: usize,
}
/** Explicit transfers let a pending imported Promise unwind Rust while retaining the real Wasm state. */
pub(super) enum Step {
    Next,
    Call(ObjectId, Arc<Module>, u32, Vec<JsValue>),
    Tail(ObjectId, Arc<Module>, u32, Vec<JsValue>),
    Done(Vec<JsValue>),
    Pending(ObjectId),
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
    fn import(&mut self, vm: &mut Vm) -> Res<Step> {
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
            .any(|t| !t.js_compatible())
        {
            return Err(vm.type_error(
                "type incompatibility when transforming from/to JS: unsupported value type",
            ));
        }
        let suspending = jspi::is_suspending(vm, &function);
        let previous = get(vm, self.instance, jspi::CONTEXT).unwrap_or(JsValue::Undefined);
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
            return Err(
                vm.type_error("WebAssembly imported function returned the wrong number of values")
            );
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
    pub fn step(&mut self, vm: &mut Vm) -> Res<Step> {
        self.roots.sync(
            vm,
            self.locals
                .iter()
                .chain(&self.stack)
                .chain(std::iter::once(&JsValue::Object(self.instance))),
        )?;
        if (self.index as usize) < self.module.imported_functions() {
            return self.import(vm);
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
        let returns = module
            .signature(self.index)
            .map_err(|e| compile_failure(vm, e))?
            .results
            .len();
        let mut labels = vec![returns];
        labels.extend(self.controls.iter().map(|control| {
            let scope = &self.program.scopes[control.scope];
            if scope.kind == 3 {
                scope.signature.params.len()
            } else {
                scope.signature.results.len()
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
            caught: vec![],
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
                self.controls.push(Control { scope: *id, height });
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
                        return Err(trap(vm, "null function"));
                    }
                    if !instance::function_matches(vm, module, *ty, &function)? {
                        return Err(trap(vm, "indirect call signature mismatch"));
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
