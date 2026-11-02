/** Native continuation snapshots retain typed locals and checked source positions as ordinary traced heap arrays. */
use super::*;
/** Every saved slot is inaccessible to JS and remains visible to V9's normal object graph tracing. */
pub(super) fn save(vm: &mut Vm, frames: &[Frame]) -> Res<ObjectId> {
    let total = frames
        .iter()
        .try_fold(0usize, |total, frame| {
            total.checked_add(frame.locals.len() + frame.stack.len() + frame.controls.len() * 4 + 6)
        })
        .ok_or_else(|| vm.type_error("oversized Wasm continuation"))?;
    if total > vm.limits.properties {
        return Err(crate::protocol::EngineError::limit(
            "V9 continuation fields",
            vm.limits.properties,
        )
        .into());
    }
    let mut records = Vec::new();
    for frame in frames {
        let locals = vm.new_array(frame.locals.clone())?;
        let stack = vm.new_array(frame.stack.clone())?;
        let mut encoded_controls = Vec::new();
        for control in &frame.controls {
            let record = vm.new_array(vec![
                JsValue::Number(control.height as f64),
                JsValue::Boolean(control.caught.is_some()),
                control.caught.clone().unwrap_or(JsValue::Undefined),
            ])?;
            encoded_controls.push(JsValue::Object(record));
        }
        let controls = vm.new_array(encoded_controls)?;
        let record = vm.new_array(vec![
            JsValue::Object(frame.instance),
            JsValue::Number(frame.index as f64),
            JsValue::Number(frame.pc as f64),
            JsValue::Object(locals),
            JsValue::Object(stack),
            JsValue::Object(controls),
        ])?;
        records.push(JsValue::Object(record));
    }
    let saved = vm.new_array(records)?;
    vm.collect_if_needed()?;
    Ok(saved)
}
/** Dense native records cannot materialize arbitrary logical holes from a forged sparse array. */
fn list(vm: &mut Vm, value: &JsValue, maximum: usize) -> Res<Vec<JsValue>> {
    let valid = value
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|o| match &o.class {
            Class::Array(values)
                if values.len() <= maximum && values.resident_len() == values.len() =>
            {
                Some(values.iter().cloned().collect())
            }
            _ => None,
        });
    valid.ok_or_else(|| vm.type_error("invalid Wasm continuation array"))
}
fn index(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    match value {
        JsValue::Number(value)
            if value.is_finite()
                && *value >= 0.0
                && value.fract() == 0.0
                && *value <= (BUFFER_BYTES * 2) as f64 =>
        {
            Ok(*value as usize)
        }
        _ => Err(vm.type_error("invalid Wasm continuation index")),
    }
}
/** Scalar values are already Wasm values; validation never invokes authored numeric coercion hooks. */
fn typed(vm: &mut Vm, m: &Module, ty: Type, value: &JsValue) -> Res<bool> {
    Ok(match ty {
        Type::I32 => {
            matches!(value,JsValue::Number(v)if v.fract()==0.0&&*v>=i32::MIN as f64&&*v<=i32::MAX as f64)
        }
        Type::I64 => {
            matches!(value,JsValue::BigInt(v)if *v>=i64::MIN as i128&&*v<=i64::MAX as i128)
        }
        Type::F32 | Type::F64 => matches!(value, JsValue::Number(_)),
        Type::V128 => matches!(value,JsValue::String(v)if v.len()==8),
        _ => !matches!(value, JsValue::Hole) && gc::matches(vm, m, ty, value)?,
    })
}
/** Local initialization and the parent's suspended operand stack are recomputed from validated original code. */
pub(super) fn restore(vm: &mut Vm, value: &JsValue) -> Res<Vec<Frame>> {
    restore_inner(vm, value, false)
}
/** Effect suspensions end at a real suspend instruction instead of an imported Promise. */
pub(super) fn restore_effect(vm: &mut Vm, value: &JsValue) -> Res<Vec<Frame>> {
    restore_inner(vm, value, true)
}
fn restore_inner(vm: &mut Vm, value: &JsValue, effect: bool) -> Res<Vec<Frame>> {
    let records = list(vm, value, vm.limits.frames)?;
    if records.is_empty() {
        return Err(vm.type_error("empty Wasm continuation"));
    }
    let mut frames = Vec::new();
    for (position, record) in records.iter().enumerate() {
        let fields = list(vm, record, 6)?;
        if fields.len() != 6 {
            return Err(vm.type_error("invalid Wasm continuation frame"));
        }
        let instance = fields[0]
            .object()
            .ok_or_else(|| vm.type_error("invalid Wasm continuation instance"))?;
        let source = get(vm, instance, INSTANCE)
            .ok_or_else(|| vm.type_error("invalid Wasm continuation source"))?;
        let (_, module) = module(vm, &source)?;
        let index = index(vm, &fields[1])? as u32;
        let pc = self::index(vm, &fields[2])?;
        let sig = module
            .signature(index)
            .map_err(|e| compile_failure(vm, e))?;
        let imported = (index as usize) < module.imported_functions();
        let mut local_types = sig.params.clone();
        if !imported {
            local_types.extend(&module.bodies[index as usize - module.imported_functions()].locals);
        }
        let locals = list(vm, &fields[3], local_types.len())?;
        let stack = list(vm, &fields[4], vm.limits.array_length)?;
        if locals.len() != local_types.len() {
            return Err(vm.type_error("invalid Wasm continuation locals"));
        }
        let program = Arc::new(if imported {
            super::program::Program::compile(&[])?
        } else {
            super::program::Program::compile(
                &module.bodies[index as usize - module.imported_functions()].code,
            )?
        });
        let entries = list(vm, &fields[5], super::super::super::constants::MAX_DEPTH)?;
        let mut heights = Vec::new();
        let mut caught = Vec::new();
        for value in entries {
            if matches!(value, JsValue::Number(_)) {
                heights.push(self::index(vm, &value)?);
                caught.push(None);
            } else {
                let record = list(vm, &value, 3)?;
                if record.len() != 3 {
                    return Err(vm.type_error("invalid Wasm continuation control record"));
                }
                heights.push(self::index(vm, &record[0])?);
                caught.push(match record[1] {
                    JsValue::Boolean(false) if record[2] == JsValue::Undefined => None,
                    JsValue::Boolean(true) => Some(record[2].clone()),
                    _ => return Err(vm.type_error("invalid Wasm continuation catch state")),
                });
            }
        }
        let (stack_types, initialized, expected_heights) = if imported {
            if position + 1 != records.len() || pc != 1 || !stack.is_empty() {
                return Err(vm.type_error("invalid suspended Wasm import"));
            }
            let function = instance::item(vm, instance, "wasm.imports", index)?;
            if !jspi::is_suspending(vm, &function) {
                return Err(vm.type_error("invalid suspended Wasm callback"));
            }
            (vec![], vec![true; locals.len()], vec![])
        } else {
            if position + 1 == records.len() && !effect {
                return Err(vm.type_error("Wasm continuation must await an imported Promise"));
            }
            let op = pc
                .checked_sub(1)
                .and_then(|pc| program.ops.get(pc))
                .ok_or_else(|| vm.type_error("invalid Wasm continuation position"))?;
            if position + 1 == records.len() && effect {
                let super::program::Code::Plain(instruction) = &op.code else {
                    return Err(vm.type_error("invalid suspended effect"));
                };
                let mut inner = instruction;
                while let Instruction::Located(_, next) = inner {
                    inner = next;
                }
                if !matches!(inner, Instruction::Suspend(_) | Instruction::Switch(_, _)) {
                    return Err(vm.type_error("invalid suspended effect"));
                }
            }
            super::super::super::validate::suspended_stack_path(&module, index, &op.path)
                .map_err(|e| compile_failure(vm, e))?
        };
        if heights != expected_heights {
            return Err(vm.type_error("invalid Wasm continuation control heights"));
        }
        let scopes = if imported {
            &[][..]
        } else {
            program
                .active(pc)
                .ok_or_else(|| vm.type_error("invalid Wasm continuation control position"))?
        };
        if scopes.len() != heights.len() {
            return Err(vm.type_error("invalid Wasm continuation control stack"));
        }
        let mut controls = Vec::new();
        for ((scope, height), caught) in scopes.iter().zip(heights).zip(caught) {
            let active_catch = program.scopes[*scope]
                .legacy
                .iter()
                .any(|(_, start)| *start <= pc && pc <= program.scopes[*scope].end);
            if caught.is_some() != active_catch {
                return Err(vm.type_error("invalid Wasm continuation catch scope"));
            }
            controls.push(super::frame::Control {
                scope: *scope,
                height,
                caught,
            });
        }
        if stack_types.len() != stack.len() {
            return Err(vm.type_error("invalid Wasm continuation operand stack"));
        }
        for ((ty, value), initialized) in local_types.iter().zip(&locals).zip(initialized) {
            if initialized && !typed(vm, &module, *ty, value)? {
                return Err(vm.type_error("invalid typed Wasm continuation local"));
            }
        }
        for (ty, value) in stack_types.iter().zip(&stack) {
            if !typed(vm, &module, *ty, value)? {
                return Err(vm.type_error("invalid typed Wasm continuation operand"));
            }
        }
        frames.push(Frame {
            trace: None,
            program,
            controls,
            instance,
            module: Arc::new(module),
            index,
            pc,
            locals,
            stack,
            roots: Default::default(),
        });
    }
    Ok(frames)
}
