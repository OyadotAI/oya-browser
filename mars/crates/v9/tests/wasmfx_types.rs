/* WasmFX continuation types retain actual signature identity behind the proposal flag. */
#![cfg(feature = "conformance")]
use crate::common;

/** Continuation types are gated and must refer to an actual function signature. */
#[test]
fn continuation_types_require_a_function_signature_and_feature() {
    let source = "let m=n=>new Uint8Array([0,97,115,109,1,0,0,0,1,6,2,96,0,0,93,n]);let r=[WebAssembly.validate(m(0)),WebAssembly.validate(m(1)),WebAssembly.validate(m(2))].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "false,false,false");
    environment.conformance_wasm_wasmfx = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true,false,false");
}

/** A fresh continuation resumes its real Wasm function once and returns its typed result. */
#[test]
fn fresh_continuation_executes_its_wasm_function() {
    let source = "let sec=(i,p)=>[i,p.length,...p],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[2,96,0,1,127,93,0]),...sec(3,[2,0,0]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,4,0,65,42,11,9,0,210,0,224,1,227,1,0,11])]);let r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "42");
}

/** Consumed continuations stay consumed even when retained in a traced Wasm global. */
#[test]
fn continuation_is_one_shot_through_a_global() {
    let source = "let sec=(i,p)=>[i,p.length,...p],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,1,127,93,0,96,0,0]),...sec(3,[3,0,2,0]),...sec(6,[1,99,1,1,208,1,11]),...sec(7,[3,1,102,0,0,1,99,0,1,1,114,0,2]),...sec(10,[3,4,0,65,42,11,8,0,210,0,224,1,36,0,11,7,0,35,0,227,1,0,11])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;x.c();let first=x.r(),trapped=false;try{x.r()}catch(e){trapped=e instanceof WebAssembly.RuntimeError}let r=[first,trapped].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "42,true");
}

/** Traced continuations preserve their function and one-shot state across VM snapshots. */
#[test]
fn continuation_survives_state_restoration() {
    let source = "let sec=(i,p)=>[i,p.length,...p],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,1,127,93,0,96,0,0]),...sec(3,[3,0,2,0]),...sec(6,[1,99,1,1,208,1,11]),...sec(7,[3,1,102,0,0,1,99,0,1,1,114,0,2]),...sec(10,[3,4,0,65,42,11,8,0,210,0,224,1,36,0,11,7,0,35,0,227,1,0,11])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;x.c();let r='pending';setTimeout(()=>{let first=x.r(),trapped=false;try{x.r()}catch(e){trapped=e instanceof WebAssembly.RuntimeError}r=[first,trapped].join()},1)";
    let sources = [source.to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let result =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 2.0, 1).unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "42,true");
}

/** Binding consumes a continuation and prepends its arguments to the later resume. */
#[test]
fn continuation_bind_retains_real_argument_prefix() {
    let source = "let sec=(i,p)=>[i,p.length,...p],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[5,96,2,127,127,1,127,93,0,96,1,127,1,127,93,2,96,0,1,127]),...sec(3,[2,0,4]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,7,0,32,0,32,1,107,11,16,0,65,3,65,4,210,0,224,1,225,1,3,227,3,0,11])]);let r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "1");
}

/** Suspension without an effect handler traps and consumes the running continuation. */
#[test]
fn unhandled_suspend_traps_with_a_typed_effect_tag() {
    let source = "let sec=(i,p)=>[i,p.length,...p],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,1,127,1,127,96,0,1,127,93,1]),...sec(3,[2,1,1]),...sec(13,[1,0,0]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,6,0,65,4,226,0,11,9,0,210,0,224,2,227,2,0,11])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;let r=false;try{x.r()}catch(e){r=e instanceof WebAssembly.RuntimeError}";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true");
}

/** A handled effect returns a live continuation that accepts tag results on resumption. */
#[test]
fn effect_handler_resumes_the_actual_suspended_frame() {
    let source = "let sec=(i,p)=>[i,p.length,...p],fn0=[0,65,7,226,0,65,1,106,11],fn1=[1,1,99,3,2,4,210,0,224,2,227,2,1,0,0,0,0,11,33,0,26,65,10,32,0,227,3,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[6,96,0,1,127,96,1,127,1,127,93,0,93,1,96,0,2,127,100,3,96,0,1,127]),...sec(3,[2,0,5]),...sec(13,[1,0,1]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,fn0.length,...fn0,fn1.length,...fn1])]);let r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "11");
}

/** A suspended child and its caller resume without replaying either function. */
#[test]
fn effect_suspension_preserves_nested_wasm_frames() {
    let source = "let sec=(i,p)=>[i,p.length,...p],child=[0,65,7,226,0,65,1,106,11],parent=[0,16,0,65,2,106,11],run=[1,1,99,3,2,4,210,1,224,2,227,2,1,0,0,0,0,11,33,0,26,65,10,32,0,227,3,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[6,96,0,1,127,96,1,127,1,127,93,0,93,1,96,0,2,127,100,3,96,0,1,127]),...sec(3,[3,0,0,5]),...sec(13,[1,0,1]),...sec(7,[2,1,102,0,1,1,114,0,2]),...sec(10,[3,child.length,...child,parent.length,...parent,run.length,...run])]);let r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "13");
}

/** A handled suspension restores its actual activation stack from serialized VM state. */
#[test]
fn suspended_effect_survives_state_restoration() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,65,7,226,0,65,1,106,11],c=[0,2,4,210,0,224,2,227,2,1,0,0,0,0,11,36,0,26,11],g=[0,65,10,35,0,227,3,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[7,96,0,1,127,96,1,127,1,127,93,0,93,1,96,0,2,127,100,3,96,0,0,96,0,1,127]),...sec(3,[3,0,5,6]),...sec(13,[1,0,1]),...sec(6,[1,99,3,1,208,3,11]),...sec(7,[3,1,102,0,0,1,99,0,1,1,114,0,2]),...sec(10,[3,f.length,...f,c.length,...c,g.length,...g])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;x.c();let r=0;setTimeout(()=>r=x.r(),1)";
    let sources = [source.to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    environment.conformance_wasm_wasmfx = false;
    let result =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 2.0, 1).unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "11");
}

/** An effect can suspend inside a try_table and resume the same structured scope. */
#[test]
fn effect_suspends_inside_try_table() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[1,1,127,31,64,0,65,7,226,0,33,0,11,32,0,65,1,106,11],run=[1,1,99,3,2,4,210,0,224,2,227,2,1,0,0,0,0,11,33,0,26,65,10,32,0,227,3,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[6,96,0,1,127,96,1,127,1,127,93,0,93,1,96,0,2,127,100,3,96,0,1,127]),...sec(3,[2,0,5]),...sec(13,[1,0,1]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,f.length,...f,run.length,...run])]);let r=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "11");
}

/** Rejected JSPI calls unwind into a retained WasmFX try_table and can resolve normally. */
#[test]
fn jspi_rejection_is_caught_in_a_wasmfx_frame() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,2,64,31,64,1,2,0,16,0,11,0,11,11],main=[0,210,1,224,1,227,1,0,65,7,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,0,93,0,96,0,1,127]),...sec(2,[1,1,109,1,102,0,0]),...sec(3,[2,0,2]),...sec(7,[2,1,102,0,1,1,114,0,2]),...sec(10,[2,f.length,...f,main.length,...main])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b),{m:{f:new WebAssembly.Suspending(()=>Promise.reject('boom'))}}).exports,r='pending';WebAssembly.promising(x.r)().then(v=>r=v,e=>r='rejected')";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "7");
}

/** JSPI can await after handling and resuming a saved WasmFX effect frame. */
#[test]
fn jspi_runs_saved_effect_continuation() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,65,7,226,0,65,1,106,11],run=[1,1,99,3,2,4,210,1,224,2,227,2,1,0,0,0,0,11,33,0,26,65,10,32,0,227,3,0,16,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[7,96,0,1,127,96,1,127,1,127,93,0,93,1,96,0,2,127,100,3,96,0,1,127,96,0,0]),...sec(2,[1,1,109,1,102,0,6]),...sec(3,[2,0,5]),...sec(13,[1,0,1]),...sec(7,[2,1,102,0,1,1,114,0,2]),...sec(10,[2,f.length,...f,run.length,...run])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b),{m:{f:new WebAssembly.Suspending(()=>Promise.resolve())}}).exports,r='pending';WebAssembly.promising(x.r)().then(v=>r=v,e=>r='rejected')";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "11");
}

/** A promising wrapper preserves a nested WasmFX switch handler and target continuation. */
#[test]
fn jspi_runs_nested_switch_handler() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,11],s=[0,210,0,224,3,230,3,0,11],r=[0,210,1,224,1,227,1,1,1,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[4,96,0,0,93,0,96,1,100,1,0,93,2]),...sec(3,[3,2,0,0]),...sec(13,[1,0,0]),...sec(7,[3,1,102,0,0,1,115,0,1,1,114,0,2]),...sec(10,[3,f.length,...f,s.length,...s,r.length,...r])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports,result='pending';WebAssembly.promising(x.r)().then(()=>result='done',()=>result='rejected')";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "result"), "done");
}

/** A thrown exception resumes into an active legacy catch without losing the suspended scope. */
#[test]
fn resume_throw_enters_legacy_catch() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,6,64,226,0,25,1,11,65,9,11],r=[0,2,100,2,210,0,224,2,227,2,1,0,0,0,0,11,228,2,1,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,1,127,96,0,0,93,0]),...sec(3,[2,0,0]),...sec(13,[2,0,1,0,1]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,f.length,...f,r.length,...r])]);let result=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    environment.conformance_wasm_allow_mixed_eh = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "result"), "9");
}

/** A legacy catch can suspend, restore its caught exception, and rethrow it after resume. */
#[test]
fn legacy_catch_retains_exception_across_suspension() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,6,64,226,0,25,226,0,9,0,11,65,9,11],r=[0,2,64,31,64,1,0,1,0,2,100,2,2,100,2,210,0,224,2,227,2,1,0,0,0,0,11,228,2,1,1,0,0,0,0,11,227,2,0,26,0,11,11,65,9,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,1,127,96,0,0,93,0]),...sec(3,[2,0,0]),...sec(13,[2,0,1,0,1]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,f.length,...f,r.length,...r])]);let result=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    environment.conformance_wasm_allow_mixed_eh = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "result"), "9");
}

/** Delegated exceptions retain their destination while their inner scope is suspended. */
#[test]
fn suspended_delegate_routes_to_outer_catch() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,6,64,6,64,226,0,24,0,25,1,11,65,9,11],r=[0,2,100,2,210,0,224,2,227,2,1,0,0,0,0,11,228,2,1,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,1,127,96,0,0,93,0]),...sec(3,[2,0,0]),...sec(13,[2,0,1,0,1]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,f.length,...f,r.length,...r])]);let result=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.r()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    environment.conformance_wasm_allow_mixed_eh = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "result"), "9");
}

/** A promising wrapper injects into a saved effect frame and observes the real catch result. */
#[test]
fn jspi_resumes_a_saved_continuation_with_an_exception() {
    let source = "let sec=(i,p)=>[i,p.length,...p],f=[0,6,64,226,0,25,1,11,65,9,11],r=[0,2,100,2,210,0,224,2,227,2,1,0,0,0,0,11,228,2,1,0,11],b=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,96,0,1,127,96,0,0,93,0]),...sec(3,[2,0,0]),...sec(13,[2,0,1,0,1]),...sec(7,[2,1,102,0,0,1,114,0,1]),...sec(10,[2,f.length,...f,r.length,...r])]);let x=new WebAssembly.Instance(new WebAssembly.Module(b)).exports,result='pending';WebAssembly.promising(x.r)().then(v=>result=v,()=>result='rejected')";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    environment.conformance_wasm_allow_mixed_eh = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "result"), "9");
}

/** The null-continuation bottom type can initialize a nullable exact continuation global. */
#[test]
fn null_continuation_is_subtype_of_nullable_exact_continuation() {
    let source = "let b=new Uint8Array([0,97,115,109,1,0,0,0,1,6,2,96,0,0,93,0,6,8,1,99,98,1,0,208,117,11]);let r=WebAssembly.validate(b)";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_wasmfx = true;
    environment.conformance_wasm_custom_descriptors = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true");
}
