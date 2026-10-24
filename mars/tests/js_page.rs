/** Page scripts run through the app the way a browser runs them: lifecycle, DOM building,
timers, same-origin requests, cookies, clicks and navigation, crossing the real owned services. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, desktop, html};

/** A 200 response of `mime` with any extra header lines. */
fn reply(mime: &str, headers: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/** A redirect to `location`. */
fn redirect(location: &str) -> Vec<u8> {
    format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .into_bytes()
}

/** Text of the inspected nodes, joined, so assertions read what a person sees. */
fn texts(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** Navigate one fixture page whose body is `body`, with `routes` beside it. */
fn load(
    body: &str,
    routes: Vec<(&'static str, Vec<u8>)>,
) -> (Fixture, mars::runtime::Runtime, Value) {
    let page: &'static str = Box::leak(body.to_string().into_boxed_str());
    let mut all = vec![("/", html("", page))];
    all.extend(routes);
    let fixture = Fixture::routes(all);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    (fixture, runtime, page)
}

/** Script-built content (createElement, appendChild, attributes) is part of the first commit. */
#[test]
fn script_built_content_renders() {
    let (_fixture, _runtime, page) = load(
        "<div id=root></div><script>const p=document.createElement('p');p.className='built';\
         p.textContent='built by script';document.getElementById('root').appendChild(p);</script>",
        vec![],
    );
    assert!(texts(&page).contains("built by script"), "{page}");
}

/** DOMContentLoaded and load handlers run before the page commits. */
#[test]
fn lifecycle_handlers_run() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>document.addEventListener('DOMContentLoaded',()=>{\
         document.getElementById('out').textContent='ready'});window.addEventListener('load',()=>{\
         document.getElementById('out').textContent+=' loaded'});</script>",
        vec![],
    );
    assert!(texts(&page).contains("ready loaded"), "{page}");
}

/** setTimeout(0) and requestAnimationFrame rendering lands before the first commit. */
#[test]
fn early_timers_run_before_commit() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>setTimeout(()=>{requestAnimationFrame(()=>{\
         document.getElementById('out').textContent='after frame'})},0);</script>",
        vec![],
    );
    assert!(texts(&page).contains("after frame"), "{page}");
}

/** A same-origin fetch is answered and its body reaches the page. */
#[test]
fn same_origin_fetch_feeds_the_page() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>fetch('/data.json').then(r=>r.json()).then(d=>{\
         document.getElementById('out').textContent=d.message})</script>",
        vec![(
            "/data.json",
            reply("application/json", "", "{\"message\":\"fetched\"}"),
        )],
    );
    assert!(texts(&page).contains("fetched"), "{page}");
}

/** A runtime granting two loopback origins, the page's and another one. */
fn two_origins(page: &str, other: &str) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![page.into(), other.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** A cross-origin response the server did not share (no Access-Control-Allow-Origin) fails
the fetch with no details; tests/cors_fetch.rs covers the allowed cases. */
#[test]
fn cross_origin_fetch_without_allow_origin_fails() {
    let other = Fixture::routes(vec![("/data", reply("text/plain", "", "secret"))]);
    let body = format!(
        "<p id=out>initial</p><script>fetch('{}/data').then(r=>r.text()).then(t=>{{\
         document.getElementById('out').textContent='read '+t}},e=>{{\
         document.getElementById('out').textContent='failed '+e.message}})</script>",
        other.url
    );
    let page: &'static str = Box::leak(body.into_boxed_str());
    let fixture = Fixture::routes(vec![("/", html("", page))]);
    let mut runtime = two_origins(&fixture.url, &other.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&page).contains("failed Failed to fetch"), "{page}");
    assert!(!texts(&page).contains("secret"), "{page}");
}

/** A same-origin request that redirects to another origin fails and sends nothing there. */
#[test]
fn redirect_to_another_origin_fails_and_sends_no_cookies() {
    let other = Fixture::routes(vec![("/data", reply("text/plain", "", "secret"))]);
    let target: &'static str = Box::leak(format!("{}/data", other.url).into_boxed_str());
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "Set-Cookie: session=abc; Path=/\r\n",
                "<p id=out>initial</p><script>fetch('/hop',{credentials:'include'}).then(r=>r.text())\
                 .then(t=>{document.getElementById('out').textContent='read '+t},\
                 e=>{document.getElementById('out').textContent='failed'})</script>",
            ),
        ),
        ("/hop", redirect(target)),
    ]);
    let mut runtime = two_origins(&fixture.url, &other.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&page).contains("failed"), "{page}");
    assert!(!texts(&page).contains("secret"), "{page}");
    assert_eq!(other.requests.load(std::sync::atomic::Ordering::Relaxed), 0);
}

/** `credentials: 'omit'` sends no cookies even to the page's own origin. */
#[test]
fn omitted_credentials_send_no_cookies() {
    let (fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>document.cookie='visible=1';fetch('/echo',{credentials:'omit'})\
         .then(r=>r.text()).then(t=>{document.getElementById('out').textContent='done'})</script>",
        vec![("/echo", reply("text/plain", "", "ok"))],
    );
    assert!(texts(&page).contains("done"), "{page}");
    let last = fixture.last_request.lock().unwrap().clone();
    assert!(last.starts_with("GET /echo"), "{last}");
    assert!(!last.to_ascii_lowercase().contains("cookie:"), "{last}");
}

/** document.cookie never shows HttpOnly cookies, and script-written cookies reach the server. */
#[test]
fn document_cookie_hides_http_only_and_persists_writes() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "Set-Cookie: secret=1; Path=/; HttpOnly\r\nSet-Cookie: open=2; Path=/\r\n",
                "<p id=out>initial</p><script>document.getElementById('out').textContent='seen '+document.cookie;\
                 document.cookie='written=3; path=/';</script>",
            ),
        ),
        ("/next", html("", "<p>next</p>")),
    ]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let seen = texts(&page);
    assert!(seen.contains("seen open=2"), "{seen}");
    assert!(!seen.contains("secret"), "{seen}");
    agent_call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/next", fixture.url)}),
    )
    .unwrap();
    let last = fixture.last_request.lock().unwrap().clone();
    assert!(last.contains("written=3"), "{last}");
}

/** A click listener on an ancestor runs for a click on a plain element inside it. */
#[test]
fn delegated_click_listener_runs() {
    let (_fixture, mut runtime, page) = load(
        "<p id=out>initial</p><div id=menu><a id=item href='#'>Open menu</a></div><script>\
         document.getElementById('menu').addEventListener('click',e=>{e.preventDefault();\
         document.getElementById('out').textContent='menu opened'})</script>",
        vec![],
    );
    let item = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "item")
        .unwrap()["node_id"]
        .clone();
    let clicked = agent_call(
        &mut runtime,
        "page.click",
        json!({"page_id":page["page_id"],"node_id":item,"expected_revision":page["revision"]}),
    )
    .unwrap();
    assert!(texts(&clicked).contains("menu opened"), "{clicked}");
    assert_eq!(clicked["page_id"], page["page_id"]);
}

/** A script's location assignment navigates through the ordinary path. */
#[test]
fn location_assignment_navigates() {
    let (fixture, _runtime, page) = load(
        "<p>first</p><script>location.href='/second'</script>",
        vec![("/second", html("", "<p>second page</p>"))],
    );
    assert_eq!(page["url"], format!("{}/second", fixture.url));
    assert!(texts(&page).contains("second page"), "{page}");
}

/** console output reaches page.console; an uncaught error does not undo earlier changes. */
#[test]
fn console_output_reaches_page_console() {
    let (_fixture, mut runtime, page) = load(
        "<p id=out>initial</p><script>console.log('hello from page');\
         document.getElementById('out').textContent='kept';</script>\
         <script>setTimeout(()=>{throw new Error('late')},0)</script>",
        vec![],
    );
    assert!(texts(&page).contains("kept"), "{page}");
    let console = agent_call(&mut runtime, "page.console", json!({})).unwrap();
    let entries = console["entries"].to_string();
    assert!(entries.contains("hello from page"), "{entries}");
}

/** Password values never reach page scripts. */
#[test]
fn password_values_never_reach_scripts() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><input id=pw type=password value=hunter2><script>\
         document.getElementById('out').textContent='pw='+document.getElementById('pw').value+\
         document.getElementById('pw').getAttribute('value')</script>",
        vec![],
    );
    assert!(!texts(&page).contains("hunter2"), "{page}");
}

/** Later timers run on the coordinator's idle schedule and publish a new revision. */
#[test]
fn later_timers_publish_a_new_revision() {
    let (_fixture, mut runtime, page) = load(
        "<p id=out>initial</p><script>setTimeout(()=>{\
         document.getElementById('out').textContent='later'},300)</script>",
        vec![],
    );
    assert!(texts(&page).contains("initial"), "{page}");
    let mut published = false;
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        published |= runtime.run_page_scripts();
    }
    assert!(published);
    let now = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(texts(&now).contains("later"), "{now}");
    assert_eq!(now["revision"], 2);
}
