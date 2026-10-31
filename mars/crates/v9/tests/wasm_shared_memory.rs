/** Shared Wasm memory exposes frozen wrappers and stable identity for no-op growth. */
use crate::common;

/** Fixed and resizable shared views stay frozen without changing the shared backing. */
#[test]
fn shared_memory_wrappers_are_frozen_and_zero_growth_is_identity_preserving() {
    let source = "const m=new WebAssembly.Memory({initial:1,maximum:2,shared:true});const fixed=m.buffer;let r=[Object.isFrozen(fixed),m.grow(0)===1,m.buffer===fixed];const growable=m.toResizableBuffer();r.push(Object.isFrozen(growable),m.grow(0)===1,m.buffer===growable);r=r.join()";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,true");
}
