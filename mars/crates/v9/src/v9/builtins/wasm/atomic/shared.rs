/** Shared Wasm atomics use one backing mutex for read/modify/write and genuine indexed wait queues. */
use super::*;
use crate::v9::vm::data::{SharedBytes, WaitResult};

/** Waiting is bounded by the VM deadline and worker cancellation, including an infinite Wasm timeout. */
fn wait(
    vm: &mut Vm,
    bytes: &SharedBytes,
    op: u32,
    at: usize,
    expected: JsValue,
    timeout: JsValue,
) -> Res<JsValue> {
    let expected = if op == 1 {
        numeric::i32_value(&expected)?.to_le_bytes().to_vec()
    } else {
        numeric::i64_value(&expected)?.to_le_bytes().to_vec()
    };
    let nanos = numeric::i64_value(&timeout)?;
    let timeout = if nanos < 0 {
        None
    } else {
        Some(std::time::Duration::from_nanos(nanos as u64))
    };
    let result = bytes
        .wait(at, &expected, timeout, || vm.expired())
        .map_err(|()| crate::v9::platform::timeout())?;
    Ok(JsValue::Number(match result {
        WaitResult::Ok => 0.0,
        WaitResult::NotEqual => 1.0,
        WaitResult::TimedOut => 2.0,
    }))
}
/** No script callback, allocation or VM access occurs while atomic backing bytes are locked. */
pub(super) fn run(
    vm: &mut Vm,
    bytes: SharedBytes,
    op: u32,
    width: usize,
    ty: Type,
    at: usize,
    stored: Option<JsValue>,
    expected: Option<JsValue>,
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    if bytes.read(at, width).is_none() {
        return Err(trap(vm, "memory access out of bounds"));
    }
    if op == 0 {
        let count = numeric::i32_value(&stored.unwrap())? as u32 as usize;
        stack.push(JsValue::Number(
            crate::v9::builtins::notify_shared(vm, &bytes, at, count)? as f64,
        ));
        return Ok(());
    }
    if matches!(op, 1 | 2) {
        stack.push(wait(
            vm,
            &bytes,
            op,
            at,
            expected.unwrap(),
            stored.unwrap(),
        )?);
        return Ok(());
    }
    let value = stored.as_ref().map(|v| integer(v, ty)).transpose()?;
    let expected = expected
        .as_ref()
        .map(|v| integer(v, ty))
        .transpose()?
        .unwrap_or(0);
    let old = bytes
        .atomic(at, width, value.is_some(), |target| {
            let mut data = [0; 8];
            data[..width].copy_from_slice(target);
            let old = u64::from_le_bytes(data);
            if let Some(value) = value {
                target.copy_from_slice(
                    &calculate(op, old, value, expected, width).to_le_bytes()[..width],
                )
            }
            old
        })
        .ok_or_else(|| trap(vm, "memory access out of bounds"))?;
    push_result(op, ty, old, stack);
    Ok(())
}
