/** The Wasm exception surface validates payloads and exposes stacks with native brands. */
use crate::common;

/** Missing payload length differs from a wrong payload count, with one length getter read. */
#[test]
fn exception_payload_checks_length_once() {
    let source = "let n=0,r='';const t=new WebAssembly.Tag({parameters:['i32']});const v={get length(){n++;return 1},0:42};let x=new WebAssembly.Exception(t,v);r=[n,x.getArg(t,0)];try{new WebAssembly.Exception(t,{})}catch(e){r.push(e instanceof TypeError,e.message.includes('has no length'))}r=r.join()";
    assert_eq!(common::eval(source, "r"), "1,42,true,true");
}

/** Untraced exceptions have an undefined stack and the getter rejects an unbranded receiver. */
#[test]
fn exception_stack_has_a_branded_undefined_default() {
    let source = "const t=new WebAssembly.Tag({parameters:[]}),e=new WebAssembly.Exception(t,[]);let r=['stack' in e,e.stack===undefined];try{WebAssembly.Exception.prototype.stack}catch(x){r.push(x instanceof TypeError)}const d=Object.getOwnPropertyDescriptor(WebAssembly,'JSTag');r.push(d.writable,d.enumerable,d.configurable);r=r.join()";
    assert_eq!(common::eval(source, "r"), "true,true,true,false,false,true");
}

/** Tag export bounds diagnostics point to the actual encoded index. */
#[test]
fn tag_export_bounds_report_the_index_offset() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,13,3,1,0,0,7,10,1,6,101,120,95,111,111,98,4,1]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('tag index 1 out of bounds (1 entry) @+30')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}
