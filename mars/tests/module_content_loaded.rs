/**
 * DOMContentLoaded waits for the parser's module scripts, as a browser's does: a module is
 * deferred, so a DOMContentLoaded handler sees what it defined. DuckDuckGo's results page
 * builds itself from a module bundle inside that handler and stayed blank while the event
 * came first.
 */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** A module response; the body is the whole response. */
fn module(source: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{source}",
        source.len()
    )
    .into_bytes()
}

/** Text of the `out` element after the run. */
fn out(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "out")
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string()
}

/** A DOMContentLoaded handler declared before a `<script type=module src>` sees the module's globals. */
#[test]
fn dom_content_loaded_sees_what_the_parsers_module_scripts_defined() {
    let page = "<p id=out>initial</p>\
        <script>document.addEventListener('DOMContentLoaded',function(){\
          document.getElementById('out').textContent='utils '+typeof window.utils;});</script>\
        <script type=module src=/m.js></script>";
    let fixture = Fixture::routes(vec![
        ("/", html("", page)),
        ("/m.js", module("window.utils={ready:true};")),
    ]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(out(&page), "utils object");
}
