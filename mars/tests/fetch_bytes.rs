/** Native HTTP fetch preserves exact bytes without relaxing CORS or legacy XHR text decoding. */
#[allow(dead_code)]
mod support;
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner},
};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, html};

/** Binary fixtures avoid passing through any text serializer before Relay receives them. */
fn bytes(mime: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(body);
    response
}

/** Agent navigation settles fixture fetches before inspection; human navigation intentionally publishes sooner. */
fn evaluate(fixture: &Fixture, mut grants: Vec<String>) -> Value {
    grants.push(fixture.url.clone());
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        grants,
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    call(&mut runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"result","options":{"return_by_value":true}}}))["result"]["value"].clone()
}

/** Requests retain real tab ownership and never inject fake network results. */
fn call(runtime: &mut Runtime, op: &str, params: Value) -> Value {
    support::agent_call(runtime, op, params).unwrap()
}

/** Fetch byte readers are exact and text readers use UTF8; XHR still obeys its declared charset. */
#[test]
fn http_fetch_is_binary_safe_without_changing_xhr_charset() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                r#"<script>
            let result='waiting';
            const xhr=new Promise(resolve=>{const x=new XMLHttpRequest();x.open('GET','/latin');x.onload=()=>resolve(x.responseText);x.send()});
            Promise.all([
                fetch('/raw').then(r=>r.arrayBuffer()).then(b=>Array.from(new Uint8Array(b)).join(',')),
                fetch('/latin').then(r=>r.text()),xhr
            ]).then(values=>result=values.join('|'));
        </script>"#,
            ),
        ),
        ("/raw", bytes("application/octet-stream", &[0, 255, 128])),
        ("/latin", bytes("text/plain; charset=windows-1252", &[128])),
    ]);
    assert_eq!(
        evaluate(&fixture, Vec::new()),
        "0,255,128|\u{fffd}|\u{20ac}"
    );
}

/** Raw response transport does not expose cross-origin bytes without a server CORS grant. */
#[test]
fn cors_refusal_and_opaque_responses_do_not_expose_binary_data() {
    let remote = Fixture::routes(vec![(
        "/raw",
        bytes("application/octet-stream", &[255, 128]),
    )]);
    let script = format!(
        r#"<script>
        let result='waiting';const address={:?}+'/raw';
        Promise.all([
            fetch(address).then(()=> 'unexpected',e=>e.name),
            fetch(address,{{mode:'no-cors'}}).then(r=>r.arrayBuffer().then(b=>r.status+':'+b.byteLength))
        ]).then(values=>result=values.join('|'));
    </script>"#,
        remote.url
    );
    let fixture = Fixture::routes(vec![("/", html("", &script))]);
    assert_eq!(
        evaluate(&fixture, vec![remote.url.clone()]),
        "TypeError|0:0"
    );
}
