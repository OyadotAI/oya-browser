/* The conformance inspector: a page-side `send` that answers protocol requests through `receive`. */
#![cfg(feature = "conformance")]
use crate::common;

/** Loads a page with the inspector enabled and returns the environment for it. */
fn inspector_environment() -> mars_v9::PageEnvironment {
    let mut environment = common::environment();
    environment.conformance_inspector = true;
    environment
}

/** `send` is exposed only when the shell flag enables the inspector; a page without it has no agent. */
#[test]
fn inspector_send_is_absent_without_the_flag() {
    assert_eq!(common::eval("const r = typeof send;", "r"), "undefined");
}

/** Runtime.evaluate returns the completion value of the expression as its RemoteObject. */
#[test]
fn inspector_evaluate_returns_the_completion_value() {
    let source = r#"const replies = [];
globalThis.receive = (text) => replies.push(text);
send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: '1 + 2' } }));
const r = JSON.parse(replies[0]).result.result.value === 3 && JSON.parse(replies[0]).id === 1;"#;
    let execution = common::load_with(source, &inspector_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** A throwing expression answers with exceptionDetails, so the caller can see the failure. */
#[test]
fn inspector_evaluate_reports_a_thrown_exception_in_details() {
    let source = r#"const replies = [];
globalThis.receive = (text) => replies.push(text);
send(JSON.stringify({ id: 2, method: 'Runtime.evaluate', params: { expression: "throw new Error('boom')" } }));
const r = JSON.parse(replies[0]).result.exceptionDetails !== undefined;"#;
    let execution = common::load_with(source, &inspector_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** An unknown method is a JSON-RPC error that keeps the request id, not a crash. */
#[test]
fn inspector_rejects_unknown_methods_with_a_protocol_error() {
    let source = r#"const replies = [];
globalThis.receive = (text) => replies.push(text);
send(JSON.stringify({ id: 7, method: 'Nope.nothing' }));
const reply = JSON.parse(replies[0]);
const r = reply.id === 7 && reply.error !== undefined;"#;
    let execution = common::load_with(source, &inspector_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** A pause is refused until Debugger.enable, as V8 refuses it; the refusal is an error reply. */
#[test]
fn inspector_refuses_pause_before_debugger_enable() {
    let source = r#"const replies = [];
globalThis.receive = (text) => replies.push(text);
send(JSON.stringify({ id: 3, method: 'Debugger.pause' }));
const r = JSON.parse(replies[0]).error !== undefined;"#;
    let execution = common::load_with(source, &inspector_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}
