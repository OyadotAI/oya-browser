/** Round-5 "network" ports: XMLHttpRequest's state machine and events as the XHR standard and
Blink's XMLHttpRequest run them (WPT xhr/event-*.any.js, xhr/abort-*, xhr/open-*,
xhr/responsetype.html, xhr/overridemimetype-*): loadstart on send, HEADERS_RECEIVED, LOADING
and DONE each with readystatechange, ProgressEvents, abort's request error steps, and the
canonical statusText when the host passes none. */
use crate::common;
use common::{console, input};
use mars_v9::HostResponse;

/** Load `script`, answer its first request with `response`, and return every console line. */
fn answered(script: &str, response: HostResponse) -> Vec<String> {
    let sources = vec![script.to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let mut out = console(&loaded);
    let response = HostResponse {
        id: loaded.effects.requests[0].id,
        ..response
    };
    let resumed = mars_v9::resume_requests(
        input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    out.extend(console(&resumed));
    out
}

/** Logs every event with readyState; progress events with loaded/total. */
const LOGGER: &str = "const x=new XMLHttpRequest();\
    const log=e=>console.log(e.type,x.readyState,e instanceof ProgressEvent?e.loaded+'/'+e.total:'');\
    for(const t of ['readystatechange','loadstart','progress','load','loadend','error','abort'])x.addEventListener(t,log);";

#[test]
fn a_successful_request_walks_every_state_with_progress_events() {
    let script = format!("{LOGGER}x.open('GET','/a');x.send();");
    let out = answered(
        &script,
        HostResponse {
            status: 200,
            body: "abcd".into(),
            ..HostResponse::default()
        },
    );
    assert_eq!(
        out,
        [
            "readystatechange 1 ",
            "loadstart 1 0/0",
            "readystatechange 2 ",
            "readystatechange 3 ",
            "progress 3 4/4",
            "readystatechange 4 ",
            "load 4 4/4",
            "loadend 4 4/4",
        ]
    );
}

#[test]
fn a_network_error_goes_straight_to_done_with_error_and_loadend() {
    let script = format!("{LOGGER}x.open('GET','/a');x.send();");
    let out = answered(
        &script,
        HostResponse {
            error: Some("Failed to fetch".into()),
            ..HostResponse::default()
        },
    );
    assert_eq!(
        out[2..],
        ["readystatechange 4 ", "error 4 0/0", "loadend 4 0/0"]
    );
}

#[test]
fn abort_in_flight_runs_the_error_steps_then_returns_to_unsent() {
    let out = common::load(&format!(
        "{LOGGER}x.open('GET','/a');x.send();x.abort();console.log('after',x.readyState,x.status);\
         const y=new XMLHttpRequest();y.onabort=()=>console.log('unexpected');y.abort();"
    ));
    assert_eq!(
        console(&out)[2..],
        [
            "readystatechange 4 ",
            "abort 4 0/0",
            "loadend 4 0/0",
            "after 0 0"
        ]
    );
}

#[test]
fn a_handler_that_reopens_stops_the_old_requests_events() {
    let script = "const x=new XMLHttpRequest();x.onreadystatechange=()=>{console.log(x.readyState);\
        if(x.readyState===2)x.open('GET','/b')};x.onload=()=>console.log('load');x.open('GET','/a');x.send();";
    let out = answered(
        script,
        HostResponse {
            status: 200,
            ..HostResponse::default()
        },
    );
    assert_eq!(out, ["1", "2", "1"]);
}

#[test]
fn reopening_an_opened_request_fires_no_second_readystatechange() {
    let out = common::load(
        "const x=new XMLHttpRequest();x.onreadystatechange=()=>console.log(x.readyState);\
         x.open('GET','/a');x.open('GET','/b');",
    );
    assert_eq!(console(&out), ["1"]);
}

#[test]
fn response_type_ignores_unknown_values_and_locks_once_done() {
    let script = "const x=new XMLHttpRequest();x.responseType='bogus';console.log(JSON.stringify(x.responseType));\
        x.responseType='json';x.open('GET','/a');\
        x.onload=()=>{try{x.responseType='text'}catch(e){console.log(e.name)}\
        try{x.responseText}catch(e){console.log(e.name)}try{x.overrideMimeType('text/plain')}catch(e){console.log(e.name)}\
        console.log(x.response.a,x.statusText)};x.send();";
    let out = answered(
        script,
        HostResponse {
            status: 200,
            body: "{\"a\":1}".into(),
            ..HostResponse::default()
        },
    );
    assert_eq!(
        out,
        [
            "\"\"",
            "InvalidStateError",
            "InvalidStateError",
            "InvalidStateError",
            "1 OK"
        ]
    );
}
