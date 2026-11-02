/** Module workers execute real isolated graphs through the governed native loading boundary. */
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

/** JavaScript responses can carry worker policy and explicit CORS metadata. */
fn script(source: &str, headers: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{source}", source.len()).into_bytes()
}

/** Every request is admitted as the human owning the fixture tab at its current epoch. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Value {
    params["tab_id"] = json!("tab-1");
    runtime
        .handle_as(
            Request::new("module-worker", op, params),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap()
}

/** One temporary runtime reads only the explicitly admitted fixture origins. */
fn load(fixture: &Fixture, other: Option<&Fixture>) -> Runtime {
    let origins = std::iter::once(fixture.url.clone())
        .chain(other.map(|fixture| fixture.url.clone()))
        .collect();
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins,
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    runtime
}

/** Observation reads retained page results without executing or repairing the worker. */
fn result(runtime: &mut Runtime) -> Value {
    call(runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"result","options":{"return_by_value":true}}}))["result"]["value"].clone()
}

/** Constructor options reach module scope, import.meta and the real dedicated-worker realm. */
#[test]
fn module_worker_evaluates_static_graph_before_queued_messages() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js',{type:'module',name:'compute'});w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;w.postMessage(41)</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "import {increment,meta} from './dependency.js';let denied=false;try{importScripts()}catch(e){denied=e instanceof TypeError}onmessage=e=>postMessage([e.data+increment,import.meta.url.endsWith('/worker.js'),meta.endsWith('/dependency.js'),this===undefined,typeof document,name,denied].join('|'));",
                "",
            ),
        ),
        (
            "/dependency.js",
            script(
                "export const increment=1;export const meta=import.meta.url;",
                "",
            ),
        ),
    ]);
    assert_eq!(
        result(&mut load(&fixture, None)),
        "42|true|true|true|undefined|compute|true"
    );
}

/** A later dynamic import uses the same credentials, module cache and message lifecycle. */
#[test]
fn module_worker_resumes_dynamic_import_and_evaluates_once() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js',{type:'module'});w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;w.postMessage('start')</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "onmessage=async()=>{const a=await import('./dynamic.js');const b=await import('./dynamic.js');postMessage([a===b,a.answer,a.meta.endsWith('/dynamic.js')].join('|'))}",
                "",
            ),
        ),
        (
            "/dynamic.js",
            script(
                "export const answer=42;export const meta=import.meta.url;",
                "",
            ),
        ),
    ]);
    assert_eq!(result(&mut load(&fixture, None)), "true|42|true");
    assert_eq!(counts.lock().unwrap().get("/dynamic.js"), Some(&1));
    assert_eq!(counts.lock().unwrap().get("/worker.js"), Some(&1));
}

/** Early messages wait through top-level await while initialization timers are still serviced. */
#[test]
fn module_worker_top_level_await_preserves_early_messages() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js',{type:'module'});w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;w.postMessage(41)</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "await new Promise(resolve=>setTimeout(resolve,0));onmessage=e=>postMessage(e.data+1)",
                "",
            ),
        ),
    ]);
    assert_eq!(result(&mut load(&fixture, None)), 42);
}

/** Rejected asynchronous initialization reports failure instead of processing queued input. */
#[test]
fn module_worker_rejected_initialization_is_terminal() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js',{type:'module'});w.onmessage=()=>result='incorrect message';w.onerror=()=>result='failed';w.postMessage(41)</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "await new Promise((resolve,reject)=>setTimeout(()=>reject(Error('initialization failed')),0));onmessage=e=>postMessage(e.data)",
                "",
            ),
        ),
    ]);
    assert_eq!(result(&mut load(&fixture, None)), "failed");
}

/** The worker response's script policy governs dependencies, independently of connect-src. */
#[test]
fn module_worker_dependencies_enforce_script_policy() {
    for (policy, expected) in [
        ("script-src 'none'", "failed"),
        ("script-src 'self'; connect-src 'none'", "loaded"),
    ] {
        let fixture = Fixture::routes(vec![
            (
                "/",
                html(
                    "",
                    "<script>let result='waiting';const w=new Worker('/worker.js',{type:'module'});w.onmessage=e=>result=e.data;w.onerror=()=>result='failed'</script>",
                ),
            ),
            (
                "/worker.js",
                script(
                    "import './dependency.js';postMessage('loaded')",
                    &format!("Content-Security-Policy: {policy}\r\n"),
                ),
            ),
            ("/dependency.js", script("export const value=1", "")),
        ]);
        assert_eq!(result(&mut load(&fixture, None)), expected);
    }
}

/** Cross-origin dependencies need CORS and credentialed requests cannot use wildcard permission. */
#[test]
fn module_worker_cross_origin_imports_enforce_cors_credentials() {
    for (credentials, headers, expected) in [
        ("omit", "Access-Control-Allow-Origin: *\r\n", "loaded"),
        ("same-origin", "", "failed"),
        ("include", "Access-Control-Allow-Origin: *\r\n", "failed"),
    ] {
        let remote = Fixture::routes(vec![(
            "/dependency.js",
            script("export const value='loaded'", headers),
        )]);
        let fixture = Fixture::routes(vec![
            (
                "/",
                html(
                    "Set-Cookie: module_secret=secret; Path=/\r\n",
                    &format!(
                        "<script>let result='waiting';const w=new Worker('/worker.js',{{type:'module',credentials:'{credentials}'}});w.onmessage=e=>result=e.data;w.onerror=()=>result='failed'</script>"
                    ),
                ),
            ),
            (
                "/worker.js",
                script(
                    &format!(
                        "import {{value}} from '{}/dependency.js';postMessage(value)",
                        remote.url
                    ),
                    "",
                ),
            ),
        ]);
        assert_eq!(result(&mut load(&fixture, Some(&remote))), expected);
        if credentials == "omit" {
            assert!(
                !fixture
                    .last_request
                    .lock()
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("cookie:")
            );
            assert!(
                !remote
                    .last_request
                    .lock()
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("cookie:")
            );
        }
    }
}
