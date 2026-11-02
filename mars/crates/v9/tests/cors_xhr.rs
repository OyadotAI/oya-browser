/** Round-2 "cors" owner ports: XMLHttpRequest's open, setRequestHeader, withCredentials and
response-header rules from the XHR standard as Blink's XMLHttpRequest implements them (WPT
cors/request-headers.htm and xhr/). */
use crate::common;
use common::{console, input};
use mars_v9::HostResponse;

/** Run one script and return its console lines. */
fn lines(script: &str) -> Vec<String> {
    console(&common::load(script))
}

#[test]
fn set_request_header_before_open_is_an_invalid_state_error() {
    let out = lines(
        "const x=new XMLHttpRequest();try{x.setRequestHeader('x-a','1')}catch(e){console.log(e.name,e instanceof DOMException)}",
    );
    assert_eq!(out, ["InvalidStateError true"]);
}

#[test]
fn set_request_header_rejects_bad_names_and_ignores_forbidden_ones() {
    let loaded = common::load(
        "const x=new XMLHttpRequest();x.open('GET','/a');\
         try{x.setRequestHeader('bad name','1')}catch(e){console.log(e.name)}\
         try{x.setRequestHeader('x-a','a\\nb')}catch(e){console.log(e.name)}\
         x.setRequestHeader('Cookie','a=b');x.setRequestHeader('X-A',' 1 ');x.setRequestHeader('x-a','2');x.send();",
    );
    assert_eq!(console(&loaded), ["SyntaxError", "SyntaxError"]);
    assert_eq!(
        loaded.effects.requests[0].headers,
        [("x-a".to_string(), "1, 2".to_string())]
    );
}

#[test]
fn open_checks_and_normalises_the_method() {
    let loaded = common::load(
        "const x=new XMLHttpRequest();\
         try{x.open('TRACE','/a')}catch(e){console.log(e.name)}\
         try{x.open('a b','/a')}catch(e){console.log(e.name)}\
         try{x.open('GET','http://[bad')}catch(e){console.log(e.name, e instanceof DOMException)}\
         x.open('post','/a');x.send();const y=new XMLHttpRequest();y.open('patch','/b');y.send();",
    );
    assert_eq!(
        console(&loaded),
        ["SecurityError", "SyntaxError", "SyntaxError true"]
    );
    assert_eq!(loaded.effects.requests[0].method, "POST");
    assert_eq!(loaded.effects.requests[1].method, "patch");
}

#[test]
fn with_credentials_cannot_change_after_send() {
    let out = lines(
        "const x=new XMLHttpRequest();x.open('GET','/a');x.withCredentials=true;x.send();\
         try{x.withCredentials=false}catch(e){console.log(e.name,x.withCredentials)}",
    );
    assert_eq!(out, ["InvalidStateError true"]);
}

#[test]
fn response_headers_are_sorted_lowercase_and_combined() {
    let sources = vec![
        "const x=new XMLHttpRequest();x.open('GET','/a');\
         console.log(JSON.stringify(x.getAllResponseHeaders()),x.getResponseHeader('x-b'));\
         x.onload=()=>console.log(JSON.stringify(x.getAllResponseHeaders()),x.getResponseHeader('X-B'));x.send();"
            .to_string(),
    ];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert_eq!(console(&loaded), ["\"\" null"]);
    let response = HostResponse {
        id: loaded.effects.requests[0].id,
        status: 200,
        headers: vec![
            ("x-b".into(), "1".into()),
            ("content-type".into(), "text/plain".into()),
            ("x-b".into(), "2".into()),
        ],
        ..HostResponse::default()
    };
    let resumed = mars_v9::resume_requests(
        input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    assert_eq!(
        console(&resumed),
        ["\"content-type: text/plain\\r\\nx-b: 1, 2\\r\\n\" 1, 2"]
    );
}
