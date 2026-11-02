/*! Actual native services admit semicolonless programs without publishing failed candidates. */
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

/** Both successful and failing actions execute ordinary semicolonless source through the services. */
const PAGE: &str = r#"<p id=out>initial</p>
<button id=run type=button onclick='run()'>Run</button>
<button id=bad type=button onclick='bad()'>Bad</button>
<button id=read type=button onclick='read()'>Read</button>
<script>
var count=0
const model={n:0}
function run(){
  count++
  model.n++
  window.localStorage.setItem('count',count)
  document.getElementById('out').textContent=count+' '+model.n
}
function bad(){
  count+=10
  model.n+=10
  window.localStorage.setItem('count',count)
  document.getElementById('out').textContent='proposed'
  unavailable()
}
function read(){
  document.getElementById('out').textContent=count+' '+model.n+' '+window.localStorage.getItem('count')
}
</script>"#;

/** The only HTTP endpoints are disposable explicitly granted loopback fixtures. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes())
}

/** No profile or existing service is shared with the actual three-engine candidate. */
fn runtime(origins: Vec<String>) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins,
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Every mutation retains current tab, control ownership and the exact admitted epoch. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("asi", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}

/** Public handles are derived only from the latest committed inspection. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}

/** People and agents consume this same committed visible output. */
fn text(page: &Value) -> &str {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "out")
        .unwrap()["text"]
        .as_str()
        .unwrap()
}

/** A handler that throws keeps its earlier model, Storage and DOM effects, as browsers do, and the page keeps working. */
#[test]
fn semicolonless_actions_keep_effects_before_a_throw_and_recover() {
    let fixture = fixture(PAGE);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
    assert_eq!(text(&page), "1 1");
    let thrown = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
    assert_eq!(text(&thrown), "proposed");
    let read = call(&mut runtime, "page.click", target(&thrown, "read")).unwrap();
    assert_eq!(text(&read), "11 11 11");
    let recovered = call(&mut runtime, "page.click", target(&read, "run")).unwrap();
    assert_eq!(text(&recovered), "12 12");
    let reloaded = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_ne!(reloaded["page_id"], recovered["page_id"]);
    let reset = call(&mut runtime, "page.click", target(&reloaded, "run")).unwrap();
    assert_eq!(text(&reset), "1 1");
}

/** A later script that fails to parse stops only itself; the earlier script's DOM and storage commit. */
#[test]
fn later_invalid_sources_retain_the_committed_native_page_and_storage() {
    for source in [
        "var first=1 var second=2",
        "function unused(){return\nlet broken=}",
        "function unused(){const f=(n)\n=>n}",
        "function unused(){throw\n1}",
    ] {
        let good = fixture(PAGE);
        let invalid = fixture(&format!(
            "<p id=out>bad page</p><script>window.localStorage.setItem('count','proposed')\ndocument.getElementById('out').textContent='proposed'</script><script>{source}</script>"
        ));
        let mut runtime = runtime(vec![good.url.clone(), invalid.url.clone()]);
        call(&mut runtime, "page.navigate", json!({"url":good.url})).unwrap();
        let page = call(&mut runtime, "page.navigate", json!({"url":invalid.url})).unwrap();
        assert_eq!(text(&page), "proposed", "{source}");
        let diagnostics = page["diagnostics"].to_string();
        assert!(
            diagnostics.contains("Script 2 (inline) did not run"),
            "{source}: {diagnostics}"
        );
        assert!(
            diagnostics.contains("syntax error"),
            "{source}: {diagnostics}"
        );
    }
}

/** Discovery describes scoped grammar admission and retains every original budget. */
#[test]
fn capabilities_disclose_supported_grammar_asi_with_unchanged_budgets() {
    let fixture = fixture(PAGE);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    assert!(
        capabilities["subsets"]["v9"]
            .as_str()
            .unwrap()
            .contains("automatic semicolon insertion in the supported classic-script grammar")
    );
    for (name, expected) in [
        ("tokens", mars::constants::TOKENS),
        ("statements", mars::constants::STATEMENTS),
        ("operations", mars::constants::OPERATIONS),
    ] {
        assert_eq!(capabilities["limits"][name], expected);
    }
}
