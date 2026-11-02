/** d8.test.verifySourcePositions checks that a function's instruction-to-source table is ordered and stays inside its own source. */
use super::*;
use crate::v9::value::ObjectId;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::FunctionKind;
use crate::v9::vm::error::ErrorKind;

/** V8's error for any argument that is not exactly one function. */
const SHAPE_ERROR: &str = "Expected function as single argument.";
/** V8's error for a native builtin, which has no bytecode to verify. */
const NO_CODE_ERROR: &str = "Function has no BytecodeArray attached.";
/** A position entry must name an instruction that exists. */
const PC_ERROR: &str = "Source position refers to a missing instruction.";
/** Entries are sorted by instruction, so lookups by binary search stay correct. */
const ORDER_ERROR: &str = "Source position table is not sorted by instruction.";
/** Every position must lie within the function's own source span. */
const RANGE_ERROR: &str = "Source position lies outside its function.";

/** The verifier is a namespace method, so it is reachable exactly where d8.test is. */
pub(super) fn install(builder: &mut Builder, namespace: ObjectId) {
    builder.method(namespace, "verifySourcePositions", 1, verify);
}

/** A refusal is an ordinary Error, as V8's ThrowError is; only engine failures escape as fatal. */
fn verify(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let outcome = match call.args {
        [target] => resolve(vm, target)?.and_then(|site| check(vm, site)),
        _ => Err(SHAPE_ERROR),
    };
    match outcome {
        Ok(()) => Ok(JsValue::Undefined),
        Err(message) => Err(error(vm, message)),
    }
}

/** Intrinsics are materialized first, so a builtin is seen as a function and refused as one without code. */
fn resolve(vm: &mut Vm, value: &JsValue) -> Res<Result<u32, &'static str>> {
    let Some(mut id) = value.object() else {
        return Ok(Err(SHAPE_ERROR));
    };
    loop {
        vm.materialize(id)?;
        let Some(function) = vm.heap.get(id).and_then(|object| object.function()) else {
            return Ok(Err(SHAPE_ERROR));
        };
        match &function.kind {
            FunctionKind::Closure { site, .. } => return Ok(Ok(*site)),
            FunctionKind::Bound { target, .. } => id = *target,
            FunctionKind::Native { .. } => match super::shadow::wrapped_target(vm, id) {
                Some(JsValue::Object(target)) => id = target,
                _ => return Ok(Err(NO_CODE_ERROR)),
            },
        }
    }
}

/** Entries must be strictly increasing by instruction and each position must lie inside the span. */
fn check(vm: &Vm, site: u32) -> Result<(), &'static str> {
    let code = vm.registry.get(site).ok_or(NO_CODE_ERROR)?;
    let Some((start, end)) = code.source_span else {
        return Ok(());
    };
    let mut previous: Option<u32> = None;
    for (pc, position) in &code.positions {
        if *pc as usize >= code.ops.len() {
            return Err(PC_ERROR);
        }
        if previous.is_some_and(|last| last >= *pc) {
            return Err(ORDER_ERROR);
        }
        if position.offset < start || position.offset > end {
            return Err(RANGE_ERROR);
        }
        previous = Some(*pc);
    }
    Ok(())
}

/** Ordinary Error objects carry the message V8 uses for the same refusal. */
fn error(vm: &mut Vm, message: &str) -> Thrown {
    match vm.make_error(ErrorKind::Error, message) {
        Ok(id) => Thrown::Value(JsValue::Object(id)),
        Err(error) => error.into(),
    }
}
