/* Conformance page limits bound real Wasm memory creation and growth. */
#![cfg(feature = "conformance")]
mod common;

/** A lowered host limit preserves declared maximum while bounding allocation and growth. */
#[test]
fn shell_memory_page_limit_bounds_creation_and_growth() {
    let source = "let m=new WebAssembly.Memory({initial:1,maximum:4}),r=[m.grow(1),m.buffer.byteLength];try{m.grow(1)}catch(e){r.push(e instanceof RangeError)}try{new WebAssembly.Memory({initial:3})}catch(e){r.push(e instanceof RangeError)}r=r.join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_max_memory_pages = Some(2);
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "1,131072,true,true");
}

/** Growable shared wrappers retain their declared maximum but cannot exceed the host cap. */
#[test]
fn shell_memory_page_limit_bounds_shared_buffer_growth() {
    let source = "let m=new WebAssembly.Memory({initial:1,maximum:4,shared:true}),b=m.toResizableBuffer(),r=[b.maxByteLength];b.grow(131072);try{b.grow(196608)}catch(e){r.push(e instanceof RangeError)}r.push(b.byteLength);r=r.join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_max_memory_pages = Some(2);
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "262144,true,131072");
}
