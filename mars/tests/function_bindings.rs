/** Real Rust services preserve bound controllers and publish their candidate state atomically. */
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

/** The private fixture has a literal loopback origin and never opens a user's profile. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes())
}
/** Each runtime launches the genuine network, document and V9 Rust subprocesses. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
/** Native events keep ownership epochs, tab isolation and normal mutation authorization. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("function-bindings", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** Node handles are acquired from the current page and include its observed revision. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Assertions inspect the same native text projection available to external agents. */
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

/** Only bound receiver and prefix edges retain this factory's objects and callback after return. */
#[test]
fn bound_controllers_keep_receiver_and_prefix_edges_across_native_events_and_reload() {
    let body = "<input id=name><p id=out>initial</p><button id=run onclick='run()'>Run</button><script>function make(){const receiver={n:0};const prefix={label:'Hello'};const decorate=function(value){return value||'world';};function action(prefix,decorate,kind,value){this.n=this.n+1;return prefix.label+' '+decorate(value)+' '+kind+' '+this.n;}return action.bind(receiver,prefix,decorate).bind({n:900},'bound');}const saved=make();function run(){document.getElementById('out').textContent=saved(document.getElementById('name').value);}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (count, name) in [(1, "Ada"), (2, "Lin"), (3, "Rust 🐙"), (4, "")] {
        let mut params = target(&page, "name");
        params["text"] = json!(name);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
        let expected_name = if name.is_empty() { "world" } else { name };
        assert_eq!(
            text(&page, "out"),
            format!("Hello {expected_name} bound {count}")
        );
    }
    page = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_eq!(text(&page, "out"), "initial");
    page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
    assert_eq!(text(&page, "out"), "Hello world bound 1");
}

/** Both adapters read the current input once and keep their receiver through service serialization. */
#[test]
fn call_and_apply_adapt_current_input_once_per_native_event_adapter() {
    let body = "<input id=name><p id=out>initial</p><button id=run onclick='run()'>Run</button><script>let reads=0;const controller={label:'C',adapt:function(prefix,value,suffix){return this.label+prefix+value+suffix;}};function current(){reads=reads+1;return document.getElementById('name').value;}function run(){const direct=controller.adapt.call(controller,'<',current(),'>');const array=controller.adapt.apply(controller,['[',current(),']']);document.getElementById('out').textContent=direct+' '+array+' '+reads;}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (reads, name) in [(2, "Ada"), (4, "Lin")] {
        let mut params = target(&page, "name");
        params["text"] = json!(name);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
        assert_eq!(text(&page, "out"), format!("C<{name}> C[{name}] {reads}"));
    }
}

/** A handler that throws keeps the receiver, prefix, function, global and Storage effects it made first, as browsers do. */
#[test]
fn throwing_handler_keeps_every_bound_controller_effect_before_the_throw() {
    let body = "<p id=out>initial</p><button id=good onclick='good()'>Good</button><button id=bad onclick='bad()'>Bad</button><button id=read onclick='read()'>Read</button><script>function make(){const receiver={n:0};const record={n:0};function mutate(record,fail){this.n=this.n+1;record.n=record.n+1;mutate.extra=mutate.extra+1;globalThis.last=this.n;window.localStorage.setItem('count',this.n);document.getElementById('out').textContent=this.n+' '+record.n+' '+mutate.extra;if(fail){document.getElementById('missing').textContent='fail';}}mutate.extra=0;return mutate.bind(receiver,record);}const saved=make();function good(){saved(false);}function bad(){saved(true);}function read(){document.getElementById('out').textContent=last+' '+window.localStorage.getItem('count');}</script>";
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "1 1 1");
    let stale = target(&page, "good");
    let page = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&page, "out"), "2 2 2");
    let page = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&page, "out"), "2 2");
    assert_eq!(
        call(&mut runtime, "page.click", stale).unwrap_err().code,
        "revision_conflict"
    );
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "3 3 3");
    let projection = page.to_string();
    assert!(!projection.contains("bound_receiver"));
    assert!(!projection.contains("next_function"));
}

/** Discovery describes only implemented adapters and metadata while retaining the full-JavaScript caveat. */
#[test]
fn capabilities_expose_function_adapters_bound_state_and_supported_metadata() {
    let fixture = fixture("<p>capabilities</p>");
    let mut runtime = runtime(&fixture.url);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    assert!(capabilities["subsets"]["v9"].as_str().unwrap().contains("Function.prototype.call, apply and bind; bound receivers and prefixed arguments; supported function name/length metadata"));
    assert!(
        capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("full_javascript"))
    );
}
