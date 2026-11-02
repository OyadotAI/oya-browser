/** Strict envelopes preserve notification silence and distinguish protocol failures from native tool failures. */
use super::*;
/** Parse and envelope failures omit unrecoverable IDs instead of inventing JSON-RPC null correlations. */
#[test]
fn parse_batch_and_invalid_id_errors_have_no_fabricated_null_id() {
    let now = Instant::now();
    let mut session = ready(now);
    let parse = sent(session.receive(b"{broken", now));
    assert_eq!(parse["error"]["code"], -32700);
    assert!(!parse.as_object().unwrap().contains_key("id"));
    for value in [
        json!([]),
        json!({}),
        json!({"jsonrpc":"2.0","method":"ping","id":null}),
        json!({"jsonrpc":"2.0","method":"ping","id":1.5}),
        json!({"jsonrpc":"2.0","method":"ping","id":9007199254740992u64}),
    ] {
        let response = sent(receive(&mut session, value, now));
        assert_eq!(response["error"]["code"], -32600);
        assert!(!response.as_object().unwrap().contains_key("id"));
    }
}
/** Unknown, invalid and duplicate initialized notifications produce no replies or unintended dispatch. */
#[test]
fn notifications_are_silent_even_when_unknown_or_malformed() {
    let now = Instant::now();
    let mut session = ready(now);
    for value in [
        json!({"jsonrpc":"2.0","method":"unknown","params":{}}),
        json!({"jsonrpc":"bad","method":"ping"}),
        json!({"jsonrpc":"2.0","method":false}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":null}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized","unknown":true}),
    ] {
        assert!(receive(&mut session, value, now).is_empty());
    }
    assert!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
            now
        ))
        .get("result")
        .is_some()
    );
}
/** Well-formed client responses are irrelevant because this server issues no client-directed requests. */
#[test]
fn client_responses_are_ignored_without_consuming_request_ids() {
    let now = Instant::now();
    let mut session = ready(now);
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":7,"result":{}}),
            now
        )
        .is_empty()
    );
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":"e","error":{"code":-1,"message":"ignored","data":{}}}),
            now
        )
        .is_empty()
    );
    assert_eq!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":7,"method":"ping"}),
            now
        ))["id"],
        7
    );
}
/** Valid IDs survive invalid envelope errors and remain permanently consumed afterward. */
#[test]
fn envelope_error_ids_are_recoverable_and_cannot_be_reused() {
    let now = Instant::now();
    let mut session = ready(now);
    let response = sent(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":"bad-envelope","method":"ping","extra":true}),
        now,
    ));
    assert_eq!(response["id"], "bad-envelope");
    assert_eq!(response["error"]["code"], -32600);
    assert!(closed(&receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":"bad-envelope","method":"ping"}),
        now
    )));
}
/** Cursor, task augmentation and wrapper types are protocol failures; catalog argument refusals are tool errors. */
#[test]
fn strict_tool_wrappers_and_discovery_reject_unimplemented_protocol_features() {
    let now = Instant::now();
    let mut session = ready(now);
    for (index, method, params) in [
        (1, "tools/list", json!({"cursor":"next"})),
        (
            2,
            "tools/call",
            json!({"name":"runtime_capabilities","task":{}}),
        ),
        (
            3,
            "tools/call",
            json!({"name":"runtime_capabilities","arguments":[]}),
        ),
        (4, "tools/call", json!({"name":"missing"})),
        (5, "ping", json!({"_meta":false})),
    ] {
        assert_eq!(
            sent(receive(
                &mut session,
                json!({"jsonrpc":"2.0","id":index,"method":method,"params":params}),
                now
            ))["error"]["code"],
            -32602
        );
    }
    let response = sent(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"page_inspect","arguments":{}}}),
        now,
    ));
    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        response["result"]["structuredContent"]["error"]["code"],
        "invalid_request"
    );
    let listed = sent(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":7,"method":"tools/list","params":{"cursor":"","_meta":{}}}),
        now,
    ));
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 30);
    assert!(listed["result"].get("nextCursor").is_none());
    assert_eq!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":8,"method":"server/discover"}),
            now
        ))["error"]["code"],
        -32601
    );
}
/** Duplicate top-level IDs are ambiguous and therefore cannot be attached to an error response. */
#[test]
fn duplicate_envelope_members_do_not_silently_change_request_intent() {
    let now = Instant::now();
    let mut session = ready(now);
    let response =
        sent(session.receive(br#"{"jsonrpc":"2.0","id":1,"id":2,"method":"ping"}"#, now));
    assert_eq!(response["error"]["code"], -32600);
    assert!(response.get("id").is_none());
    let response = sent(session.receive(
        br#"{"jsonrpc":"2.0","id":"key","method":"ping","method":"tools/list"}"#,
        now,
    ));
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(response["id"], "key");
}
