/** Async stack frames come from real suspended awaiters, with strict/realm privacy and ordinary traced heap lifetimes. */
mod common;

/** Public page execution drains the genuine Promise job queue before returning observable globals. */
fn eval(source: &str) -> String {
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let errors: Vec<_> = execution
        .effects
        .console
        .iter()
        .filter(|entry| entry.level == "error")
        .map(|entry| &entry.text)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    common::global(&execution, "r")
}

/** A running async function is physical; only the suspended caller reconstructed from its promise is async. */
#[test]
fn physical_and_reconstructed_frames_have_distinct_async_markers() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.isAsync()]);async function one(){return await two()}async function two(){await 0;return new Error().stack}let r;one().then(v=>r=JSON.stringify(v));"
        ),
        "[[\"two\",false],[\"one\",true]]"
    );
}

/** A strict older continuation hides its function while the younger sloppy activation remains visible. */
#[test]
fn strict_parent_hides_only_its_own_and_older_objects() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.getFunction()===undefined]);async function one(){'use strict';return await two()}async function two(){await 0;return new Error().stack}let r;one().then(v=>r=JSON.stringify(v));"
        ),
        "[[\"two\",false],[\"one\",true]]"
    );
}

/** Strict opacity propagates from the youngest activation across the async boundary. */
#[test]
fn strict_current_frame_hides_the_sloppy_parent_too() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.getFunction()===undefined,f.getThis()===undefined]);async function one(){return await two()}async function two(){'use strict';await 0;return new Error().stack}let r;one().then(v=>r=JSON.stringify(v));"
        ),
        "[[\"two\",true,true],[\"one\",true,true]]"
    );
}

/** Several pending output promises reconstruct their real await order rather than a copied call-time stack. */
#[test]
fn multiple_awaited_callers_keep_their_actual_order() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.isAsync()]);async function root(){return await one()}async function one(){return await two()}async function two(){await 0;return new Error().stack}let r;root().then(v=>r=JSON.stringify(v));"
        ),
        "[[\"two\",false],[\"one\",true],[\"root\",true]]"
    );
}

/** Unrelated observers prevent guessing which reaction represents the sole async parent. */
#[test]
fn multiple_subscribers_stop_ambiguous_ancestry() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.isAsync()]);async function one(){const p=two();p.then(()=>{});return await p}async function two(){await 0;return new Error().stack}let r;one().then(v=>r=JSON.stringify(v));"
        ),
        "[[\"two\",false]]"
    );
}

/** The existing stack limit applies to the physical and reconstructed sequence together. */
#[test]
fn async_ancestry_obeys_the_existing_stack_trace_limit() {
    assert_eq!(
        eval(
            "Error.stackTraceLimit=1;Error.prepareStackTrace=(_,s)=>s.map(f=>f.getFunctionName());async function one(){return await two()}async function two(){await 0;return new Error().stack}let r;one().then(v=>r=JSON.stringify(v));"
        ),
        "[\"two\"]"
    );
}

/** Structured locations belong to the supplied source identity and real await coordinates. */
#[test]
fn suspended_frames_preserve_source_metadata() {
    let source = "Error.prepareStackTrace=(_,s)=>s;\nasync function one(){return await two()}\nasync function two(){await 0;return new Error().stack}\nlet r;one().then(s=>r=[s[0].getFileName(),s[1].getFileName(),s[0].getLineNumber(),s[1].getLineNumber()].join(','));";
    let mut environment = common::environment();
    environment.scripts = vec!["https://fixtures.example/async.js".to_owned()];
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(
        common::global(&ran, "r"),
        "https://fixtures.example/async.js,https://fixtures.example/async.js,3,2"
    );
}

/** Host state transport retains the actual parked frame/reaction graph rather than serializing an ancestry copy. */
#[test]
fn awaiting_parent_frames_survive_state_transport() {
    let sources=vec!["Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.isAsync()]);let release;const gate=new Promise(r=>release=r);async function one(){return await two()}async function two(){await gate;return new Error().stack}one().then(s=>console.log(JSON.stringify(s)));document.getElementById('go').addEventListener('click',()=>release());".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(
        common::console(&clicked),
        ["[[\"two\",false],[\"one\",true]]"]
    );
}

/** Real GC can run while the current native driver owns its output promise and pending caller frames. */
#[cfg(feature = "conformance")]
#[test]
fn collection_during_resumption_preserves_continuation_roots() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>{gc();return s.map(f=>[f.getFunctionName(),f.getFunction()===one])};async function one(){return await two()}async function two(){await 0;gc();return new Error().stack}let r;one().then(s=>r=JSON.stringify(s));"
        ),
        "[[\"two\",false],[\"one\",true]]"
    );
}

/** The real host switch disables only continuation enrichment, not the current physical frame. */
#[cfg(feature = "conformance")]
#[test]
fn host_can_disable_async_stack_enrichment() {
    let mut environment = common::environment();
    environment.conformance_async_stack_traces = false;
    let ran = common::load_with(
        "Error.prepareStackTrace=(_,s)=>s.map(f=>[f.getFunctionName(),f.isAsync()]);async function one(){return await two()}async function two(){await 0;return new Error().stack}let r;one().then(v=>r=JSON.stringify(v));",
        &environment,
    );
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::global(&ran, "r"), "[[\"two\",false]]");
}

/** The gate opens after native promise adoption has installed the allowed foreign continuation. */
#[cfg(feature = "conformance")]
#[test]
fn cross_realm_continuations_use_their_actual_function() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const realm=Realm.createAllowCrossRealmAccess();const parent=Realm.eval(realm,'(async function foreign(fn){return await fn()})');let release;const gate=new Promise(r=>release=r);async function leaf(){await gate;return new Error().stack}let r;parent(leaf).then(s=>r=(s[0].getFunction()===leaf)+','+(s[1].getFunction()===parent)+','+s[1].isAsync());queueMicrotask(()=>release());"
        ),
        "true,true,true"
    );
}

/** After adoption is attached, a different token filters the actual foreign continuation before exposure. */
#[cfg(feature = "conformance")]
#[test]
fn isolated_realm_continuations_are_filtered_before_exposure() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const realm=Realm.create();const parent=Realm.eval(realm,'(async function hidden(fn){return await fn()})');let release;const gate=new Promise(r=>release=r);async function leaf(){await gate;return new Error().stack}let r;parent(leaf).then(s=>r=(s.length===1)+','+(s[0].getFunction()===leaf));queueMicrotask(()=>release());"
        ),
        "true,true"
    );
}

/** Promise.all contributes its actual intrinsic, captured input position and awaiting parent. */
#[test]
fn promise_all_frames_preserve_native_capability_metadata() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;async function one(){return await Promise.all([two()])}async function two(){await 0;return new Error().stack}let r;one().then(values=>{const s=values[0];r=JSON.stringify(s.map(f=>[f.getFunctionName(),f.isAsync(),f.isPromiseAll(),f.getPromiseIndex()]));r+=','+(s[1].getFunction()===Promise.all)+','+(s[2].getFunction()===one)+','+(s[1].getFileName()===null)+','+(s[1].getLineNumber()===null)});"
        ),
        "[[\"two\",false,false,null],[\"all\",true,true,0],[\"one\",true,false,null]],true,true,true,true"
    );
}

/** The recorded index is the actual element position, including earlier already-fulfilled inputs. */
#[test]
fn promise_all_frames_use_the_actual_nonzero_element_index() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;async function one(){return await Promise.all([42,two()])}async function two(){await 0;return new Error().stack}let r;one().then(values=>r=values[1][1].getPromiseIndex()+','+values[1][1].toString());"
        ),
        "1,async Promise.all (index 1)"
    );
}

/** Replacing the public property cannot rename the native operation that already registered its real element callbacks. */
#[test]
fn pending_combinators_retain_the_executed_intrinsic_identity() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const original=Promise.all;async function one(){return await original.call(Promise,[two()])}async function two(){await 0;return new Error().stack}let r;const pending=one();Promise.all=function replacement(){throw 'should not be called'};pending.then(values=>r=(values[0][1].getFunction()===original)+','+values[0][1].getFunctionName());"
        ),
        "true,all"
    );
}

/** Native intrinsic frames do not expose a user closure and do not reset cumulative strict opacity for older callers. */
#[test]
fn strict_privacy_survives_a_native_combinator_frame() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;async function one(){return await Promise.all([two()])}async function two(){'use strict';await 0;return new Error().stack}let r;one().then(values=>{const s=values[0];r=(s[0].getFunction()===undefined)+','+(s[1].getFunction()===Promise.all)+','+(s[2].getFunction()===undefined)});"
        ),
        "true,true,true"
    );
}

/** Restored native element closures retain actual aggregate capability and input-index metadata across requests. */
#[test]
fn promise_all_ancestry_survives_state_transport() {
    let sources=vec!["Error.prepareStackTrace=(_,s)=>s;let release;const gate=new Promise(r=>release=r);async function one(){return await Promise.all([1,two()])}async function two(){await gate;return new Error().stack}one().then(values=>{const s=values[1];console.log(s[1].getFunction()===Promise.all,s[1].getPromiseIndex(),s[2].getFunction()===one)});document.getElementById('go').addEventListener('click',()=>release());".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert!(
        clicked
            .effects
            .console
            .iter()
            .all(|entry| entry.level != "error"),
        "{:?}",
        common::console(&clicked)
    );
    assert_eq!(common::console(&clicked), ["true 1 true"]);
}

/** Fatal host termination preserves earlier bounded native console diagnostics without changing the exit status. */
#[cfg(feature = "conformance")]
#[test]
fn nonzero_host_exit_retains_the_printed_failure_diagnostic() {
    let sources = vec!["print('before-exit');quit(7);".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let error = mars_v9::load_page(common::input(&sources, &host, &environment))
        .err()
        .unwrap();
    assert_eq!(error.code, "conformance_exit");
    assert_eq!(error.message, "7");
    assert!(error.hint.contains("before-exit"), "{}", error.hint);
}
