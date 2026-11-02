/** Real native services preserve hoisted var controllers and reject invalid document candidates atomically. */
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
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("var-bindings", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
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

/** Hoisted local vars and duplicate functions operate inside a global saved bound callback across service requests. */
#[test]
fn var_controllers_process_current_inputs_via_call_and_apply_and_reload_fresh() {
    let body = "<input id=name><p id=out>initial</p><button id=direct onclick='direct()'>Direct</button><button id=array onclick='array()'>Array</button><script>var reads=0,saved=make();function make(){var count=0;return function(value){var prefix=choose();var prefix;count=count+1;return prefix+' '+value+' '+count;function choose(){return 'old';}function choose(){return 'Hello';}}.bind(null);}function current(){reads=reads+1;return document.getElementById('name').value||'world';}function direct(){var value=current();var value;document.getElementById('out').textContent=saved.call(null,value)+' reads '+reads;}function array(){document.getElementById('out').textContent=saved.apply(null,[current()])+' reads '+reads;}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (count, name, button) in [
        (1, "Ada", "direct"),
        (2, "Lin", "array"),
        (3, "Rust 🐙", "direct"),
    ] {
        let mut params = target(&page, "name");
        params["text"] = json!(name);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, button)).unwrap();
        assert_eq!(
            text(&page, "out"),
            format!("Hello {name} {count} reads {count}")
        );
    }
    page = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_eq!(text(&page, "out"), "initial");
    page = call(&mut runtime, "page.click", target(&page, "array")).unwrap();
    assert_eq!(text(&page, "out"), "Hello world 1 reads 1");
}

/** A handler that throws keeps its captured vars, global var mirrors, receivers, Storage and text from before the throw, and stale handles stay refused. */
#[test]
fn throwing_var_handler_keeps_earlier_effects_and_revision_protection() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>var globalCount=0,saved=make();function make(){var count=0;return function(fail){var next=count+1;var next;count=next;globalCount=globalCount+1;this.n=this.n+1;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count+' '+globalCount+' '+this.n;if(fail){document.getElementById('missing').textContent='fail';}}.bind({n:0});}function good(){saved(false);}function bad(){saved(true);}function read(){document.getElementById('out').textContent=globalCount+' '+this.globalCount+' '+window.localStorage.getItem('count');}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1 1 1");
    let stale = target(&page, "good");
    let page = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&page, "out"), "2 2 2");
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "2 2 2");
    assert_eq!(
        call(&mut runtime, "page.click", stale).unwrap_err().code,
        "revision_conflict"
    );
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "3 3 3");
}

/** Capabilities expose the supported declaration rules without claiming full JavaScript conformance. */
#[test]
fn native_capabilities_describe_var_hoisting_and_supported_redeclarations() {
    let fixture = fixture("<p>capabilities</p>");
    let mut runtime = runtime(&fixture.url);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    assert!(capabilities["subsets"]["v9"].as_str().unwrap().contains("function-scoped and script-global var declarations; declaration hoisting and supported var/function redeclarations"));
    assert!(
        capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("full_javascript"))
    );
}
