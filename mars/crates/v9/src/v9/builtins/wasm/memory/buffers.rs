/** Memory buffer conversions preserve real backing identity, growth visibility and detachment semantics. */
use super::*;
const OWNER: &str = "wasm.memory.buffer.owner";
/** Both conversion methods are branded native operations with no parameters. */
pub(super) fn install(b: &mut Builder, proto: ObjectId) {
    b.method(proto, "toFixedLengthBuffer", 0, fixed);
    b.method(proto, "toResizableBuffer", 0, resizable);
}
fn fixed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    convert(vm, &call.this, false)
}
fn resizable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    convert(vm, &call.this, true)
}
/** A missing declared maximum prevents resizable conversion before any storage is changed. */
fn maximum(vm: &mut Vm, memory: ObjectId) -> Res<usize> {
    match get(vm, memory, MAXIMUM) {
        Some(JsValue::Number(n))
            if n.is_finite()
                && n >= 0.0
                && n.fract() == 0.0
                && n <= address::MAX_MEMORY64_PAGES as f64 =>
        {
            Ok(n as usize * PAGE_BYTES)
        }
        _ => Err(vm.type_error("Memory must have a maximum")),
    }
}
/** Shared wrappers alias one native store; changing an unshared kind detaches the previous ArrayBuffer. */
fn convert(vm: &mut Vm, value: &JsValue, resizable: bool) -> Res<JsValue> {
    let memory = brand(vm, value, MEMORY)?;
    let old = storage(vm, value)?;
    let maximum = if resizable {
        Some(maximum(vm, memory)?)
    } else {
        None
    };
    if let Some(Class::SharedArrayBuffer(bytes)) = vm.heap.get(old).map(|o| &o.class) {
        let bytes = bytes.clone();
        if bytes.wasm_maximum().is_some() == resizable {
            return shared_memory::refresh(vm, memory, old).map(JsValue::Object);
        }
        let bytes = if let Some(maximum) = maximum {
            bytes
                .growable_view(maximum)
                .ok_or_else(|| vm.type_error("invalid growable Wasm memory window"))?
        } else {
            bytes.fixed_view(bytes.backing_len()).unwrap()
        };
        let next = super::super::super::import_shared_buffer(vm, bytes)?;
        set(vm, memory, MEMORY, JsValue::Object(next));
        return Ok(JsValue::Object(next));
    }
    if super::super::super::buffer_maximum(vm, old).is_some() == resizable {
        return Ok(JsValue::Object(old));
    }
    let Some(Class::ArrayBuffer(bytes)) = vm.heap.get(old).map(|o| &o.class) else {
        return Err(vm.type_error("invalid memory buffer"));
    };
    let bytes = bytes.0.clone();
    let next = super::super::buffer(vm, bytes)?;
    super::super::super::protect_buffer(vm, next);
    if let Some(maximum) = maximum {
        super::super::super::set_buffer_maximum(vm, next, maximum);
        set(vm, next, OWNER, JsValue::Object(memory));
    }
    super::super::super::detach_buffer(vm, old);
    set(vm, memory, MEMORY, JsValue::Object(next));
    Ok(JsValue::Object(next))
}
/** Author resizing of a Wasm buffer routes through page growth and validates its actual owning memory. */
pub(in crate::v9::builtins::wasm) fn resize_buffer(
    vm: &mut Vm,
    buffer: ObjectId,
    length: usize,
) -> Res<bool> {
    let Some(memory) = get(vm, buffer, OWNER).and_then(|v| v.object()) else {
        return Ok(false);
    };
    let current = storage(vm, &JsValue::Object(memory))?;
    let old = super::length(vm, &JsValue::Object(memory))?;
    if current != buffer || length < old || length % PAGE_BYTES != 0 {
        return Err(vm.range_error("WebAssembly buffer resize requires nonshrinking whole pages"));
    }
    super::resize(
        vm,
        &JsValue::Object(memory),
        ((length - old) / PAGE_BYTES) as u32,
    )?;
    Ok(true)
}
