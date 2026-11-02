/* The shell's dedicated Wasm testing opcode is a gated, ordinary no-op. */
#![cfg(feature = "conformance")]
use crate::common;

/** Enabling the flag accepts the NOP without altering the operand stack or result. */
#[test]
fn wasm_testing_nop_requires_explicit_flag() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,5,1,1,102,0,0,10,7,1,5,0,22,65,42,11]);let r=String(WebAssembly.validate(b));if(r==='true')r+=','+new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "false");
    environment.conformance_wasm_testing_opcode = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true,42");
}
