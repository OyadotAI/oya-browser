/** Bulk-memory validation reports reserved numeric opcodes as malformed binaries. */
mod common;

/** Unknown prefixed opcodes throw CompileError without marking a supported instruction unavailable. */
#[test]
fn unknown_numeric_opcode_is_invalid() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,7,1,5,0,252,144,15,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('invalid numeric opcode: 0xfc790')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** An unknown atomic opcode reports the decoded prefixed opcode. */
#[test]
fn unknown_atomic_opcode_is_invalid() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,7,1,5,0,254,144,15,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('invalid atomic opcode: 0xfe790')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}
