/** Async setup releases temporary roots while promises, continuations and thrown values stay live. */
mod common;

/** Exercise page listener pressure through the public API and validate the resulting snapshot. */
fn run(source: &str) -> mars_v9::Execution {
    let execution = common::load(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    common::round_trip(execution.state.clone())
        .validate_document(&common::page())
        .unwrap();
    execution
}

/** Discarded completed async results cannot keep disposable targets alive until the turn ends. */
#[test]
fn discarded_async_results_release_targets_under_listener_pressure() {
    let execution = run(
        "const kind='x'.repeat(1048576),noop=()=>{};let hits=0;
         async function discard(){const target=new EventTarget();target.addEventListener(kind,noop);return target}
         for(let i=0;i<6;i++)discard();
         const keep=new EventTarget();keep.addEventListener(kind,()=>hits++);
         keep.dispatchEvent(new Event(kind));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
}

/** A reaction-owned suspended frame must survive collections after setup roots are released.
`void` drops each result: a script's completion value legitimately keeps its last expression's
value alive (as in V8), which is not the temporary root under test. */
#[test]
fn pending_async_continuation_keeps_its_target_during_collection() {
    let execution = run(
        "const kind='x'.repeat(1048576),noop=()=>{};let resume,hits=0,result='pending';
         const gate=new Promise(resolve=>resume=resolve);
         async function waiting(){const target=new EventTarget();target.addEventListener(kind,()=>hits++);await gate;target.dispatchEvent(new Event(kind));result=String(hits)}
         async function discard(){const target=new EventTarget();target.addEventListener(kind,noop);return target}
         waiting();for(let i=0;i<6;i++)void discard();resume();",
    );
    assert_eq!(common::global(&execution, "result"), "1");
}

/** A throwing iterator next getter remains rooted through unwinding and a later pressure GC. */
#[test]
fn async_iterator_setup_preserves_thrown_object_until_catch() {
    let execution = run(
        "const kind='x'.repeat(1048576),noop=()=>{};let hits=0,result='pending';
         function source(){return {[Symbol.asyncIterator](){const target=new EventTarget();target.addEventListener(kind,()=>hits++);return {get next(){throw target}}}}}
         async function discard(){const target=new EventTarget();target.addEventListener(kind,noop);return target}
         async function check(){try{for await(const item of source()){} }catch(target){for(let i=0;i<6;i++)discard();target.dispatchEvent(new Event(kind));result=String(hits)}}check();",
    );
    assert_eq!(common::global(&execution, "result"), "1");
}
