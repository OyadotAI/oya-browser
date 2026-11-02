/** Real native services execute programmatic click listeners with ownership and rollback intact. */
#[allow(dead_code)]
mod support;
use mars::{
    protocol::{EngineError, Request},
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit},
    },
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use support::Fixture;

/** Synthetic loopback pages isolate the test from user profiles and remote network state. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",body.len()).into_bytes())
}
/** The coordinator launches the actual Rust network, document and V9 subprocesses. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
/** Native requests preserve the public ownership epoch, tab and mutation checks. */
fn call(runtime: &mut Runtime, op: &str, params: Value) -> Result<Value, EngineError> {
    call_as(
        runtime,
        op,
        params,
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** Explicit admissions let control and tab tests exercise the same public dispatch boundary. */
fn call_as(
    runtime: &mut Runtime,
    op: &str,
    mut params: Value,
    permit: Permit,
) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!(format!("tab-{}", permit.tab_id.unwrap()));
    }
    if mars::runtime::mutating(op) && permit.owner == Owner::Agent {
        params["control_epoch"] = json!(permit.epoch);
    }
    runtime.handle_as(Request::new("dom-listeners", op, params), permit)
}

/** Every handle carries the currently inspected page and document revision. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Observable text comes from the same document projection used by native agents. */
fn text(page: &Value, id: &str) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .into()
}

#[path = "dom_listeners/control.rs"]
mod control;
#[path = "dom_listeners/inline.rs"]
mod inline;
#[path = "dom_listeners/journey.rs"]
mod journey;
#[path = "dom_listeners/strings.rs"]
mod strings;
