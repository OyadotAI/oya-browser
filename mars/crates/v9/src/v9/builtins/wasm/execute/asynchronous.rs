/** Owned straight-line Wasm activations suspend into traced heap records and resume through real Promise jobs. */
use super::*;
use crate::v9::vm::data::ReactionHandler;
use std::sync::Arc;
mod frame;
mod program;
mod snapshot;
use frame::{Frame, Step};

/** A bounded WasmFX run either finishes or returns an effect and traced suspended frames. */
pub(super) enum EffectResult {
    Done(Vec<JsValue>),
    Suspended(ObjectId, Vec<JsValue>, ObjectId),
}

/** Drive explicit frames without replaying instructions or authored callbacks after a suspension. */
pub(super) fn run_effect(
    vm: &mut Vm,
    function: &JsValue,
    saved: Option<JsValue>,
    args: Vec<JsValue>,
    switch_tags: &[ObjectId],
    injection: Option<JsValue>,
) -> Res<EffectResult> {
    let mut frames = if let Some(saved) = saved {
        let mut frames = snapshot::restore_effect(vm, &saved)?;
        if injection.is_none() {
            frames
                .last_mut()
                .ok_or_else(|| vm.type_error("empty effect continuation"))?
                .stack
                .extend(args);
        }
        frames
    } else {
        if let Some(value) = injection {
            return Err(Thrown::Value(value));
        }
        let (instance, module, index) = instance::target(vm, function)?;
        vec![Frame::new(vm, instance, Arc::new(module), index, args)?]
    };
    let mut guard = super::super::roots::Roots::default();
    guard.sync(vm, std::iter::once(function))?;
    let trace = vm.native_activations.len();
    for frame in &mut frames {
        frame.enter_trace(vm);
    }
    let result = if let Some(value) = injection {
        match route_exception(vm, &mut frames, &value) {
            Ok(Some(Step::Next)) => {
                drive_effect(vm, &mut frames, function.clone(), switch_tags, &mut guard)
            }
            Ok(Some(Step::Done(values))) => {
                frames.pop().unwrap().release(vm);
                if let Some(parent) = frames.last_mut() {
                    parent.stack.extend(values);
                    drive_effect(vm, &mut frames, function.clone(), switch_tags, &mut guard)
                } else {
                    Ok(EffectResult::Done(values))
                }
            }
            Ok(_) => Err(Thrown::Value(value)),
            Err(error) => Err(error),
        }
    } else {
        drive_effect(vm, &mut frames, function.clone(), switch_tags, &mut guard)
    };
    for frame in frames.iter_mut().rev() {
        frame.roots.release(vm);
    }
    vm.end_native_activation(trace);
    guard.release(vm);
    result
}

/** Limits and frame transfer follow the same boundaries as the Promise continuation driver. */
fn drive_effect(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    mut function: JsValue,
    switch_tags: &[ObjectId],
    guard: &mut super::super::roots::Roots,
) -> Res<EffectResult> {
    loop {
        vm.charge(0)?;
        if vm.expired() {
            return Err(crate::protocol::EngineError::limit("Wasm continuation time", 0).into());
        }
        if frames.iter().map(|f| f.locals.len()).sum::<usize>() > vm.limits.bindings {
            return Err(crate::protocol::EngineError::limit(
                "V9 native Wasm local bindings",
                vm.limits.bindings,
            )
            .into());
        }
        if vm.frames.len() + vm.native_depth + frames.len() >= vm.limits.frames {
            return Err(vm.range_error("Maximum call stack size exceeded"));
        }
        let pin_mark = vm.pins.len();
        let frame = frames
            .last_mut()
            .ok_or_else(|| vm.type_error("empty effect continuation"))?;
        let bank = vm.object_realm(frame.instance);
        let step = match vm.with_realm(bank, |vm| frame.step(vm, false)) {
            Ok(step) => step,
            Err(Thrown::Value(value)) if exceptions::catchable(vm, &value) => {
                match route_exception(vm, frames, &value)? {
                    Some(step) => step,
                    None => return Err(Thrown::Value(value)),
                }
            }
            Err(error) => return Err(error),
        };
        match step {
            Step::Next => (),
            Step::Call(instance, module, index, args) => {
                let mut next = Frame::new(vm, instance, module, index, args)?;
                next.enter_trace(vm);
                frames.push(next);
            }
            Step::ResumeException(owner, saved, value) => {
                let mut resumed = snapshot::restore_effect(vm, &saved)?;
                set(vm, owner, "wasm.cont.saved", JsValue::Undefined);
                set(vm, owner, "wasm.cont.bound", JsValue::Undefined);
                for frame in &mut resumed {
                    frame.enter_trace(vm);
                }
                frames.extend(resumed);
                match route_exception(vm, frames, &value)? {
                    Some(Step::Next) => (),
                    Some(Step::Done(values)) => {
                        frames.pop().unwrap().release(vm);
                        if let Some(parent) = frames.last_mut() {
                            parent.stack.extend(values);
                        } else {
                            return Ok(EffectResult::Done(values));
                        }
                    }
                    Some(_) => return Err(vm.type_error("invalid Wasm exception transfer")),
                    None => return Err(Thrown::Value(value)),
                }
            }
            Step::ResumeSaved(owner, saved, args) => {
                let mut resumed = snapshot::restore_effect(vm, &saved)?;
                set(vm, owner, "wasm.cont.saved", JsValue::Undefined);
                set(vm, owner, "wasm.cont.bound", JsValue::Undefined);
                resumed
                    .last_mut()
                    .ok_or_else(|| vm.type_error("empty effect continuation"))?
                    .stack
                    .extend(args);
                for frame in &mut resumed {
                    frame.enter_trace(vm);
                }
                frames.extend(resumed);
            }
            Step::Tail(instance, module, index, args) => {
                frames.pop().unwrap().release(vm);
                let mut next = Frame::new(vm, instance, module, index, args)?;
                next.enter_trace(vm);
                frames.push(next);
            }
            Step::Done(values) => {
                frames.pop().unwrap().release(vm);
                if let Some(parent) = frames.last_mut() {
                    parent.stack.extend(values);
                } else {
                    return Ok(EffectResult::Done(values));
                }
            }
            Step::Pending(_) => {
                let instance = frames
                    .last()
                    .ok_or_else(|| vm.type_error("empty effect continuation"))?
                    .instance;
                let context = get(vm, instance, jspi::CONTEXT);
                let message = if matches!(context, Some(JsValue::Number(_))) {
                    "trying to suspend JS frames"
                } else {
                    "trying to suspend without WebAssembly.promising"
                };
                return Err(failure(vm, ids(vm).suspend_error, message));
            }
            Step::Effect(tag, values) => match nested_effect(vm, frames, tag, &values)? {
                Some(Step::Next) => continue,
                Some(Step::Done(values)) => {
                    frames.pop().unwrap().release(vm);
                    if let Some(parent) = frames.last_mut() {
                        parent.stack.extend(values);
                        continue;
                    }
                    return Ok(EffectResult::Done(values));
                }
                Some(_) => return Err(vm.type_error("invalid nested effect transfer")),
                None => {
                    let saved = snapshot::save(vm, frames)?;
                    return Ok(EffectResult::Suspended(tag, values, saved));
                }
            },
            Step::Switch(tag, target, args, return_type) => {
                let id = target
                    .object()
                    .ok_or_else(|| trap(vm, "null continuation reference"))?;
                if get(vm, id, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
                    return Err(trap(vm, "resuming an invalid continuation"));
                }
                let start = if let Some(start) =
                    nested_switch(vm, frames, tag, target.clone(), args.clone(), return_type)?
                {
                    start
                } else {
                    if !switch_tags.contains(&tag) {
                        return Err(trap(vm, "WasmFX: unhandled suspend"));
                    }
                    function = switch_frames(vm, frames, function, target, args, return_type)?;
                    0
                };
                let roots = std::iter::once(&function).chain(frames.iter().flat_map(|frame| {
                    frame.locals.iter().chain(&frame.stack).chain(
                        frame
                            .controls
                            .iter()
                            .filter_map(|control| control.caught.as_ref()),
                    )
                }));
                guard.sync(vm, roots)?;
                vm.pins.truncate(pin_mark);
                while vm.pins.len() > guard.pin_floor()
                    && vm.pins.last() == Some(&JsValue::Undefined)
                {
                    vm.pins.pop();
                }
                for frame in frames.iter_mut().skip(start) {
                    frame.enter_trace(vm);
                }
                vm.collect_if_needed()?;
            }
        }
    }
}
/** A matching inner resume handler captures only its own child stack; unmatched effects propagate. */
fn nested_effect(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    tag: ObjectId,
    values: &[JsValue],
) -> Res<Option<Step>> {
    if frames.len() < 2 {
        return Ok(None);
    }
    for parent in (0..frames.len() - 1).rev() {
        let Some(handlers) = frames[parent].resume_handlers() else {
            continue;
        };
        let instance = frames[parent].instance;
        for handler in handlers {
            if handler.switch {
                continue;
            }
            let actual = instance::item(vm, instance, exceptions::TAGS, handler.tag)?.object();
            if actual != Some(tag) {
                continue;
            }
            let ty = frames[parent]
                .effect_target(handler.depth)
                .ok_or_else(|| vm.type_error("invalid effect handler type"))?;
            let child = &frames[parent + 1];
            let function = instance::item(vm, child.instance, FUNCTIONS, child.index)?;
            let module = get(vm, instance, INSTANCE)
                .ok_or_else(|| vm.type_error("invalid continuation module"))?;
            let saved = snapshot::save(vm, &frames[parent + 1..])?;
            let next = vm.new_object()?;
            set(vm, next, "wasm.cont.function", function);
            set(vm, next, "wasm.cont.saved", JsValue::Object(saved));
            set(vm, next, "wasm.gc.module", module);
            set(vm, next, "wasm.gc.type", JsValue::Number(ty as f64));
            let mut transferred = values.to_vec();
            transferred.push(JsValue::Object(next));
            while frames.len() > parent + 1 {
                frames.pop().unwrap().release(vm);
            }
            return frames[parent]
                .effect_branch(vm, handler.depth, transferred)
                .map(Some);
        }
    }
    Ok(None)
}

/** A nested on_switch handler transfers only the child frame segment it owns. */
fn nested_switch(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    tag: ObjectId,
    target: JsValue,
    args: Vec<JsValue>,
    return_type: u32,
) -> Res<Option<usize>> {
    if frames.len() < 2 {
        return Ok(None);
    }
    for parent in (0..frames.len() - 1).rev() {
        let Some(handlers) = frames[parent].resume_handlers() else {
            continue;
        };
        let instance = frames[parent].instance;
        for handler in handlers {
            if !handler.switch
                || instance::item(vm, instance, exceptions::TAGS, handler.tag)?.object()
                    != Some(tag)
            {
                continue;
            }
            let start = parent + 1;
            let child = &frames[start];
            let function = instance::item(vm, child.instance, FUNCTIONS, child.index)?;
            let mut tail = frames.split_off(start);
            let result = switch_frames(vm, &mut tail, function, target, args, return_type);
            frames.extend(tail);
            result?;
            return Ok(Some(start));
        }
    }
    Ok(None)
}

/** Search live frames from inner to outer and discard only frames unwound by a matching handler. */
fn route_exception(vm: &mut Vm, frames: &mut Vec<Frame>, value: &JsValue) -> Res<Option<Step>> {
    for index in (0..frames.len()).rev() {
        let bank = vm.object_realm(frames[index].instance);
        if let Some(step) = vm.with_realm(bank, |vm| frames[index].handle_exception(vm, value))? {
            while frames.len() > index + 1 {
                frames.pop().unwrap().release(vm);
            }
            return Ok(Some(step));
        }
    }
    Ok(None)
}

/** Switching moves the current frame stack into the target's final continuation argument. */
fn switch_frames(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    function: JsValue,
    target: JsValue,
    args: Vec<JsValue>,
    return_type: u32,
) -> Res<JsValue> {
    let target = target
        .object()
        .ok_or_else(|| trap(vm, "null continuation reference"))?;
    if get(vm, target, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
        return Err(trap(vm, "resuming an invalid continuation"));
    }
    let next_function =
        get(vm, target, "wasm.cont.function").ok_or_else(|| trap(vm, "invalid continuation"))?;
    let next_saved = get(vm, target, "wasm.cont.saved");
    let mut next_args = Activation::bound_arguments(vm, target);
    next_args.extend(args);
    let instance = frames
        .last()
        .ok_or_else(|| vm.type_error("empty effect continuation"))?
        .instance;
    let module =
        get(vm, instance, INSTANCE).ok_or_else(|| vm.type_error("invalid continuation module"))?;
    let saved = snapshot::save(vm, frames)?;
    let returned = vm.new_object()?;
    set(vm, returned, "wasm.cont.function", function);
    set(vm, returned, "wasm.cont.saved", JsValue::Object(saved));
    set(vm, returned, "wasm.gc.module", module);
    set(
        vm,
        returned,
        "wasm.gc.type",
        JsValue::Number(return_type as f64),
    );
    next_args.push(JsValue::Object(returned));
    set(vm, target, "wasm.cont.consumed", JsValue::Boolean(true));
    let next_frames = if let Some(saved) = next_saved {
        let mut resumed = snapshot::restore_effect(vm, &saved)?;
        resumed
            .last_mut()
            .ok_or_else(|| vm.type_error("empty switched continuation"))?
            .stack
            .extend(next_args);
        resumed
    } else {
        let (instance, module, index) = instance::target(vm, &next_function)?;
        vec![Frame::new(
            vm,
            instance,
            Arc::new(module),
            index,
            next_args,
        )?]
    };
    set(vm, target, "wasm.cont.saved", JsValue::Undefined);
    set(vm, target, "wasm.cont.bound", JsValue::Undefined);
    for frame in frames.iter_mut().rev() {
        frame.release(vm);
    }
    *frames = next_frames;
    Ok(next_function)
}

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
            .any(|t| !gc_types::js_compatible(&module, *t))
        {
            return Err(vm.type_error(
                "type incompatibility when transforming from/to JS: unsupported value type",
            ));
        }
        let mut converted = Vec::new();
        for (index, ty) in sig.params.iter().enumerate() {
            let value = global::convert(vm, *ty, args.get(index).unwrap_or(&JsValue::Undefined))?;
            if ty.reference().is_some() && !gc::matches(vm, &module, *ty, &value)? {
                return Err(vm.type_error("type incompatibility when transforming from/to JS"));
            }
            converted.push(value);
        }
        Frame::new(vm, instance, Arc::new(module), index, converted)
    })?;
    run(vm, vec![first], output)
}
/** Every exit releases only the temporary native roots belonging to these explicit activations. */
fn run(vm: &mut Vm, mut frames: Vec<Frame>, output: ObjectId) -> Res<()> {
    let mut guard = super::super::roots::Roots::default();
    let output_value = JsValue::Object(output);
    guard.sync(
        vm,
        std::iter::once(&output_value).chain(frames.iter().flat_map(|frame| {
            frame.locals.iter().chain(&frame.stack).chain(
                frame
                    .controls
                    .iter()
                    .filter_map(|control| control.caught.as_ref()),
            )
        })),
    )?;
    let trace = vm.native_activations.len();
    for frame in &mut frames {
        frame.enter_trace(vm);
    }
    let result = drive(vm, &mut frames, output, &mut guard);
    for frame in frames.iter_mut().rev() {
        frame.roots.release(vm);
    }
    vm.end_native_activation(trace);
    guard.release(vm);
    result
}
/** Child calls retain explicit bounded frames; completed frames transfer only their typed result values. */
fn drive(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    output: ObjectId,
    guard: &mut super::super::roots::Roots,
) -> Res<()> {
    loop {
        vm.charge(0)?;
        if vm.expired() {
            return Err(crate::protocol::EngineError::new(
                "limit_exceeded",
                "Script execution stopped: time limit exceeded.",
                "Reduce the script work; what it did before it stopped stays, as in browsers.",
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
        let pin_mark = vm.pins.len();
        let frame = frames
            .last_mut()
            .ok_or_else(|| vm.type_error("empty Wasm continuation"))?;
        let bank = vm.object_realm(frame.instance);
        let step = match vm.with_realm(bank, |vm| frame.step(vm, true)) {
            Ok(step) => step,
            Err(Thrown::Value(value)) if exceptions::catchable(vm, &value) => {
                match route_exception(vm, frames, &value)? {
                    Some(step) => step,
                    None => return Err(Thrown::Value(value)),
                }
            }
            Err(error) => return Err(error),
        };
        match step {
            Step::Next => (),
            Step::Call(instance, module, index, args) => {
                let mut frame = Frame::new(vm, instance, module, index, args)?;
                frame.enter_trace(vm);
                frames.push(frame)
            }
            Step::ResumeException(owner, saved, value) => {
                let mut resumed = snapshot::restore_effect(vm, &saved)?;
                set(vm, owner, "wasm.cont.saved", JsValue::Undefined);
                set(vm, owner, "wasm.cont.bound", JsValue::Undefined);
                for frame in &mut resumed {
                    frame.enter_trace(vm);
                }
                frames.extend(resumed);
                match route_exception(vm, frames, &value)? {
                    Some(Step::Next) => (),
                    Some(Step::Done(values)) => {
                        frames.pop().unwrap().release(vm);
                        if let Some(parent) = frames.last_mut() {
                            parent.stack.extend(values);
                        } else {
                            let values: Vec<_> = values
                                .into_iter()
                                .map(|value| gc::expose(vm, value))
                                .collect();
                            let value = match values.len() {
                                0 => JsValue::Undefined,
                                1 => values[0].clone(),
                                _ => JsValue::Object(vm.new_array(values)?),
                            };
                            return vm.resolve_promise(output, value);
                        }
                    }
                    Some(_) => return Err(vm.type_error("invalid Wasm exception transfer")),
                    None => return Err(Thrown::Value(value)),
                }
            }
            Step::ResumeSaved(owner, saved, args) => {
                let mut resumed = snapshot::restore_effect(vm, &saved)?;
                set(vm, owner, "wasm.cont.saved", JsValue::Undefined);
                set(vm, owner, "wasm.cont.bound", JsValue::Undefined);
                resumed
                    .last_mut()
                    .ok_or_else(|| vm.type_error("empty effect continuation"))?
                    .stack
                    .extend(args);
                for frame in &mut resumed {
                    frame.enter_trace(vm);
                }
                frames.extend(resumed);
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
                    let values: Vec<_> = values
                        .into_iter()
                        .map(|value| gc::expose(vm, value))
                        .collect();
                    let value = match values.len() {
                        0 => JsValue::Undefined,
                        1 => values[0].clone(),
                        _ => JsValue::Object(vm.new_array(values)?),
                    };
                    return vm.resolve_promise(output, value);
                }
            }
            Step::Pending(promise) => return schedule(vm, frames, output, promise),
            Step::Effect(tag, values) => match nested_effect(vm, frames, tag, &values)? {
                Some(Step::Next) => (),
                Some(Step::Done(values)) => {
                    frames.pop().unwrap().release(vm);
                    if let Some(parent) = frames.last_mut() {
                        parent.stack.extend(values);
                    } else {
                        let values: Vec<_> = values
                            .into_iter()
                            .map(|value| gc::expose(vm, value))
                            .collect();
                        let value = match values.len() {
                            0 => JsValue::Undefined,
                            1 => values[0].clone(),
                            _ => JsValue::Object(vm.new_array(values)?),
                        };
                        return vm.resolve_promise(output, value);
                    }
                }
                Some(_) => return Err(vm.type_error("invalid nested effect transfer")),
                None => return Err(trap(vm, "WasmFX: unhandled suspend")),
            },
            Step::Switch(tag, target, args, return_type) => {
                let id = target
                    .object()
                    .ok_or_else(|| trap(vm, "null continuation reference"))?;
                if get(vm, id, "wasm.cont.consumed") == Some(JsValue::Boolean(true)) {
                    return Err(trap(vm, "resuming an invalid continuation"));
                }
                let start = nested_switch(vm, frames, tag, target, args, return_type)?
                    .ok_or_else(|| trap(vm, "WasmFX: unhandled suspend"))?;
                let output_value = JsValue::Object(output);
                guard.sync(
                    vm,
                    std::iter::once(&output_value).chain(frames.iter().flat_map(|frame| {
                        frame.locals.iter().chain(&frame.stack).chain(
                            frame
                                .controls
                                .iter()
                                .filter_map(|control| control.caught.as_ref()),
                        )
                    })),
                )?;
                vm.pins.truncate(pin_mark);
                while vm.pins.len() > guard.pin_floor()
                    && vm.pins.last() == Some(&JsValue::Undefined)
                {
                    vm.pins.pop();
                }
                for frame in frames.iter_mut().skip(start) {
                    frame.enter_trace(vm);
                }
                vm.collect_if_needed()?;
            }
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
            let value = call.arg(0);
            match route_exception(vm, &mut frames, &value)? {
                Some(Step::Next) => return run(vm, frames, output),
                Some(Step::Done(values)) => {
                    frames.pop().unwrap().release(vm);
                    if let Some(parent) = frames.last_mut() {
                        parent.stack.extend(values);
                        return run(vm, frames, output);
                    }
                    let values: Vec<_> = values
                        .into_iter()
                        .map(|value| gc::expose(vm, value))
                        .collect();
                    let value = match values.len() {
                        0 => JsValue::Undefined,
                        1 => values[0].clone(),
                        _ => JsValue::Object(vm.new_array(values)?),
                    };
                    return vm.resolve_promise(output, value);
                }
                Some(_) => return Err(vm.type_error("invalid Wasm rejection transfer")),
                None => return Err(Thrown::Value(value)),
            }
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
