/*! Real Rust services preserve spread identities and atomically reject failed expansion. */
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

/** The source and copied arrays retain the same object through successive real events. */
const ALIASES: &str = "<p id=out>initial</p><button id=run type=button onclick='run()'>Run</button><script>const model=[{n:0}];const sparse=[];sparse.length=3;sparse[1]=model[0];const copy=[...sparse,...'A😀B'];function run(){model[0].n++;let visits=0;copy.map(value=>{visits++;return value;});document.getElementById('out').textContent=copy[1].n+' '+copy[4].length+' '+copy.length+' '+visits+' '+copy.some(value=>value===undefined);}</script>";
/** A later spread failure in `bad` follows an earlier model, Storage and DOM change. */
const FAILURE: &str = "<p id=out>initial</p><button id=good type=button onclick='good()'>Good</button><button id=bad type=button onclick='bad()'>Bad</button><button id=read type=button onclick='read()'>Read</button><script>var count=0;const model=[{n:0}];function good(){count++;model[0].n++;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count+' '+model[0].n;}function bad(){count+=10;model[0].n+=10;window.localStorage.setItem('count',count);document.getElementById('out').textContent='proposed';FAILURE;}function read(){document.getElementById('out').textContent=count+' '+model[0].n+' '+window.localStorage.getItem('count');}</script>";

/** The only network access is the explicitly granted disposable loopback origin. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes())
}
/** Each test owns all three actual Rust services and no saved user profile. */
fn runtime(origin: &str) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
/** Public native mutations preserve the exact tab, owner and admitted epoch. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("spread", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** DOM identities always come from the current committed inspection. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Literal visible output is shared by people, native painting and agent inspection. */
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

/** Sparse materialization, shallow aliases and astral code points survive service round trips. */
#[test]
fn spread_values_keep_identity_dense_holes_and_utf16_pairs_between_native_events() {
    let fixture = fixture(ALIASES);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let identity = page["page_id"].clone();
    for expected in ["1 2 6 6 true", "2 2 6 6 true", "3 2 6 6 true"] {
        page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
        assert_eq!(text(&page), expected);
        assert_eq!(page["page_id"], identity);
    }
    let reloaded = call(&mut runtime, "page.reload", json!({})).unwrap();
    assert_ne!(reloaded["page_id"], identity);
    let fresh = call(&mut runtime, "page.click", target(&reloaded, "run")).unwrap();
    assert_eq!(text(&fresh), "1 2 6 6 true");
}

/** A spread that throws keeps the handler's earlier globals, heap and Storage effects, as browsers do. */
#[test]
fn throwing_spread_keeps_earlier_handler_effects_and_recovers() {
    for failure in [
        "const result=[...model,...null]",
        "const result=[...model,...({length:3})]",
    ] {
        let fixture = fixture(&FAILURE.replace("FAILURE", failure));
        let mut runtime = runtime(&fixture.url);
        let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
        let thrown = call(&mut runtime, "page.click", target(&page, "bad")).unwrap();
        assert_eq!(text(&thrown), "proposed", "{failure}");
        let read = call(&mut runtime, "page.click", target(&thrown, "read")).unwrap();
        assert_eq!(text(&read), "11 11 11", "{failure}");
        let next = call(&mut runtime, "page.click", target(&read, "good")).unwrap();
        assert_eq!(text(&next), "12 12", "{failure}");
    }
}

/** Expansion past the array bound stops the handler uncatchably and discards the whole candidate. */
#[test]
fn exhausted_spread_rolls_back_the_complete_native_candidate_and_recovers() {
    let failure = "let values=[];for(let i=0;i<20;i++){values=[...values,...values,'x'];}";
    let fixture = fixture(&FAILURE.replace("FAILURE", failure));
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    let error = call(&mut runtime, "page.click", target(&page, "bad")).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), page);
    let read = call(&mut runtime, "page.click", target(&page, "read")).unwrap();
    assert_eq!(text(&read), "1 1 1");
    let next = call(&mut runtime, "page.click", target(&read, "good")).unwrap();
    assert_eq!(text(&next), "2 2");
}

/** Clients discover the actual bounded literal feature without a generic iterator claim. */
#[test]
fn capabilities_name_only_the_owned_array_and_string_spread_subset() {
    let fixture = fixture(ALIASES);
    let mut runtime = runtime(&fixture.url);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    let language = capabilities["subsets"]["v9"].as_str().unwrap();
    assert!(language.contains("array-literal spread of owned arrays and primitive strings"));
    assert!(language.contains("bounded code-point string iteration"));
    assert_eq!(
        capabilities["limits"]["operations"],
        mars::constants::OPERATIONS
    );
    assert_eq!(
        capabilities["limits"]["vm_array_length"],
        mars::constants::VM_ARRAY_LENGTH
    );
}
