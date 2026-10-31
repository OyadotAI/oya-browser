/** Structured Wasm stacks capture genuine running interpreter frames before callbacks unwind, preserving privacy and GC roots. */
use crate::common;

/** Compact fixtures use one-byte section/body lengths and one void signature, with an optional real JS import. */
fn module(bodies: &[&[u8]], imported: bool, named: bool) -> (Vec<u8>, Vec<usize>) {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[1, 0x60, 0, 0]);
    if imported {
        section(&mut bytes, 2, &[1, 1, b'e', 1, b'g', 0, 0]);
    }
    let first = usize::from(imported);
    let mut functions = vec![bodies.len() as u8];
    functions.extend(std::iter::repeat_n(0, bodies.len()));
    section(&mut bytes, 3, &functions);
    section(
        &mut bytes,
        7,
        &[1, 3, b'r', b'u', b'n', 0, (first + bodies.len() - 1) as u8],
    );
    let mut code = vec![bodies.len() as u8];
    let mut offsets = Vec::new();
    for body in bodies {
        assert!(body.len() + 2 < 128);
        offsets.push(bytes.len() + 2 + code.len() + 2);
        code.extend([body.len() as u8 + 2, 0]);
        code.extend(*body);
        code.push(0x0b);
    }
    section(&mut bytes, 10, &code);
    if named {
        let mut names = vec![
            4, b'n', b'a', b'm', b'e', 0, 6, 5, b'O', b'w', b'n', b'e', b'd',
        ];
        let mut functions = vec![bodies.len() as u8];
        for index in 0..bodies.len() {
            let name: &[u8] = if index == 0 { b"inner" } else { b"outer" };
            functions.extend([(first + index) as u8, name.len() as u8]);
            functions.extend(name);
        }
        names.extend([1, functions.len() as u8]);
        names.extend(functions);
        section(&mut bytes, 0, &names);
    }
    (bytes, offsets)
}

/** Section sizes remain small by construction; production parsing still consumes the real binary format. */
fn section(bytes: &mut Vec<u8>, kind: u8, payload: &[u8]) {
    assert!(payload.len() < 128);
    bytes.extend([kind, payload.len() as u8]);
    bytes.extend(payload);
}

/** Promise callbacks must succeed as well as the initial script, so asynchronous assertion failures cannot disappear. */
fn eval(bytes: &[u8], source: &str) -> String {
    let ran = common::load(&format!("const bytes=new Uint8Array({bytes:?});{source}"));
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert!(
        ran.effects
            .console
            .iter()
            .all(|entry| entry.level != "error"),
        "{:?}",
        common::console(&ran)
    );
    common::global(&ran, "r")
}

/** The import reads the stack while its caller is still running, before any thrown error can unwind. */
#[test]
fn import_callbacks_observe_the_live_instance_index_and_opcode() {
    let (bytes, positions) = module(&[&[0x10, 0]], true, false);
    let source = format!(
        "Error.prepareStackTrace=(_,s)=>s;let frames;const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g:function callback(){{frames=new Error().stack}}}}}});instance.exports.run();const f=frames[1];let r=(f.getThis()===instance)+','+(f.getFunction()===1)+','+(f.getFunctionName()===null)+','+(f.getTypeName()===null)+','+(f.getMethodName()===null)+','+(f.getPosition()==={})+','+(f.getLineNumber()===1)+','+(f.getColumnNumber()==={})+','+f.getFileName().startsWith('wasm://wasm/')+','+!f.isNative()+','+!f.isAsync()+','+!f.isEval()+','+!f.isConstructor();",
        positions[0],
        positions[0] + 1
    );
    assert_eq!(
        eval(&bytes, &source),
        "true,true,true,true,true,true,true,true,true,true,true,true,true"
    );
}

/** Native frames and Wasm functions nest in one actual order even without an intervening JS activation. */
#[test]
fn native_to_wasm_interleaving_keeps_the_real_entry_order() {
    let (bytes, _) = module(&[&[0x10, 0]], true, false);
    assert_eq!(
        eval(
            &bytes,
            "Error.prepareStackTrace=(_,s)=>s;let frames;const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:function leaf(){frames=new Error().stack}}});[1].map(instance.exports.run);let r=frames[0].getFunctionName()+','+frames[1].getFunction()+','+frames[2].getFunctionName()+','+(frames[1].getThis()===instance);"
        ),
        "leaf,1,map,true"
    );
}

/** Wasm's index/instance semantics stay distinct from a younger strict builtin's JavaScript opacity. */
#[test]
fn wasm_to_native_interleaving_does_not_reset_js_strict_opacity() {
    let (bytes, _) = module(&[&[0x10, 0]], true, false);
    assert_eq!(
        eval(
            &bytes,
            "Error.prepareStackTrace=(_,s)=>s;let frames;const callback=Array.prototype.map.bind([1],function leaf(){frames=new Error().stack});const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:callback}});function caller(){instance.exports.run()}caller();let r=frames[0].getFunctionName()+','+frames[1].getFunctionName()+','+frames[2].getFunction()+','+(frames[2].getThis()===instance)+','+(frames[3].getFunction()===undefined);"
        ),
        "leaf,map,1,true,true"
    );
}

/** Regular calls retain both actual function indices, while return_call replaces its own frame. */
#[test]
fn real_calls_and_tail_calls_have_different_activation_ancestry() {
    let (regular, _) = module(&[&[0x10, 0], &[0x10, 1]], true, false);
    let (tail, _) = module(&[&[0x10, 0], &[0x12, 1]], true, false);
    let source = "Error.prepareStackTrace=(_,s)=>s;let frames;const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:function leaf(){frames=new Error().stack}}});instance.exports.run();let r='';for(const f of frames)if(typeof f.getFunction()==='number')r+=f.getFunction()+',';";
    assert_eq!(eval(&regular, source), "1,2,");
    assert_eq!(eval(&tail, source), "1,");
}

/** Names come from the genuine custom section; changing public export names cannot rewrite them. */
#[test]
fn actual_module_and_function_names_drive_default_serialization() {
    let (bytes, _) = module(&[&[0]], false, true);
    assert_eq!(
        eval(
            &bytes,
            "const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes));Object.defineProperty(instance.exports.run,'name',{value:'spoofed'});let error;try{instance.exports.run()}catch(e){error=e}const rendered=error.stack;Error.prepareStackTrace=(_,s)=>s;let second;try{instance.exports.run()}catch(e){second=e}const f=second.stack[0];let r=(f.getFunctionName()==='inner')+','+rendered.includes('Owned.inner (wasm://wasm/')+','+(rendered.split('wasm-function[').length===2);"
        ),
        "true,true,true"
    );
}

/** New structured traces, including an empty trace, must not regain raw frames during native unwind. */
#[test]
fn stack_trace_limit_zero_prevents_raw_unwind_frame_reappearance() {
    let (bytes, _) = module(&[&[0]], false, false);
    assert_eq!(
        eval(
            &bytes,
            "Error.stackTraceLimit=0;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.run;let r;try{f()}catch(e){r=!e.stack.includes('wasm-function[')+','+(e.stack.split('\\n').length===1)}"
        ),
        "true,true"
    );
}

/** A callback throw removes active scopes before a later unrelated Error is captured. */
#[test]
fn abrupt_unwind_removes_native_wasm_activations() {
    let (bytes, _) = module(&[&[0x10, 0]], true, false);
    assert_eq!(
        eval(
            &bytes,
            "Error.prepareStackTrace=(_,s)=>s;const token={};const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g(){throw token}}}).exports.run;let caught;try{f()}catch(e){caught=e}let leaked=false;for(const frame of new Error().stack)if(typeof frame.getFunction()==='number')leaked=true;let r=(caught===token)+','+leaked;"
        ),
        "true,false"
    );
}

/** Real JSPI continuation records restore active Wasm ancestry without serializing a second trace stack. */
#[test]
fn promising_resumption_captures_the_real_saved_function_stack() {
    let (bytes, _) = module(&[&[0x10, 0, 0x10, 0], &[0x10, 1]], true, false);
    assert_eq!(
        eval(
            &bytes,
            "Error.prepareStackTrace=(_,s)=>s;let r='';const callback=new WebAssembly.Suspending(function leaf(){const s=new Error().stack;for(const f of s)if(typeof f.getFunction()==='number')r+=f.getFunction()+',';r+=';';return Promise.resolve()});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{e:{g:callback}}).exports.run;WebAssembly.promising(f)().then(()=>{r+='done'});"
        ),
        "1,2,;1,2,;done"
    );
}

/** A transported CallSite retains its actual instance through normal private graph edges. */
#[test]
fn wasm_frame_metadata_survives_state_transport() {
    let (bytes, _) = module(&[&[0x10, 0]], true, false);
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});Error.prepareStackTrace=(_,s)=>s;let frames;const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g(){{frames=new Error().stack}}}}}});instance.exports.run();document.getElementById('go').onclick=()=>console.log(frames[1].getThis()===instance,frames[1].getFunction(),frames[1].getFileName().startsWith('wasm://wasm/'));"
    );
    let sources = vec![source];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true 1 true"]);
}

/** Actual GC can run inside the imported callback and after only the captured instance edge remains. */
#[cfg(feature = "conformance")]
#[test]
fn captured_instances_stay_traced_across_real_collection() {
    let (bytes, _) = module(&[&[0x10, 0]], true, false);
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});Error.prepareStackTrace=(_,s)=>s;let frames,instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g(){{gc();frames=new Error().stack}}}}}});const weak=new WeakRef(instance);instance.exports.run();instance=null;setTimeout(()=>{{gc();gc();console.log(frames[1].getThis()===weak.deref(),frames[1].getFunction())}},0);"
    );
    let sources = vec![source];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let ran = mars_v9::run_timers(input(), common::round_trip(loaded.state), 10.0, 10).unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::console(&ran), ["true 1"]);
}

/** Numeric function indices carry no object capability, while real foreign instances remain behind the membrane. */
#[cfg(feature = "conformance")]
#[test]
fn shadow_realm_callbacks_cannot_expose_foreign_wasm_instances() {
    let (bytes, _) = module(&[&[0x10, 0]], true, false);
    let foreign = format!(
        "const module=new WebAssembly.Module(new Uint8Array({bytes:?}));(function(cb){{const instance=new WebAssembly.Instance(module,{{e:{{g:cb}}}});instance.exports.run()}})"
    );
    let quoted = serde_json::to_string(&foreign).unwrap();
    let source = format!(
        "let frames;Error.prepareStackTrace=(_,s)=>{{frames=s;return 'captured'}};const run=new ShadowRealm().evaluate({quoted});function thrower(){{const e=new Error();e.stack;throw e}}try{{run(thrower)}}catch(e){{}}const f=frames[1];let refused=false;try{{f.getThis()}}catch(e){{refused=e instanceof TypeError}}let r=(f.getFunction()===1)+','+(f.getFunctionName()===null)+','+refused+','+(frames[2].getFunctionName()==='eval');"
    );
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let ran = common::load_with(&source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::global(&ran, "r"), "true,true,true,true");
}
