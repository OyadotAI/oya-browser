//! A small coordinator for IndexedDB tests: it answers the engine's load and list requests from
//! an `IndexedDbOrigin`, applies its commits, and runs timers until the page is quiet.
#![allow(dead_code)]
use crate::common;
use mars_v9::{
    Execution, HostResponse, INDEXED_DB_KIND, INDEXED_DB_LIST, INDEXED_DB_OPEN, IndexedDbOrigin,
    VmState,
};

/** Round trips one settle may take before the test calls the page stuck. */
const SETTLE_ROUNDS: usize = 400;
/** Timer callbacks one timer step may run. */
const TIMER_BUDGET: usize = 1000;

/** The answer to one engine request. */
fn answer(origin: &IndexedDbOrigin, request: &mars_v9::HostRequest) -> HostResponse {
    let body = match request.method.as_str() {
        INDEXED_DB_OPEN => origin
            .image(request.body.as_deref().unwrap_or_default())
            .unwrap(),
        INDEXED_DB_LIST => origin.listing().unwrap(),
        other => panic!("unexpected IndexedDB method {other}"),
    };
    HostResponse {
        id: request.id,
        status: 200,
        body,
        ..HostResponse::default()
    }
}

/** Apply an execution's commits and answer its IndexedDB requests; returns the responses. */
fn coordinate(execution: &Execution, origin: &mut IndexedDbOrigin) -> Vec<HostResponse> {
    for commit in &execution.effects.indexed_db {
        origin.apply(commit).expect("commit applies");
    }
    execution
        .effects
        .requests
        .iter()
        .filter(|request| request.kind == INDEXED_DB_KIND)
        .map(|request| answer(origin, request))
        .collect()
}

/** Drive a page loaded from `source` until no request or timer is pending. */
pub fn settle(source: &str, mut execution: Execution, origin: &mut IndexedDbOrigin) -> Execution {
    let host = common::host();
    let environment = common::environment();
    let sources = vec![source.to_string()];
    for _ in 0..SETTLE_ROUNDS {
        let responses = coordinate(&execution, origin);
        let state: VmState = execution.state;
        execution = if !responses.is_empty() {
            mars_v9::resume_requests(
                common::input(&sources, &host, &environment),
                state,
                responses,
            )
            .unwrap()
        } else if let Some(due) = execution.effects.next_timer_ms {
            mars_v9::run_timers(
                common::input(&sources, &host, &environment),
                state,
                due,
                TIMER_BUDGET,
            )
            .unwrap()
        } else {
            return Execution { state, ..execution };
        };
    }
    panic!("the page never settled");
}

/** Load a script against `origin`, settle, and return the `result` global. */
pub fn run_with(source: &str, origin: &mut IndexedDbOrigin) -> String {
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let settled = settle(source, execution, origin);
    common::global(&settled, "result")
}

/** Load a script against an empty origin and return the settled `result` global. */
pub fn run(source: &str) -> String {
    run_with(source, &mut IndexedDbOrigin::default())
}
