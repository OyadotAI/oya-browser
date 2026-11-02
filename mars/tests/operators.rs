/** Real Rust service journeys exercise expression references and atomic operator candidate publication. */
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

/** The synthetic fixture binds loopback only and carries no user profile or remote state. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",body.len()).into_bytes())
}
/** Every test launches the genuine network, document and V9 Rust subprocesses. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
/** Agent requests preserve ownership epochs, tab isolation and the existing mutation policy. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("operators", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** Native handles always originate in the observed document and carry its current revision. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Assertions observe the same document text exposed to native agents. */
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

/** Normal input events combine postfix updates, compound sums, typeof and lazy conditional names. */
#[test]
fn operator_input_journeys_persist_counters_and_reload_fresh_state() {
    let body = "<input id=name><p id=out>initial</p><button id=run onclick='run()'>Run</button><script>var count=0;function run(){var old=count++;var total=0;for(var i=1;i<=count;i++){total+=i;}var raw=document.getElementById('name').value;var shown=raw?raw:'world';document.getElementById('out').textContent=typeof shown+' '+shown+' '+old+' '+count+' '+total;}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (name, expected) in [
        ("Ada", "string Ada 0 1 1"),
        ("Lin", "string Lin 1 2 3"),
        ("", "string world 2 3 6"),
    ] {
        let mut params = target(&page, "name");
        params["text"] = json!(name);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
        assert_eq!(text(&page, "out"), expected);
    }
    page = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_eq!(text(&page, "out"), "initial");
    page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
    assert_eq!(text(&page, "out"), "string world 0 1 1");
}

/** A handler that throws keeps its earlier operator updates, the assignment's evaluated right side and Storage, as browsers do. */
#[test]
fn throwing_handler_keeps_operator_updates_storage_and_text_made_before_the_throw() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>var globalCount=0;const array=[0];function holder(){}holder.n=0;function make(){let n=0;return fail=>{const old=n++;++globalCount;array[0]+=2;holder.n++;window.localStorage.setItem('count',globalCount);document.getElementById('out').textContent=old+' '+n+' '+globalCount+' '+array[0]+' '+holder.n;if(fail){document.getElementById('missing').textContent=void(globalCount+=100);}};}const saved=make();function good(){saved(false);}function bad(){saved(true);}function read(){document.getElementById('out').textContent=globalCount+' '+window.localStorage.getItem('count');}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "0 1 1 2 1");
    let stale = target(&page, "good");
    let page = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&page, "out"), "1 2 2 4 2");
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "102 2");
    assert_eq!(
        call(&mut runtime, "page.click", stale).unwrap_err().code,
        "revision_conflict"
    );
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "2 3 103 6 3");
}

/** Repeated update operators consume the request work budget and cannot publish earlier proposals after exhaustion. */
#[test]
fn operator_budget_exhaustion_keeps_the_last_committed_page_and_storage() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>var count=0;function good(){count++;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count;}function bad(){count+=10;window.localStorage.setItem('count',count);document.getElementById('out').textContent='proposed';while(true){count++;}}function read(){document.getElementById('out').textContent=count+' '+window.localStorage.getItem('count');}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1");
    assert_eq!(
        call(&mut runtime, "page.click", target(&page, "bad"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), page);
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "1 1");
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "2");
}
