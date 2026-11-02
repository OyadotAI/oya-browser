/* The conformance host serializes real modules into an owned envelope with checked source identity. */
#![cfg(feature = "conformance")]
use crate::common;

/** Each test runs the public shell in an isolated page without native IO or foreign engines. */
fn run(source: &str) -> String {
    run_with_limit(source, None)
}
/** Explicit host caps only lower the same isolated page's existing compiler byte budget. */
fn run_with_limit(source: &str, limit: Option<usize>) -> String {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_max_module_bytes = limit;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    common::global(&execution, "r")
}
/** The reconstituted module executes copied source even after both caller-owned buffers mutate. */
#[test]
fn serialized_modules_reconstruct_validated_owned_code() {
    assert_eq!(
        run(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,5,1,1,102,0,0,10,6,1,4,0,65,42,11]);const module=new WebAssembly.Module(bytes);const saved=d8.wasm.serializeModule(module);const clone=d8.wasm.deserializeModule(saved,bytes);bytes.fill(0);new Uint8Array(saved).fill(0);let r=new WebAssembly.Instance(clone).exports.f();"
        ),
        "42"
    );
}
/** Mismatching wire bytes reject before construction rather than executing a different module. */
#[test]
fn serialization_rejects_manipulated_wire_bytes() {
    assert_eq!(
        run(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0]);const saved=d8.wasm.serializeModule(new WebAssembly.Module(bytes));bytes[0]=1;let r=false;try{d8.wasm.deserializeModule(saved,bytes)}catch(e){r=e instanceof Error && /Trying to deserialize manipulated bytes/.test(e.message)}"
        ),
        "true"
    );
}
/** Deserialization option getters can detach inputs; their resulting state is checked after the getter. */
#[test]
fn serialization_checks_detachment_after_option_getters() {
    assert_eq!(
        run(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0]);const saved=d8.wasm.serializeModule(new WebAssembly.Module(bytes));let r=false;try{d8.wasm.deserializeModule(saved,bytes,{get builtins(){saved.transfer();return []}})}catch(e){r=e instanceof TypeError}"
        ),
        "true"
    );
}

/** Native byte streaming compiles the actual fulfilled source before executing its real exported code. */
#[test]
fn streaming_compilation_uses_real_promise_jobs_and_source_bytes() {
    assert_eq!(
        run(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,5,1,1,102,0,0,10,6,1,4,0,65,42,11]);let log='',r='';const source={get then(){log+='t';return resolve=>{log+='c';resolve(bytes)}}};const p=WebAssembly.compileStreaming(source,{get builtins(){log+='o';return []}});log+='s';p.then(module=>{r=log+','+new WebAssembly.Instance(module).exports.f()});"
        ),
        "otsc,42"
    );
}

/** Streaming instantiation links and executes the same module while preserving its specified pair result. */
#[test]
fn streaming_instantiation_returns_actual_module_and_instance() {
    assert_eq!(
        run(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,5,1,1,102,0,0,10,6,1,4,0,65,42,11]);let r='';WebAssembly.instantiateStreaming(Promise.resolve(bytes)).then(value=>{r=(value.module instanceof WebAssembly.Module)+','+(value.instance instanceof WebAssembly.Instance)+','+value.instance.exports.f()});"
        ),
        "true,true,42"
    );
}

/** An original shell flag can tighten module input limits without changing the global quota ceiling. */
#[test]
fn explicit_module_cap_is_enforced_by_streaming_and_validate() {
    assert_eq!(
        run_with_limit(
            "const bytes=new Uint8Array(101);let r=WebAssembly.validate(bytes)+',';WebAssembly.compileStreaming(Promise.resolve(bytes)).catch(e=>{r+=(e instanceof WebAssembly.CompileError)+','+e.message});",
            Some(100)
        ),
        "false,true,WebAssembly.compileStreaming(): size > maximum module size (100): 101 @+0"
    );
}

/** Compilation promises reject with the original source rejection value without rewriting authored errors. */
#[test]
fn streaming_source_rejections_preserve_error_identity() {
    assert_eq!(
        run(
            "const sentinel={};let r=false;WebAssembly.compileStreaming(Promise.reject(sentinel)).catch(e=>{r=e===sentinel});"
        ),
        "true"
    );
}

/** The shell exposes streaming only when its embedder callback is installed. */
#[test]
fn streaming_methods_follow_shell_callback_configuration() {
    let source = "let r=[typeof WebAssembly.compileStreaming,typeof WebAssembly.instantiateStreaming].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_test_streaming = Some(false);
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "undefined,undefined");
    environment.conformance_wasm_test_streaming = Some(true);
    let enabled = common::load_with(source, &environment);
    assert_eq!(common::global(&enabled, "r"), "function,function");
}

/** Original shell flags may reduce capacity without changing table brands or runtime return conventions. */
#[test]
fn explicit_table_capacity_limits_actual_growth() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_max_table_entries = Some(3);
    let execution = common::load_with(
        "const table=new WebAssembly.Table({element:'externref',initial:1,maximum:10});let r=String(table.grow(2));try{table.grow(1)}catch(e){r+=','+(e instanceof RangeError)}r+=','+table.length;",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "1,true,3");
}

/** A larger shell flag cannot raise the bounded logical table ceiling. */
#[test]
fn explicit_table_capacity_cannot_raise_native_limits() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_max_table_entries = Some(usize::MAX);
    let execution = common::load_with(
        "let r=false;try{new WebAssembly.Table({element:'externref',initial:10000001})}catch(e){r=e instanceof RangeError}",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** Sparse table slots still obey the ordinary resident-array quota. */
#[test]
fn sparse_table_writes_cannot_exceed_resident_array_limit() {
    let execution = common::load(
        "const table=new WebAssembly.Table({element:'externref',initial:300000});for(let i=0;i<262145;i++)table.set(i,i)",
    );
    let failure = common::failure(&execution).unwrap_or_default();
    assert!(failure.contains("V9 resident array elements"), "{failure}");
}

/** Large table32 ranges retain fill identity and sparse overrides after state restoration. */
#[test]
fn sparse_table32_values_survive_state_restoration() {
    let sources = ["const value={},other={},t=new WebAssembly.Table({element:'externref',initial:300000},value);t.set(200000,other);t.grow(5,other);let r='';setTimeout(()=>{r=[t.length,t.get(1)===value,t.get(200000)===other,t.get(300004)===other].join()},1)".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let result = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        2.0,
        1,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "300005,true,true,true");
}

/** The configured table cap is enforced during validation and module construction. */
#[test]
fn explicit_table_capacity_limits_module_decoding() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_max_table_entries = Some(3);
    let source = "const h=[0,97,115,109,1,0,0,0,4,4,1,112,0],ok=new Uint8Array([...h,3]),bad=new Uint8Array([...h,4]);let r=[WebAssembly.validate(ok),WebAssembly.validate(bad)];try{new WebAssembly.Module(bad)}catch(e){r.push(e instanceof WebAssembly.CompileError,/implementation limit \\(3 elements\\)/.test(e.message))}r=r.join()";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,false,true,true");
}

/** Many untouched memories share the resident budget while preserving view, grow and detach semantics. */
#[test]
fn many_zero_memories_use_bounded_sparse_backing() {
    let source = "const memories=Array.from({length:600},()=>new WebAssembly.Memory({initial:1,maximum:2})),m=memories[599],old=m.buffer,v=new Uint8Array(old);v[123]=42;const before=v[123],pages=m.grow(1),detached=old.byteLength===0,next=new Uint8Array(m.buffer);let r=[before,pages,detached,next[123],next[65536]].join()";
    assert_eq!(common::eval(source, "r"), "42,1,true,42,0");
}

/** Repeated calls and alternating modules retain their own validated code and copied source. */
#[test]
fn repeated_module_calls_preserve_copied_source_identity() {
    let source = "const a=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,5,1,1,102,0,0,10,6,1,4,0,65,42,11]),b=a.slice();b[b.length-2]=7;const x=new WebAssembly.Instance(new WebAssembly.Module(a)).exports,y=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;a.fill(0);b.fill(0);let r=[x.f(),x.f(),y.f(),x.f(),y.f()].join()";
    assert_eq!(run(source), "42,42,7,42,7");
}
