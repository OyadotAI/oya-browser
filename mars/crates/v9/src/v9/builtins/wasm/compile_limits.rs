/** Conformance flags may tighten the existing Wasm byte ceiling, but can never increase a VM quota. */
use super::*;

/** Production always uses the existing buffer ceiling; the shell can supply a smaller explicit cap. */
pub(super) fn maximum(vm: &Vm) -> usize {
    #[cfg(feature = "conformance")]
    {
        return vm
            .page
            .environment
            .conformance_wasm_max_module_bytes
            .unwrap_or(BUFFER_BYTES)
            .min(BUFFER_BYTES);
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        BUFFER_BYTES
    }
}
/** Ordinary compilation rejects oversized input before option getters; streaming reports its byte position. */
pub(super) fn check(vm: &mut Vm, length: usize, streaming: bool) -> Res<()> {
    let maximum = maximum(vm);
    if length <= maximum {
        return Ok(());
    }
    let error = if streaming {
        Error::invalid(&format!("size > maximum module size ({maximum}): {length}")).at(0)
    } else {
        Error::invalid(&format!(
            "buffer source exceeds maximum size of {maximum} (is {length})"
        ))
    };
    Err(compile_failure(vm, error))
}
