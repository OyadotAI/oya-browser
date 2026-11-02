/** Page scripts send PUT, PATCH, DELETE, OPTIONS and HEAD through the real runtime and owned
services: same-origin requests go straight out, cross-origin unsafe methods are preflighted, HEAD
reads no body, unsafe requests are never retried, and they invalidate the cached GET. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use support::{Fixture, agent_call, html};

/** A 200 text response with extra header lines. */
fn reply(headers: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** Text of the inspected nodes, joined. */
fn texts(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** A runtime granting every origin in `origins`. */
fn runtime(origins: &[&str]) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins.iter().map(|origin| (*origin).to_owned()).collect(),
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** A page at `/` running `script`, with `api` served at `/api` on the same origin. */
fn same_origin(script: &str, api: Vec<u8>) -> Fixture {
    let body = format!("<p id=out>initial</p><script>{script}</script>");
    let page: &'static str = Box::leak(body.into_boxed_str());
    Fixture::routes(vec![("/", html("", page)), ("/api", api)])
}

/** Navigate to the fixture's page and return its text after scripts settle. */
fn visit(page: &Fixture, origins: &[&str], fragment: &str) -> String {
    let mut runtime = runtime(origins);
    let url = format!("{}/#{fragment}", page.url);
    texts(&agent_call(&mut runtime, "page.navigate", json!({"url":url})).unwrap())
}

/** Loads `script` against a cross-origin `api`; `'API` in it names the API origin. */
fn cross_origin(script: &str, api: &Fixture) -> String {
    let script = script.replace("'API", "location.hash.slice(1)+'");
    let body = format!("<p id=out>initial</p><script>{script}</script>");
    let page: &'static str = Box::leak(body.into_boxed_str());
    let page = Fixture::routes(vec![("/", html("", page))]);
    visit(&page, &[&page.url, &api.url], &api.url)
}

/** Reads status and body into #out, or the error message on failure. */
const SHOW: &str = ".then(r=>r.text().then(t=>{document.getElementById('out').textContent='read '+r.status+' ['+t+']'}),\
                    e=>{document.getElementById('out').textContent='failed '+e.message})";

/** Requests the fixture received. */
fn requests(fixture: &Fixture) -> usize {
    fixture.requests.load(Ordering::Relaxed)
}

/** The fixture's last raw request. */
fn last(fixture: &Fixture) -> String {
    fixture.last_request.lock().unwrap().clone()
}

/** Each method a script names reaches a same-origin server under that name, unpreflighted. */
#[test]
fn same_origin_scripts_send_every_method() {
    for method in ["PUT", "PATCH", "DELETE", "OPTIONS"] {
        let script = format!("fetch('/api',{{method:'{method}',body:'x'}}){SHOW}");
        let fixture = same_origin(&script, reply("", "done"));
        let out = visit(&fixture, &[&fixture.url], "");
        assert!(out.contains("read 200 [done]"), "{method}: {out}");
        assert_eq!(requests(&fixture), 2, "{method}: page, then the request");
        let last = last(&fixture);
        assert!(last.starts_with(&format!("{method} /api")), "{last}");
        assert!(
            last.contains("Origin: "),
            "an unsafe method sends Origin: {last}"
        );
    }
}

/** XMLHttpRequest sends the new methods too. */
#[test]
fn xhr_sends_put() {
    let script = "const x=new XMLHttpRequest();x.open('PUT','/api');\
                  x.onload=()=>{document.getElementById('out').textContent='read '+x.responseText};\
                  x.onerror=()=>{document.getElementById('out').textContent='failed'};x.send('x');";
    let fixture = same_origin(script, reply("", "put"));
    let out = visit(&fixture, &[&fixture.url], "");
    assert!(out.contains("read put"), "{out}");
    assert!(last(&fixture).starts_with("PUT /api"), "{}", last(&fixture));
}

/** A HEAD answer reads as an empty body, whatever its Content-Length. */
#[test]
fn a_head_fetch_reads_no_body() {
    let script = format!("fetch('/api',{{method:'HEAD'}}){SHOW}");
    let fixture = same_origin(&script, reply("", "hello"));
    let out = visit(&fixture, &[&fixture.url], "");
    assert!(out.contains("read 200 []"), "{out}");
    assert!(
        last(&fixture).starts_with("HEAD /api"),
        "{}",
        last(&fixture)
    );
}

/** A server that hangs up on an unsafe request fails it once; it is never resent. */
#[test]
fn an_unsafe_script_request_is_not_retried() {
    let script = format!("fetch('/api',{{method:'DELETE'}}){SHOW}");
    let fixture = same_origin(&script, Vec::new());
    let out = visit(&fixture, &[&fixture.url], "");
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert_eq!(requests(&fixture), 2, "page, then one DELETE");
}

/** PUT and DELETE drop the GET the page cached for their URL. */
#[test]
fn put_and_delete_invalidate_the_cached_get() {
    let get = "()=>fetch('/api')";
    let script = format!(
        "fetch('/api').then({get}).then(()=>fetch('/api',{{method:'PUT',body:'x'}})).then({get})\
         .then(()=>fetch('/api',{{method:'DELETE'}})).then({get}){SHOW}"
    );
    let fixture = same_origin(&script, reply("Cache-Control: max-age=600\r\n", "v"));
    let out = visit(&fixture, &[&fixture.url], "");
    assert!(out.contains("read 200 [v]"), "{out}");
    // The page, the first GET (the second is cached), PUT, a refetched GET, DELETE, another.
    assert_eq!(requests(&fixture), 6);
}

/** Cross-origin PUT, PATCH and DELETE are preflighted, then sent when the server allows them. */
#[test]
fn cross_origin_unsafe_methods_are_preflighted() {
    for method in ["PUT", "PATCH", "DELETE", "OPTIONS"] {
        let api = Fixture::routes(vec![(
            "/api",
            reply(
                "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: PUT, PATCH, DELETE, OPTIONS\r\n",
                "done",
            ),
        )]);
        let out = cross_origin(
            &format!("fetch('API/api',{{method:'{method}'}}){SHOW}"),
            &api,
        );
        assert!(out.contains("read 200 [done]"), "{method}: {out}");
        assert_eq!(requests(&api), 2, "{method}: preflight, then the request");
        assert!(
            last(&api).starts_with(&format!("{method} /api")),
            "{}",
            last(&api)
        );
    }
}

/** Without Access-Control-Allow-Methods naming it, the real request never goes out. */
#[test]
fn a_preflight_that_does_not_allow_the_method_stops_it() {
    let api = Fixture::routes(vec![(
        "/api",
        reply("Access-Control-Allow-Origin: *\r\n", "no"),
    )]);
    let out = cross_origin(&format!("fetch('API/api',{{method:'PUT'}}){SHOW}"), &api);
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert_eq!(requests(&api), 1);
    let last = last(&api);
    assert!(last.starts_with("OPTIONS /api"), "{last}");
    assert!(
        last.contains("Access-Control-Request-Method: PUT"),
        "{last}"
    );
}

/** HEAD is CORS-safelisted: one request, no preflight. */
#[test]
fn a_cross_origin_head_is_not_preflighted() {
    let api = Fixture::routes(vec![(
        "/api",
        reply("Access-Control-Allow-Origin: *\r\n", "x"),
    )]);
    let out = cross_origin(&format!("fetch('API/api',{{method:'HEAD'}}){SHOW}"), &api);
    assert!(out.contains("read 200 []"), "{out}");
    assert_eq!(requests(&api), 1);
    assert!(last(&api).starts_with("HEAD /api"), "{}", last(&api));
}

/** no-cors admits only GET, HEAD and POST; a PUT never leaves the page. */
#[test]
fn no_cors_refuses_an_unsafelisted_method() {
    let api = Fixture::routes(vec![("/api", reply("", "x"))]);
    let out = cross_origin(
        &format!("fetch('API/api',{{method:'PUT',mode:'no-cors'}}){SHOW}"),
        &api,
    );
    assert!(out.contains("failed"), "{out}");
    assert_eq!(requests(&api), 0);
}
