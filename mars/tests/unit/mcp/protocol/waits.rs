/** page_wait re-checks between other work and answers once: satisfied, timed out or cancelled. */
use super::*;
use std::time::Duration;

/** Start a page_wait call and return its first native check. */
fn wait(session: &mut Session, id: u64, timeout_ms: u64, now: Instant) -> Request {
    dispatched(receive(
        session,
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"page_wait","arguments":{"tab_id":"tab-1","text":"Done","timeout_ms":timeout_ms}}}),
        now,
    ))
}

/** A native check result for the given request. */
fn checked(request: &Request, satisfied: bool) -> Response {
    Response::new(
        Some(request.id.clone()),
        Ok(json!({"satisfied":satisfied,"page_id":"page-1","revision":1})),
    )
}

/** An unsatisfied check is parked silently and dispatched again under a new private ID. */
#[test]
fn an_unsatisfied_wait_is_rechecked_until_it_holds() {
    let now = Instant::now();
    let mut session = ready(now);
    let first = wait(&mut session, 1, 1000, now);
    assert_eq!(first.op, "page.wait");
    assert!(session.complete(checked(&first, false), now).is_empty());
    assert!(session.poll(now + Duration::from_millis(10)).is_empty());
    let second = dispatched(session.poll(now + Duration::from_millis(60)));
    assert_ne!(second.id, first.id);
    assert_eq!(second.params, first.params);
    let reply = sent(session.complete(checked(&second, true), now + Duration::from_millis(70)));
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["result"]["structuredContent"]["satisfied"], true);
}

/** A wait that never holds is answered once, marked timed out, at its deadline. */
#[test]
fn a_wait_times_out_with_its_last_observation() {
    let now = Instant::now();
    let mut session = ready(now);
    let first = wait(&mut session, 2, 100, now);
    assert!(session.complete(checked(&first, false), now).is_empty());
    let reply = sent(session.poll(now + Duration::from_millis(150)));
    assert_eq!(reply["id"], 2);
    assert_eq!(reply["result"]["structuredContent"]["timed_out"], true);
    assert_eq!(reply["result"]["structuredContent"]["satisfied"], false);
    assert!(session.poll(now + Duration::from_millis(300)).is_empty());
}

/** Cancelling a parked wait drops it without a reply or another native check. */
#[test]
fn a_cancelled_wait_is_never_rechecked() {
    let now = Instant::now();
    let mut session = ready(now);
    let first = wait(&mut session, 3, 1000, now);
    assert!(session.complete(checked(&first, false), now).is_empty());
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3}}),
            now
        )
        .is_empty()
    );
    assert!(session.poll(now + Duration::from_millis(200)).is_empty());
}

/** Native errors end the wait at once rather than being retried. */
#[test]
fn a_failed_check_is_answered_immediately() {
    let now = Instant::now();
    let mut session = ready(now);
    let first = wait(&mut session, 4, 1000, now);
    let error = mars::protocol::EngineError::invalid("bad condition");
    let reply = sent(session.complete(Response::new(Some(first.id.clone()), Err(error)), now));
    assert_eq!(reply["result"]["isError"], true);
}
