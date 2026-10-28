/* Compact Wasm imports expand into real ordered declarations under an explicit feature gate. */
#![cfg(feature = "conformance")]
mod common;

/** Both compact forms preserve import names, types and JavaScript lookup order. */
#[test]
fn compact_imports_are_gated_and_expand_in_order() {
    let source = "let x=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,2,14,1,1,109,0,127,2,1,97,0,0,1,98,0,0]),y=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,2,12,1,1,109,0,126,0,0,2,1,97,1,98]),r=[WebAssembly.validate(x),WebAssembly.validate(y)].join();if(r==='true,true'){const m=new WebAssembly.Module(y),names=WebAssembly.Module.imports(m).map(i=>i.module+'.'+i.name+':'+i.kind).join();let seen=[];new WebAssembly.Instance(m,{get m(){seen.push('module');return {get a(){seen.push('a');return ()=>{}},get b(){seen.push('b');return ()=>{}}}}});r+=','+names+','+seen.join('-')}";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(disabled.failed.is_empty(), "{:?}", common::failure(&disabled));
    assert_eq!(common::global(&disabled, "r"), "false,false");
    environment.conformance_wasm_compact_imports = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true,true,m.a:function,m.b:function,module-a-module-b");
}

/** Truncated grouped names are rejected before a partially expanded module escapes. */
#[test]
fn compact_imports_reject_truncated_groups() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,2,11,1,1,109,0,126,0,0,2,1,97]);let r=WebAssembly.validate(b)";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_compact_imports = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "false");
}
