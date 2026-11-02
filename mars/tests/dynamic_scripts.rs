/**
 * Dynamically inserted classic scripts at the page's script cap and in long sequential chains:
 * a script past SCRIPT_LOADS fails its element with `error` rather than hanging, and a chain of
 * several hundred loads each runs once through the real services.
 */
#[allow(dead_code)]
mod support;
use mars::constants::SCRIPT_LOADS;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** Scripts this test serves run one counter bump each; the body is the whole response. */
fn script(source: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{source}",
        source.len()
    )
    .into_bytes()
}

/** Length of the sequential chain in the second test: several hundred loads, one after another. */
const CHAIN: usize = 300;

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

/**
 * One more script than SCRIPT_LOADS: the last element is refused, fires `error` in a later task,
 * and every element before it still loads and runs.
 */
#[test]
fn refused_dynamic_script_fires_error_and_the_page_continues() {
    let page = format!(
        "<p id=out>initial</p><script>\
        var out=document.getElementById('out');\
        function report(){{out.textContent='ran '+(window.ran||0)+' failed '+(window.failed||0);}}\
        for(var i=0;i<={SCRIPT_LOADS};i++){{\
          var s=document.createElement('script');\
          s.src='/s.js';\
          s.addEventListener('error',function(){{window.failed=(window.failed||0)+1;report();}});\
          document.head.appendChild(s);\
        }}</script>"
    );
    let fixture = Fixture::routes(vec![
        ("/", html("", &page)),
        ("/s.js", script("window.ran=(window.ran||0)+1;report();")),
    ]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(out(&page), format!("ran {SCRIPT_LOADS} failed 1"));
}

/**
 * A chain of CHAIN dynamic scripts, each inserted by the previous one's `load`, each at its own
 * URL: every one is fetched, runs once, and the chain reaches its end.
 */
#[test]
fn several_hundred_sequential_dynamic_scripts_all_load_and_run() {
    let page = format!(
        "<p id=out>initial</p><script>\
        var n=0;\
        function next(){{\
          if(n=={CHAIN}){{document.getElementById('out').textContent='done '+window.ran;return;}}\
          n++;\
          var s=document.createElement('script');\
          s.src='/s/'+n+'.js';\
          s.addEventListener('load',next);\
          document.head.appendChild(s);\
        }}\
        next();</script>"
    );
    let index = html("", &page);
    let fixture = support::Fixture::start(
        Box::new(move |request| {
            if request.contains("/s/") {
                script("window.ran=(window.ran||0)+1;")
            } else {
                index.clone()
            }
        }),
        false,
    );
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(out(&page), format!("done {CHAIN}"));
}
