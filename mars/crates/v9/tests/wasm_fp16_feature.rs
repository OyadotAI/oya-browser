/* Binary16 memory operations follow the shell flag and real Wasm bounds. */
#![cfg(feature = "conformance")]
mod common;

/** Half-float loads and stores retain IEEE values and trap on invalid addresses. */
#[test]
fn binary16_memory_follows_feature_state_and_bounds() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,11,2,96,1,127,1,125,96,2,127,125,0,3,3,2,0,1,5,3,1,0,1,7,13,3,1,109,2,0,1,108,0,0,1,115,0,1,10,21,2,8,0,32,0,252,48,1,0,11,10,0,32,0,32,1,252,49,1,0,11]);let valid=WebAssembly.validate(b),r='disabled';if(valid){let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;x.s(0,1.5);let trap=false;try{x.l(65535)}catch(e){trap=e instanceof WebAssembly.RuntimeError}r=[x.l(0),new Uint8Array(x.m.buffer)[1],trap].join()}";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "disabled");
    environment.conformance_wasm_fp16 = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "1.5,62,true");
}

/** SIMD binary16 instructions use the same feature gate as scalar memory operations. */
#[test]
fn binary16_simd_follows_feature_state() {
    let source = "let r=WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,13,1,11,0,67,0,0,0,0,253,160,2,26,11]))";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "false");
    environment.conformance_wasm_fp16 = true;
    let enabled = common::load_with(source, &environment);
    assert_eq!(common::global(&enabled, "r"), "true");
}
