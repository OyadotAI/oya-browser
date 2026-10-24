/** A page loads the way browsers load it: each script that fails stops only itself. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** Script responses carry an executable media type with exact framing. */
fn script(source: &str) -> Vec<u8> {
    response("text/javascript", source)
}

/** A complete response with the given media type. */
fn response(mime: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** Text of the first inspected node with this HTML id. */
fn text<'a>(page: &'a Value, id: &str) -> &'a str {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap()["text"]
        .as_str()
        .unwrap()
}

/** Loads one page body, with extra routes, and returns the committed inspection. */
fn load(body: &str, routes: Vec<(&'static str, Vec<u8>)>) -> (Value, mars::runtime::Runtime) {
    let mut all = vec![("/", html("", body))];
    all.extend(routes);
    let fixture = Fixture::routes(all);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    (page, runtime)
}

/** True when some diagnostic contains this text. */
fn noted(page: &Value, needle: &str) -> bool {
    page["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| message.as_str().unwrap().contains(needle))
}

/** Script 2 of 3 throws: scripts 1 and 3 still commit and the page says it loaded with gaps. */
#[test]
fn a_throwing_script_stops_only_itself() {
    let (page, _) = load(
        "<p id=a>a</p><p id=b>b</p><p id=c>c</p>\
         <script>document.getElementById('a').textContent='one';</script>\
         <script>document.getElementById('b').textContent='two';missing();</script>\
         <script>document.getElementById('c').textContent='three';</script>",
        Vec::new(),
    );
    assert_eq!(text(&page, "a"), "one");
    assert_eq!(
        text(&page, "b"),
        "b",
        "the failing script's own change is rolled back"
    );
    assert_eq!(text(&page, "c"), "three");
    assert_eq!(page["degraded"], json!(["scripts"]));
    assert!(noted(&page, "Script 2 (inline) did not run"));
}

/** A syntax error and an unsupported construct each fail alone. */
#[test]
fn syntax_errors_and_unsupported_features_fail_alone() {
    let (page, _) = load(
        "<p id=a>a</p><p id=c>c</p>\
         <script>var x = ;</script>\
         <script>with ({}) {}</script>\
         <script>document.getElementById('a').textContent='one';</script>\
         <script>var later = function() { with ({}) {} }; document.getElementById('c').textContent='ran';</script>",
        Vec::new(),
    );
    assert_eq!(text(&page, "a"), "one");
    assert_eq!(
        text(&page, "c"),
        "ran",
        "unsupported code that never runs costs nothing"
    );
    assert!(noted(&page, "Script 1 (inline)"));
    assert!(noted(&page, "Script 2 (inline)"));
}

/** Missing, mistyped and failing external scripts are skipped and named by URL. */
#[test]
fn unloadable_external_scripts_are_skipped() {
    let (page, _) = load(
        "<p id=a>a</p>\
         <script src=/missing.js?token=secret></script>\
         <script src=/style.js></script>\
         <script src=/ok.js></script>",
        vec![
            ("/style.js", response("text/css", "p{}")),
            (
                "/ok.js",
                script("document.getElementById('a').textContent='ok';"),
            ),
        ],
    );
    assert_eq!(text(&page, "a"), "ok");
    assert!(noted(&page, "/missing.js) did not run"));
    assert!(
        !noted(&page, "secret"),
        "script URLs never carry their query"
    );
    assert!(noted(
        &page,
        "/style.js) did not run: it did not return JavaScript"
    ));
}

/** A script that never ends is stopped by the work or time guard and the others still commit. */
#[test]
fn a_runaway_script_is_stopped_alone() {
    let (page, _) = load(
        "<p id=a>a</p>\
         <script>document.getElementById('a').textContent='first';</script>\
         <script>for(;;){}</script>",
        Vec::new(),
    );
    assert_eq!(text(&page, "a"), "first");
    assert!(noted(&page, "limit exceeded"), "{}", page["diagnostics"]);
}

/** Functions from scripts that succeeded still answer clicks after another script failed. */
#[test]
fn clicks_work_after_a_failed_script() {
    let (page, mut runtime) = load(
        "<p id=a>a</p><button id=go onclick=go()>Go</button>\
         <script>throw 1;</script>\
         <script>function go(){document.getElementById('a').textContent='clicked';}</script>",
        Vec::new(),
    );
    let button = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "go")
        .unwrap();
    let clicked = agent_call(
        &mut runtime,
        "page.click",
        json!({"page_id":page["page_id"],"node_id":button["node_id"],"expected_revision":page["revision"]}),
    )
    .unwrap();
    assert_eq!(text(&clicked, "a"), "clicked");
}

/** A page whose scripts all succeed reports no gaps. */
#[test]
fn a_clean_page_has_no_gaps() {
    let (page, _) = load(
        "<p id=a>a</p><script>document.getElementById('a').textContent='ok';</script>",
        Vec::new(),
    );
    assert_eq!(page["degraded"], json!([]));
}
