/* Native profiling measures physical VM execution, retains real listeners and exports bounded actual call trees. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{Execution, PageEnvironment};

/** A long interval isolates explicit samples; no test relies on invented periodic ticks. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        conformance_d8: true,
        conformance_cpu_profiler_interval_us: i32::MAX as u64,
        ..common::environment()
    }
}

/** All assertions outside listeners observe the actual committed outcome. */
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
    execution
}

/** Immediate starts and explicit samples refer to the actual nested names and valid tree leaves. */
#[test]
fn manual_samples_capture_physical_nested_function_paths() {
    let execution = run(
        "let p;d8.profiler.setOnProfileEndListener(s=>p=JSON.parse(s));function outer(){inner()}function inner(){d8.profiler.triggerSample()}console.profile();outer();console.profileEnd();const leaf=p.nodes.find(n=>n.id===p.samples[1]);const parent=p.nodes.find(n=>n.children.includes(leaf.id));const r=[p.samples.length,p.timeDeltas.length,leaf.callFrame.functionName,parent.callFrame.functionName,leaf.hitCount,p.endTime>=p.startTime,p.timeDeltas.every(n=>n>=0)].join();",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "2,2,inner,outer,1,true,true"
    );
}

/** Already-active profile starts add a real stack observation rather than resetting or creating another session. */
#[test]
fn repeated_start_preserves_session_and_records_each_current_stack() {
    let execution = run(
        "let p;d8.profiler.setOnProfileEndListener(s=>p=JSON.parse(s));function restart(){console.profile()}console.profile();restart();console.profileEnd();console.profileEnd();const r=[p.samples.length,p.nodes.find(n=>n.id===p.samples[1]).callFrame.functionName].join();",
    );
    assert_eq!(common::global(&execution, "r"), "2,restart");
}

/** Labels are deliberately ignored by the pinned native console callbacks, including coercion side effects. */
#[test]
fn labels_are_not_coerced_and_samples_before_start_are_ignored() {
    let execution = run(
        "let p,touches=0;const label={toString(){touches++;throw 1}};d8.profiler.triggerSample();console.profileEnd(label);d8.profiler.setOnProfileEndListener(s=>p=JSON.parse(s));console.profile(label);console.profileEnd(label);const r=[touches,p.samples.length].join();",
    );
    assert_eq!(common::global(&execution, "r"), "0,1");
}

/** Stops dispose state before invoking listeners, allowing real callback-controlled restart without stale disposal. */
#[test]
fn listeners_can_restart_and_stop_a_new_session_reentrantly() {
    let execution = run(
        "const counts=[];d8.profiler.setOnProfileEndListener(s=>{counts.push(JSON.parse(s).samples.length);if(counts.length===1){console.profile();d8.profiler.triggerSample();console.profileEnd()}});console.profile();console.profileEnd();console.profileEnd();const r=counts.join();",
    );
    assert_eq!(common::global(&execution, "r"), "1,2");
}

/** Invalid listener replacement throws the intrinsic Error while preserving the previously registered callback. */
#[test]
fn listener_validation_preserves_registered_callback_and_native_error_brand() {
    let execution = run(
        "let called=0,good=0;d8.profiler.setOnProfileEndListener(()=>called++);for(const value of [undefined,null,{},1]){try{d8.profiler.setOnProfileEndListener(value)}catch(e){if(e.constructor===Error&&e.message==='The OnProfileEnd listener has to be a function')good++}}console.profile();console.profileEnd();const r=[good,called].join();",
    );
    assert_eq!(common::global(&execution, "r"), "4,1");
}

/** Pinned d8 catches ordinary listener exceptions, prints them verbosely, and lets the calling script continue. */
#[test]
fn ordinary_listener_exceptions_are_reported_without_becoming_script_failures() {
    let execution = common::load_with(
        "let continued=false;d8.profiler.setOnProfileEndListener(()=>{throw Error('profile-listener-failed')});console.profile();console.profileEnd();continued=true;",
        &environment(),
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::global(&execution, "continued"), "true");
    assert!(execution.effects.conformance_error.is_none());
    assert!(
        common::console(&execution)
            .iter()
            .any(|text| text.contains("profile-listener-failed"))
    );
}

/** Real elapsed time produces additional actual loop observations; the virtual page clock is irrelevant. */
#[test]
fn elapsed_native_intervals_sample_executing_bytecode_without_manual_triggers() {
    let environment = PageEnvironment {
        conformance_cpu_profiler_interval_us: 1,
        ..environment()
    };
    let execution = common::load_with(
        "let p;d8.profiler.setOnProfileEndListener(s=>p=JSON.parse(s));console.profile();function work(){let n=0;for(let i=0;i<20000;i++)n+=i;return n}work();console.profileEnd();const r=[p.samples.length>1,p.timeDeltas.some(n=>n>0),p.nodes.some(n=>n.callFrame.functionName==='work')].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true");
}

/** Foreign methods share the isolate session, while callback execution obeys its original realm. */
#[test]
fn realms_share_the_session_but_listener_runs_in_its_creation_context() {
    let execution = run(
        "const foreign=Realm.create();Realm.eval(foreign,\"globalThis.count=0;d8.profiler.setOnProfileEndListener(function(s){count++;globalThis.seen=JSON.parse(s).samples.length})\");console.profile();Realm.eval(foreign,'d8.profiler.triggerSample()');console.profileEnd();const r=[Realm.eval(foreign,'count'),Realm.eval(foreign,'seen'),typeof count].join();",
    );
    assert_eq!(common::global(&execution, "r"), "1,2,undefined");
}

/** The state graph roots a listener after author references disappear and survives the real JSON request boundary. */
#[test]
fn active_session_and_listener_survive_gc_and_json_transport() {
    let sources=vec!["let r;{const captured=41;d8.profiler.setOnProfileEndListener(s=>r=captured+JSON.parse(s).samples.length)}console.profile();setTimeout(()=>{gc();d8.profiler.triggerSample();console.profileEnd()},0);".to_owned()];
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
    assert_eq!(common::global(&execution, "r"), "43");
}

/** Without listeners actual profiles become inert host effects, not filesystem operations inside the VM. */
#[test]
fn completed_profiles_export_real_json_as_host_owned_artifacts() {
    let execution = run(
        "function sampled(){d8.profiler.triggerSample()}console.profile();sampled();console.profileEnd();console.profile();console.profileEnd();",
    );
    assert_eq!(execution.effects.conformance_cpu_profiles.len(), 2);
    let profile: serde_json::Value =
        serde_json::from_str(&execution.effects.conformance_cpu_profiles[0]).unwrap();
    assert_eq!(profile["samples"].as_array().unwrap().len(), 2);
    assert!(
        profile["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["callFrame"]["functionName"] == "sampled")
    );
}

/** Child isolates own independent sessions, but their real artifacts reach the same bounded session owner. */
#[test]
fn workers_export_their_own_profile_artifacts_without_foreign_heap_handles() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_workers: Some(session.host()),
        ..environment()
    };
    let execution = common::load_with(
        "console.profile();const w=new Worker(\"function childSample(){d8.profiler.triggerSample()}console.profile();childSample();console.profileEnd();postMessage('done')\",{type:'string'});let r=w.getMessage();w.terminateAndWait();console.profileEnd();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "done");
    session.finish().unwrap();
    let profiles = session.take_cpu_profiles();
    assert_eq!(profiles.len(), 2);
    assert_eq!(
        profiles
            .iter()
            .filter(|(endpoint, _)| endpoint.is_some())
            .count(),
        1
    );
    assert!(
        profiles
            .iter()
            .any(|(_, json)| json.contains("childSample"))
    );
}

/** Saved sample references cannot forge out-of-range tree identities or evade graph validation. */
#[test]
fn malformed_profile_snapshot_is_rejected_before_timer_callback() {
    let sources = vec!["let r=false;console.profile();setTimeout(()=>{r=true},0);".to_owned()];
    let host = common::host();
    let environment = environment();
    let first = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut state = serde_json::to_value(first.state).unwrap();
    state["page"]["cpu_profile"]["session"]["samples"][0] = serde_json::json!(999999);
    let state = serde_json::from_value(state).unwrap();
    let error = mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1)
        .unwrap_err();
    assert!(error.message.contains("CPU profile"), "{error:?}");
}

/** Function::Call changes active callback context without inventing a new host Context::Scope. */
#[test]
fn imported_listeners_keep_creation_globals_without_changing_last_entered_context() {
    let execution = run(
        "const foreign=Realm.create();const callback=Realm.eval(foreign,\"globalThis.entered=-1;globalThis.local='foreign';function listener(s){entered=Realm.current();globalThis.observed=local};listener\");d8.profiler.setOnProfileEndListener(callback);console.profile();console.profileEnd();const r=[Realm.eval(foreign,'entered'),Realm.eval(foreign,'observed'),Realm.current()].join();",
    );
    assert_eq!(common::global(&execution, "r"), "0,foreign,0");
}

/** Immutable instance bytes supply the real function name while JS callbacks remain actual physical samples. */
#[test]
fn explicit_samples_include_the_actual_executing_wasm_function() {
    let source = "let p;d8.profiler.setOnProfileEndListener(s=>p=JSON.parse(s));const bytes=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,2,7,1,1,109,1,102,0,0,3,2,1,0,7,8,1,4,109,97,105,110,0,1,10,6,1,4,0,16,0,11]);const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes),{m:{f:d8.profiler.triggerSample}});console.profile();instance.exports.main();console.profileEnd();const r=p.nodes.some(n=>n.callFrame.functionName==='wasm-function[1]'&&n.callFrame.url.startsWith('wasm://wasm/'));";
    let execution = run(source);
    assert_eq!(common::global(&execution, "r"), "true");
}

/** The broker caps aggregate artifacts rather than allowing one full state allowance for every completed profile. */
#[test]
fn artifact_queue_enforces_existing_aggregate_count_without_silent_drops() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_workers: Some(session.host()),
        ..environment()
    };
    let execution = common::load_with(
        "for(let i=0;i<10000;i++){console.profile();console.profileEnd()}",
        &environment,
    );
    let error = execution
        .failed
        .first()
        .expect("bounded native artifact failure");
    assert_eq!(error.error.code, "limit_exceeded");
    let profiles = session.take_cpu_profiles();
    assert!(!profiles.is_empty());
    assert!(profiles.len() < 10000);
    assert!(
        profiles
            .iter()
            .all(|(_, json)| serde_json::from_str::<serde_json::Value>(json).is_ok())
    );
    session.finish().unwrap();
}

/** Pinned shell FunctionTemplates have zero arity, writable prototype slots and real subclass construction. */
#[test]
fn profiler_namespace_uses_actual_function_template_metadata_and_construction() {
    let execution = run(
        "const p=d8.profiler;class Sample extends p.triggerSample{};const value=new Sample();let installed=0;class Install extends p.setOnProfileEndListener{};const listener=new Install(()=>installed++);console.profile();console.profileEnd();const r=[Object.keys(p).join(':'),p.triggerSample.length,p.setOnProfileEndListener.length,Object.getOwnPropertyDescriptor(p.triggerSample,'prototype').writable,value instanceof Sample,listener instanceof Install,installed].join();",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "setOnProfileEndListener:triggerSample,0,0,true,true,true,1"
    );
}

/** Ordinary stack exhaustion is caught at the real host boundary without suppressing or skipping recursive calls. */
#[test]
fn recursive_listener_calls_reach_the_real_stack_limit_and_the_caller_continues() {
    let execution = run(
        "let count=0;d8.profiler.setOnProfileEndListener(function listener(){count++;console.profile();console.profileEnd()});console.profile();console.profileEnd();const r=[count>1,typeof count].join();",
    );
    assert_eq!(common::global(&execution, "r"), "true,number");
}

/** The scorer retains genuine assertion provenance, including assertions raised inside a caught native callback. */
#[test]
fn callback_assertions_remain_failed_tests_after_verbose_reporting() {
    let execution = common::load_with(
        "class MjsUnitAssertionError extends Error{};d8.profiler.setOnProfileEndListener(()=>{throw new MjsUnitAssertionError('actual assertion')});console.profile();console.profileEnd();",
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
    assert!(
        common::console(&execution)
            .iter()
            .any(|message| message.contains("actual assertion"))
    );
}

/** Reaching an unsupported native capability still preserves its explicit native failure channel. */
#[test]
fn caught_listener_native_capability_failures_keep_native_provenance() {
    let execution = common::load_with(
        "d8.profiler.setOnProfileEndListener(()=>{gc({type:'major-snapshot'})});console.profile();console.profileEnd();",
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

/** D8's ReportException sends the original value to onerror, whose literal true result suppresses printing. */
#[test]
fn verbose_reporting_invokes_real_onerror_and_observes_its_true_result() {
    let execution = run(
        "const marker={toString(){throw 'must not stringify'}};let seen=false;onerror=(message,source,line,column,error)=>{seen=error===marker;return true};d8.profiler.setOnProfileEndListener(()=>{throw marker});console.profile();console.profileEnd();const r=seen;",
    );
    assert_eq!(common::global(&execution, "r"), "true");
    assert!(execution.effects.console.is_empty());
}

/** A native resource-admission failure retains its fatal category when raised directly by a listener. */
#[test]
fn listener_native_resource_limits_are_not_changed_into_caught_ordinary_errors() {
    let source = format!(
        "d8.profiler.setOnProfileEndListener(()=> 'x'.repeat({}));console.profile();console.profileEnd();",
        mars_v9::constants::JS_STRING_UNITS + 1
    );
    let execution = common::load_with(&source, &environment());
    assert_eq!(
        execution
            .failed
            .first()
            .expect("existing string resource failure")
            .error
            .code,
        "limit_exceeded"
    );
}

/** Author error formatting is real script work, and its native resource failures must remain fatal as well. */
#[test]
fn verbose_error_formatting_preserves_native_fatal_admission_errors() {
    let source = format!(
        "d8.profiler.setOnProfileEndListener(()=>{{throw {{toString(){{return 'x'.repeat({})}}}}}});console.profile();console.profileEnd();",
        mars_v9::constants::JS_STRING_UNITS + 1
    );
    let execution = common::load_with(&source, &environment());
    assert_eq!(
        execution
            .failed
            .first()
            .expect("existing formatter resource failure")
            .error
            .code,
        "limit_exceeded"
    );
}
