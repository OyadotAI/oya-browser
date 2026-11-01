/** A synchronous XMLHttpRequest stops its script until the app answers it, then the step runs again with the answer. */
use crate::common;
use mars_v9::{HostRequest, HostResponse, SyncAnswer};

/** The request a synchronous GET of `url` records, as V9 builds it with no headers or body. */
fn get(url: &str) -> HostRequest {
    HostRequest {
        id: 0,
        kind: "xhr".into(),
        method: "GET".into(),
        url: url.into(),
        headers: Vec::new(),
        body: None,
        body_bytes: Vec::new(),
        blob_source: None,
        credentials: "same-origin".into(),
        mode: "cors".into(),
    }
}

/** A synchronous request with no answer yet stops the script and is recorded for the app. */
#[test]
fn unanswered_synchronous_request_stops_the_script_and_is_recorded() {
    let execution = common::load(
        "var x=new XMLHttpRequest();x.open('GET','https://example.com/a',false);x.send();var after=1;",
    );
    assert_eq!(
        execution.effects.sync_requests,
        vec![get("https://example.com/a")]
    );
    assert!(
        common::failure(&execution)
            .is_some_and(|message| message.contains("synchronous XMLHttpRequest"))
    );
}

/** With the answer in the environment, the same step completes: the status and body are there
before the script's next statement, as a blocked send gives them. */
#[test]
fn answered_synchronous_request_completes_in_the_same_run() {
    let mut environment = common::environment();
    environment.sync_answers.push(SyncAnswer {
        request: get("https://example.com/a"),
        response: HostResponse {
            status: 200,
            status_text: "OK".into(),
            url: "https://example.com/a".into(),
            body: "hi".into(),
            ..HostResponse::default()
        },
    });
    let execution = common::load_with(
        "const x=new XMLHttpRequest();x.open('GET','https://example.com/a',false);x.send();const r=x.status+':'+x.responseText;",
        &environment,
    );
    assert!(execution.effects.sync_requests.is_empty());
    assert!(common::failure(&execution).is_none());
    assert_eq!(common::global(&execution, "r"), "200:hi");
}

/** An answer for a different URL does not complete this request: the app must answer what was asked. */
#[test]
fn answer_for_another_url_does_not_complete_the_request() {
    let mut environment = common::environment();
    environment.sync_answers.push(SyncAnswer {
        request: get("https://example.com/other"),
        response: HostResponse::default(),
    });
    let execution = common::load_with(
        "var x=new XMLHttpRequest();x.open('GET','https://example.com/a',false);x.send();",
        &environment,
    );
    assert_eq!(
        execution.effects.sync_requests,
        vec![get("https://example.com/a")]
    );
}

/** A page that sends the same URL twice gets two requests: each send takes the answer at its own
place in the step, so a repeat is never answered from the first send's answer. */
#[test]
fn repeated_synchronous_request_is_not_answered_by_the_first_answer() {
    let mut environment = common::environment();
    environment.sync_answers.push(SyncAnswer {
        request: get("https://example.com/a"),
        response: HostResponse {
            status: 200,
            body: "first".into(),
            ..HostResponse::default()
        },
    });
    let execution = common::load_with(
        "const x=new XMLHttpRequest();x.open('GET','https://example.com/a',false);x.send();const first=x.responseText;x.open('GET','https://example.com/a',false);x.send();",
        &environment,
    );
    assert_eq!(common::global(&execution, "first"), "first");
    assert_eq!(
        execution.effects.sync_requests,
        vec![get("https://example.com/a")]
    );
}

/** A synchronous body past the request bound fails its request like a network error and records
nothing for the app, so the page keeps running. */
#[test]
fn oversized_synchronous_body_fails_the_request_without_recording_it() {
    let bytes = mars_v9::constants::VM_REQUEST_BODY_BYTES + 1;
    let source = format!(
        "let s='';const x=new XMLHttpRequest();x.open('POST','https://example.com/a',false);x.onerror=()=>{{s='error'}};x.send('a'.repeat({bytes}));if(s==='')s='none';const after=s;"
    );
    let execution = common::load_with(&source, &common::environment());
    assert!(execution.effects.sync_requests.is_empty());
    assert!(common::failure(&execution).is_none());
    assert_eq!(common::global(&execution, "after"), "error");
}

/** A synchronous request the network refused throws NetworkError at send(), where the page's own
try/catch can take it, and the script carries on after the catch. */
#[test]
fn failed_synchronous_request_throws_network_error_that_the_page_catches() {
    let mut environment = common::environment();
    environment.sync_answers.push(SyncAnswer {
        request: get("https://other.example/a"),
        response: HostResponse {
            error: Some("Failed to fetch".into()),
            ..HostResponse::default()
        },
    });
    let execution = common::load_with(
        "let r='';let after='no';const x=new XMLHttpRequest();x.open('GET','https://other.example/a',false);try{x.send()}catch(e){r=e.name}after='yes';",
        &environment,
    );
    assert!(common::failure(&execution).is_none());
    assert_eq!(common::global(&execution, "r"), "NetworkError");
    assert_eq!(common::global(&execution, "after"), "yes");
}
