/** Context saturation preserves live pages, rollback and navigation headroom through native APIs. */
#[allow(dead_code)]
mod support;
use mars::protocol::{EngineError, Request};
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner, Permit},
};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, html};

/** Real services with synthetic loopback authority and native human tab ownership. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Address the actual tab without changing the visible selection for reads. */
fn call(
    runtime: &mut Runtime,
    tab: u64,
    op: &str,
    mut params: Value,
) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!(format!("tab-{tab}"));
    }
    runtime.handle_as(
        Request::new("residency", op, params),
        Permit {
            owner: Owner::Human,
            epoch: 1,
            tab_id: op.starts_with("page.").then_some(tab),
        },
    )
}

/** A read-only observation cannot repair or recreate the state being checked. */
fn peek(runtime: &mut Runtime, tab: u64, expression: &str) -> Value {
    call(runtime, tab, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":expression,"options":{"return_by_value":true}}})).unwrap()["result"]["value"].clone()
}

/** Click through current native identity and revision, then let the normal lifecycle settle. */
fn click(runtime: &mut Runtime, id: &str) {
    let page = call(runtime, 1, "page.inspect", json!({})).unwrap();
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    call(runtime, 1, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":node["node_id"]})).unwrap();
}

/** Each child retains its counter across two asynchronous request/reply rounds. */
const CHILD: &str = "<script>let count=0;parent.postMessage('ready','*');addEventListener('message',()=>{parent.postMessage(++count,'*')})</script>";
/** Parent replies twice and records only the second response from each admitted context. */
const PARENT: &str = "let done=0;addEventListener('message',e=>{if(e.data==='ready'||e.data===1)e.source.postMessage('ping','*');if(e.data===2)done++});";

/** Static admission reserves the incoming parent before loading any child. */
#[test]
fn static_saturation_keeps_parent_and_child_realms_operational() {
    let body = format!(
        "{}<script>{PARENT}</script>",
        "<iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe>".repeat(17)
    );
    let (fixture, counts) =
        Fixture::routes_counted(vec![("/", html("", &body)), ("/child", html("", CHILD))]);
    let mut browser = runtime(&fixture.url);
    call(&mut browser, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(counts.lock().unwrap().get("/child").copied(), Some(6));
    assert_eq!(peek(&mut browser, 1, "done"), 6);
}

/** A refused replacement releases its reservation; the old saturated tree still answers. */
#[test]
fn failed_navigation_preserves_state_and_replacement_headroom() {
    let body = format!(
        "{}<script>{PARENT}</script>",
        "<iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe>".repeat(6)
    );
    let oversized = "<p>x</p>".repeat(mars::constants::NODES + 1);
    let fixture = Fixture::routes(vec![
        ("/", html("", &body)),
        ("/child", html("", CHILD)),
        ("/bad", html("", &oversized)),
        ("/next", html("", "<script>let next=42</script>")),
    ]);
    let mut browser = runtime(&fixture.url);
    call(&mut browser, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    for _ in 0..2 {
        assert!(
            call(
                &mut browser,
                1,
                "page.navigate",
                json!({"url":format!("{}/bad",fixture.url)})
            )
            .is_err()
        );
        assert_eq!(peek(&mut browser, 1, "done"), 6);
    }
    call(
        &mut browser,
        1,
        "page.navigate",
        json!({"url":format!("{}/next",fixture.url)}),
    )
    .unwrap();
    assert_eq!(peek(&mut browser, 1, "next"), 42);
}

/** Removing a child releases its lease on publication; a later action can admit a new child. */
#[test]
fn removed_child_capacity_is_reused_without_stale_delivery() {
    let body = format!(
        "<button id=remove>Remove</button><button id=add>Add</button>{}<script>{PARENT}function add(){{let f=document.createElement('iframe');f.src='/child';f.setAttribute('sandbox','allow-scripts allow-same-origin');document.body.appendChild(f)}}document.getElementById('remove').addEventListener('click',()=>document.querySelector('iframe').remove());document.getElementById('add').addEventListener('click',add);</script>",
        "<iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe>".repeat(6)
    );
    let (fixture, counts) =
        Fixture::routes_counted(vec![("/", html("", &body)), ("/child", html("", CHILD))]);
    let mut browser = runtime(&fixture.url);
    call(&mut browser, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(peek(&mut browser, 1, "done"), 6);
    click(&mut browser, "remove");
    click(&mut browser, "add");
    assert_eq!(peek(&mut browser, 1, "done"), 7);
    assert_eq!(counts.lock().unwrap().get("/child").copied(), Some(7));
}

/** Saturation in one tab never evicts another tab; closing one makes room for a new navigation. */
#[test]
fn all_tabs_keep_their_state_at_capacity() {
    let (fixture, counts) =
        Fixture::routes_counted(vec![("/", html("", "<script>let counter=1;</script>"))]);
    let mut browser = runtime(&fixture.url);
    for tab in 1..=7 {
        if tab > 1 {
            call(&mut browser, tab - 1, "tab.open", json!({})).unwrap();
        }
        call(
            &mut browser,
            tab,
            "page.navigate",
            json!({"url":fixture.url}),
        )
        .unwrap();
    }
    call(&mut browser, 7, "tab.open", json!({})).unwrap();
    assert_eq!(
        call(&mut browser, 8, "page.navigate", json!({"url":fixture.url}))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(counts.lock().unwrap().get("/").copied(), Some(7));
    for tab in 1..=7 {
        assert_eq!(peek(&mut browser, tab, "counter"), 1);
    }
    call(&mut browser, 1, "page.navigate", json!({"url":fixture.url})).unwrap();
    call(&mut browser, 1, "tab.close", json!({"tab_id":"tab-1"})).unwrap();
    call(&mut browser, 8, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(peek(&mut browser, 8, "counter"), 1);
    for tab in 2..=7 {
        assert_eq!(peek(&mut browser, tab, "counter"), 1);
    }
}
