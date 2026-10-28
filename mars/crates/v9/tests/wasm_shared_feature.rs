/* Shared-everything Wasm syntax is independently gated from ordinary shared memories. */
#![cfg(feature = "conformance")]
mod common;

/** The proposal flag enables shared GC types while ordinary shared memory remains available. */
#[test]
fn shared_gc_types_follow_shell_feature_state() {
    let source = "let h=[0,97,115,109,1,0,0,0],gc=new Uint8Array([...h,1,4,1,101,95,0]),memory=new Uint8Array([...h,5,4,1,3,0,1]),r=[WebAssembly.validate(gc),WebAssembly.validate(memory)].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(disabled.failed.is_empty(), "{:?}", common::failure(&disabled));
    assert_eq!(common::global(&disabled, "r"), "false,true");
    environment.conformance_wasm_shared = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true,true");
}

/** A shared ref.test preserves its target qualifier and accepts shared anyref operands. */
#[test]
fn shared_reference_casts_preserve_their_hierarchy() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,11,2,101,95,0,96,1,99,101,110,1,127,3,2,1,1,7,5,1,1,102,0,0,10,9,1,7,0,32,0,251,20,0,11]),x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports,r=[x.f(null),x.f(1.5),x.f('shared')].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_shared = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "0,0,0");
}

/** Shared function, exception, and continuation references are not part of this proposal. */
#[test]
fn unsupported_shared_reference_hierarchies_are_rejected() {
    let source = "let h=[0,97,115,109,1,0,0,0],r=[112,115,105,116,104,117].map(t=>WebAssembly.validate(new Uint8Array([...h,1,7,1,96,1,99,101,t,0]))).join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_shared = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "false,false,false,false,false,false");
}

/** A shared descriptor remains the same native arena reference after allocation and retrieval. */
#[test]
fn shared_custom_descriptor_retains_identity() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,21,2,78,2,80,0,101,77,1,95,0,80,0,101,76,0,95,0,96,0,1,127,3,2,1,2,7,7,1,3,114,117,110,0,0,10,22,1,20,1,1,100,98,1,251,1,1,34,0,251,32,0,251,34,0,32,0,211,11,0,13,4,110,97,109,101,1,6,1,0,3,114,117,110]),r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.run()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_shared = true;
    environment.conformance_wasm_custom_descriptors = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "1");
}

/** Shared descriptor edges and wrapper identity survive saved-state transport. */
#[test]
fn shared_custom_descriptor_survives_saved_state() {
    let sources = vec!["const b=new Uint8Array([0,97,115,109,1,0,0,0,1,38,4,78,2,80,0,101,77,1,95,0,80,0,101,76,0,95,0,96,0,1,100,98,1,96,1,100,98,1,1,100,0,96,1,100,0,1,100,1,3,4,3,2,3,4,7,21,3,4,100,101,115,99,0,0,4,109,97,107,101,0,1,3,103,101,116,0,2,10,23,3,5,0,251,1,1,11,7,0,32,0,251,32,0,11,7,0,32,0,251,34,0,11,0,25,4,110,97,109,101,1,18,3,0,4,100,101,115,99,1,4,109,97,107,101,2,3,103,101,116]);const x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;const d=x.desc(),o=x.make(d);document.getElementById('go').addEventListener('click',()=>console.log(x.get(o)===d));".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_wasm_shared = true;
    environment.conformance_wasm_custom_descriptors = true;
    let input = common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true"]);
}

/** A JS shared arena created first is reused for subsequent Wasm GC nodes. */
#[test]
fn shared_js_arena_is_reused_when_wasm_allocates_later() {
    let source = "const a=SharedArray(1);Atomics.store(a,0,0);let b=new Uint8Array([0,97,115,109,1,0,0,0,1,38,4,78,2,80,0,101,77,1,95,0,80,0,101,76,0,95,0,96,0,1,100,98,1,96,1,100,98,1,1,100,0,96,1,100,0,1,100,1,3,4,3,2,3,4,7,21,3,4,100,101,115,99,0,0,4,109,97,107,101,0,1,3,103,101,116,0,2,10,23,3,5,0,251,1,1,11,7,0,32,0,251,32,0,11,7,0,32,0,251,34,0,11,0,25,4,110,97,109,101,1,18,3,0,4,100,101,115,99,1,4,109,97,107,101,2,3,103,101,116]);const x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports,d=x.desc();a[0]=d;let r=a[0]===d";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_wasm_shared = true;
    environment.conformance_wasm_custom_descriptors = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true");
}

/** Recursive groups cannot refer to later groups or declare a later supertype. */
#[test]
fn invalid_forward_type_and_supertype_references_are_rejected() {
    let source = "const a=new Uint8Array([0,97,115,109,1,0,0,0,1,16,1,78,2,80,1,1,94,100,0,0,80,0,94,100,0,0]),b=new Uint8Array([0,97,115,109,1,0,0,0,1,18,2,78,1,80,0,94,100,1,0,78,1,80,1,0,94,100,1,0]),r=[WebAssembly.validate(a),WebAssembly.validate(b)].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_shared = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "false,false");
}
