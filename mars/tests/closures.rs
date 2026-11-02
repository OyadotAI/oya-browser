/** Real network/document/V9 services exercise closure candidates through the native agent boundary. */
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
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use support::Fixture;

/** Synthetic pages use literal loopback HTTP and never read a user profile. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",body.len()).into_bytes())
}
/** Tests run the actual three Rust services rather than an interpreter mock. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
/** Public mutations retain the observed tab, ownership epoch, page and revision checks. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("closures", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** Handles come only from the currently inspected document. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Text assertions use the same native document projection available to an agent. */
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
/** Each event processes a callback and retains a private count; reload instantiates a fresh controller. */
#[test]
fn greeting_controller_persists_through_native_events_and_resets_on_reload() {
    let fixture = fixture(include_str!("../fixtures/desktop.html"));
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(text(&page, "greeting-count"), "Greetings created: 0");
    for (count, name) in [
        (1, "Ada"),
        (2, "Lin"),
        (3, "Lin"),
        (4, "Rust 🐙"),
        (5, ""),
        (6, "  Ada  "),
        (7, " \t "),
    ] {
        let mut params = target(&page, "name");
        params["text"] = json!(name);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "greet")).unwrap();
        assert_eq!(
            text(&page, "greeting-count"),
            format!("Greetings created: {count}")
        );
        assert_eq!(
            text(&page, "result"),
            format!(
                "Hello, {}! From your own Rust V9.",
                if name.trim().is_empty() {
                    "world"
                } else {
                    name.trim()
                }
            )
        );
    }
    page = call(&mut runtime, "page.click", target(&page, "reset")).unwrap();
    assert_eq!(text(&page, "greeting-count"), "Greetings created: 7");
    page = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_eq!(text(&page, "greeting-count"), "Greetings created: 0");
    assert_eq!(text(&page, "result"), "Your greeting appears here.");
}
/** A handler that throws keeps the captured cells, aliases, Storage and text it changed first, as browsers do. */
#[test]
fn throwing_closure_keeps_earlier_heap_storage_and_document_effects() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><script>function make(){let count=0;const box={count:0};return function(fail){count=count+1;box.count=box.count+1;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count+' '+box.count;if(fail){document.getElementById('missing').textContent='fail';}};}const controller=make();function good(){controller(false);}function bad(){controller(true);}function read(){document.getElementById('out').textContent=window.localStorage.getItem('count');}</script><button id=read onclick='read()'>Read</button>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1 1");
    let stale = target(&page, "good");
    let page = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&page, "out"), "2 2");
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "2");
    assert_eq!(
        call(&mut runtime, "page.click", stale).unwrap_err().code,
        "revision_conflict"
    );
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "3 3");
    let output = page.to_string();
    assert!(!output.contains("scope_id"));
    assert!(!output.contains("next_function"));
    assert!(!output.contains("function self"));
}
/** Exhausting the work budget stops the handler uncatchably and discards its counter, heap, Storage and text changes. */
#[test]
fn work_budget_failure_rolls_back_all_native_candidates() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>function make(){let count=0;const box={n:0};return fail=>{count=count+1;box.n=box.n+1;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count+' '+box.n;if(fail){try{for(;;){}}catch(error){}}};}const controller=make();function good(){controller(false);}function bad(){controller(true);}function read(){document.getElementById('out').textContent=window.localStorage.getItem('count');}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1 1");
    let error = call(&mut runtime, "page.click", target(&page, "bad")).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), page);
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "1");
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "2 2");
}
/** A step that fails after it wrote the title and the DOM leaves the committed document as it was: the candidate copied the shared document on its first write, and the copy goes with it. */
#[test]
fn a_discarded_step_leaves_the_committed_document_untouched() {
    let body = "<title>committed</title><p id=out>initial</p><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>function bad(){document.title='candidate';document.getElementById('out').textContent='candidate';try{for(;;){}}catch(error){}}function read(){document.getElementById('out').textContent=document.title;}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let before = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let error = call(&mut runtime, "page.click", target(&page, "bad")).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(
        call(&mut runtime, "page.inspect", json!({})).unwrap(),
        before
    );
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "committed");
}
/** Capability discovery advertises implemented closures and explicit graph caps without claiming complete JS. */
#[test]
fn native_capabilities_expose_closures_and_their_bounds() {
    let fixture = fixture("<p>capabilities</p>");
    let mut runtime = runtime(&fixture.url);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    let v9 = capabilities["subsets"]["v9"].as_str().unwrap();
    assert!(v9.contains("lexical closures with shared captured bindings"));
    assert!(v9.contains(
        "arrow functions with simple parameters, expression/block bodies and lexical capture"
    ));
    assert!(v9.contains("receiver-aware ordinary function calls; this expressions; lexical this capture in arrows; supported strict and non-strict receiver behavior"));
    assert_eq!(
        capabilities["limits"]["vm_environments"],
        mars::constants::VM_ENVIRONMENTS
    );
    assert_eq!(
        capabilities["limits"]["vm_functions"],
        mars::constants::VM_FUNCTIONS
    );
    assert_eq!(
        capabilities["limits"]["vm_captured_bindings"],
        mars::constants::VM_BINDINGS
    );
    assert!(
        capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("full_javascript"))
    );
}
/** A controller retained only by this edges keeps its global property, Storage and DOM effects when it later throws. */
#[test]
fn receiver_candidates_commit_across_the_three_native_services_even_when_the_handler_throws() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>function make(){return {n:0,create:function(){'use strict';return fail=>{this.n=this.n+1;globalThis.last=this.n;window.localStorage.setItem('count',this.n);document.getElementById('out').textContent=this.n;if(fail){document.getElementById('missing').textContent='fail';}};}}.create();}const saved=make();function good(){saved(false);}function bad(){saved(true);}function read(){document.getElementById('out').textContent=last+' '+window.localStorage.getItem('count');}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1");
    let page = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&page, "out"), "2");
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "2 2");
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "3");
}
