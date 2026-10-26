/** Identical dynamic function programs share validated code while every call creates a fresh closure. */
use super::*;

/** Reusing this exact global-scope expression does not retain another copy of its source. */
pub(super) fn evaluate_function(vm: &mut Vm, source: String) -> Res<JsValue> {
    let existing = vm.dynamic.iter().position(|entry| {
        entry.context.as_deref() == Some(INDIRECT_CONTEXT) && entry.source == source
    });
    if let Some(index) = existing {
        let offset = crate::v9::vm::dynamic_source_base(&vm.dynamic, index);
        let site = crate::v9::syntax::site_of(crate::v9::syntax::Pos {
            offset: offset as u32,
            line: 1,
            column: 1,
        });
        if let Some(code) = vm.registry.get(site).cloned() {
            return vm.run_script(code);
        }
    }
    evaluate(vm, source)
}
