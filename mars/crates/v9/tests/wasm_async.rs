/** Real Promise jobs resume owned Wasm frames without retaining native stacks or replaying imports. */
mod common;
/** Encode vector byte counts using the binary grammar, including fixtures beyond a one-byte section. */
fn unsigned(bytes: &mut Vec<u8>, mut value: usize) {
    loop {
        let byte = (value & 127) as u8;
        value >>= 7;
        bytes.push(byte | if value == 0 { 0 } else { 128 });
        if value == 0 {
            break;
        }
    }
}
fn section(bytes: &mut Vec<u8>, kind: u8, payload: &[u8]) {
    bytes.push(kind);
    unsigned(bytes, payload.len());
    bytes.extend(payload);
}
/** Functions share one typed imported callback, with optional intermediate Wasm calls. */
fn module(bodies: &[&[u8]], results: &[u8]) -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    let mut types = vec![1, 0x60, 1, 0x7f, results.len() as u8];
    types.extend(results);
    section(&mut bytes, 1, &types);
    section(&mut bytes, 2, &[1, 1, b'e', 1, b'g', 0, 0]);
    let mut functions = vec![bodies.len() as u8];
    functions.extend(std::iter::repeat_n(0, bodies.len()));
    section(&mut bytes, 3, &functions);
    section(&mut bytes, 7, &[1, 1, b'f', 0, bodies.len() as u8]);
    let mut code = vec![bodies.len() as u8];
    for body in bodies {
        unsigned(&mut code, body.len());
        code.extend(*body);
    }
    section(&mut bytes, 10, &code);
    bytes
}
fn run(bytes: &[u8], source: &str) -> String {
    common::eval(
        &format!("const bytes=new Uint8Array({bytes:?});{source}"),
        "r",
    )
}

/** Settled imported Promises still resume in jobs, preserving code after each suspension and callback counts. */
#[test]
fn promising_resumes_twice_in_actual_promise_order() {
    let bytes = module(
        &[&[
            1, 1, 0x7f, 0x20, 0, 0x10, 0, 0x21, 1, 0x20, 1, 0x41, 1, 0x6a, 0x10, 0, 0x41, 2, 0x6a,
            0x0b,
        ]],
        &[0x7f],
    );
    assert_eq!(
        run(
            &bytes,
            "let r='';const callback=new WebAssembly.Suspending(x=>{r+='g'+x+',';return Promise.resolve(x*2)});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:callback}}).exports.f;WebAssembly.promising(f)(4).then(value=>r+=value);r+='returned,';"
        ),
        "g4,returned,g9,20"
    );
}

/** An awaited child retains every real caller's locals and exact next source position. */
#[test]
fn promising_retains_nested_wasm_caller_frames() {
    let bytes = module(
        &[
            &[0, 0x20, 0, 0x10, 0, 0x41, 1, 0x6a, 0x0b],
            &[0, 0x20, 0, 0x10, 1, 0x41, 2, 0x6a, 0x0b],
        ],
        &[0x7f],
    );
    assert_eq!(
        run(
            &bytes,
            "let r='';const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(x=>Promise.resolve(x*2))}}).exports.f;WebAssembly.promising(f)(4).then(value=>r=String(value));"
        ),
        "11"
    );
}

/** Rejection values cross the continuation boundary by identity, without running remaining Wasm instructions. */
#[test]
fn promising_preserves_imported_promise_rejection_identity() {
    let bytes = module(&[&[0, 0x20, 0, 0x10, 0, 0x0b]], &[0x7f]);
    assert_eq!(
        run(
            &bytes,
            "let r=false;const token={};const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(()=>Promise.reject(token))}}).exports.f;WebAssembly.promising(f)(4).catch(error=>r=error===token);"
        ),
        "true"
    );
}

/** A pending import is resumed only when its actual resolver settles the captured native Promise. */
#[test]
fn promising_waits_for_real_deferred_settlement() {
    let bytes = module(&[&[0, 0x20, 0, 0x10, 0, 0x41, 1, 0x6a, 0x0b]], &[0x7f]);
    assert_eq!(
        run(
            &bytes,
            "let resolve,r='';const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(()=>new Promise(done=>resolve=done))}}).exports.f;WebAssembly.promising(f)(4).then(value=>r+=value);r+='pending,';queueMicrotask(()=>resolve(7));"
        ),
        "pending,8"
    );
}

/** Multiple returned values use the settled iterable once and retain source order after suspension. */
#[test]
fn promising_converts_real_multi_value_import_results() {
    let bytes = module(&[&[0, 0x20, 0, 0x10, 0, 0x0b]], &[0x7f, 0x7f]);
    assert_eq!(
        run(
            &bytes,
            "let r='';const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(x=>Promise.resolve([x,x+1]))}}).exports.f;WebAssembly.promising(f)(4).then(values=>r=values.join());"
        ),
        "4,5"
    );
}

/** Fulfillment coercion happens during resumption and rejects with an authored thrown value unchanged. */
#[test]
fn promising_rejects_with_the_actual_fulfillment_coercion_error() {
    let bytes = module(&[&[0, 0x20, 0, 0x10, 0, 0x0b]], &[0x7f]);
    assert_eq!(
        run(
            &bytes,
            "let r=false;const token={};const value={valueOf(){throw token}};const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(()=>Promise.resolve(value))}}).exports.f;WebAssembly.promising(f)(4).catch(error=>r=error===token);"
        ),
        "true"
    );
}

/** Saved frames and their instance/module roots survive a real GC before Promise reaction delivery. */
#[cfg(feature = "conformance")]
#[test]
fn promising_saved_frames_survive_collection_before_resume() {
    let bytes = module(&[&[0, 0x20, 0, 0x10, 0, 0x41, 1, 0x6a, 0x0b]], &[0x7f]);
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});let r='';let f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g:new WebAssembly.Suspending(x=>Promise.resolve(x))}}}}).exports.f;WebAssembly.promising(f)(41).then(value=>r=String(value));f=null;gc();"
    );
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(&source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "42");
}

/** Repeated awaits inside a real loop retain both operand prefixes and changing locals without replay. */
#[test]
fn promising_resumes_inside_loops_with_live_stack_prefixes() {
    let bytes = module(
        &[&[
            1, 2, 0x7f, 0x20, 0, 0x21, 2, 0x41, 0, 0x21, 1, 0x02, 0x7f, 0x03, 0x40, 0x20, 1, 0x20,
            2, 0x10, 0, 0x6a, 0x21, 1, 0x20, 2, 0x41, 1, 0x6b, 0x22, 2, 0x0d, 0, 0x0b, 0x20, 1,
            0x0b, 0x0b,
        ]],
        &[0x7f],
    );
    assert_eq!(
        run(
            &bytes,
            "let calls=0,r='';const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(x=>{calls++;return Promise.resolve(x)})}}).exports.f;WebAssembly.promising(f)(3).then(value=>r=value+','+calls);"
        ),
        "6,3"
    );
}

/** The saved PC identifies the selected conditional arm and skips the other arm after resumption. */
#[test]
fn promising_retains_the_actual_if_and_else_control_paths() {
    let bytes = module(
        &[&[
            0, 0x20, 0, 0x04, 0x7f, 0x41, 1, 0x10, 0, 0x41, 10, 0x6a, 0x05, 0x41, 2, 0x10, 0, 0x41,
            20, 0x6a, 0x0b, 0x0b,
        ]],
        &[0x7f],
    );
    assert_eq!(
        run(
            &bytes,
            "let r='';const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(x=>Promise.resolve(x+10))}}).exports.f;const p=WebAssembly.promising(f);Promise.all([p(1),p(0)]).then(values=>r=values.join());"
        ),
        "21,32"
    );
}

/** Branches after an await carry only the target block's values and skip abandoned control scopes. */
#[test]
fn promising_preserves_branches_out_of_nested_blocks() {
    let bytes = module(
        &[&[
            0, 0x02, 0x7f, 0x02, 0x40, 0x20, 0, 0x10, 0, 0x0c, 1, 0x0b, 0x41, 0, 0x0b, 0x0b,
        ]],
        &[0x7f],
    );
    assert_eq!(
        run(
            &bytes,
            "let r='';const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:new WebAssembly.Suspending(x=>Promise.resolve(x+2))}}).exports.f;WebAssembly.promising(f)(5).then(value=>r=String(value));"
        ),
        "7"
    );
}

/** Bounded control metadata is checked before materializing a large cloned resumable program. */
#[test]
fn promising_control_metadata_keeps_the_existing_native_byte_bound() {
    let mut body = vec![0];
    body.extend(std::iter::repeat_n(1, 20000));
    body.extend([0x20, 0, 0x10, 0, 0x0b]);
    let bytes = module(&[&body], &[0x7f]);
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g:new WebAssembly.Suspending(x=>Promise.resolve(x))}}}}).exports.f;WebAssembly.promising(f)(1);"
    );
    let execution = common::load(&source);
    let error = common::failure(&execution)
        .expect("resumable control metadata exceeds the unchanged native byte bound");
    assert!(error.contains("control bytes"), "{error}");
}

/** Promise resolution adopts the real compiled module and propagates an authored thenable rejection. */
#[test]
fn async_instantiation_adopts_the_compiled_module_promise() {
    assert_eq!(
        common::eval(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0]);const token={};WebAssembly.Module.prototype.then=()=>{throw token};let r=false;WebAssembly.instantiate(bytes).catch(error=>r=error===token);",
            "r"
        ),
        "true"
    );
}

/** If an authored module thenable resolves to a scalar, the actual instantiate callback rejects its brand. */
#[test]
fn async_instantiation_rechecks_the_resolved_module_brand() {
    assert_eq!(
        common::eval(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0]);WebAssembly.Module.prototype.then=resolve=>resolve(17);let r=false;WebAssembly.instantiate(bytes).catch(error=>r=error instanceof TypeError&&error.message==='Illegal call to WebAssembly.instantiate callback: Argument is not a WasmModuleObject');",
            "r"
        ),
        "true"
    );
}
