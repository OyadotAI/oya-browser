/** Cancellation suppresses responses and carries only exact native authority metadata. */
use super::*;
use mars::constants::MCP_PENDING_REQUESTS;
/** Typed IDs match only the corresponding pending native request and cancellation emits once without acknowledgement. */
#[test]
fn cancellation_preserves_typed_matching_and_suppresses_late_native_replies() {
    let now = Instant::now();
    let mut session = ready(now);
    let number = dispatched(call(&mut session, json!(1), now));
    let text = dispatched(call(&mut session, json!("1"), now));
    let cancellation = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"1","reason":"ignored"}});
    let actions = receive(&mut session, cancellation.clone(), now);
    assert!(
        matches!(actions.as_slice(),[Action::Cancel{native_id,mutating:false,..}] if native_id==&text.id)
    );
    assert!(receive(&mut session, cancellation, now).is_empty());
    assert!(session.complete(completed(&text), now).is_empty());
    assert_eq!(sent(session.complete(completed(&number), now))["id"], 1);
    assert!(closed(&call(&mut session, json!("1"), now)));
}
/** A rounded fractional cancellation token must not cancel an integral pending request. */
#[test]
fn cancellation_uses_exact_numeric_tokens_and_ignores_unknown_or_completed_ids() {
    let now = Instant::now();
    let mut session = ready(now);
    let request = dispatched(call(&mut session, json!(1), now));
    assert!(session.receive(br#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1.00000000000000001}}"#,now).is_empty());
    assert!(receive(&mut session,json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"unknown"}}),now).is_empty());
    assert!(
        sent(session.complete(completed(&request), now))
            .get("result")
            .is_some()
    );
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}),
            now
        )
        .is_empty()
    );
}
/** Cancel actions carry supplied tab and epoch, using the shared native mutation policy without argument retention. */
#[test]
fn mutation_cancellation_retains_exact_authority_metadata_without_typed_text() {
    let now = Instant::now();
    let mut session = ready(now);
    let request = dispatched(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":"type","method":"tools/call","params":{"name":"page_type","arguments":{"tab_id":"tab","control_epoch":9,"page_id":"page","node_id":"node","expected_revision":4,"text":"do-not-retain-password"}}}),
        now,
    ));
    let actions = receive(
        &mut session,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"type"}}),
        now,
    );
    assert!(
        matches!(actions.as_slice(),[Action::Cancel{native_id,tab_id:Some(tab),control_epoch:Some(9),mutating:true}] if native_id==&request.id && tab=="tab")
    );
    assert!(!format!("{actions:?}").contains("do-not-retain-password"));
}
/** Cancelled work still occupies a slot until native completion so cancellation cannot bypass admission bounds. */
#[test]
fn cancelled_requests_retain_pending_slots_until_late_replies_are_drained() {
    let now = Instant::now();
    let mut session = ready(now);
    let mut requests = Vec::new();
    for index in 0..MCP_PENDING_REQUESTS {
        requests.push(dispatched(call(&mut session, json!(index), now)));
        assert!(matches!(receive(&mut session,json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":index}}),now).as_slice(),[Action::Cancel{..}]));
    }
    assert_eq!(
        sent(call(&mut session, json!("full"), now))["error"]["code"],
        -32000
    );
    assert!(session.complete(completed(&requests[0]), now).is_empty());
    assert!(matches!(
        call(&mut session, json!("released"), now).as_slice(),
        [Action::Dispatch(_)]
    ));
}
