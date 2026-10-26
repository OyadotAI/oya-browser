/** Owned straight-line Wasm activations suspend into traced heap records and resume through real Promise jobs. */
use super::*;
use crate::v9::vm::data::ReactionHandler;
use std::sync::Arc;
mod frame;
mod program;
mod snapshot;
use frame::{Frame, Step};

/** A promising boundary creates its Promise in the wrapper's realm and runs typed Wasm in its own realm. */
pub(in crate::v9::builtins::wasm) fn start(
    vm: &mut Vm,
    function: JsValue,
    args: Vec<JsValue>,
    output: ObjectId,
) -> Res<()> {
    let function_id = function
        .object()
        .ok_or_else(|| vm.type_error("invalid promising function"))?;
    let bank = vm.object_realm(function_id);
    let first = vm.with_realm(bank, |vm| {
        let (instance, module, index) = instance::target(vm, &function)?;
        let sig = module
            .signature(index)
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
        let mut converted = Vec::new();
        for (index, ty) in sig.params.iter().enumerate() {
            let value = global::convert(vm, *ty, args.get(index).unwrap_or(&JsValue::Undefined))?;
            if ty.reference().is_some() && !gc::matches(vm, &module, *ty, &value)? {
                return Err(vm.type_error("WebAssembly parameter reference type mismatch"));
            }
            converted.push(value);
        }
        Frame::new(vm, instance, Arc::new(module), index, converted)
    })?;
    run(vm, vec![first], output)
}
/** Every exit releases only the temporary native roots belonging to these explicit activations. */
fn run(vm: &mut Vm, mut frames: Vec<Frame>, output: ObjectId) -> Res<()> {
    let trace = vm.native_activations.len();
    for frame in &mut frames {
        frame.enter_trace(vm);
    }
    let result = drive(vm, &mut frames, output);
    for frame in frames.iter_mut().rev() {
        frame.roots.release(vm);
    }
    vm.end_native_activation(trace);
    result
}
/** Child calls retain explicit bounded frames; completed frames transfer only their typed result values. */
fn drive(vm: &mut Vm, frames: &mut Vec<Frame>, output: ObjectId) -> Res<()> {
    loop {
        vm.charge(0)?;
        if vm.expired() {
            return Err(crate::protocol::EngineError::new(
                "limit_exceeded",
                "Script execution stopped: time limit exceeded.",
                "Reduce the script work; that script's changes were not applied.",
            )
            .into());
        }
        if frames.iter().map(|frame| frame.locals.len()).sum::<usize>() > vm.limits.bindings {
            return Err(crate::protocol::EngineError::limit(
                "V9 native Wasm local bindings",
                vm.limits.bindings,
            )
            .into());
        }
        if vm.frames.len() + vm.native_depth + frames.len() >= vm.limits.frames {
            return Err(vm.range_error("Maximum call stack size exceeded"));
        }
        let frame = frames
            .last_mut()
            .ok_or_else(|| vm.type_error("empty Wasm continuation"))?;
        let bank = vm.object_realm(frame.instance);
        let step = vm.with_realm(bank, |vm| frame.step(vm))?;
        match step {
            Step::Next => (),
            Step::Call(instance, module, index, args) => {
                let mut frame = Frame::new(vm, instance, module, index, args)?;
                frame.enter_trace(vm);
                frames.push(frame)
            }
            Step::Tail(instance, module, index, args) => {
                frames.pop().unwrap().release(vm);
                let mut frame = Frame::new(vm, instance, module, index, args)?;
                frame.enter_trace(vm);
                frames.push(frame);
            }
            Step::Done(values) => {
                for value in &values {
                    vm.pin(value.clone());
                }
                frames.pop().unwrap().release(vm);
                if let Some(parent) = frames.last_mut() {
                    parent.stack.extend(values);
                } else {
                    let value = match values.len() {
                        0 => JsValue::Undefined,
                        1 => values[0].clone(),
                        _ => JsValue::Object(vm.new_array(values)?),
                    };
                    return vm.resolve_promise(output, value);
                }
            }
            Step::Pending(promise) => return schedule(vm, frames, output, promise),
        }
    }
}
/** Reactions hold native closures over the actual saved frame records, never a process-local continuation pointer. */
fn schedule(vm: &mut Vm, frames: &[Frame], output: ObjectId, promise: ObjectId) -> Res<()> {
    let saved = snapshot::save(vm, frames)?;
    let fulfilled = vm.native_closure(
        ids(vm).async_resume,
        vec![
            JsValue::Object(output),
            JsValue::Object(saved),
            JsValue::Boolean(false),
        ],
    )?;
    let rejected = vm.native_closure(
        ids(vm).async_resume,
        vec![
            JsValue::Object(output),
            JsValue::Object(saved),
            JsValue::Boolean(true),
        ],
    )?;
    vm.perform_then(
        promise,
        ReactionHandler::Function(JsValue::Object(fulfilled)),
        ReactionHandler::Function(JsValue::Object(rejected)),
        None,
    )
}
/** Resume executes after Promise settlement, preserving rejection identity and remaining execution budgets. */
pub(in crate::v9::builtins::wasm) fn resume(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let output = slots
        .first()
        .and_then(JsValue::object)
        .ok_or_else(|| vm.type_error("invalid Wasm continuation promise"))?;
    if !matches!(
        vm.heap.get(output).map(|o| &o.class),
        Some(Class::Promise(_))
    ) {
        return Err(vm.type_error("invalid Wasm continuation promise"));
    }
    let result = (|| {
        let mut frames = snapshot::restore(vm, slots.get(1).unwrap_or(&JsValue::Undefined))?;
        if slots.get(2) == Some(&JsValue::Boolean(true)) {
            return Err(Thrown::Value(call.arg(0)));
        }
        let frame = frames
            .last_mut()
            .ok_or_else(|| vm.type_error("missing suspended Wasm import"))?;
        let bank = vm.object_realm(frame.instance);
        let converted = vm.with_realm(bank, |vm| frame.convert_return(vm, call.arg(0)))?;
        frame.stack = converted;
        run(vm, frames, output)
    })();
    match result {
        Ok(()) => (),
        Err(Thrown::Value(value)) => vm.settle(output, PromiseState::Rejected(value))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Undefined)
}
