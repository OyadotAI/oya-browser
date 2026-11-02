/* Wasm Math imports use genuine builtin identity and preserve ordinary JS fallback. */
#![cfg(feature = "conformance")]
use crate::common;

/** Builtin and authored imports agree with Math semantics with optimization enabled or disabled. */
#[test]
fn wasm_math_imports_preserve_numbers_and_authored_callbacks() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,11,2,96,1,124,1,124,96,1,124,1,124,2,7,1,1,109,1,102,0,0,3,2,1,1,7,7,1,3,114,117,110,0,1,10,8,1,6,0,32,0,16,0,11,0,13,4,110,97,109,101,1,6,1,1,3,114,117,110]),m=new WebAssembly.Module(b),f=new WebAssembly.Instance(m,{m:{f:Math.sin}}).exports.run;let calls=0,g=new WebAssembly.Instance(m,{m:{f(x){calls++;return x+5}}}).exports.run;let r=[Object.is(f(-0),-0),Number.isNaN(f(Infinity)),f(1)===Math.sin(1),g(2),calls].join()";
    for enabled in [false, true] {
        let mut environment = common::environment();
        environment.conformance_d8 = true;
        environment.conformance_wasm_math_intrinsics = Some(enabled);
        let result = common::load_with(source, &environment);
        assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
        assert_eq!(common::global(&result, "r"), "true,true,true,7,1");
    }
}
