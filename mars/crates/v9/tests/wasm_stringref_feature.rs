/* The shell's explicit Wasm stringref feature state controls binary validation. */
#![cfg(feature = "conformance")]
mod common;

/** The same module validates only when the host enables the optional proposal. */
#[test]
fn stringref_types_follow_the_shell_feature_state() {
    let source =
        "let r=WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,1,103,0]))";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "false");
    environment.conformance_wasm_stringref = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "true");
}

/** JS descriptors obey the same feature state as binary type validation. */
#[test]
fn stringref_descriptors_follow_the_shell_feature_state() {
    let source = "let a=false,b=false;try{new WebAssembly.Global({value:'stringref'});a=true}catch(e){}try{new WebAssembly.Table({element:'stringref',initial:0},null);b=true}catch(e){}let r=[a,b].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "false,false");
    environment.conformance_wasm_stringref = true;
    let enabled = common::load_with(source, &environment);
    assert_eq!(common::global(&enabled, "r"), "true,true");
}

/** String opcodes require the feature even when no signature contains a stringref type. */
#[test]
fn stringref_instructions_follow_the_shell_feature_state() {
    let source = "let r=WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,11,1,9,0,65,193,0,251,169,1,26,11]))";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert_eq!(common::global(&disabled, "r"), "false");
    environment.conformance_wasm_stringref = true;
    let enabled = common::load_with(source, &environment);
    assert_eq!(common::global(&enabled, "r"), "true");
}

/** Literal sections preserve UTF-16 values through real exported string.const instructions. */
#[test]
fn string_literals_execute_from_the_real_module() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,103,3,2,1,0,14,6,0,1,3,102,111,111,6,8,1,103,0,251,130,1,0,11,7,9,2,1,102,0,0,1,103,3,0,10,8,1,6,0,251,130,1,0,11]);const e=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;let r=e.f()+','+e.g.value";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_stringref = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "foo,foo");
}

/** Invalid literal encodings, deferred entries, and references fail at compilation. */
#[test]
fn malformed_string_literals_and_indices_are_rejected() {
    let source = "const h=[0,97,115,109,1,0,0,0],t=[1,5,1,96,0,1,103],f=[3,2,1,0],c=[10,8,1,6,0,251,130,1,0,11],cases=[[...h,14,5,0,1,2,192,128],[...h,14,2,1,0],[...h,...t,...f,...c]];let r=cases.map(b=>{try{new WebAssembly.Module(new Uint8Array(b));return false}catch(e){return e instanceof WebAssembly.CompileError}}).join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_stringref = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true,true,true");
}

/** Iterators return Unicode code points, retain lone surrogates, and signal their actual end. */
#[test]
fn stringref_iterator_next_reads_code_points() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,6,1,96,1,103,1,127,3,2,1,0,7,5,1,1,102,0,0,10,12,1,10,0,32,0,251,160,1,251,161,1,11]);const f=new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f;let r=[f('😀'),f('\\ud800'),f('')].join()";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_stringref = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "128512,55296,-1");
}

/** String conversion instructions read and write real packed GC array elements. */
#[test]
fn stringref_array_instructions_use_real_gc_storage() {
    let source = "const sec=(id,p)=>[id,p.length,...p],a=[0,65,193,0,65,194,0,65,195,0,251,8,0,3,65,0,65,3,251,176,1,11],b=[1,1,99,0,65,3,251,7,0,33,1,32,0,32,1,65,0,251,178,1,26,32,1,65,0,251,13,0,11],bytes=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[3,94,120,1,96,0,1,103,96,1,103,1,127]),...sec(3,[2,1,2]),...sec(7,[2,1,102,0,0,1,103,0,1]),...sec(10,[2,a.length,...a,b.length,...b])]);const e=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;let r=e.f()+','+e.g('Z')";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_stringref = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "ABC,90");
}
