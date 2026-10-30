/** Native script clocks advance within tasks while serialized state preserves scheduler determinism. */
mod common;
use mars_v9::{PageEnvironment, VmState};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

/** Real page time origins keep elapsed assertions independent of the calendar date. */
fn live() -> PageEnvironment {
    PageEnvironment {
        live_clock: true,
        time_origin_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.0,
        ..common::environment()
    }
}

/** Existing serialized environments and library fixtures retain exact virtual timestamps. */
#[test]
fn standalone_clocks_remain_virtual_and_old_environments_default_to_virtual() {
    let mut wire = serde_json::to_value(common::environment()).unwrap();
    wire.as_object_mut().unwrap().remove("live_clock");
    let environment: PageEnvironment = serde_json::from_value(wire).unwrap();
    assert!(!environment.live_clock);
    let result = common::load_with(
        "const r=[performance.now(),Date.now(),new Date().getTime(),performance.timeOrigin].join('|');",
        &environment,
    );
    assert_eq!(
        common::global(&result, "r"),
        "0|1700000000000|1700000000000|1700000000000"
    );
}

/** Date and performance expose one progressing timeline without requiring another browser turn. */
#[test]
fn native_clocks_advance_during_a_task_and_share_the_same_epoch() {
    let result = common::load_with(
        "const start=performance.now(),date=Date.now();while(performance.now()-start<2){}const low=performance.now(),wall=Date.now(),high=performance.now();const r=[high>start,wall>date,wall>=Math.floor(performance.timeOrigin+low),wall<=Math.floor(performance.timeOrigin+high)].join('|');",
        &live(),
    );
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true|true|true|true");
}

/** Reads cannot execute a timer inside the task that registered it, even after its delay elapsed. */
#[test]
fn live_reads_do_not_pump_timers_or_change_virtual_registration_order() {
    let environment = live();
    let host = common::host();
    let sources = vec!["let order='';setTimeout(()=>{order+='timer';console.log(order)},0);Promise.resolve().then(()=>order+='micro|');const start=performance.now();while(performance.now()-start<2){}order+='sync|';console.log(order);".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::console(&loaded), ["sync|"]);
    assert_eq!(loaded.effects.next_timer_ms, Some(0.0));
    let ran = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        0.0,
        1,
    )
    .unwrap();
    assert_eq!(common::console(&ran), ["sync|micro|timer"]);
}

/** A far-future virtual advance must not freeze elapsed-time loops in later native callbacks. */
#[test]
fn observed_time_survives_transport_and_progresses_after_a_virtual_jump() {
    let environment = live();
    let host = common::host();
    let sources = vec!["let last=performance.now();let result='';document.getElementById('go').addEventListener('click',()=>{const start=performance.now();while(performance.now()-start<1){}result=[start>=last,performance.now()>start].join('|');last=performance.now()});".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let jumped = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        loaded.state,
        1_000_000.0,
        0,
    )
    .unwrap();
    let saved = serde_json::to_value(&jumped.state).unwrap();
    assert!(saved["page"]["observed_clock"].as_f64().unwrap() >= 1_000_000.0);
    assert_eq!(saved["page"]["clock"], 1_000_000.0);
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(jumped.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::global(&clicked, "result"), "true|true");
    let after = serde_json::to_value(&clicked.state).unwrap();
    assert!(
        after["page"]["observed_clock"].as_f64().unwrap()
            >= saved["page"]["observed_clock"].as_f64().unwrap()
    );
    assert_eq!(after["page"]["clock"], saved["page"]["clock"]);
}

/** Forged negative clocks are rejected before an event can run; non-finite JSON clocks cannot load. */
#[test]
fn invalid_serialized_clock_values_are_refused_before_execution() {
    let environment = live();
    let host = common::host();
    let source = vec![
        "document.getElementById('go').addEventListener('click',()=>{throw 'must not run'})".into(),
    ];
    let loaded = mars_v9::load_page(common::input(&source, &host, &environment)).unwrap();
    let original = serde_json::to_value(&loaded.state).unwrap();
    for name in ["clock", "observed_clock"] {
        let mut wire = original.clone();
        wire["page"][name] = json!(-1.0);
        let state: VmState = serde_json::from_value(wire).unwrap();
        let error =
            mars_v9::click_page(common::input(&source, &host, &environment), state, 3).unwrap_err();
        assert_eq!(error.message, "Invalid V9 script clock.");
        let mut wire = original.clone();
        wire["page"][name] = json!(f64::INFINITY);
        assert!(serde_json::from_value::<VmState>(wire).is_err());
    }
}

/** The observed field is additive, so saved deterministic states from earlier builds still run. */
#[test]
fn old_saved_states_have_no_observed_time_requirement() {
    let environment = common::environment();
    let host = common::host();
    let source = vec!["document.getElementById('go').addEventListener('click',()=>console.log(performance.now()))".into()];
    let loaded = mars_v9::load_page(common::input(&source, &host, &environment)).unwrap();
    let mut wire = serde_json::to_value(&loaded.state).unwrap();
    wire["page"]
        .as_object_mut()
        .unwrap()
        .remove("observed_clock");
    let state = serde_json::from_value(wire).unwrap();
    let clicked =
        mars_v9::click_page(common::input(&source, &host, &environment), state, 3).unwrap();
    assert_eq!(common::console(&clicked), ["0"]);
}

/** A resumed network callback observes the persisted high-water and can measure its own work. */
#[test]
fn resumed_requests_keep_live_time_monotonic() {
    let environment = live();
    let host = common::host();
    let source = vec!["const before=performance.now();fetch('/clock').then(()=>{const start=performance.now();while(performance.now()-start<1){}console.log(start>=before,performance.now()>start)});".into()];
    let loaded = mars_v9::load_page(common::input(&source, &host, &environment)).unwrap();
    let request = &loaded.effects.requests[0];
    let response = mars_v9::HostResponse {
        id: request.id,
        status: 200,
        url: request.url.clone(),
        ..mars_v9::HostResponse::default()
    };
    let resumed = mars_v9::resume_requests(
        common::input(&source, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    assert_eq!(common::console(&resumed), ["true true"]);
}

/** Invalid native origins are refused before the live baseline can produce an infinite clock. */
#[test]
fn nonfinite_environment_origins_are_refused() {
    let host = common::host();
    let sources = vec!["throw 'must not run'".into()];
    for origin in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let mut environment = live();
        environment.time_origin_ms = origin;
        let error = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap_err();
        assert_eq!(error.message, "Invalid V9 script clock.");
    }
}

/** Rolling back a failed script cannot discard elapsed time seen by the next script. */
#[test]
fn script_rollback_does_not_rewind_the_execution_anchor() {
    let environment = live();
    let host = common::host();
    let sources = vec![
        "const before=performance.now();".into(),
        "const failedStart=performance.now();while(performance.now()-failedStart<2){}throw 'rollback';".into(),
        "console.log(performance.now()-before>=2);".into(),
    ];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(loaded.failed.len(), 1);
    assert_eq!(common::console(&loaded), ["true"]);
}
