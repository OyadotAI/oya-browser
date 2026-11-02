/** Blob workers execute captured source under creator policy through real retained V9 contexts. */
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

/** All source is fixture-owned; the runtime keeps its normal authority and publication rules. */
fn load(fixture: &Fixture) -> Runtime {
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    runtime
}

/** Read through the normal tab-bound front door without injecting another executable script. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Value {
    params["tab_id"] = json!("tab-1");
    runtime
        .handle_as(
            Request::new("blob-worker-test", op, params),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap()
}

/** Navigation settling must deliver the actual worker reply or error to the creator. */
fn result(fixture: &Fixture) -> Value {
    call(
        &mut load(fixture),
        "page.devtools_peek",
        json!({"request":{
            "kind":"evaluate", "expression":"result", "options":{"return_by_value":true}
        }}),
    )["result"]["value"]
        .clone()
}

#[test]
fn blob_worker_roundtrip_survives_immediate_revocation_and_keeps_location() {
    let fixture = Fixture::routes(vec![(
        "/",
        html(
            "",
            r#"<script>
        let result='waiting';
        const u=URL.createObjectURL(new Blob(["onmessage=e=>postMessage([e.data+1,location.protocol,location.origin,name,typeof document].join('|'))"]));
        const w=new Worker(u,{name:'blob-compute'});URL.revokeObjectURL(u);
        w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;w.postMessage(41);
    </script>"#,
        ),
    )]);
    assert_eq!(
        result(&fixture),
        format!(
            "42|blob:|{}|blob-compute|undefined",
            fixture.url.trim_end_matches('/')
        )
    );
}

#[test]
fn revoked_blob_worker_fires_error_without_network_fallback() {
    let fixture = Fixture::routes(vec![(
        "/",
        html(
            "",
            r#"<script>
        let result='waiting';const u=URL.createObjectURL(new Blob(['postMessage("wrong")']));
        URL.revokeObjectURL(u);const w=new Worker(u);w.onmessage=e=>result=e.data;w.onerror=()=>result='error';
    </script>"#,
        ),
    )]);
    assert_eq!(result(&fixture), "error");
}

#[test]
fn worker_src_blocks_captured_blob_bytes() {
    let fixture = Fixture::routes(vec![(
        "/",
        html(
            "Content-Security-Policy: script-src 'unsafe-inline'; worker-src 'none'\r\n",
            r#"<script>
        let result='waiting';const w=new Worker(URL.createObjectURL(new Blob(['postMessage("wrong")'])));
        w.onmessage=e=>result=e.data;w.onerror=()=>result='blocked';
    </script>"#,
        ),
    )]);
    assert_eq!(result(&fixture), "blocked");
}

#[test]
fn blob_workers_inherit_creator_import_and_eval_restrictions() {
    let fixture = Fixture::routes(vec![(
        "/",
        html(
            "Content-Security-Policy: script-src 'unsafe-inline'; worker-src blob:\r\n",
            r#"<script>
        let result='waiting';const source="let errors='';try{importScripts("+JSON.stringify(location.origin+'/blocked.js')+")}catch(e){errors+=e.name+'|'}try{eval('1')}catch(e){errors+=e.name}postMessage(errors)";
        const w=new Worker(URL.createObjectURL(new Blob([source])));w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;
    </script>"#,
        ),
    )]);
    assert_eq!(result(&fixture), "NetworkError|EvalError");
    assert!(!fixture.last_request.lock().unwrap().contains("/blocked.js"));
}

#[test]
fn blob_workers_import_absolute_scripts_under_inherited_self_policy() {
    let body = "function increment(value){return value+1}";
    let script = format!("HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes();
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "Content-Security-Policy: script-src 'self' 'unsafe-inline'; worker-src blob:\r\n",
                r#"<script>
        let result='waiting';const source="importScripts("+JSON.stringify(location.origin+'/increment.js')+");onmessage=e=>postMessage(increment(e.data))";
        const w=new Worker(URL.createObjectURL(new Blob([source])));w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;w.postMessage(41);
    </script>"#,
            ),
        ),
        ("/increment.js", script),
    ]);
    assert_eq!(result(&fixture), 42);
}
