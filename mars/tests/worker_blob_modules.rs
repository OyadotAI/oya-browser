/** Captured Blob module entries use the existing governed graph and worker initialization lifecycle. */
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

/** Preserve source text exactly while embedding it as a page-side Blob constructor argument. */
fn literal(source: &str) -> String {
    serde_json::to_string(source).unwrap()
}

/** Construction always queues input and revokes the URL before the runtime can start the worker. */
fn page(expression: &str, headers: &str, mime: &str, credentials: &str) -> Vec<u8> {
    html(
        headers,
        &format!(
            r#"<script>
        let result='waiting';const source={expression};
        const u=URL.createObjectURL(new Blob([source],{{type:{mime}}}));
        const w=new Worker(u,{{type:'module',name:'blob-module',credentials:{credentials}}});
        URL.revokeObjectURL(u);w.onmessage=e=>result=e.data;w.onerror=()=>result='failed';w.postMessage(41);
    </script>"#,
            mime = literal(mime),
            credentials = literal(credentials)
        ),
    )
}

/** HTTP dependencies pass real MIME and CORS handling using only admitted fixture origins. */
fn script(source: &str, headers: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{source}", source.len()).into_bytes()
}

/** Native publication and owner checks remain active throughout fixture execution. */
fn call(runtime: &mut Runtime, operation: &str, mut params: Value) -> Value {
    params["tab_id"] = json!("tab-1");
    runtime
        .handle_as(
            Request::new("blob-module-test", operation, params),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap()
}

/** Observe the retained result after normal graph loading, timer and worker-message settling. */
fn result(fixture: &Fixture, remote: Option<&Fixture>) -> Value {
    let origins = std::iter::once(fixture.url.clone())
        .chain(remote.map(|item| item.url.clone()))
        .collect();
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins,
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    call(
        &mut runtime,
        "page.devtools_peek",
        json!({"request":{
            "kind":"evaluate", "expression":"result", "options":{"return_by_value":true}
        }}),
    )["result"]["value"]
        .clone()
}

#[test]
fn blob_module_top_level_await_preserves_early_messages_and_module_identity() {
    let source = literal(
        "await new Promise(resolve=>setTimeout(resolve,0));let denied=false;try{importScripts()}catch(e){denied=e instanceof TypeError}onmessage=e=>postMessage([e.data+1,import.meta.url===location.href,this===undefined,name,denied].join('|'))",
    );
    let fixture = Fixture::routes(vec![(
        "/",
        page(&source, "", "text/javascript", "same-origin"),
    )]);
    assert_eq!(result(&fixture, None), "42|true|true|blob-module|true");
}

#[test]
fn blob_module_rejected_initialization_does_not_deliver_queued_input() {
    let source = literal(
        "await new Promise((resolve,reject)=>setTimeout(()=>reject(Error('failed initialization')),0));onmessage=()=>postMessage('incorrect')",
    );
    let fixture = Fixture::routes(vec![(
        "/",
        page(&source, "", "text/javascript", "same-origin"),
    )]);
    assert_eq!(result(&fixture, None), "failed");
}

#[test]
fn blob_module_fetch_requires_a_javascript_mime_essence() {
    for (mime, expected) in [
        ("", "failed"),
        ("text/plain", "failed"),
        ("image/png", "failed"),
        ("text/javascript; charset=utf-8", "loaded"),
    ] {
        let fixture = Fixture::routes(vec![(
            "/",
            page(&literal("postMessage('loaded')"), "", mime, "same-origin"),
        )]);
        assert_eq!(result(&fixture, None), expected, "{mime}");
    }
}

#[test]
fn blob_module_static_and_dynamic_http_dependencies_share_the_module_cache() {
    let expression = r#"'import {increment} from '+JSON.stringify(location.origin+'/static.js')+';onmessage=async e=>{const a=await import('+JSON.stringify(location.origin+'/dynamic.js')+');const b=await import('+JSON.stringify(location.origin+'/dynamic.js')+');postMessage([e.data+increment,a===b,a.answer].join("|"))}'"#;
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            page(
                expression,
                "Content-Security-Policy: script-src 'self' 'unsafe-inline'; worker-src blob:\r\n",
                "text/javascript",
                "same-origin",
            ),
        ),
        ("/static.js", script("export const increment=1", "")),
        ("/dynamic.js", script("export const answer=42", "")),
    ]);
    assert_eq!(result(&fixture, None), "42|true|42");
    assert_eq!(counts.lock().unwrap().get("/dynamic.js"), Some(&1));
}

#[test]
fn blob_module_http_dependencies_enforce_cors_and_constructor_credentials() {
    for (credentials, headers, expected) in [
        ("omit", "Access-Control-Allow-Origin: *\r\n", "loaded"),
        ("same-origin", "", "failed"),
        ("include", "Access-Control-Allow-Origin: *\r\n", "failed"),
    ] {
        let remote = Fixture::routes(vec![(
            "/dependency.js",
            script("export const value='loaded'", headers),
        )]);
        let source = literal(&format!(
            "import {{value}} from {};postMessage(value)",
            literal(&format!("{}/dependency.js", remote.url))
        ));
        let fixture = Fixture::routes(vec![(
            "/",
            page(&source, "", "text/javascript", credentials),
        )]);
        assert_eq!(result(&fixture, Some(&remote)), expected, "{credentials}");
        assert!(
            remote
                .last_request
                .lock()
                .unwrap()
                .to_ascii_lowercase()
                .contains(&format!("origin: {}", fixture.url))
        );
    }
}

#[test]
fn inherited_csp_refuses_blob_entries_and_http_dependencies_independently() {
    for (policy, expression) in [
        (
            "script-src 'unsafe-inline'; worker-src 'none'",
            literal("postMessage('incorrect')"),
        ),
        (
            "script-src 'unsafe-inline'; worker-src blob:",
            "'import '+JSON.stringify(location.origin+'/blocked.js')+';postMessage(\"incorrect\")'"
                .into(),
        ),
    ] {
        let fixture = Fixture::routes(vec![(
            "/",
            page(
                &expression,
                &format!("Content-Security-Policy: {policy}\r\n"),
                "text/javascript",
                "same-origin",
            ),
        )]);
        assert_eq!(result(&fixture, None), "failed");
        assert!(!fixture.last_request.lock().unwrap().contains("/blocked.js"));
    }
}

#[test]
fn relative_imports_do_not_fall_back_to_the_creator_document_url() {
    let fixture = Fixture::routes(vec![(
        "/",
        page(
            &literal("import './relative.js';postMessage('incorrect')"),
            "",
            "text/javascript",
            "same-origin",
        ),
    )]);
    assert_eq!(result(&fixture, None), "failed");
    assert!(
        !fixture
            .last_request
            .lock()
            .unwrap()
            .contains("/relative.js")
    );
}
