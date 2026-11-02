/** MCP protocol tests use a fake caller-supplied clock and public session actions without I/O or a browser. */
mod bounds;
mod cancellation;
mod correlation;
mod envelopes;
mod lifecycle;
mod waits;
use mars::{
    mcp::{Action, Session},
    protocol::{Request, Response},
};
use serde_json::{Value, json};
use std::time::Instant;
/** Every helper input represents one complete decoded transport frame. */
fn receive(session: &mut Session, value: Value, now: Instant) -> Vec<Action> {
    session.receive(&serde_json::to_vec(&value).unwrap(), now)
}
/** Inspect one protocol response without conflating it with native dispatch or terminal cleanup. */
fn sent(actions: Vec<Action>) -> Value {
    let mut actions = actions.into_iter();
    let Some(Action::Send(value)) = actions.next() else {
        panic!("expected one protocol response");
    };
    assert!(actions.next().is_none());
    value
}
/** Native requests use private correlation independently of external JSON-RPC ID spelling. */
fn dispatched(actions: Vec<Action>) -> Request {
    let mut actions = actions.into_iter();
    let Some(Action::Dispatch(request)) = actions.next() else {
        panic!("expected one native dispatch");
    };
    assert!(actions.next().is_none());
    request
}
/** A ready connection has completed both initialize and its separate notification. */
fn ready(now: Instant) -> Session {
    let mut session = Session::new(now);
    let result = sent(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":"initialize","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        now,
    ));
    assert!(result.get("result").is_some());
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            now
        )
        .is_empty()
    );
    session
}
/** Parameterless capabilities exercises admission without needing page or control metadata. */
fn call(session: &mut Session, id: Value, now: Instant) -> Vec<Action> {
    receive(
        session,
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"runtime_capabilities"}}),
        now,
    )
}
/** A normal native result is object-valued and carries the exact private request ID. */
fn completed(request: &Request) -> Response {
    Response::new(Some(request.id.clone()), Ok(json!({"native":true})))
}
/** Terminal actions are fixed diagnostics rather than correlated or page-derived strings. */
fn closed(actions: &[Action]) -> bool {
    matches!(actions, [Action::Close(_)])
}
