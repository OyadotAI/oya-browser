/* Native Wasm object intrinsics expose genuine, typed GC values. */
#![cfg(feature = "conformance")]
use crate::common;

/** Struct and array helpers create distinct canonical types with the pinned i64 payload. */
#[test]
fn native_wasm_objects_have_real_types_and_payload() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,24,4,80,0,95,1,126,0,80,0,94,126,0,96,1,100,0,1,126,96,1,100,1,1,126,3,3,2,2,3,7,11,2,2,114,115,0,0,2,114,97,0,1,10,20,2,8,0,32,0,251,2,0,0,11,9,0,32,0,65,0,251,11,1,11,0,16,4,110,97,109,101,1,9,2,0,2,114,115,1,2,114,97]);const x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports,s=%WasmStruct(),a=%WasmArray(),v=0x7aadf00dbaadf00dn;let bad=false;try{x.rs(a)}catch(e){bad=e instanceof TypeError}let r=[x.rs(s)===v,x.ra(a)===v,bad].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true,true,true");
}
