/** Listener pressure reclaims dead registrations while preserving live callbacks and native signals. */
mod common;

/** Run the actual page event bindings and require an ordinary successful execution. */
fn run(source: &str) -> mars_v9::Execution {
    let execution = common::load(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    execution
}

/** Aborting removes registrations before abort handlers run, even if their event never fired. */
#[test]
fn abort_releases_capacity_before_abort_callbacks() {
    let execution = run(
        "const c=new AbortController();let hits=0;const callback=()=>hits++;Object.defineProperty(c.signal,'aborted',{get(){throw Error('public aborted getter')}});for(let i=0;i<16384;i++)window.addEventListener('unused'+i,callback,{signal:c.signal});c.signal.onabort=()=>window.addEventListener('after',callback);c.abort();window.dispatchEvent(new Event('unused0'));window.dispatchEvent(new Event('after'));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
}

/** Unreachable EventTargets do not consume the live-listener cap forever; native temporaries survive GC. */
#[test]
fn listener_pressure_collects_dead_targets_and_pins_new_signal() {
    let execution = run(
        "const noop=()=>{};function discard(){for(let i=0;i<16384;i++){const target=new EventTarget();target.addEventListener('x',noop)}}discard();let hits=0;const keep=new EventTarget();keep.addEventListener('x',()=>hits++,{get signal(){return new AbortController().signal}});keep.dispatchEvent(new Event('x'));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
    let restored: mars_v9::VmState =
        serde_json::from_slice(&serde_json::to_vec(&execution.state).unwrap()).unwrap();
    restored.validate_document(&common::page()).unwrap();
}

/** Truly live Window registrations remain rooted and retain the unchanged explicit limit. */
#[test]
fn live_listener_limit_is_still_enforced() {
    let execution = common::load(
        "const callback=()=>{};for(let i=0;i<16385;i++)window.addEventListener('live'+i,callback);",
    );
    assert!(
        execution.failed.iter().any(
            |failure| failure.error.message.contains("V9 event listeners")
                && failure.error.message.contains("16384")
        ),
        "{:?}",
        execution.failed
    );
}

/** An AbortSignal lookalike cannot register callbacks or run an author-controlled aborted getter. */
#[test]
fn listener_signal_requires_native_identity() {
    let execution = run(
        "let reads=0,rejected=false,hits=0;try{window.addEventListener('x',()=>hits++,{signal:{get aborted(){reads++;return false}}})}catch(error){rejected=error instanceof TypeError}window.dispatchEvent(new Event('x'));const result=[rejected,reads,hits].join('|');",
    );
    assert_eq!(common::global(&execution, "result"), "true|0|0");
}

/** Aborting within a dispatch removes later listeners before their snapshot entries are visited. */
#[test]
fn abort_removes_pending_dispatch_callbacks_synchronously() {
    let execution = run(
        "const target=new EventTarget(),c=new AbortController();let calls=[];target.addEventListener('x',()=>{calls.push('first');c.abort()});target.addEventListener('x',()=>calls.push('stale'),{signal:c.signal});target.dispatchEvent(new Event('x'));const result=calls.join(',');",
    );
    assert_eq!(common::global(&execution, "result"), "first");
}

/** Restoring a known heap object as a listener signal cannot forge native cancellation identity. */
#[test]
fn restored_listener_signals_must_retain_native_branding() {
    let execution =
        run("const c=new AbortController();window.addEventListener('x',()=>{},{signal:c.signal});");
    let mut wire = serde_json::to_value(&execution.state).unwrap();
    let callback = wire["page"]["listeners"][0]["callback"].clone();
    wire["page"]["listeners"][0]["signal"] = callback;
    let restored: mars_v9::VmState = serde_json::from_value(wire).unwrap();
    assert!(restored.validate_document(&common::page()).is_err());
}
