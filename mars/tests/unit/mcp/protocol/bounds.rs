/** Per-frame, whole-session and admitted-request bounds fail explicitly without fabricating outcomes. */
use super::*;
use mars::{
    constants::{MCP_FRAME_BYTES, MCP_REQUEST_DEADLINE, MCP_SESSION_REQUESTS, VERSION},
    mcp::encode,
    protocol::EngineError,
};
use std::time::Duration;
/** IDs remain consumed after immediate protocol replies and the whole-connection cap is terminal. */
#[test]
fn whole_session_id_cap_includes_completed_protocol_requests() {
    let now = Instant::now();
    let mut session = ready(now);
    for index in 0..MCP_SESSION_REQUESTS - 1 {
        assert!(
            sent(receive(
                &mut session,
                json!({"jsonrpc":"2.0","id":index,"method":"ping"}),
                now
            ))
            .get("result")
            .is_some()
        );
    }
    assert!(closed(&receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":"over-limit","method":"ping"}),
        now
    )));
}
/** MCP serialization includes the newline and cannot silently truncate an otherwise valid response. */
#[test]
fn encoder_enforces_exact_frame_limit_including_newline() {
    let value = json!("x".repeat(MCP_FRAME_BYTES - 3));
    let encoded = encode(&value).unwrap();
    assert_eq!(encoded.len(), MCP_FRAME_BYTES);
    assert_eq!(encoded.last(), Some(&b'\n'));
    assert_eq!(
        encode(&json!("x".repeat(MCP_FRAME_BYTES - 2)))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(serde_json::from_slice::<Value>(&encoded).unwrap(), value);
}
/** Oversized input ends the connection before JSON parsing or native admission. */
#[test]
fn oversized_complete_input_retires_the_session() {
    let now = Instant::now();
    let mut session = ready(now);
    assert!(closed(
        &session.receive(&vec![b' '; MCP_FRAME_BYTES + 1], now)
    ));
    assert!(call(&mut session, json!(1), now).is_empty());
}
/** Output overflow becomes an original-ID internal error, never a partial success or omitted native nodes. */
#[test]
fn oversized_tool_results_use_a_bounded_correlated_internal_error_fallback() {
    let now = Instant::now();
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!("large"), now));
    let response = sent(session.complete(
        Response::new(
            Some(request.id),
            Ok(json!({"nodes":"x".repeat(MCP_FRAME_BYTES)})),
        ),
        now,
    ));
    assert_eq!(response["id"], "large");
    assert_eq!(response["error"]["code"], -32603);
    assert!(response.get("result").is_none());
    assert!(encode(&response).unwrap().len() < MCP_FRAME_BYTES);
    assert!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":"still-ready","method":"ping"}),
            now
        ))
        .get("result")
        .is_some()
    );
}
/** Admitted work times out from original admission even when cancelled; no fake failure claims execution stopped. */
#[test]
fn fake_clock_native_deadline_is_terminal_for_pending_and_cancelled_work() {
    let now = Instant::now();
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!(1), now));
    assert!(
        session
            .poll(now + MCP_REQUEST_DEADLINE - Duration::from_millis(1))
            .is_empty()
    );
    assert!(closed(&session.poll(now + MCP_REQUEST_DEADLINE)));
    assert!(
        session
            .complete(completed(&request), now + MCP_REQUEST_DEADLINE)
            .is_empty()
    );
    let mut session = ready(now);
    call(&mut session, json!(1), now);
    receive(
        &mut session,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}),
        now,
    );
    assert!(closed(&session.poll(now + MCP_REQUEST_DEADLINE)));
}
/** Native envelope faults and malformed scalar successes cannot masquerade as valid tool results. */
#[test]
fn malformed_native_responses_retire_the_session() {
    let now = Instant::now();
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!(1), now));
    let response = Response {
        version: VERSION,
        id: Some(request.id),
        result: Some(json!({})),
        error: Some(EngineError::invalid("invalid")),
    };
    assert!(closed(&session.complete(response, now)));
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!(1), now));
    let actions = session.complete(Response::new(Some(request.id), Ok(json!(7))), now);
    assert!(
        matches!(actions.as_slice(),[Action::Send(value),Action::Close(_)] if value["error"]["code"]==-32603)
    );
}
/** Ordinary native refusals remain useful structured tool errors without terminating an otherwise healthy connection. */
#[test]
fn native_business_errors_preserve_all_fields_as_tool_results() {
    let now = Instant::now();
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!(1), now));
    let mut error = EngineError::new(
        "submission_uncertain",
        "Outcome is uncertain.",
        "Check before retrying.",
    );
    error.previous_page_retained = true;
    error.invalid_node = Some(Box::new(4));
    let expected = serde_json::to_value(&error).unwrap();
    let result = sent(session.complete(Response::new(Some(request.id), Err(error)), now));
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(result["result"]["structuredContent"]["error"], expected);
    assert_eq!(
        serde_json::from_str::<Value>(result["result"]["content"][0]["text"].as_str().unwrap())
            .unwrap(),
        result["result"]["structuredContent"]
    );
    assert!(matches!(
        call(&mut session, json!(2), now).as_slice(),
        [Action::Dispatch(_)]
    ));
}
