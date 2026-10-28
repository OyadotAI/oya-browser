/* The shell feature flag controls acquire and release atomic encodings. */
#![cfg(feature = "conformance")]
mod common;

/** Atomic fences with an acquire/release order require the proposal flag. */
#[test]
fn atomic_fence_order_follows_shell_feature_state() {
    let source = "let module = order => new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,7,1,5,0,254,3,order,11]); let r=[WebAssembly.validate(module(0)),WebAssembly.validate(module(1))].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "true,false");
    environment.conformance_wasm_acquire_release = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true,true");
}
