//! The d8 debug delegate tracks actual VM exception and promise lifecycle events per isolate.
#![cfg(feature = "conformance")]
mod common;

fn run(source: &str) -> mars_v9::Execution {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    execution
}

/** The installed delegate sees real exception and promise transitions, but stops on disable. */
#[test]
fn delegate_tracks_events_only_while_enabled() {
    let execution = run(
        "d8.debugger.enable();try{throw 42}catch(e){}Promise.resolve(1);d8.debugger.disable();try{throw 2}catch(e){}Promise.resolve(3);",
    );
    let state = serde_json::to_value(execution.state).unwrap();
    let debug = &state["page"]["debug_delegate"];
    assert_eq!(debug["enabled"], false);
    assert_eq!(debug["exceptions"], 1);
    assert_eq!(debug["promises_created"], 1);
    assert_eq!(debug["promises_settled"], 1);
}

/** The delegate is isolate-wide, follows the executing realm, and round-trips with the VM. */
#[test]
fn delegate_tracks_cross_realm_events_and_survives_transport() {
    let execution = run(
        "d8.debugger.enable();const child=Realm.create();Realm.eval(child,'try { throw 1 } catch(e) {}');",
    );
    let state = common::round_trip(execution.state);
    let state = serde_json::to_value(state).unwrap();
    let debug = &state["page"]["debug_delegate"];
    assert_eq!(debug["enabled"], true);
    assert_eq!(debug["exceptions"], 1);
    assert_eq!(debug["last_realm"], 1);
}

/** An uninstalled delegate neither sees events nor leaves state in the snapshot. */
#[test]
fn disabled_delegate_does_not_receive_events() {
    let execution = run("try{throw 42}catch(e){}Promise.resolve(1);");
    let state = serde_json::to_value(execution.state).unwrap();
    assert!(state["page"].get("debug_delegate").is_none());
}

/** A worker's delegate cannot receive parent events or install the parent's delegate. */
#[test]
fn delegate_is_isolated_between_workers() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_workers = Some(session.host());
    let execution = common::load_with(
        "const w=new Worker('d8.debugger.enable();try{throw 1}catch(e){};postMessage(7)',{type:'string'});let r=w.getMessage();w.terminateAndWait();try{throw 2}catch(e){}",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "7");
    let state = serde_json::to_value(execution.state).unwrap();
    assert!(state["page"].get("debug_delegate").is_none());
    session.finish().expect("worker terminates");
}
