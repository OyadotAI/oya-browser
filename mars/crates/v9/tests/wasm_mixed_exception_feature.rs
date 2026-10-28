/* The d8 testing flag controls modules mixing legacy and new exception constructs. */
#![cfg(feature = "conformance")]
mod common;

/** The same mixed module is invalid by default and can execute under the testing flag. */
#[test]
fn mixed_exception_modes_follow_shell_feature_state() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,7,5,1,1,102,0,0,10,11,1,9,0,6,64,11,31,64,0,11,11]);let r=WebAssembly.validate(b)?(new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f(),'valid'):'invalid'";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "invalid");
    environment.conformance_wasm_allow_mixed_eh = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "valid");
}
