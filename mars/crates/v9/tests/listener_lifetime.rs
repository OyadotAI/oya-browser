/** Listener pressure reclaims dead registrations while preserving live callbacks and native signals. */
use crate::common;

/** Run the actual page event bindings and require an ordinary successful execution. */
fn run(source: &str) -> mars_v9::Execution {
    let execution = common::load(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    execution
}

/** Aborting removes registrations before abort handlers run, even if their event never fired. */
#[test]
fn abort_releases_capacity_before_abort_callbacks() {
    let execution = run(
        "const c=new AbortController();let hits=0;const callback=()=>hits++;Object.defineProperty(c.signal,'aborted',{get(){throw Error('public aborted getter')}});const kind='x'.repeat(1048576);window.addEventListener(kind+'a',callback,{signal:c.signal});window.addEventListener(kind+'b',callback,{signal:c.signal});c.signal.onabort=()=>window.addEventListener(kind+'c',callback);c.abort();window.dispatchEvent(new Event(kind+'a'));window.dispatchEvent(new Event(kind+'c'));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
}

/** Unreachable EventTargets do not consume the live-listener cap forever; native temporaries survive GC. */
#[test]
fn listener_pressure_collects_dead_targets_and_pins_new_signal() {
    let execution = run(
        "const noop=()=>{},kind='x'.repeat(1048576);function discard(){for(let i=0;i<2;i++){const target=new EventTarget();target.addEventListener(kind,noop)}}discard();let hits=0;const keep=new EventTarget();keep.addEventListener(kind,()=>hits++,{get signal(){return new AbortController().signal}});keep.dispatchEvent(new Event(kind));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
    let restored: mars_v9::VmState =
        serde_json::from_slice(&serde_json::to_vec(&execution.state).unwrap()).unwrap();
    restored.validate_document(&common::page()).unwrap();
}

/** Short framework event registrations may exceed the old count ceiling within the byte budget. */
#[test]
fn short_live_registrations_exceed_the_old_count_limit() {
    let execution = run(
        "let hits=0;const callback=()=>hits++;for(let i=0;i<20000;i++)window.addEventListener('live'+i,callback);window.dispatchEvent(new Event('live19999'));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
    let restored: mars_v9::VmState =
        serde_json::from_slice(&serde_json::to_vec(&execution.state).unwrap()).unwrap();
    restored.validate_document(&common::page()).unwrap();
    restored
        .summary()
        .validate_summary_document(&common::page())
        .unwrap();
}

/** Long live event names hit accounted storage even with only three registrations. */
#[test]
fn live_event_name_bytes_are_bounded() {
    let execution = common::load(
        "const kind='x'.repeat(1048576),callback=()=>{};window.addEventListener(kind+'a',callback);window.addEventListener(kind+'b',callback);window.addEventListener(kind+'c',callback);",
    );
    assert!(
        execution.failed.iter().any(|failure| failure
            .error
            .message
            .contains("V9 event listener storage bytes")),
        "{:?}",
        execution.failed
    );
}

/** Explicit removal releases the same storage that registration admission charges. */
#[test]
fn removal_releases_event_name_storage() {
    let execution = run(
        "const kind='x'.repeat(1048576);let hits=0;const callback=()=>hits++;window.addEventListener(kind+'a',callback);window.addEventListener(kind+'b',callback);window.removeEventListener(kind+'a',callback);window.addEventListener(kind+'c',callback);window.dispatchEvent(new Event(kind+'c'));const result=String(hits);",
    );
    assert_eq!(common::global(&execution, "result"), "1");
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
