/*! Real Rust services select ordinary model entries and reject failed predicates atomically. */
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

/** The literal synthetic page has no external dependencies or persistent user profile. */
const CHOOSE: &str = include_str!("../fixtures/choose-option.html");
/** A separate failing page exercises rollback without altering the public compatibility fixture. */
const FAILURE: &str = "<p id=out>initial</p><button id=good type=button onclick='good()'>Good</button><button id=bad type=button onclick='bad()'>Bad</button><button id=read type=button onclick='read()'>Read</button><script>var count=0;const model={n:0};function good(){count++;model.n++;window.localStorage.setItem('count',count);document.getElementById('out').textContent=count+' '+model.n;}function bad(){count+=10;model.n+=10;window.localStorage.setItem('count',count);document.getElementById('out').textContent='proposed';FAILURE;}function read(){document.getElementById('out').textContent=count+' '+model.n+' '+window.localStorage.getItem('count');}</script>";

/** Only an exact owned fixture origin receives the desktop loopback exception. */
fn fixture(body: &str) -> Fixture {
    Fixture::raw(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes())
}
/** Each journey owns the genuine network, document and V9 subprocesses. */
fn runtime(origin: &str, owner: Owner) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        Authority::shared(owner),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
/** All mutation admission retains the observed owner and exact original tab epoch. */
fn call(
    runtime: &mut Runtime,
    owner: Owner,
    op: &str,
    mut params: Value,
) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if owner == Owner::Agent && mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("selection", op, params),
        Permit {
            owner,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** Public handles originate in the latest inspection rather than fixture-local numeric IDs. */
fn target(page: &Value, id: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Assertions consume exactly the text visible to a native agent. */
fn text<'a>(page: &'a Value, id: &str) -> &'a str {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()["text"]
        .as_str()
        .unwrap()
}

/** Both admitted owners use the same ordinary callbacks and publish the same selection results. */
#[test]
fn human_and_agent_selection_use_equalities_and_both_array_predicates() {
    for owner in [Owner::Human, Owner::Agent] {
        let fixture = fixture(CHOOSE);
        let mut runtime = runtime(&fixture.url, owner);
        let mut page = call(
            &mut runtime,
            owner,
            "page.navigate",
            json!({"url":fixture.url}),
        )
        .unwrap();
        assert_eq!(text(&page, "option-status"), "Ready to check.");
        assert_eq!(text(&page, "option-result"), "No option selected.");
        let page_id = page["page_id"].clone();
        for (query, status, detail) in [
            ("2", "Match found.", "Selected: Español (2, es)."),
            ("en", "Match found.", "Selected: English (1, en)."),
            ("ja", "Match found.", "Selected: 日本語 (3, ja)."),
            ("99", "No matching option.", "Try 1, 2, 3, en, es or ja."),
            (
                "",
                "Enter a number or language code.",
                "No option selected.",
            ),
            (
                " \t ",
                "Enter a number or language code.",
                "No option selected.",
            ),
            ("02", "Match found.", "Selected: Español (2, es)."),
            ("  es  ", "Match found.", "Selected: Español (2, es)."),
        ] {
            let mut params = target(&page, "option-query");
            params["text"] = json!(query);
            call(&mut runtime, owner, "page.type", params).unwrap();
            page = call(&mut runtime, owner, "page.inspect", json!({})).unwrap();
            page = call(
                &mut runtime,
                owner,
                "page.click",
                target(&page, "option-check"),
            )
            .unwrap();
            assert_eq!(text(&page, "option-status"), status, "{query}");
            assert_eq!(text(&page, "option-result"), detail, "{query}");
            assert_eq!(page["page_id"], page_id);
        }
        assert_eq!(
            fixture.requests.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        let reset = call(&mut runtime, owner, "page.reload", json!({})).unwrap();
        assert_eq!(text(&reset, "option-status"), "Ready to check.");
    }
}

/** A predicate that throws keeps the globals, heap, Storage and text written before the throw,
as a browser does, and reports the error to the console; an engine limit discards all four and
leaves the next event free to run. */
#[test]
fn failed_selection_candidates_preserve_state_and_allow_the_next_event() {
    let (_thrown, mut runtime, page) = after_good("const result=[1].find(undefined)");
    let failed = call(
        &mut runtime,
        Owner::Agent,
        "page.click",
        target(&page, "bad"),
    )
    .unwrap();
    assert_eq!(text(&failed, "out"), "proposed");
    let console = call(&mut runtime, Owner::Agent, "page.console", json!({})).unwrap();
    assert!(
        console["entries"]
            .to_string()
            .contains("Uncaught TypeError"),
        "{console}"
    );
    let read = call(
        &mut runtime,
        Owner::Agent,
        "page.click",
        target(&failed, "read"),
    )
    .unwrap();
    assert_eq!(text(&read, "out"), "11 11 11");
    let (_fixture, mut runtime, page) =
        after_good("const result=[1].some(value=>{while(true){}return false;})");
    let error = call(
        &mut runtime,
        Owner::Agent,
        "page.click",
        target(&page, "bad"),
    )
    .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.location.is_some());
    assert_eq!(
        call(&mut runtime, Owner::Agent, "page.inspect", json!({})).unwrap(),
        page
    );
    let read = call(
        &mut runtime,
        Owner::Agent,
        "page.click",
        target(&page, "read"),
    )
    .unwrap();
    assert_eq!(text(&read, "out"), "1 1 1");
    let recovered = call(
        &mut runtime,
        Owner::Agent,
        "page.click",
        target(&read, "good"),
    )
    .unwrap();
    assert_eq!(text(&recovered, "out"), "2 2");
}

/** The FAILURE page with `failure` as the bad handler's last statement, after one good click. */
fn after_good(failure: &str) -> (Fixture, Runtime, Value) {
    let fixture = fixture(&FAILURE.replace("FAILURE", failure));
    let mut runtime = runtime(&fixture.url, Owner::Agent);
    let page = call(
        &mut runtime,
        Owner::Agent,
        "page.navigate",
        json!({"url":fixture.url}),
    )
    .unwrap();
    let page = call(
        &mut runtime,
        Owner::Agent,
        "page.click",
        target(&page, "good"),
    )
    .unwrap();
    (fixture, runtime, page)
}

/** Discovery accurately describes added operations while retaining every enforced resource bound. */
#[test]
fn selection_capabilities_disclose_the_bounded_value_and_callback_subset() {
    let fixture = fixture(CHOOSE);
    let mut runtime = runtime(&fixture.url, Owner::Agent);
    let capabilities = call(
        &mut runtime,
        Owner::Agent,
        "runtime.capabilities",
        json!({}),
    )
    .unwrap();
    let language = capabilities["subsets"]["v9"].as_str().unwrap();
    assert!(language.contains("abstract equality (==, !=)"));
    assert!(language.contains("some and find"));
    assert_eq!(
        capabilities["limits"]["operations"],
        mars::constants::OPERATIONS
    );
    assert_eq!(
        capabilities["limits"]["vm_functions"],
        mars::constants::VM_FUNCTIONS
    );
}
