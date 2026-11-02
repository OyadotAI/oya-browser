/* The shell feature flag controls custom descriptor binary syntax. */
#![cfg(feature = "conformance")]
use crate::common;

/** A paired descriptor validates only when the proposal is enabled. */
#[test]
fn descriptors_follow_shell_feature_state() {
    let source = "let r=WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,11,1,78,2,77,1,95,0,76,0,95,0]))";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "false");
    environment.conformance_wasm_custom_descriptors = true;
    let enabled = common::load_with(source, &environment);
    assert_eq!(common::global(&enabled, "r"), "true");
}

/** Exact imports reject a proper function subtype while ordinary imports accept it. */
#[test]
fn exact_function_import_rejects_proper_subtypes() {
    let source = "const h=[0,97,115,109,1,0,0,0],sec=(id,p)=>[id,p.length,...p],t=sec(1,[1,78,2,80,0,96,0,0,79,1,0,96,0,0]),producer=new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array([...h,...t,...sec(3,[1,1]),...sec(7,[1,1,102,0,0]),...sec(10,[1,2,0,11])]))),build=k=>new WebAssembly.Module(new Uint8Array([...h,...t,...sec(2,[1,1,109,1,102,k,0])]));let ordinary=false,exact=false;try{new WebAssembly.Instance(build(0),{m:producer.exports});ordinary=true}catch(e){}try{new WebAssembly.Instance(build(32),{m:producer.exports})}catch(e){exact=e instanceof WebAssembly.LinkError}let r=[ordinary,exact].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_custom_descriptors = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true,true");
}
