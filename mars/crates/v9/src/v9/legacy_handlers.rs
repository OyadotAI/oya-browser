/** Legacy event handlers use the bounded source ledger so every retained code site can be validated. */
use super::{Code, EngineError, EngineResult, Vm, VmState, builtins, compile_dynamic, fatal, vm};
use std::sync::Arc;

/** Handler markers must identify real ordinary Script units, never eval contexts or absent code. */
pub(super) fn validate(state: &VmState) -> EngineResult<()> {
    let sources = &state.dynamic;
    let valid = state.page.legacy_handlers.len() <= sources.len()
        && state.page.legacy_handlers.iter().all(|base| {
            sources.iter().enumerate().any(|(index, source)| {
                vm::dynamic_source_base(sources, index) == *base
                    && source.context.is_none()
                    && source.origin.is_none()
            })
        });
    if valid {
        Ok(())
    } else {
        Err(EngineError::invalid("Invalid retained legacy handler."))
    }
}

/** Repeated handlers reuse immutable sites; changed handlers retain earlier closures and literal cells. */
pub(super) fn compile(vm: &mut Vm, source: &str) -> EngineResult<(Arc<Code>, usize)> {
    if let Some((index, _)) = vm.dynamic.iter().enumerate().find(|(index, saved)| {
        saved.source == source
            && vm
                .page
                .state
                .legacy_handlers
                .contains(&vm::dynamic_source_base(&vm.dynamic, *index))
    }) {
        let base = vm::dynamic_source_base(&vm.dynamic, index);
        return compile_at(vm, source, base);
    }
    retain(vm, source)
}

/** Admit source bytes before creating a stable handler site under the existing dynamic code quotas. */
fn retain(vm: &mut Vm, source: &str) -> EngineResult<(Arc<Code>, usize)> {
    vm.prepare_dynamic(source.len()).map_err(fatal)?;
    let base = builtins::dynamic_base(&vm.dynamic);
    let compiled = compile_at(vm, source, base)?;
    vm.dynamic.push(vm::DynamicSource {
        source: source.into(),
        context: None,
        base: Some(base),
        origin: None,
    });
    vm.page.state.legacy_handlers.insert(base);
    Ok(compiled)
}

/** Handler errors retain their public source-relative locations despite internal dynamic offsets. */
fn compile_at(vm: &mut Vm, source: &str, base: usize) -> EngineResult<(Arc<Code>, usize)> {
    compile_dynamic(&mut vm.registry, source, base, None, false)
        .map(|code| (code, base))
        .map_err(|error| location(error, base))
}

/** Runtime and syntax diagnostics name the handler, excluding the private source ledger offset. */
pub(super) fn location(mut error: EngineError, base: usize) -> EngineError {
    error.input_kind = Some("handler".into());
    if let Some(location) = error.location.as_deref_mut() {
        location.script_index = None;
        if location.byte_offset >= base {
            location.byte_offset -= base;
        }
    }
    error
}
