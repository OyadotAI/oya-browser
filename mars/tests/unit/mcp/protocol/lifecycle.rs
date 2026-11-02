/** Initialization readiness, version offers and clock bounds never imply native mutation authority. */
use super::*;
use mars::constants::{MCP_INITIALIZE_DEADLINE, MCP_PROTOCOL_VERSION};
use std::time::Duration;
/** Ping works before initialization, while native tools wait for the initialized notification. */
#[test]
fn ping_and_tools_obey_both_initialization_stages() {
    let now = Instant::now();
    let mut session = Session::new(now);
    assert_eq!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
            now
        ))["result"],
        json!({})
    );
    assert_eq!(
        sent(call(&mut session, json!(2), now))["error"]["code"],
        -32000
    );
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            now
        )
        .is_empty()
    );
    let init = json!({"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"different","capabilities":{},"clientInfo":{"name":"test","version":"1"}}});
    let result = sent(receive(&mut session, init, now));
    assert_eq!(result["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
    assert_eq!(result["result"]["capabilities"], json!({"tools":{}}));
    assert_eq!(
        sent(call(&mut session, json!(4), now))["error"]["code"],
        -32000
    );
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            now
        )
        .is_empty()
    );
    assert!(matches!(
        call(&mut session, json!(5), now).as_slice(),
        [Action::Dispatch(_)]
    ));
}
/** Initialization is not replayable under a new ID and protocol errors do not accidentally grant readiness. */
#[test]
fn duplicate_initialize_and_malformed_parameters_are_protocol_errors() {
    let now = Instant::now();
    let mut session = ready(now);
    assert_eq!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            now
        ))["error"]["code"],
        -32600
    );
    let mut fresh = Session::new(now);
    for (index,params) in [json!({}),json!({"protocolVersion":"pin","capabilities":[],"clientInfo":{"name":"t","version":"1"}}),json!({"protocolVersion":"pin","capabilities":{},"clientInfo":{"name":"t","version":"1"},"task":{}})].into_iter().enumerate() {
        assert_eq!(sent(receive(&mut fresh,json!({"jsonrpc":"2.0","id":index,"method":"initialize","params":params}),now))["error"]["code"],-32602);
    }
}
/** Standard client implementation metadata is accepted as inert information and is never echoed. */
#[test]
fn implementation_metadata_and_meta_are_accepted_without_echo() {
    let now = Instant::now();
    let mut session = Session::new(now);
    let result = sent(receive(
        &mut session,
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"experimental":{"value":true}},"clientInfo":{"name":"secret-client","version":"1","title":"Title","description":"Details","websiteUrl":"https://example.invalid","icons":[{"src":"data:image/png;base64,A","mimeType":"image/png","sizes":["any"],"theme":"dark"}]},"_meta":{"request":"secret"}}}),
        now,
    ));
    assert!(result.get("result").is_some());
    assert!(!result.to_string().contains("secret"));
    assert!(
        receive(
            &mut session,
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{"_meta":{}}}),
            now
        )
        .is_empty()
    );
    assert!(matches!(
        call(&mut session, json!(1), now).as_slice(),
        [Action::Dispatch(_)]
    ));
}
/** Pings cannot extend initialization, while fully ready idle connections have no idle timeout. */
#[test]
fn fake_clock_enforces_initialization_deadline_without_ready_idle_timeout() {
    let now = Instant::now();
    let mut session = Session::new(now);
    let just_before = now + MCP_INITIALIZE_DEADLINE - Duration::from_millis(1);
    assert!(
        sent(receive(
            &mut session,
            json!({"jsonrpc":"2.0","id":"ping","method":"ping"}),
            just_before
        ))
        .get("result")
        .is_some()
    );
    assert!(closed(&session.poll(now + MCP_INITIALIZE_DEADLINE)));
    assert!(session.poll(now + MCP_INITIALIZE_DEADLINE).is_empty());
    assert!(call(&mut session, json!(1), now).is_empty());
    assert!(ready(now).poll(now + Duration::from_secs(1000)).is_empty());
}
