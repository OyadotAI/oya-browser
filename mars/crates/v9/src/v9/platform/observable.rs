/** The platform's JavaScript-defined interfaces (Observable, Subscriber, EventTarget.prototype.when, TextEncoderStream, TextDecoderStream, VTTCue, PerformanceObserver, the Geometry Interfaces), installed once per fresh page. */
use crate::protocol::EngineResult;
use crate::v9::compile::Code;
use crate::v9::value::JsValue;
use crate::v9::vm::Vm;
use std::sync::Arc;

/** The bootstrap that defines the interfaces. It is JavaScript because its semantics are closures over promises and abort signals, which the host objects already implement. */
const SOURCE: &str = concat!(
    include_str!("observable.js"),
    include_str!("platform_apis/open.js"),
    include_str!("platform_apis/clone.js"),
    include_str!("platform_apis/clipboard.js"),
    include_str!("platform_apis/locks.js"),
    include_str!("platform_apis/cookie_store.js"),
    include_str!("platform_apis/access.js"),
    include_str!("platform_apis/devices.js"),
    include_str!("platform_apis/eme.js"),
    include_str!("platform_apis/caches.js"),
    include_str!("platform_apis/service_worker.js"),
    include_str!("platform_apis/service_worker_scope.js"),
    include_str!("platform_apis/close.js"),
    "\n",
    include_str!("performance_observer.js"),
    "\n",
    include_str!("file_reader.js"),
    "\n",
    include_str!("geometry.js"),
    "\n",
    include_str!("view_transitions.js"),
    "\n",
    include_str!("presentation.js"),
    // Last: the bootstrap's completion value must be the media installer that `run` calls.
    "\n",
    include_str!("media.js"),
);
/** The bootstrap's length, so the IndexedDB bootstrap compiled after it can prove they never overlap. */
pub(in crate::v9) const SOURCE_LEN: usize = SOURCE.len();
/** Where the bootstrap compiles: past every page source (their total is bounded by SCRIPT_TOTAL_BYTES) and before dynamic code (which starts at DYNAMIC_OFFSET), so its sites never collide with either. */
pub(in crate::v9) const BOOTSTRAP_OFFSET: usize = crate::constants::SCRIPT_TOTAL_BYTES;

/** Compile the bootstrap into this session's registry. Every step does it, because a page's heap refers to the bootstrap's code by site; the parse is cached, so a later step only adopts it. */
pub(in crate::v9) fn compile(vm: &mut Vm) -> EngineResult<Arc<Code>> {
    crate::v9::compile_dynamic(&mut vm.registry, SOURCE, BOOTSTRAP_OFFSET, None, false)
}

/** Whether a code site belongs to this bootstrap; its functions implement platform interfaces
(Observable, the text streams, PerformanceObserver...), so they print as native code, as the
interfaces do in Chrome, rather than showing their JavaScript source to detection scripts. */
pub(in crate::v9) fn is_bootstrap_site(site: u32) -> bool {
    let offset = (site / crate::v9::syntax::SITE_TAGS) as usize;
    (BOOTSTRAP_OFFSET..BOOTSTRAP_OFFSET + SOURCE.len()).contains(&offset)
}

/** Run the bootstrap's top level once, for a fresh page, from the code the session compiled. A failure here would break every page, so the caller reports it as an engine error rather than a script error. */
pub(in crate::v9) fn run(vm: &mut Vm, code: Arc<Code>) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    let installed = vm.run_script(code).and_then(|install| {
        // The media part evaluates to its installer; only it receives the private host object.
        let host = JsValue::Object(crate::v9::vm::realm::realm().ids.media_host);
        vm.call(install, JsValue::Undefined, vec![host]).map(|_| ())
    });
    match installed.and_then(|()| vm.drain_jobs()) {
        Ok(()) => Ok(()),
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
