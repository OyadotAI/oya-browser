/** Fetch signals retain native identity, cancellation and exact reasons across host-response turns. */
mod common;
use common::{console, input, round_trip};
use mars_v9::{Execution, HostResponse, VmState};

/** A hermetic load records requests without opening any connection. */
fn load(sources: &[String]) -> Execution {
    let host = common::host();
    let environment = common::environment();
    mars_v9::load_page(input(sources, &host, &environment)).unwrap()
}

/** Restore the same state representation the native coordinator receives before responding. */
fn answer(sources: &[String], state: VmState, response: HostResponse) -> Execution {
    let host = common::host();
    let environment = common::environment();
    mars_v9::resume_requests(
        input(sources, &host, &environment),
        round_trip(state),
        vec![response],
    )
    .unwrap()
}

/** Request and clone signals are stable dependent objects, with all states set before abort events. */
#[test]
fn request_and_clone_signals_follow_the_native_source() {
    let sources = vec![
        r#"
        const controller = new AbortController();
        const request = new Request('/resource', { signal: controller.signal });
        const copy = request.clone();
        const reason = { why: 'cancelled' };
        console.log(request.signal === request.signal, request.signal !== controller.signal,
                    copy.signal !== request.signal);
        request.signal.onabort = () => console.log('event', copy.signal.aborted,
                                                   copy.signal.reason === reason);
        controller.abort(reason);
        controller.abort('ignored');
        console.log(request.signal.aborted, request.signal.reason === reason,
                    copy.signal.reason === reason);
    "#
        .into(),
    ];
    assert_eq!(
        console(&load(&sources)),
        ["true true true", "event true true", "true true true"]
    );
}

/** Signal-shaped objects reject asynchronously without invoking public aborted/reason getters. */
#[test]
fn fetch_rejects_fake_signals_without_reading_their_properties() {
    let sources = vec![
        r#"
        let reads = 0;
        let synchronous = false;
        const fake = { get aborted() { reads++; return false; },
                       get reason() { reads++; return 'fake'; } };
        try { fetch('/resource', { signal: fake }).catch(error =>
            console.log(error instanceof TypeError, reads)); }
        catch (error) { synchronous = true; }
        console.log('synchronous', synchronous);
    "#
        .into(),
    ];
    let loaded = load(&sources);
    assert_eq!(console(&loaded), ["synchronous false", "true 0"]);
    assert!(loaded.effects.requests.is_empty());
}

/** Pre-abort and same-turn cancellation never publish a request or an unnecessary cancellation ID. */
#[test]
fn aborted_fetches_do_not_escape_the_current_turn() {
    let sources = vec![r#"
        const before = new AbortController();
        before.abort(0);
        Object.defineProperty(before.signal, 'aborted', { value: false });
        fetch('/before', { signal: before.signal }).catch(error => console.log('before', error === 0));
        const during = new AbortController();
        fetch('/during', { signal: during.signal }).catch(error => console.log('during', error === false));
        during.abort(false);
    "#.into()];
    let loaded = load(&sources);
    assert_eq!(console(&loaded), ["before true", "during true"]);
    assert!(loaded.effects.requests.is_empty());
    assert!(loaded.effects.cancelled_requests.is_empty());
}

/** Restored cancellation releases a previously published request and ignores its late response. */
#[test]
fn cancellation_survives_restore_and_fences_late_responses() {
    let sources = vec![
        r#"
        const controller = new AbortController();
        const reason = { why: 'later' };
        fetch('/slow', { signal: controller.signal }).then(
            () => console.log('unexpected fulfillment'),
            error => console.log('cancelled', error === reason));
        setTimeout(() => controller.abort(reason), 1);
    "#
        .into(),
    ];
    let loaded = load(&sources);
    let id = loaded.effects.requests[0].id;
    let host = common::host();
    let environment = common::environment();
    let cancelled = mars_v9::run_timers(
        input(&sources, &host, &environment),
        round_trip(loaded.state),
        10.0,
        1,
    )
    .unwrap();
    assert_eq!(cancelled.effects.cancelled_requests, [id]);
    assert_eq!(console(&cancelled), ["cancelled true"]);
    let late = answer(
        &sources,
        cancelled.state,
        HostResponse {
            id,
            status: 200,
            body: "late".into(),
            ..HostResponse::default()
        },
    );
    assert!(console(&late).is_empty());
    assert!(late.effects.requests.is_empty());
}

/** Fulfilled responses and their clones retain cancellation until body consumption begins. */
#[test]
fn fetched_response_and_clone_body_readers_reject_the_abort_reason() {
    let sources = vec![
        r#"
        const controller = new AbortController();
        const reason = { why: 'body' };
        fetch('/body', { signal: controller.signal }).then(response => {
            const copy = response.clone();
            controller.abort(reason);
            response.arrayBuffer().catch(error => console.log('bytes', error === reason));
            copy.text().catch(error => console.log('text', error === reason));
        });
    "#
        .into(),
    ];
    let loaded = load(&sources);
    let id = loaded.effects.requests[0].id;
    let responded = answer(
        &sources,
        loaded.state,
        HostResponse {
            id,
            status: 200,
            body_bytes: vec![0, 255, 128],
            ..HostResponse::default()
        },
    );
    assert_eq!(console(&responded), ["bytes true", "text true"]);
    assert!(responded.effects.cancelled_requests.is_empty());
}
