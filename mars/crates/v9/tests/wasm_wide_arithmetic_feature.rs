/* Wide arithmetic opcodes follow the shell's proposal feature state. */
#![cfg(feature = "conformance")]
use crate::common;

/** A valid wide multiply is rejected without the flag and executes when enabled. */
#[test]
fn wide_multiply_follows_feature_state() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,6,1,96,0,2,126,126,3,2,1,0,7,5,1,1,102,0,0,10,10,1,8,0,66,3,66,4,252,22,11]);let valid=WebAssembly.validate(b),r=valid?new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f().join():'disabled'";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "disabled");
    environment.conformance_wasm_wide_arithmetic = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "12,0");
}
