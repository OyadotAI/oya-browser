/* The explicitly gated V8 cast experiment forwards values while retaining static hierarchy checks. */
#![cfg(feature = "conformance")]
use crate::common;

/** An enabled unchecked cast forwards null, while the disabled opcode and wrong hierarchy fail validation. */
#[test]
fn unchecked_cast_is_gated_and_preserves_hierarchy_validation() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,9,2,80,0,95,0,96,0,1,127,3,2,1,1,7,5,1,1,102,0,0,10,10,1,8,0,208,110,251,76,0,209,11,0,11,4,110,97,109,101,1,4,1,0,1,102]),wrong=b.slice();wrong[36]=111;let r=String(WebAssembly.validate(b));if(r==='true')r+=','+new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f()+','+WebAssembly.validate(wrong)";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "false");
    environment.conformance_wasm_ref_cast_nop = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true,1,false");
}
