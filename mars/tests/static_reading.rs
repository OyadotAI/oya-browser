/** Static reading crosses the real supervised services without rewriting the retained public fixtures. */
#[allow(dead_code)]
mod support;
use mars::{
    document::{Appearance, RenderContext},
    protocol::{EngineError, Request},
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit},
    },
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use support::Fixture;

/** Loopback is explicitly granted and all responses remain deterministic in-memory fixtures. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes())
}

/** These tests launch only the owned Rust network, document and V9 engines. */
fn runtime(origins: &[&str]) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins.iter().map(|origin| (*origin).into()).collect(),
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Agent mutations carry the current tab's real owner and epoch rather than bypassing admission. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("static-reading", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}

/** Native tab selections use their existing local human action path without changing page ownership. */
fn human_tab(runtime: &mut Runtime, op: &str, params: Value) -> Value {
    runtime
        .handle_as(
            Request::new("static-reading-tab", op, params),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: None,
            },
        )
        .unwrap()
}

/** Observed node IDs remain authoritative across edits, presentation updates and tab activation. */
fn node<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()
}

/** Each action names the exact current document revision. */
fn target(page: &Value, id: &str) -> Value {
    json!({"page_id":page["page_id"],"node_id":node(page,id)["node_id"],"expected_revision":page["revision"]})
}

#[path = "static_reading/failures.rs"]
mod failures;
#[path = "static_reading/paragraph.rs"]
mod paragraph;
#[path = "static_reading/presentation.rs"]
mod presentation;
