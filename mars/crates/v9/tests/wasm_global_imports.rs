/** Wasm globals distinguish JavaScript constructor defaults from strict binary import types. */
use crate::common;

/** Undefined numeric initializers use zero while omitted externref remains undefined. */
#[test]
fn javascript_global_defaults_follow_the_declared_type() {
    assert_eq!(
        common::eval(
            "let r=[new WebAssembly.Global({value:'i32'},undefined).value,new WebAssembly.Global({value:'f32'},undefined).value,new WebAssembly.Global({value:'f64'},undefined).value,String(new WebAssembly.Global({value:'i64'},undefined).value),String(new WebAssembly.Global({value:'externref'}).value)].join()",
            "r"
        ),
        "0,0,0,0,undefined"
    );
}

/** Primitive numeric imports do not invoke object conversion hooks and reject as link errors. */
#[test]
fn global_import_rejects_objects_without_coercion() {
    let source = "const bytes=new Uint8Array([0,97,115,109,1,0,0,0,2,8,1,1,109,1,103,3,127,0]);let called=false,r='';const v={valueOf(){called=true;return 1}};try{new WebAssembly.Instance(new WebAssembly.Module(bytes),{m:{g:v}})}catch(e){r=(e instanceof WebAssembly.LinkError)+','+called}";
    assert_eq!(common::eval(source, "r"), "true,false");
}

/** Defined immutable globals may initialize later globals but cannot be referenced forward. */
#[test]
fn constant_global_reads_require_an_earlier_immutable_global() {
    let source = "const header=[0,97,115,109,1,0,0,0];const exportSection=[7,5,1,1,103,3,1];function module(first,second){return new Uint8Array([...header,6,11,2,127,first,65,42,11,127,0,35,second,11,...exportSection])}let r=String(new WebAssembly.Instance(new WebAssembly.Module(module(0,0))).exports.g.value);for(const bytes of [module(1,0),module(0,1)]){try{new WebAssembly.Module(bytes)}catch(e){r+=','+(e instanceof WebAssembly.CompileError)}}";
    assert_eq!(common::eval(source, "r"), "42,true,true");
}

/** Invalid constant function indices are rejected before decoding later invalid opcodes. */
#[test]
fn constant_function_index_error_precedes_later_invalid_instruction() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,6,7,1,112,0,210,1,0,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('function index #1 is out of bounds')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}
