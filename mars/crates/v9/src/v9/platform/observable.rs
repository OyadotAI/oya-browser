/** Observable, Subscriber and EventTarget.prototype.when: the platform's JavaScript-defined interfaces, installed once per fresh page. */
use crate::protocol::EngineResult;
use crate::v9::compile::Code;
use crate::v9::vm::Vm;
use std::sync::Arc;

/** The bootstrap that defines the interfaces. It is JavaScript because its semantics are closures over promises and abort signals, which the host objects already implement. */
const SOURCE: &str = include_str!("observable.js");
/** Where the bootstrap compiles: past every page source (their total is bounded by SCRIPT_TOTAL_BYTES) and before dynamic code (which starts at DYNAMIC_OFFSET), so its sites never collide with either. */
const BOOTSTRAP_OFFSET: usize = crate::constants::SCRIPT_TOTAL_BYTES;

/** Compile the bootstrap into this session's registry. Every step does it, because a page's heap refers to the bootstrap's code by site; the parse is cached, so a later step only adopts it. */
pub(in crate::v9) fn compile(vm: &mut Vm) -> EngineResult<Arc<Code>> {
    crate::v9::compile_dynamic(&mut vm.registry, SOURCE, BOOTSTRAP_OFFSET, None)
}

/** Run the bootstrap's top level once, for a fresh page, from the code the session compiled. A failure here would break every page, so the caller reports it as an engine error rather than a script error. */
pub(in crate::v9) fn run(vm: &mut Vm, code: Arc<Code>) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    match vm.run_script(code) {
        Ok(_) => Ok(()),
        Err(thrown) => {
            let location = vm.page.throw_location.unwrap_or_default();
            Err(vm.uncaught(thrown, location))
        }
    }
}

#[cfg(test)]
mod tests {
    /** The bootstrap must reach the engine as one dynamic source at the first dynamic base, never a page offset. */
    #[test]
    fn bootstrap_source_is_nonempty_and_declares_the_globals() {
        let text = super::SOURCE;
        assert!(text.contains("define(global, 'Observable', Observable);"));
        assert!(text.contains("define(global, 'Subscriber', Subscriber);"));
        assert!(text.contains("define(global.EventTarget.prototype, 'when', when);"));
    }
}
