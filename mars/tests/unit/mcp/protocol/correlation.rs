/** External IDs preserve type, exact safe integral meaning and whole-session uniqueness. */
use super::*;
use mars::constants::ID_BYTES;
/** Numeric and string IDs with matching digits correlate independently through private native IDs. */
#[test]
fn typed_external_ids_never_enter_native_request_correlation() {
    let now = Instant::now();
    let mut session = ready(now);
    let numeric = dispatched(call(&mut session, json!(1), now));
    let text = dispatched(call(&mut session, json!("1"), now));
    assert_ne!(numeric.id, text.id);
    assert_ne!(numeric.id, "1");
    assert!(numeric.id.starts_with("mcp-"));
    assert_eq!(sent(session.complete(completed(&text), now))["id"], "1");
    assert_eq!(sent(session.complete(completed(&numeric), now))["id"], 1);
    assert!(closed(&call(&mut session, json!(1), now)));
}
/** Equivalent decimal and exponent spellings share the same duplicate-detection identity. */
#[test]
fn equivalent_integral_numeric_spellings_cannot_reuse_a_session_id() {
    let now = Instant::now();
    for token in ["1.0", "1e0", "10e-1", "0.1e1"] {
        let mut session = ready(now);
        let first = format!(r#"{{"jsonrpc":"2.0","id":{token},"method":"ping"}}"#);
        assert!(
            sent(session.receive(first.as_bytes(), now))
                .get("result")
                .is_some()
        );
        assert!(
            closed(&receive(
                &mut session,
                json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
                now
            )),
            "{token}"
        );
    }
}
/** Exact decimal validation rejects values that binary64 parsing would round to an integral ID. */
#[test]
fn fractional_rounding_does_not_create_an_accepted_integer_id() {
    let now = Instant::now();
    let mut session = ready(now);
    for token in [
        "1.00000000000000001",
        "9007199254740991.1",
        "0.99999999999999999999",
        "1e-50",
    ] {
        let frame = format!(r#"{{"jsonrpc":"2.0","id":{token},"method":"ping"}}"#);
        let response = sent(session.receive(frame.as_bytes(), now));
        assert_eq!(response["error"]["code"], -32600, "{token}");
        assert!(response.get("id").is_none());
    }
    assert_eq!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
            now
        ))["id"],
        1
    );
}
/** Empty and escaped strings are legitimate external IDs while UTF-8 byte bounds remain explicit. */
#[test]
fn external_string_ids_allow_empty_control_and_escaped_characters_with_byte_caps() {
    let now = Instant::now();
    let mut session = ready(now);
    for id in ["", "\n\"\\", "é"] {
        assert_eq!(
            sent(receive(
                &mut session,
                json!({"jsonrpc":"2.0","id":id,"method":"ping"}),
                now
            ))["id"],
            id
        );
    }
    let id = "é".repeat(ID_BYTES);
    let response = sent(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":id,"method":"ping"}),
        now,
    ));
    assert_eq!(response["error"]["code"], -32600);
    assert!(response.get("id").is_none());
}
/** Missing, unknown and repeated native response correlations retire the connection without misrouting results. */
#[test]
fn invalid_native_response_correlations_are_terminal_and_idempotent() {
    let now = Instant::now();
    let mut session = ready(now);
    assert!(closed(
        &session.complete(Response::new(None, Ok(json!({}))), now)
    ));
    assert!(
        session
            .complete(Response::new(Some("unknown".into()), Ok(json!({}))), now)
            .is_empty()
    );
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!(1), now));
    assert!(
        sent(session.complete(completed(&request), now))
            .get("result")
            .is_some()
    );
    assert!(closed(&session.complete(completed(&request), now)));
}
