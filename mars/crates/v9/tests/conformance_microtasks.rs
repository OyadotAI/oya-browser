/* Shell callable microtasks enter the callback's actual realm and report through that realm's real onerror. */
#![cfg(feature = "conformance")]
use crate::common;
use mars_v9::{Execution, PageEnvironment};

/** Browser behavior is separate; these tests explicitly opt into the native shell contract. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    }
}

/** Unexpected assertions and native/ordinary uncaught failures remain visible outside reporting callbacks. */
fn run(source: &str) -> Execution {
    let execution = common::load_with(source, &environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
    assert!(
        execution.effects.conformance_native_error.is_none(),
        "{:?}",
        execution.effects.conformance_native_error
    );
    execution
}

/** The queue method's realm and thrown error's realm cannot replace the relevant callback realm. */
#[test]
fn three_realm_microtask_exceptions_report_in_the_callback_realm() {
    let execution = run(
        "var frames=[];for(let i=0;i<3;i++){frames[i]=Realm.global(Realm.createAllowCrossRealmAccess());frames[i].parent=globalThis}const calls=[];globalThis.onerror=()=>calls.push('root');frames[0].onerror=()=>calls.push('frame0');frames[1].onerror=(message,url,line,column,error)=>calls.push('frame1:'+(error instanceof frames[2].Error));frames[2].onerror=()=>calls.push('frame2');frames[0].queueMicrotask(new frames[1].Function(\"throw new parent.frames[2].Error('actual error')\"));let r;queueMicrotask(()=>r=calls.join());",
    );
    assert_eq!(common::global(&execution, "r"), "frame1:true");
}

/** The actual entered-context stack follows a callable task, then restores the host entry for subsequent root work. */
#[test]
fn callable_tasks_enter_and_restore_their_real_context() {
    let execution = run(
        "const id=Realm.createAllowCrossRealmAccess();Realm.shared={seen:[]};const callback=Realm.eval(id,'()=>Realm.shared.seen.push(Realm.current())');queueMicrotask(callback);queueMicrotask(()=>Realm.shared.seen.push(Realm.current()));let r;queueMicrotask(()=>r=Realm.shared.seen.join());",
    );
    assert_eq!(common::global(&execution, "r"), "1,0");
}

/** Relevant-context discovery unwraps bound and proxy targets without invoking ordinary get traps. */
#[test]
fn bound_proxy_callbacks_use_the_underlying_function_realm_without_get_traps() {
    let execution = run(
        "const id=Realm.createAllowCrossRealmAccess();Realm.shared={seen:[],gets:0};const f=Realm.eval(id,'()=>Realm.shared.seen.push(Realm.current())').bind(null);const p=new Proxy(f,{get(){Realm.shared.gets++;throw 1}});queueMicrotask(p);let r;queueMicrotask(()=>r=[Realm.shared.seen.join(),Realm.shared.gets].join('|'));",
    );
    assert_eq!(common::global(&execution, "r"), "1|0");
}

/** An ordinary reported error does not prevent later microtasks from running or become a promise rejection. */
#[test]
fn ordinary_unhandled_microtask_errors_report_and_continue_the_queue() {
    let execution = run(
        "let r='before';queueMicrotask(()=>{throw Error('microtask diagnostic')});queueMicrotask(()=>r='after');",
    );
    assert_eq!(common::global(&execution, "r"), "after");
    assert!(
        common::console(&execution)
            .iter()
            .any(|text| text.contains("microtask diagnostic"))
    );
    assert!(!execution.effects.conformance_rejection);
}

/** ReportException passes the original value and respects onerror's literal true before author error formatting. */
#[test]
fn onerror_receives_original_error_and_can_suppress_ordinary_diagnostics() {
    let execution = run(
        "let r=false;const marker={toString(){throw Error('unnecessary stringify')}};onerror=(message,url,line,column,error)=>{r=error===marker;return true};queueMicrotask(()=>{throw marker});",
    );
    assert_eq!(common::global(&execution, "r"), "true");
    assert!(execution.effects.console.is_empty());
}

/** Even a true onerror cannot turn a real harness assertion into a successful scored test. */
#[test]
fn suppressed_callback_assertions_keep_failure_provenance() {
    let execution = common::load_with(
        "class MjsUnitAssertionError extends Error{};onerror=()=>true;queueMicrotask(()=>{throw new MjsUnitAssertionError('real failed assertion')});",
        &environment(),
    );
    assert!(execution.failed.is_empty());
    assert!(
        execution
            .effects
            .conformance_error
            .as_deref()
            .is_some_and(|message| message.contains("MjsUnitAssertionError"))
    );
}

/** A missing native capability stays a native failure even if its thrown value is reported through onerror. */
#[test]
fn shell_reporting_retains_unsupported_native_failure_identity() {
    let execution = common::load_with(
        "onerror=()=>true;queueMicrotask(()=>gc({type:'major-snapshot'}));",
        &environment(),
    );
    assert!(execution.failed.is_empty());
    assert!(
        execution
            .effects
            .conformance_native_error
            .as_ref()
            .is_some_and(|(code, _)| code == "unsupported_feature")
    );
}

/** Real resource failures remain fatal and do not run onerror or later queued author jobs. */
#[test]
fn native_resource_failures_abort_microtask_execution_without_being_reported_as_ordinary() {
    let source = format!(
        "onerror=()=>{{console.log('reported');return true}};queueMicrotask(()=> 'x'.repeat({}));queueMicrotask(()=>console.log('after'));",
        mars_v9::constants::JS_STRING_UNITS + 1
    );
    let execution = common::load_with(&source, &environment());
    assert_eq!(
        execution
            .failed
            .first()
            .expect("native string admission failure")
            .error
            .code,
        "limit_exceeded"
    );
    assert!(execution.effects.console.is_empty());
}

/** A revoked callable proxy is queued normally; its later call reports in the fallback enqueue context. */
#[test]
fn revoked_proxy_context_fallback_defers_the_actual_call_error() {
    let execution = run(
        "const p=Proxy.revocable(()=>{},{});p.revoke();const calls=[];onerror=(message,url,line,column,error)=>{calls.push(error instanceof TypeError);return true};queueMicrotask(p.proxy);calls.push('queued');let r;queueMicrotask(()=>r=calls.join());",
    );
    assert_eq!(common::global(&execution, "r"), "queued,true");
}

/** A callback transported in a real timer survives state serialization and collection after its only author reference is cleared. */
#[test]
fn callback_context_and_native_captures_survive_gc_after_request_transport() {
    let source = "const id=Realm.createAllowCrossRealmAccess();Realm.shared={value:0};let callback=Realm.eval(id,'()=>{Realm.shared.value=Realm.current()+40}');let r;setTimeout(()=>{queueMicrotask(()=>gc());queueMicrotask(callback);callback=null;queueMicrotask(()=>r=Realm.shared.value)},0);";
    let sources = vec![source.to_owned()];
    let host = common::host();
    let environment = environment();
    let first = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(first.failed.is_empty(), "{:?}", common::failure(&first));
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(first.state),
        0.0,
        1,
    )
    .unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "41");
}

/** Timer exceptions report to the callback's creation realm after state transport. */
#[test]
fn timer_exception_uses_callback_realm_and_preserves_original_value() {
    let source = "const other=Realm.createAllowCrossRealmAccess();Realm.shared={calls:[]};Realm.eval(other,`globalThis.onerror=(message,source,line,column,error)=>{Realm.shared.calls.push(Realm.current()+':'+error.message);return true};setTimeout(()=>{throw Error('foreign')},0)`);onerror=()=>{Realm.shared.calls.push('root');return true};let r;setTimeout(()=>r=Realm.shared.calls.join(),0);";
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        loaded.effects.next_timer_ms.unwrap(),
        10,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::global(&resumed, "r"), "1:foreign");
}
