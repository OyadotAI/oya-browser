/** Data URL fetches retain binary bytes through the native host/VM response boundary. */
#[allow(dead_code)]
mod support;
use mars::protocol::Request;
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner, Permit},
};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, html};

/** Real native ownership and loopback-only fixture admission exercise ordinary page settling. */
fn evaluate(script: &str) -> Value {
    let fixture = Fixture::routes(vec![("/", html("", &format!("<script>{script}</script>")))]);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    call(&mut runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"result","options":{"return_by_value":true}}}))["result"]["value"].clone()
}

/** The fixture cannot bypass tab identity, ownership or the normal native control route. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Value {
    params["tab_id"] = json!("tab-1");
    runtime
        .handle_as(
            Request::new("data-fetch", op, params),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap()
}

/** Non-UTF8 bytes remain readable, while the text reader handles a UTF8 BOM separately. */
#[test]
fn data_fetch_retains_binary_bytes_and_text_decoding() {
    assert_eq!(
        evaluate(
            r#"
        let result='waiting';
        Promise.all([
            fetch('data:application/octet-stream;base64,AP+A').then(r=>r.arrayBuffer()).then(b=>Array.from(new Uint8Array(b)).join(',')),
            fetch('data:text/plain;base64,77u/QQ==').then(r=>r.text())
        ]).then(values=>result=values.join('|'));
    "#
        ),
        "0,255,128|A"
    );
}

/** Scheme decoding still rejects unsupported methods and preserves ordinary text XHR. */
#[test]
fn data_fetch_keeps_method_refusal_and_xhr_text() {
    assert_eq!(
        evaluate(
            r#"
        let result='waiting';
        const xhrResult=new Promise(resolve=>{const x=new XMLHttpRequest();x.onload=()=>resolve(x.responseText);x.open('GET','data:text/plain,hello');x.send()});
        const refused=fetch('data:text/plain,hello',{method:'HEAD'}).then(()=>'unexpected',e=>e.name);
        Promise.all([xhrResult,refused]).then(values=>result=values.join('|'));
    "#
        ),
        "hello|TypeError"
    );
}
