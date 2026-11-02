/** Actual Rust services share flex geometry, form behavior and immutable agent identities. */
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

/** Raw fixtures embed their required stylesheet rather than serving an unrelated HTML response as CSS. */
fn forms() -> String {
    include_str!("../fixtures/forms.html").replace(
        "<link rel=stylesheet href=/forms.css>",
        &format!("<style>{}</style>", include_str!("../fixtures/forms.css")),
    )
}

/** Synthetic pages run on an exact-granted loopback origin, without a user profile. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes())
}

/** Network, document and V9 execution use the actual supervised subprocesses. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Every agent request retains owner, exact tab and epoch checks. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("flex-forms", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}

/** Controls are located in the same inspection that supplies native widget bounds. */
fn node<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()
}

/** Each mutation uses the currently observed generation and revision. */
fn target(page: &Value, id: &str) -> Value {
    json!({"page_id":page["page_id"],"node_id":node(page,id)["node_id"],"expected_revision":page["revision"]})
}

/** Numeric comparisons tolerate only inconsequential binary64 rounding. */
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

/** Agent geometry agrees with the committed native display item for every visible form control. */
fn assert_native_bounds(runtime: &Runtime, page: &Value) {
    let snapshot = runtime.snapshot();
    let display = snapshot.display.unwrap();
    for id in [
        "user", "pass", "region", "notes", "remember", "standard", "plus", "submit",
    ] {
        let value = node(page, id);
        let local = value["node_id"]
            .as_str()
            .unwrap()
            .rsplit('-')
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let painted = display
            .items
            .iter()
            .find(|item| item.local_id == local)
            .unwrap();
        assert_eq!(
            value["bounds"],
            json!({"x":painted.x,"y":painted.y,"width":painted.width,"height":painted.height,"unit":"logical_points"})
        );
    }
}

#[path = "flex_forms/geometry.rs"]
mod geometry;
#[path = "flex_forms/server.rs"]
mod server;
#[path = "flex_forms/submission.rs"]
mod submission;
