/** A fuel-charged scalar Wasm interpreter; branches carry values across explicit control frames. */
use super::*;
use types::{Instruction, Signature};
mod asynchronous;
pub(super) use asynchronous::{resume as resume_promising, start as start_promising};

/** Transfers carry only the destination label's typed values, never a native program counter. */
enum Flow {
    Next,
    Branch(u32, Vec<JsValue>),
    Return(Vec<JsValue>),
    Delegated(JsValue, u32),
    TailLocal(u32, Vec<JsValue>),
    TailReference(JsValue, Vec<JsValue>),
}

/** A trap's Wasm frame records the actual binary opcode offset, never a fabricated JS location. */
fn trace(vm: &mut Vm, error: Thrown, index: u32, position: usize) -> Thrown {
    if let Thrown::Value(JsValue::Object(id)) = &error
        && matches!(vm.heap.get(*id).map(|o| &o.class), Some(Class::Error))
    {
        let name = Key::private(crate::v9::vm::error::TRACE_KEY);
        let prior = vm
            .heap
            .get(*id)
            .and_then(|o| o.props.get(&name))
            .and_then(|p| match &p.slot {
                Slot::Data(JsValue::String(s)) => Some(s.lossy()),
                _ => None,
            })
            .unwrap_or_default();
        let frame = format!("wasm-function[{index}]:0x{position:x}");
        let text = if prior.is_empty() {
            frame
        } else {
            format!("{prior}\n{frame}")
        };
        if text.encode_utf16().count() <= crate::constants::JS_STRING_UNITS
            && let Ok(value) = JsValue::string(&text)
        {
            vm.heap.put(
                *id,
                name,
                Prop {
                    slot: Slot::Data(value),
                    attrs: HIDDEN_ATTRS,
                },
            );
        }
    }
    error
}
/** A function activation owns its locals and operand stack for exactly one invocation. */
struct Activation<'a> {
    trace: Option<usize>,
    index: u32,
    instance: ObjectId,
    module: &'a Module,
    locals: Vec<JsValue>,
    stack: Vec<JsValue>,
    labels: Vec<Vec<Type>>,
    caught: Vec<(usize, JsValue)>,
    returns: usize,
    roots: super::roots::Roots,
}

/** Every Wasm-to-Wasm call uses V9's existing native recursion, operation and wall-time bounds. */
pub(super) fn call(
    vm: &mut Vm,
    instance: ObjectId,
    module: &Module,
    index: u32,
    args: Vec<JsValue>,
) -> Res<Vec<JsValue>> {
    let mut entered = false;
    let result = vm.deeper(|vm| {
        entered = true;
        invoke(vm, instance, module, index, args)
    });
    if !entered && let Err(Thrown::Value(JsValue::Object(id))) = &result {
        set(vm, *id, "wasm.trap", JsValue::Boolean(true));
    }
    result
}
/** Imported JS calls remain real callbacks and can throw arbitrary values into Wasm callers. */
fn invoke(
    vm: &mut Vm,
    mut instance: ObjectId,
    module: &Module,
    mut index: u32,
    mut args: Vec<JsValue>,
) -> Res<Vec<JsValue>> {
    let mut owned = None;
    loop {
        let current = owned.as_ref().unwrap_or(module);
        match activation(vm, instance, current, index, args)? {
            Flow::TailLocal(next, values) => {
                index = next;
                args = values
            }
            Flow::TailReference(function, values) => {
                let (next, m, target) = instance::target(vm, &function)?;
                instance = next;
                owned = Some(m);
                index = target;
                args = values
            }
            Flow::Return(values) => return Ok(values),
            Flow::Delegated(value, _) => return Err(Thrown::Value(value)),
            _ => return Err(vm.type_error("invalid WebAssembly branch destination")),
        }
    }
}
/** An activation can replace itself without retaining a host stack frame. */
fn activation(
    vm: &mut Vm,
    instance: ObjectId,
    module: &Module,
    index: u32,
    args: Vec<JsValue>,
) -> Res<Flow> {
    vm.charge(1)?;
    let signature = module
        .signature(index)
        .map_err(|e| compile_failure(vm, e))?;
    if (index as usize) < module.imported_functions() {
        let function = instance::item(vm, instance, "wasm.imports", index)?;
        if instance::is_function(vm, &function) {
            return Ok(Flow::TailReference(function, args));
        }
        if signature
            .params
            .iter()
            .chain(&signature.results)
            .any(|t| !gc_types::js_compatible(module, *t))
        {
            return Err(vm.type_error(
                "type incompatibility when transforming from/to JS: unsupported value type",
            ));
        }
        let value = if jspi::is_suspending(vm, &function) {
            jspi::call(vm, instance, &function, args)?
        } else {
            vm.call(function, JsValue::Undefined, args)?
        };
        vm.pin(value.clone());
        let values = if signature.results.len() > 1 {
            vm.iterate_to_vec(&value)?
        } else if signature.results.is_empty() {
            vec![]
        } else {
            vec![value]
        };
        if values.len() != signature.results.len() {
            return Err(vm.type_error("multi-return length mismatch"));
        }
        return signature
            .results
            .iter()
            .zip(values)
            .map(|(ty, value)| global::convert(vm, *ty, &value))
            .collect::<Res<Vec<_>>>()
            .map(Flow::Return);
    }
    let body = &module.bodies[index as usize - module.imported_functions()];
    let mut locals = args;
    locals.extend(body.locals.iter().map(|t| global::default(*t)));
    let position = match body.code.first() {
        Some(Instruction::Located(position, _)) => *position,
        _ => 0,
    };
    let trace = vm.begin_wasm_activation(instance, index, position);
    let mut activation = Activation {
        trace: Some(trace),
        index,
        instance,
        module,
        locals,
        stack: Vec::new(),
        labels: vec![signature.results.clone()],
        caught: vec![],
        returns: signature.results.len(),
        roots: super::roots::Roots::default(),
    };
    let result = activation.sequence(vm, &body.code);
    activation.roots.release(vm);
    vm.end_native_activation(trace);
    match result? {
        Flow::Next => Ok(Flow::Return(activation.take(signature.results.len())?)),
        Flow::Branch(0, values) => Ok(Flow::Return(values)),
        flow => Ok(flow),
    }
}

impl Activation<'_> {
    /** Resume uses explicit frames so a handled effect can retain the real execution state. */
    fn resume_effect(
        &mut self,
        vm: &mut Vm,
        index: u32,
        handlers: &[types::EffectHandler],
        throw_tag: Option<Option<u32>>,
    ) -> Res<Flow> {
        let value = numeric::pop(&mut self.stack)?;
        if value == JsValue::Null {
            return Err(trap(vm, "null continuation reference"));
        }
        let object = value
            .object()
            .ok_or_else(|| vm.type_error("invalid continuation"))?;
        if get(vm, object, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
            return Err(trap(vm, "resuming an invalid continuation"));
        }
        let function = get(vm, object, "wasm.cont.function")
            .ok_or_else(|| trap(vm, "invalid continuation"))?;
        let saved = get(vm, object, "wasm.cont.saved");
        let target = match self.module.definitions[index as usize].kind {
            gc_types::Kind::Continuation(target) => target,
            _ => unreachable!(),
        };
        let (args, injection) = match throw_tag {
            Some(Some(tag)) => {
                let count = exceptions::exception_signature(self.module, tag)
                    .map_err(|e| compile_failure(vm, e))?
                    .params
                    .len();
                let args = self.take(count)?;
                let tag = instance::item(vm, self.instance, exceptions::TAGS, tag)?;
                (Vec::new(), Some(exceptions::throw(vm, tag, args)?))
            }
            Some(None) => {
                let value = numeric::pop(&mut self.stack)?;
                if value == JsValue::Null {
                    return Err(trap(vm, "rethrowing null value"));
                }
                (Vec::new(), Some(exceptions::dereference(vm, &value)?))
            }
            None => {
                let mut args = Self::bound_arguments(vm, object);
                args.extend(self.take(self.module.types[target as usize].params.len())?);
                (args, None)
            }
        };
        set(vm, object, "wasm.cont.consumed", JsValue::Boolean(true));
        let switch_tags = handlers
            .iter()
            .filter(|h| h.switch)
            .map(|h| {
                instance::item(vm, self.instance, exceptions::TAGS, h.tag).and_then(|v| {
                    v.object()
                        .ok_or_else(|| vm.type_error("invalid switch tag"))
                })
            })
            .collect::<Res<Vec<_>>>()?;
        match vm.deeper(|vm| {
            asynchronous::run_effect(vm, &function, saved, args, &switch_tags, injection)
        })? {
            asynchronous::EffectResult::Done(values) => {
                self.stack.extend(values);
                Ok(Flow::Next)
            }
            asynchronous::EffectResult::Suspended(tag, mut values, saved) => {
                let handler = handlers
                    .iter()
                    .find(|h| {
                        !h.switch
                            && instance::item(vm, self.instance, exceptions::TAGS, h.tag)
                                .ok()
                                .and_then(|v| v.object())
                                == Some(tag)
                    })
                    .ok_or_else(|| trap(vm, "WasmFX: unhandled suspend"))?;
                let ty = self
                    .labels
                    .iter()
                    .rev()
                    .nth(handler.depth as usize)
                    .and_then(|label| label.last())
                    .and_then(|ty| ty.reference())
                    .map(|(heap, _)| heap as u32)
                    .ok_or_else(|| vm.type_error("invalid effect handler type"))?;
                let module = get(vm, self.instance, INSTANCE)
                    .ok_or_else(|| vm.type_error("invalid continuation module"))?;
                let next = vm.new_object()?;
                set(vm, next, "wasm.cont.function", function);
                set(vm, next, "wasm.cont.saved", JsValue::Object(saved));
                set(vm, next, "wasm.gc.module", module);
                set(vm, next, "wasm.gc.type", JsValue::Number(ty as f64));
                values.push(JsValue::Object(next));
                Ok(Flow::Branch(handler.depth, values))
            }
        }
    }
    /** Bound arguments are stored in a traced array, including values retained across snapshots. */
    fn bound_arguments(vm: &Vm, object: ObjectId) -> Vec<JsValue> {
        get(vm, object, "wasm.cont.bound")
            .and_then(|v| v.object())
            .and_then(|id| vm.heap.get(id))
            .and_then(|o| match &o.class {
                Class::Array(values) => Some(values.iter().cloned().collect()),
                _ => None,
            })
            .unwrap_or_default()
    }
    /** Binding consumes the old continuation and creates a new one with the fixed argument prefix. */
    fn bind_continuation(&mut self, vm: &mut Vm, source: u32, target: u32) -> Res<()> {
        let value = numeric::pop(&mut self.stack)?;
        if value == JsValue::Null {
            return Err(trap(vm, "null continuation reference"));
        }
        let old = value
            .object()
            .ok_or_else(|| vm.type_error("invalid continuation"))?;
        if get(vm, old, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
            return Err(trap(vm, "resuming an invalid continuation"));
        }
        let function =
            get(vm, old, "wasm.cont.function").ok_or_else(|| trap(vm, "invalid continuation"))?;
        let saved = get(vm, old, "wasm.cont.saved");
        let module = get(vm, self.instance, INSTANCE)
            .ok_or_else(|| trap(vm, "invalid continuation module"))?;
        let get_sig = |index: u32| match self.module.definitions[index as usize].kind {
            gc_types::Kind::Continuation(i) => &self.module.types[i as usize],
            _ => unreachable!(),
        };
        let count = get_sig(source).params.len() - get_sig(target).params.len();
        let mut bound = Self::bound_arguments(vm, old);
        bound.extend(self.take(count)?);
        set(vm, old, "wasm.cont.consumed", JsValue::Boolean(true));
        let array = vm.new_array(bound)?;
        let next = vm.new_object()?;
        set(vm, next, "wasm.cont.function", function);
        set(vm, next, "wasm.cont.bound", JsValue::Object(array));
        if let Some(saved) = saved {
            set(vm, next, "wasm.cont.saved", saved);
        }
        set(vm, next, "wasm.gc.module", module);
        set(vm, next, "wasm.gc.type", JsValue::Number(target as f64));
        self.stack.push(JsValue::Object(next));
        Ok(())
    }
    /** A continuation retains its function and becomes unusable before its first resume. */
    fn continuation(&mut self, vm: &mut Vm, op: u8, index: u32) -> Res<()> {
        let value = numeric::pop(&mut self.stack)?;
        if value == JsValue::Null {
            return Err(trap(vm, "null continuation reference"));
        }
        if op == 0xe0 {
            let module = get(vm, self.instance, INSTANCE)
                .ok_or_else(|| vm.type_error("invalid continuation module"))?;
            let object = vm.new_object()?;
            set(vm, object, "wasm.cont.function", value);
            set(vm, object, "wasm.gc.module", module);
            set(vm, object, "wasm.gc.type", JsValue::Number(index as f64));
            self.stack.push(JsValue::Object(object));
        } else {
            let object = value
                .object()
                .ok_or_else(|| vm.type_error("invalid continuation"))?;
            if get(vm, object, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
                return Err(trap(vm, "resuming an invalid continuation"));
            }
            let function = get(vm, object, "wasm.cont.function")
                .ok_or_else(|| trap(vm, "invalid continuation"))?;
            set(vm, object, "wasm.cont.consumed", JsValue::Boolean(true));
            let target = match self.module.definitions[index as usize].kind {
                gc_types::Kind::Continuation(target) => target,
                _ => unreachable!(),
            };
            let mut args = Self::bound_arguments(vm, object);
            args.extend(self.take(self.module.types[target as usize].params.len())?);
            self.stack
                .extend(instance::call_typed(vm, &function, args)?);
        }
        Ok(())
    }
    /** Pop a vector in source order, checking the stack even after module validation. */
    fn take(&mut self, count: usize) -> Res<Vec<JsValue>> {
        if count > self.stack.len() {
            return Err(crate::protocol::EngineError::invalid("Invalid WebAssembly stack.").into());
        }
        Ok(self.stack.split_off(self.stack.len() - count))
    }
    /** Branch arity belongs to its destination label, including loop parameter labels. */
    fn branch(&mut self, depth: u32) -> Res<Flow> {
        let count = self
            .labels
            .iter()
            .rev()
            .nth(depth as usize)
            .ok_or_else(|| crate::protocol::EngineError::invalid("Invalid WebAssembly label."))?
            .len();
        Ok(Flow::Branch(depth, self.take(count)?))
    }
    /** Values under a block's parameters remain untouched across all its control paths. */
    fn block(
        &mut self,
        vm: &mut Vm,
        op: u8,
        signature: &Signature,
        body: &[Instruction],
        otherwise: &[Instruction],
    ) -> Res<Flow> {
        let condition = if op == 4 {
            numeric::i32_value(&numeric::pop(&mut self.stack)?)? != 0
        } else {
            true
        };
        let height = self
            .stack
            .len()
            .checked_sub(signature.params.len())
            .ok_or_else(|| {
                crate::protocol::EngineError::invalid("Invalid WebAssembly block stack.")
            })?;
        let code = if condition { body } else { otherwise };
        self.labels.push(if op == 3 {
            signature.params.clone()
        } else {
            signature.results.clone()
        });
        loop {
            if op == 3 {
                crate::v9::builtins::loop_stack_check(vm)?;
            }
            let flow = match self.sequence(vm, code) {
                Ok(flow) => flow,
                Err(error) => {
                    self.labels.pop();
                    self.stack.truncate(height);
                    return Err(error);
                }
            };
            match flow {
                Flow::Next => {
                    let values = self.take(signature.results.len())?;
                    self.stack.truncate(height);
                    self.stack.extend(values);
                    self.labels.pop();
                    return Ok(Flow::Next);
                }
                Flow::Branch(0, values) => {
                    self.stack.truncate(height);
                    self.stack.extend(values);
                    if op == 3 {
                        continue;
                    }
                    self.labels.pop();
                    return Ok(Flow::Next);
                }
                Flow::Branch(depth, values) => {
                    self.labels.pop();
                    self.stack.truncate(height);
                    return Ok(Flow::Branch(depth - 1, values));
                }
                Flow::Delegated(value, depth) => {
                    self.labels.pop();
                    self.stack.truncate(height);
                    return if depth == 0 {
                        Err(Thrown::Value(value))
                    } else {
                        Ok(Flow::Delegated(value, depth - 1))
                    };
                }
                flow @ (Flow::Return(_) | Flow::TailLocal(..) | Flow::TailReference(..)) => {
                    self.labels.pop();
                    return Ok(flow);
                }
            }
        }
    }
    /** Each instruction consumes fuel before it can execute or invoke JavaScript. */
    fn sequence(&mut self, vm: &mut Vm, code: &[Instruction]) -> Res<Flow> {
        for instruction in code {
            vm.charge(1)?;
            if vm.expired() {
                return Err(crate::protocol::EngineError::new(
                    "limit_exceeded",
                    "Script execution stopped: time limit exceeded.",
                    "Reduce the script work; what it did before it stopped stays, as in browsers.",
                )
                .into());
            }
            if !matches!(
                instruction,
                Instruction::Located(..)
                    | Instruction::Local(..)
                    | Instruction::Constant(..)
                    | Instruction::Simple(_)
            ) {
                self.roots.sync(
                    vm,
                    self.locals
                        .iter()
                        .chain(&self.stack)
                        .chain(self.caught.iter().map(|(_, value)| value)),
                )?;
            }
            let flow = match instruction {
                Instruction::Located(position, inner) => {
                    if let Some(trace) = self.trace {
                        vm.set_wasm_position(trace, *position)?;
                    }
                    self.sequence(vm, std::slice::from_ref(inner.as_ref()))
                        .map_err(|error| trace(vm, error, self.index, *position))?
                }
                Instruction::Constant(c) => {
                    self.stack
                        .push(instance::constant_value(vm, self.instance, c)?);
                    Flow::Next
                }
                Instruction::Local(op, index) => {
                    let index = *index as usize;
                    if *op == 0x20 {
                        self.stack.push(self.locals[index].clone())
                    } else {
                        let value = numeric::pop(&mut self.stack)?;
                        self.locals[index] = value.clone();
                        if *op == 0x22 {
                            self.stack.push(value)
                        }
                    }
                    Flow::Next
                }
                Instruction::Global(op, index) => {
                    let global = instance::item(vm, self.instance, GLOBALS, *index)?;
                    if *op == 0x23 {
                        self.stack.push(global::read(vm, &global)?)
                    } else {
                        global::write(vm, &global, &numeric::pop(&mut self.stack)?)?
                    }
                    Flow::Next
                }
                Instruction::Call(index) => {
                    let sig = self
                        .module
                        .signature(*index)
                        .map_err(|e| compile_failure(vm, e))?;
                    let args = self.take(sig.params.len())?;
                    let values = call(vm, self.instance, self.module, *index, args)?;
                    self.stack.extend(values);
                    Flow::Next
                }
                Instruction::TailCall(index) => {
                    let sig = self
                        .module
                        .signature(*index)
                        .map_err(|e| compile_failure(vm, e))?;
                    Flow::TailLocal(*index, self.take(sig.params.len())?)
                }
                Instruction::TailIndirect(ty, index) => {
                    let function = self.indirect_target(vm, *ty, *index)?;
                    let args = self.take(self.module.types[*ty as usize].params.len())?;
                    Flow::TailReference(function, args)
                }
                Instruction::TailRef(ty) => {
                    let function = numeric::pop(&mut self.stack)?;
                    if function == JsValue::Null {
                        return Err(trap(vm, "dereferencing a null pointer"));
                    }
                    if !instance::function_matches(vm, self.module, *ty, &function)? {
                        return Err(trap(vm, "function signature mismatch"));
                    }
                    let args = self.take(self.module.types[*ty as usize].params.len())?;
                    Flow::TailReference(function, args)
                }
                Instruction::CallRef(ty) => {
                    let function = numeric::pop(&mut self.stack)?;
                    if function == JsValue::Null {
                        return Err(trap(vm, "dereferencing a null pointer"));
                    }
                    if !instance::function_matches(vm, self.module, *ty, &function)? {
                        return Err(trap(vm, "function signature mismatch"));
                    }
                    let args = self.take(self.module.types[*ty as usize].params.len())?;
                    self.stack
                        .extend(instance::call_typed(vm, &function, args)?);
                    Flow::Next
                }
                Instruction::Indirect(ty, index) => {
                    self.indirect(vm, *ty, *index)?;
                    Flow::Next
                }
                Instruction::Branch(depth) => self.branch(*depth)?,
                Instruction::BranchNull(op, depth) => {
                    let value = numeric::pop(&mut self.stack)?;
                    if *op == 0xd5 {
                        if value == JsValue::Null {
                            self.branch(*depth)?
                        } else {
                            self.stack.push(value);
                            Flow::Next
                        }
                    } else if value != JsValue::Null {
                        self.stack.push(value);
                        self.branch(*depth)?
                    } else {
                        Flow::Next
                    }
                }
                Instruction::BranchDescriptor(op, depth, _, target) => {
                    let descriptor = numeric::pop(&mut self.stack)?;
                    let value = numeric::pop(&mut self.stack)?;
                    let matched =
                        gc::descriptor_match(vm, self.module, &value, &descriptor, *target)?;
                    self.stack.push(value);
                    if matched == (*op == 37) {
                        self.branch(*depth)?
                    } else {
                        Flow::Next
                    }
                }
                Instruction::BranchCast(op, depth, _, target) => {
                    let value = numeric::pop(&mut self.stack)?;
                    let matched = gc::matches(vm, self.module, *target, &value)?;
                    self.stack.push(value);
                    if matched == (*op == 24) {
                        self.branch(*depth)?
                    } else {
                        Flow::Next
                    }
                }
                Instruction::BranchIf(depth) => {
                    if numeric::i32_value(&numeric::pop(&mut self.stack)?)? != 0 {
                        self.branch(*depth)?
                    } else {
                        Flow::Next
                    }
                }
                Instruction::BranchTable(labels, default) => {
                    let index =
                        numeric::i32_value(&numeric::pop(&mut self.stack)?)? as u32 as usize;
                    self.branch(labels.get(index).copied().unwrap_or(*default))?
                }
                Instruction::Throw(index) => {
                    let tag = instance::item(vm, self.instance, exceptions::TAGS, *index)?;
                    let count = exceptions::signature(self.module, *index)
                        .map_err(|e| compile_failure(vm, e))?
                        .params
                        .len();
                    let payload = self.take(count)?;
                    return Err(Thrown::Value(exceptions::throw(vm, tag, payload)?));
                }
                Instruction::ThrowRef => {
                    let value = numeric::pop(&mut self.stack)?;
                    if value == JsValue::Null {
                        return Err(trap(vm, "rethrowing null value"));
                    }
                    return Err(Thrown::Value(exceptions::dereference(vm, &value)?));
                }
                Instruction::Rethrow(depth) => {
                    let target = self
                        .labels
                        .len()
                        .checked_sub(*depth as usize + 1)
                        .ok_or_else(|| vm.type_error("invalid rethrow depth"))?;
                    let value = self
                        .caught
                        .iter()
                        .rev()
                        .find(|(label, _)| *label == target)
                        .map(|(_, v)| v.clone())
                        .ok_or_else(|| vm.type_error("invalid rethrow target"))?;
                    return Err(Thrown::Value(value));
                }
                Instruction::Delegate(sig, body, depth) => {
                    match self.block(vm, 2, sig, body, &[]) {
                        Err(Thrown::Value(value)) if exceptions::catchable(vm, &value) => {
                            Flow::Delegated(value, *depth)
                        }
                        result => result?,
                    }
                }
                Instruction::Try(sig, body, handlers) => match self.block(vm, 2, sig, body, &[]) {
                    Err(Thrown::Value(value)) if exceptions::catchable(vm, &value) => {
                        let mut handler = None;
                        for (tag, code) in handlers {
                            let payload = match tag {
                                Some(index) => {
                                    let tag = instance::item(
                                        vm,
                                        self.instance,
                                        exceptions::TAGS,
                                        *index,
                                    )?;
                                    exceptions::payload(vm, &value, &tag)?
                                }
                                None => Some(vec![]),
                            };
                            if let Some(payload) = payload {
                                handler = Some((code, payload));
                                break;
                            }
                        }
                        let Some((code, payload)) = handler else {
                            return Err(Thrown::Value(value));
                        };
                        let params = vec![Type::Extern; payload.len()];
                        self.stack.extend(payload);
                        self.caught.push((self.labels.len(), value));
                        let result = self.block(
                            vm,
                            2,
                            &Signature {
                                params,
                                results: sig.results.clone(),
                            },
                            code,
                            &[],
                        );
                        self.caught.pop();
                        result?
                    }
                    result => result?,
                },
                Instruction::TryTable(sig, handlers, body) => {
                    match self.block(vm, 2, sig, body, &[]) {
                        Err(Thrown::Value(value)) if exceptions::catchable(vm, &value) => {
                            let mut handler = None;
                            for arm in handlers {
                                let payload = match arm.tag {
                                    Some(index) => {
                                        let tag = instance::item(
                                            vm,
                                            self.instance,
                                            exceptions::TAGS,
                                            index,
                                        )?;
                                        exceptions::payload(vm, &value, &tag)?
                                    }
                                    None => Some(vec![]),
                                };
                                if let Some(mut payload) = payload {
                                    if arm.reference {
                                        payload.push(exceptions::reference(vm, &value)?)
                                    }
                                    handler = Some((arm.depth, payload));
                                    break;
                                }
                            }
                            let Some((depth, payload)) = handler else {
                                return Err(Thrown::Value(value));
                            };
                            self.stack.extend(payload);
                            self.branch(depth)?
                        }
                        result => result?,
                    }
                }
                Instruction::Block(op, signature, body, otherwise) => {
                    self.block(vm, *op, signature, body, otherwise)?
                }
                Instruction::Continuation(op, index) => {
                    self.continuation(vm, *op, *index)?;
                    Flow::Next
                }
                Instruction::ContinuationBind(source, target) => {
                    self.bind_continuation(vm, *source, *target)?;
                    Flow::Next
                }
                Instruction::Resume(index, handlers) => {
                    self.resume_effect(vm, *index, handlers, None)?
                }
                Instruction::ResumeThrow(index, tag, handlers) => {
                    self.resume_effect(vm, *index, handlers, Some(*tag))?
                }
                Instruction::Suspend(_) => return Err(trap(vm, "WasmFX: unhandled suspend")),
                Instruction::Switch(_, _) => {
                    let target = numeric::pop(&mut self.stack)?;
                    let object = target
                        .object()
                        .ok_or_else(|| trap(vm, "null continuation reference"))?;
                    if get(vm, object, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
                        return Err(trap(vm, "resuming an invalid continuation"));
                    }
                    return Err(trap(vm, "WasmFX: unhandled suspend"));
                }
                Instruction::Memory(op, _, index, offset) => {
                    self.memory(vm, *op, *index, *offset)?;
                    Flow::Next
                }
                Instruction::MemoryIndex(op, index) => {
                    self.memory_index(vm, *op, *index)?;
                    Flow::Next
                }
                Instruction::Table(op, index) => {
                    let table = instance::item(vm, self.instance, TABLES, *index)?;
                    if *op == 0x25 {
                        let at = table::operand_index(vm, &table, &numeric::pop(&mut self.stack)?)?;
                        self.stack.push(
                            table::read(vm, &table, at)
                                .map_err(|_| trap(vm, "table index is out of bounds"))?,
                        )
                    } else {
                        let value = numeric::pop(&mut self.stack)?;
                        let at = table::operand_index(vm, &table, &numeric::pop(&mut self.stack)?)?;
                        table::write(vm, &table, at, &value)
                            .map_err(|_| trap(vm, "table index is out of bounds"))?
                    }
                    Flow::Next
                }
                Instruction::Select(_) => self.simple(vm, 0x1b)?,
                Instruction::GcAtomic(op, index, slot) => {
                    gc::atomic::run(vm, self.module, *op, *index, *slot, &mut self.stack)?;
                    Flow::Next
                }
                Instruction::Atomic(op, index, offset) => {
                    super::atomic::run(vm, self.instance, *op, *index, *offset, &mut self.stack)?;
                    Flow::Next
                }
                Instruction::Simd(op, args) => {
                    super::simd::run(vm, self.instance, *op, args, &mut self.stack)?;
                    Flow::Next
                }
                Instruction::Gc(op, args) => {
                    gc::run(vm, self.instance, self.module, *op, args, &mut self.stack)?;
                    Flow::Next
                }
                Instruction::Simple(op) => self.simple(vm, *op)?,
                Instruction::Bulk(sub, args) => {
                    self.bulk(vm, *sub, args)?;
                    Flow::Next
                }
            };
            if !matches!(
                instruction,
                Instruction::Located(..)
                    | Instruction::Local(..)
                    | Instruction::Constant(..)
                    | Instruction::Simple(_)
            ) {
                let transferred: &[JsValue] = match &flow {
                    Flow::Branch(_, values)
                    | Flow::Return(values)
                    | Flow::TailLocal(_, values)
                    | Flow::TailReference(_, values) => values,
                    _ => &[],
                };
                let reference = match &flow {
                    Flow::TailReference(value, _) | Flow::Delegated(value, _) => Some(value),
                    _ => None,
                };
                self.roots.sync(
                    vm,
                    self.locals
                        .iter()
                        .chain(&self.stack)
                        .chain(self.caught.iter().map(|(_, value)| value))
                        .chain(transferred)
                        .chain(reference),
                )?;
            }
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }
    /** Scalar stack operators and explicit traps do not depend on JS truthiness. */
    fn simple(&mut self, vm: &mut Vm, op: u8) -> Res<Flow> {
        match op {
            0 => return Err(trap(vm, "unreachable")),
            1 => {}
            0x0f => return Ok(Flow::Return(self.take(self.returns)?)),
            0x1a => {
                numeric::pop(&mut self.stack)?;
            }
            0x1b => {
                let condition = numeric::i32_value(&numeric::pop(&mut self.stack)?)?;
                let b = numeric::pop(&mut self.stack)?;
                let a = numeric::pop(&mut self.stack)?;
                self.stack.push(if condition != 0 { a } else { b })
            }
            0xd1 => {
                let value = numeric::pop(&mut self.stack)?;
                self.stack.push(JsValue::Number(if value == JsValue::Null {
                    1.0
                } else {
                    0.0
                }))
            }
            0xd3 => {
                let b = numeric::pop(&mut self.stack)?;
                let a = numeric::pop(&mut self.stack)?;
                self.stack
                    .push(JsValue::Number(if a == b { 1.0 } else { 0.0 }));
            }
            0xd4 => {
                let value = numeric::pop(&mut self.stack)?;
                if value == JsValue::Null {
                    return Err(trap(vm, "dereferencing a null pointer"));
                }
                self.stack.push(value)
            }
            _ => numeric::run(vm, op, &mut self.stack)?,
        }
        Ok(Flow::Next)
    }
    /** Indirect calls require a non-null Wasm function with exactly the expected signature. */
    fn indirect_target(&mut self, vm: &mut Vm, ty: u32, index: u32) -> Res<JsValue> {
        let table = instance::item(vm, self.instance, TABLES, index)?;
        let at = table::operand_index(vm, &table, &numeric::pop(&mut self.stack)?)?;
        let function =
            table::read(vm, &table, at).map_err(|_| trap(vm, "table index is out of bounds"))?;
        if function == JsValue::Null {
            return Err(trap(vm, "null function"));
        }
        if !instance::function_matches(vm, self.module, ty, &function)? {
            return Err(trap(vm, "function signature mismatch"));
        }
        Ok(function)
    }
    /** Ordinary indirect calls retain their caller and use the same checked target resolution. */
    fn indirect(&mut self, vm: &mut Vm, ty: u32, index: u32) -> Res<()> {
        let function = self.indirect_target(vm, ty, index)?;
        let args = self.take(self.module.types[ty as usize].params.len())?;
        self.stack
            .extend(instance::call_typed(vm, &function, args)?);
        Ok(())
    }
    /** Addresses are unsigned i32 plus a checked static offset; each scalar store is atomic. */
    fn memory(&mut self, vm: &mut Vm, op: u8, index: u32, offset: u64) -> Res<()> {
        let (_, size, store) = validate::memory_type(op).unwrap();
        let value = if store {
            Some(numeric::pop(&mut self.stack)?)
        } else {
            None
        };
        let value_address = numeric::pop(&mut self.stack)?;
        let memory = instance::item(vm, self.instance, MEMORIES, index)?;
        let at = address::effective(vm, &memory, &value_address, offset)?;
        if let Some(value) = value {
            let bytes = numeric::store(op, &value)?;
            memory::write(vm, &memory, at, &bytes[..size as usize])?
        } else {
            let bytes = memory::read(vm, &memory, at, size as usize)?;
            self.stack.push(numeric::load(op, &bytes))
        }
        Ok(())
    }
    /** memory.grow returns -1 for a Wasm allocation failure but never suppresses fatal VM quotas. */
    fn memory_index(&mut self, vm: &mut Vm, op: u8, index: u32) -> Res<()> {
        let memory = instance::item(vm, self.instance, MEMORIES, index)?;
        let memory_id = memory.object().unwrap();
        let address64 = address::is64(vm, memory_id);
        let result = if op == 0x3f {
            (memory::length(vm, &memory)? / PAGE_BYTES) as i64
        } else {
            let value = numeric::pop(&mut self.stack)?;
            let delta = address::operand(vm, memory_id, &value)?;
            match address::grow(vm, &memory, delta) {
                Ok(n) => i64::from(n),
                Err(Thrown::Value(_)) => -1,
                Err(fatal) => return Err(fatal),
            }
        };
        self.stack.push(if address64 {
            JsValue::BigInt(result as i128)
        } else {
            JsValue::Number(result as f64)
        });
        Ok(())
    }
    /** Supported bulk-memory instructions check full ranges before writes and charge every byte. */
    fn bulk(&mut self, vm: &mut Vm, sub: u32, args: &[u32]) -> Res<()> {
        if sub <= 7 {
            return numeric::saturating(vm, sub, &mut self.stack);
        }
        if sub == 8 || sub == 9 {
            return segments::memory(vm, self.instance, self.module, sub, args, &mut self.stack);
        }
        if (19..=22).contains(&sub) {
            return numeric::wide(sub, &mut self.stack);
        }
        if sub >= 12 {
            return segments::table(vm, self.instance, self.module, sub, args, &mut self.stack);
        }
        if !matches!(sub, 10 | 11) {
            return Err(vm.unsupported_error("WebAssembly bulk table instruction"));
        }
        let memory = instance::item(vm, self.instance, MEMORIES, args[0])?;
        let from = if sub == 10 {
            Some(instance::item(vm, self.instance, MEMORIES, args[1])?)
        } else {
            None
        };
        let count64 = address::is64(vm, memory.object().unwrap())
            && (sub == 11
                || from
                    .as_ref()
                    .is_some_and(|v| address::is64(vm, v.object().unwrap())));
        let value = numeric::pop(&mut self.stack)?;
        let length = if count64 {
            numeric::i64_value(&value)? as u64
        } else {
            u64::from(numeric::i32_value(&value)? as u32)
        };
        if length > BUFFER_BYTES as u64 {
            return Err(trap(vm, "memory access out of bounds"));
        }
        let length = length as usize;
        let source = numeric::pop(&mut self.stack)?;
        let target = numeric::pop(&mut self.stack)?;
        let destination = address::effective(vm, &memory, &target, 0)?;
        let bytes = if let Some(from) = from {
            let source = address::effective(vm, &from, &source, 0)?;
            memory::read(vm, &from, source, length)?
        } else {
            vec![numeric::i32_value(&source)? as u8; length]
        };
        vm.charge(length)?;
        memory::write(vm, &memory, destination, &bytes)
    }
}
