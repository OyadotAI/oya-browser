/** Script scheduling, response metadata and charset checks crossing the real owned services. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** Script responses carry an executable media type with exact framing. */
fn script(source: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{source}",
        source.len()
    )
    .into_bytes()
}

/** Text of the first inspected node with this HTML id. */
fn text(page: &Value, id: &str) -> Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()["text"]
        .clone()
}

/** Blocking scripts run first, then defer, then async, each in document order. */
#[test]
fn scripts_run_blocking_then_defer_then_async() {
    let page = "<p id=out>initial</p>\
        <script async src=/a.js></script>\
        <script defer src=/d.js></script>\
        <script>var log='b1';</script>\
        <script src=/b2.js></script>";
    let fixture = Fixture::routes(vec![
        ("/", html("", page)),
        (
            "/a.js",
            script("log=log+',async';document.getElementById('out').textContent=log;"),
        ),
        ("/d.js", script("log=log+',defer';")),
        ("/b2.js", script("log=log+',b2';")),
    ]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(text(&page, "out"), "b1,b2,defer,async");
}

/** A failing script that runs last still names the script's place in the document. */
#[test]
fn script_errors_name_the_document_position() {
    let page = "<p id=out>initial</p><script async src=/a.js></script><script>var x=1;</script>";
    let fixture = Fixture::routes(vec![("/", html("", page)), ("/a.js", script("missing();"))]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let diagnostics = page["diagnostics"].to_string();
    assert!(diagnostics.contains("Script 1 ("), "{diagnostics}");
    assert!(diagnostics.contains("/a.js) did not run"), "{diagnostics}");
}

/** The fixture script every integrity case fetches; it marks `out` when it runs. */
const MARK: &str = "document.getElementById('out').textContent='ran';";
/** sha256 of MARK, from `openssl dgst -sha256 -binary | base64`. */
const MARK_SHA256: &str = "sha256-5BlIIuBbu/waHLPEipCNjobhFq+TGUrjlXGckC+BKYw=";

/** Loads `body` beside /a.js (MARK) and returns the committed page. */
fn load(body: &str) -> Value {
    let fixture = Fixture::routes(vec![
        ("/", html("", &format!("<p id=out>initial</p>{body}"))),
        ("/a.js", script(MARK)),
        (
            "/fallback.js",
            script("document.getElementById('out').textContent='fallback';"),
        ),
    ]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap()
}

/** Module scripts run and their nomodule fallbacks do not. */
#[test]
fn modules_run_and_nomodule_fallbacks_do_not() {
    let page =
        load("<script type=module src=/a.js></script><script nomodule src=/fallback.js></script>");
    assert_eq!(text(&page, "out"), "ran");
    assert_eq!(page["degraded"], json!([]), "{page}");
}

/** A fetched script whose integrity matches its body runs. */
#[test]
fn matching_integrity_runs_the_script() {
    let page = load(&format!(
        "<script integrity={MARK_SHA256} src=/a.js></script>"
    ));
    assert_eq!(text(&page, "out"), "ran");
    assert_eq!(page["degraded"], json!([]));
}

/** A fetched script whose integrity does not match is skipped and the page still commits. */
#[test]
fn mismatching_integrity_skips_the_script() {
    let wrong = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let page = load(&format!("<script integrity={wrong} src=/a.js></script>"));
    assert_eq!(text(&page, "out"), "initial");
    assert_eq!(page["degraded"], json!(["scripts"]));
    let diagnostics = page["diagnostics"].to_string();
    assert!(diagnostics.contains("/a.js) did not run"), "{diagnostics}");
    assert!(diagnostics.contains("integrity"), "{diagnostics}");
}

/** Integrity only constrains fetched scripts, so an inline script carrying one still runs. */
#[test]
fn inline_integrity_is_ignored() {
    let page = load(
        "<script integrity=sha256-AAAA>document.getElementById('out').textContent='inline';</script>",
    );
    assert_eq!(text(&page, "out"), "inline");
    assert_eq!(page["degraded"], json!([]));
}

/** `crossorigin` has no effect on how a same-origin script is fetched or run. */
#[test]
fn crossorigin_is_ignored() {
    let page = load("<script crossorigin src=/a.js></script>");
    assert_eq!(text(&page, "out"), "ran");
}

/** A meta Content-Security-Policy without 'unsafe-inline' stops inline scripts; the page still commits. */
#[test]
fn meta_csp_blocks_forbidden_inline_scripts() {
    let page = load(
        "<meta http-equiv=Content-Security-Policy content=\"script-src 'self'\"><script>document.getElementById('out').textContent='inline';</script>",
    );
    assert_ne!(text(&page, "out"), "inline");
    assert_eq!(page["degraded"], json!(["scripts"]));
    assert!(
        page["diagnostics"]
            .to_string()
            .contains("Content-Security-Policy")
    );
}

/** A matching nonce lets an inline script run under the same policy. */
#[test]
fn meta_csp_nonce_allows_inline_scripts() {
    let page = load(
        "<meta http-equiv=Content-Security-Policy content=\"script-src 'nonce-n1'\"><script nonce=n1>document.getElementById('out').textContent='inline';</script>",
    );
    assert_eq!(text(&page, "out"), "inline");
}

/** Refused resources still dispatch errors, but unsafe-eval cannot authorize markup attributes. */
#[test]
fn csp_attribute_refusal_preserves_callable_resource_handlers() {
    let page = load(
        "<meta http-equiv=Content-Security-Policy content=\"script-src 'nonce-n' 'unsafe-eval'; script-src-attr 'none'\">\
        <script nonce=n>document.addEventListener('error',()=>{document.getElementById('out').textContent+=':listener'},true)</script>\
        <script src=/a.js onerror=\"document.getElementById('out').textContent='blocked'\"></script>\
        <script nonce=n>const s=document.getElementById('callable');s.onerror=()=>{document.getElementById('out').textContent+=':callable'}</script>\
        <script id=callable src=/a.js onerror=\"document.getElementById('out').textContent='blocked'\"></script>",
    );
    assert_eq!(text(&page, "out"), "initial:listener:listener:callable");
}

/** Allowed attributes compile independently, while their own eval and Function remain blocked. */
#[test]
fn csp_unsafe_inline_handlers_do_not_grant_dynamic_code() {
    let page = load(
        "<meta http-equiv=Content-Security-Policy content=\"script-src 'self'; script-src-attr 'unsafe-inline'\">\
        <script src=/a.js onload=\"let n=0;try{eval('1')}catch(e){if(e instanceof EvalError)n++}try{Function('return 1')}catch(e){if(e instanceof EvalError)n++}document.getElementById('out').textContent='allowed:'+n\"></script>",
    );
    assert_eq!(text(&page, "out"), "allowed:2");
}

/** CSP hashes apply to original attribute source only with explicit unsafe-hashes permission. */
#[test]
fn csp_attribute_hashes_require_permission_and_exact_source() {
    for (keyword, source, expected) in [
        ("'unsafe-hashes'", MARK, "ran"),
        ("", MARK, "initial"),
        (
            "'unsafe-hashes'",
            "document.getElementById('out').textContent='wrong';",
            "initial",
        ),
    ] {
        let page = load(&format!(
            "<meta http-equiv=Content-Security-Policy content=\"script-src 'none'; script-src-attr {keyword} '{MARK_SHA256}'\"><script src=/a.js onerror=\"{source}\"></script>"
        ));
        assert_eq!(text(&page, "out"), expected);
    }
}

/** DevTools-triggered dispatch refreshes the document's handler policy as normal events do. */
#[test]
fn devtools_dispatch_cannot_bypass_attribute_policy() {
    for (policy, expected) in [("'none'", "initial"), ("'unsafe-inline'", "clicked")] {
        let body = format!(
            "<meta http-equiv=Content-Security-Policy content=\"script-src 'self' 'unsafe-eval'; script-src-attr {policy}\"><p id=out>initial</p><button id=go onclick=\"document.getElementById('out').textContent='clicked'\">Go</button>"
        );
        let fixture = Fixture::routes(vec![("/", html("", &body))]);
        let mut runtime = desktop(&fixture.url);
        agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        let result = agent_call(&mut runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"document.getElementById('go').click();document.getElementById('out').textContent","options":{"return_by_value":true}}})).unwrap();
        assert_eq!(result["result"]["value"], expected);
    }
}

/** Repeated list headers such as Vary and X-Content-Type-Options are joined and the page commits. */
#[test]
fn repeated_list_headers_commit() {
    let headers = "Vary: Accept\r\nVary: Cookie\r\nX-Content-Type-Options: nosniff\r\nx-content-type-options: nosniff\r\n";
    let fixture = Fixture::raw(html(headers, "<p id=out>joined</p>"));
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(text(&page, "out"), "joined");
}

/** With no transport charset, the page's meta charset decides, as in Chrome: UTF-8 bytes under
`<meta charset=iso-8859-1>` decode as windows-1252 (WHATWG maps the label there) and show as
mojibake rather than refusing the page. */
#[test]
fn meta_charset_decides_without_a_transport_charset() {
    let body = "<meta charset=iso-8859-1><p id=out>caf\u{e9}</p>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let fixture = Fixture::raw(response.into_bytes());
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(text(&page, "out"), "caf\u{c3}\u{a9}");
}

/** Loads `body` beside a small module graph and returns the committed page. */
fn load_modules(body: &str) -> Value {
    let plain = "export const v = 'plain';";
    let fixture = Fixture::routes(vec![
        ("/", html("", &format!("<p id=out>initial</p>{body}"))),
        ("/lib.js", script("import { d } from './dep.js'; export const v = 'graph:' + d;")),
        ("/dep.js", script("export const d = 'ok';")),
        (
            "/plain.js",
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{plain}",
                plain.len()
            )
            .into_bytes(),
        ),
    ]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap()
}

/** A module graph loads through the page's import map, its imports fetched round by round. */
#[test]
fn module_graphs_load_through_import_maps() {
    let page = load_modules(
        "<script type=importmap>{\"imports\":{\"lib\":\"/lib.js\"}}</script>\
         <script type=module>import { v } from 'lib';\
         document.getElementById('out').textContent = v;</script>",
    );
    assert_eq!(text(&page, "out"), "graph:ok");
}

/** A module whose integrity (from the import map) or media type is wrong fails its graph alone. */
#[test]
fn module_integrity_and_media_type_are_enforced() {
    let wrong = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let page = load_modules(&format!(
        "<script type=importmap>{{\"integrity\":{{\"/dep.js\":\"{wrong}\"}}}}</script>\
         <script type=module>import {{ v }} from '/lib.js';\
         document.getElementById('out').textContent = v;</script>\
         <script type=module>import {{ v }} from '/plain.js';\
         document.getElementById('out').textContent = v;</script>\
         <script>document.getElementById('out').textContent += '+classic';</script>"
    ));
    assert_eq!(text(&page, "out"), "initial+classic");
    let diagnostics = page["diagnostics"].to_string();
    assert!(diagnostics.contains("dep.js"), "{diagnostics}");
    assert!(diagnostics.contains("plain.js"), "{diagnostics}");
}

/** Parsed resource completion distinguishes empty JavaScript from a refused media type. */
#[test]
fn parsed_script_completion_reports_success_and_acquisition_refusal() {
    let page = "<p id=out>initial</p><script>var events='';</script>\
        <script src=/empty.js onload=\"events+='load';\"></script>\
        <script src=/refused.js onerror=\"events+='error';\"></script>\
        <script>document.getElementById('out').textContent=events;</script>";
    let fixture = Fixture::routes(vec![
        ("/", html("", page)),
        ("/empty.js", script("")),
        ("/refused.js", html("", "events='executed';")),
    ]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(text(&page, "out"), "loaderror");
    assert!(
        page["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature == "scripts")
    );
}
