/* JavaScript prototype imports have an independent shell proposal gate. */
#![cfg(feature = "conformance")]
use crate::common;

/** Prototype imports undergo builtin signature validation only when interop is enabled. */
#[test]
fn prototype_imports_follow_shell_feature_state() {
    let source = "let name=s=>[s.length,...Array.from(s,c=>c.charCodeAt(0))],sec=(i,p)=>[i,p.length,...p],bytes=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[1,96,0,0]),...sec(2,[1,...name('wasm:js-prototypes'),...name('configureAll'),0,0])]);let r=WebAssembly.validate(bytes,{builtins:['js-prototypes']})";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_custom_descriptors = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "true");
    environment.conformance_wasm_js_interop = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "false");
}
