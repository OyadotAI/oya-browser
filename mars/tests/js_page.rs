/** Page scripts run through the app the way a browser runs them: lifecycle, DOM building,
timers, same-origin requests, cookies, clicks and navigation, crossing the real owned services. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, desktop, html};

/** Page loads a refresh chain may make: the first page plus the runtime's bound on refresh navigations. */
const LOADS_PER_CHAIN: usize = 6;

/** Both script servers must receive a request before either sends its response. */
const CONCURRENT_SCRIPTS: usize = 2;
/** A sequential implementation fails promptly instead of leaving a fixture thread blocked. */
const SCRIPT_OVERLAP_WAIT: std::time::Duration = std::time::Duration::from_secs(3);
/** Independent fixture threads rendezvous without relying on elapsed-time performance assertions. */
type ScriptArrival = Arc<(std::sync::Mutex<usize>, std::sync::Condvar)>;

/** A script server records whether both downloads were active while it held its response. */
fn held_script(
    arrived: ScriptArrival,
    overlapped: Arc<std::sync::atomic::AtomicUsize>,
    text: &'static str,
) -> Fixture {
    Fixture::start(
        Box::new(move |_| {
            let (count, wake) = arrived.as_ref();
            let mut count = count.lock().unwrap();
            *count += 1;
            wake.notify_all();
            let (count, _) = wake
                .wait_timeout_while(count, SCRIPT_OVERLAP_WAIT, |count| {
                    *count < CONCURRENT_SCRIPTS
                })
                .unwrap();
            if *count >= CONCURRENT_SCRIPTS {
                overlapped.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            reply("text/javascript", "", text)
        }),
        false,
    )
}

/** Dynamic classic downloads overlap, while their effects still follow the page's requested order. */
#[test]
fn dynamic_classic_scripts_download_concurrently() {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    let arrived = ScriptArrival::default();
    let overlapped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let first = held_script(
        Arc::clone(&arrived),
        Arc::clone(&overlapped),
        "document.getElementById('out').textContent+='a'",
    );
    let second = held_script(
        Arc::clone(&arrived),
        Arc::clone(&overlapped),
        "document.getElementById('out').textContent+='b'",
    );
    let body = format!(
        "<p id=out></p><script>for(const src of ['{}/a.js','{}/b.js']){{const s=document.createElement('script');s.async=false;s.src=src;document.head.appendChild(s)}}</script>",
        first.url, second.url
    );
    let fixture = Fixture::raw(html("", &body));
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone(), first.url.clone(), second.url.clone()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(
        overlapped.load(std::sync::atomic::Ordering::SeqCst),
        CONCURRENT_SCRIPTS
    );
    assert!(texts(&page).contains("ab"), "{page}");
}

/** A denied dynamic script never reaches the network, including through batch prefetch. */
#[test]
fn dynamic_script_prefetch_preserves_csp_admission() {
    let body = "<p id=out></p><script nonce=run>let failed=0;for(const src of ['/a.js','/b.js']){const s=document.createElement('script');s.src=src;s.onerror=()=>document.getElementById('out').textContent=String(++failed);document.head.appendChild(s)}</script>";
    let (fixture, counts) = Fixture::routes_counted(vec![(
        "/",
        html("Content-Security-Policy: script-src 'nonce-run'\r\n", body),
    )]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&page).contains('2'), "{page}");
    let counts = counts.lock().unwrap();
    assert!(!counts.contains_key("/a.js") && !counts.contains_key("/b.js"));
}

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

/** A same-origin request redirected to another origin follows the redirect as Fetch's CORS
mode does (Chrome sends it, with an Origin header), and with no Access-Control-Allow-Origin
the page cannot read the response. Cookies are host-scoped, not port-scoped (RFC 6265), so
the two loopback fixtures share them, as they would in Chrome. */
#[test]
fn redirect_to_another_origin_is_unreadable_without_cors() {
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
    let sent = other.last_request.lock().unwrap().clone();
    assert!(
        sent.contains(&format!("Origin: {}", fixture.url.trim_end_matches('/'))),
        "{sent}"
    );
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

/** Script that measures a node it just inserted sees that node's laid-out box, as a browser's
forced layout gives it, and can position the node with it (a centred consent dialog). */
#[test]
fn measuring_an_inserted_node_sees_its_layout() {
    let (_fixture, mut runtime, page) = load(
        "<button id=go>go</button><p id=out>none</p><script>document.getElementById('go')\
         .addEventListener('click',()=>{const d=document.createElement('div');\
         d.style.width='200px';d.textContent='dialog';document.body.appendChild(d);\
         const r=d.getBoundingClientRect();document.getElementById('out').textContent=\
         'width '+r.width+' left '+(innerWidth-r.width)/2})</script>",
        vec![],
    );
    let go = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "go")
        .unwrap()["node_id"]
        .clone();
    let clicked = agent_call(
        &mut runtime,
        "page.click",
        json!({"page_id":page["page_id"],"node_id":go,"expected_revision":page["revision"]}),
    )
    .unwrap();
    let width = clicked["viewport"]["width"].as_f64().unwrap();
    let expected = format!("width 200 left {}", (width - 200.0) / 2.0);
    assert!(texts(&clicked).contains(&expected), "{clicked}");
}

/** The names page.find reports for links, in document order. */
fn link_names(runtime: &mut mars::runtime::Runtime, visible_only: bool) -> Vec<String> {
    let found = agent_call(
        runtime,
        "page.find",
        json!({"role":"link","visible_only":visible_only}),
    )
    .unwrap();
    found["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|link| link["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

/** A link whose text sits in child elements, an image's alt or an svg's aria-label is named by that
content, as a browser's accessibility tree names it; aria-hidden content is left out. */
#[test]
fn links_are_named_by_their_content() {
    let (_fixture, mut runtime, _page) = load(
        "<a href=/tech><span>Tech</span> <b>news</b></a><a href=/home><img alt=Home></a>\
         <a href=/logo><svg aria-label=Logo></svg></a><a href=/tip title=Tip></a>\
         <a href=/real><span aria-hidden=true>X</span>Real</a>",
        vec![],
    );
    assert_eq!(
        link_names(&mut runtime, false),
        ["Tech news", "Home", "Logo", "Tip", "Real"]
    );
}

/** visible_only keeps links a person sees: on screen on both axes and not clipped away, so a
skip link parked off the left edge or visually hidden by a clip is found only without it. */
#[test]
fn visible_only_leaves_out_links_off_screen_or_clipped_away() {
    let (_fixture, mut runtime, page) = load(
        "<a href=/skip style='position:absolute;left:-9999px'>Skip</a><p><a href=/on>On</a>\
         <a href=/clipped style='position:absolute;clip:rect(0 0 0 0);width:1px;height:1px;\
         overflow:hidden'>Clipped</a>",
        vec![],
    );
    assert_eq!(link_names(&mut runtime, true), ["On"]);
    assert_eq!(link_names(&mut runtime, false), ["Skip", "On", "Clipped"]);
    let found = agent_call(
        &mut runtime,
        "page.find",
        json!({"role":"link","name":"On"}),
    )
    .unwrap();
    assert!(
        found["matches"][0]["href"]
            .as_str()
            .unwrap()
            .ends_with("/on"),
        "{found}"
    );
    let skip = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["name"] == "Skip")
        .unwrap();
    assert_eq!(skip["visible"], false);
}

/** Stylesheets and scripts carried in `data:` URLs (media type with spaces and parameters,
percent-encoded or base64) load locally as facebook.com's do; nothing reaches the network. */
#[test]
fn data_url_stylesheets_and_scripts_load() {
    let (_fixture, _runtime, page) = load(
        "<link rel=stylesheet href='data:text/css; charset=utf-8,%23gone%7Bdisplay:none%7D'>\
         <p id=gone>hidden by the sheet</p><p id=out>initial</p>\
         <script src='data:application/x-javascript;base64,ZG9jdW1lbnQuZ2V0RWxlbWVudEJ5SWQoJ291dCcpLnRleHRDb250ZW50PSdmcm9tIGRhdGEn'></script>",
        vec![],
    );
    assert!(texts(&page).contains("from data"), "{page}");
    let gone = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "gone")
        .unwrap();
    assert!(gone.get("bounds").is_none(), "{gone}");
}

/** A whitespace-only text node a script inserts, as a Polymer template stamps for a text
binding, is kept, so the binding's later text reaches the page. */
#[test]
fn bound_text_in_whitespace_node_renders() {
    let (_fixture, _runtime, page) = load(
        "<div id=root></div><script>const t=document.createElement('template');\
         t.innerHTML='<a id=\"e\">\\n  <span class=\"title\">[[x]]</span>\\n</a>';\
         t.content.querySelector('span').firstChild.textContent=' ';\
         const c=document.importNode(t.content,true);const text=c.querySelector('span').firstChild;\
         document.getElementById('root').appendChild(c);\
         setTimeout(()=>{text.textContent='Home'},0);</script>",
        vec![],
    );
    assert!(texts(&page).contains("Home"), "{page}");
}

/** A gzip-encoded binary body, as YouTube posts its guide request, reaches the server as the
JSON it encodes: the runtime sends the decoded identity body without the Content-Encoding
header, as it always has, and the page reads the answer. */
#[test]
fn gzipped_binary_body_reaches_the_server_decoded() {
    let (fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>const gz=new Uint8Array([31,139,8,0,0,0,0,0,2,255,171,86,\
         74,47,205,76,73,85,178,42,41,42,77,173,5,0,148,63,164,148,14,0,0,0]);\
         fetch('/api',{method:'POST',headers:{'Content-Type':'application/json',\
         'Content-Encoding':'gzip'},body:gz}).then(r=>r.json()).then(d=>{\
         document.getElementById('out').textContent=d.items},e=>{\
         document.getElementById('out').textContent='failed '+e})</script>",
        vec![(
            "/api",
            reply("application/json", "", "{\"items\":\"Home Shorts\"}"),
        )],
    );
    assert!(texts(&page).contains("Home Shorts"), "{page}");
    let request = fixture.last_request.lock().unwrap().clone();
    assert!(request.ends_with("{\"guide\":true}"), "{request}");
    assert!(
        !request.to_ascii_lowercase().contains("content-encoding"),
        "{request}"
    );
}

/** A binary body that is not UTF-8 (a protobuf, as YouTube's SABR requests carry) reaches the server
whole: its Content-Length is the byte count, no Content-Type is invented for a buffer, and the
server sees every byte (here through the fixture's lossy view, where each invalid byte is one
replacement character). */
#[test]
fn a_binary_body_reaches_the_server_whole() {
    let (fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>const raw=new Uint8Array([8,255,0,128,195,40,10]);\
         fetch('/api',{method:'POST',body:raw}).then(r=>r.text()).then(t=>{\
         document.getElementById('out').textContent='sent '+t},e=>{\
         document.getElementById('out').textContent='failed '+e})</script>",
        vec![("/api", reply("text/plain", "", "ok"))],
    );
    assert!(texts(&page).contains("sent ok"), "{page}");
    let request = fixture.last_request.lock().unwrap().clone();
    let lossy = String::from_utf8_lossy(&[8, 255, 0, 128, 195, 40, 10]).to_string();
    assert!(request.ends_with(&lossy), "{request:?}");
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("content-length: 7"), "{request}");
    assert!(!lower.contains("content-type"), "{request}");
}

/** A string body still goes out as UTF-8 text labeled text/plain. */
#[test]
fn a_text_body_reaches_the_server_as_utf8_text() {
    let (fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>fetch('/api',{method:'POST',body:'h\\u00e9llo'})\
         .then(r=>r.text()).then(t=>{document.getElementById('out').textContent='sent '+t},e=>{\
         document.getElementById('out').textContent='failed '+e})</script>",
        vec![("/api", reply("text/plain", "", "ok"))],
    );
    assert!(texts(&page).contains("sent ok"), "{page}");
    let request = fixture.last_request.lock().unwrap().clone();
    assert!(request.ends_with("h\u{e9}llo"), "{request:?}");
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("content-length: 6"), "{request}");
    assert!(
        lower.contains("content-type: text/plain;charset=utf-8"),
        "{request}"
    );
}

/** A `data:` URL fetch is answered locally with its decoded text and media type, as Fetch's
scheme fetch does; YouTube reads its player configuration this way. */
#[test]
fn data_url_fetch_reads_its_text() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>fetch('data:application/json;base64,eyJvayI6InllcyJ9')\
         .then(r=>r.json()).then(d=>{document.getElementById('out').textContent='data '+d.ok},\
         e=>{document.getElementById('out').textContent='failed '+e})</script>",
        vec![],
    );
    assert!(texts(&page).contains("data yes"), "{page}");
}

/** A request the slice had no time left to start waits for the next slice instead of failing
as a network error: here a hung request uses up the load's settle time, and the page's own
request still answers later. */
#[test]
fn requests_past_the_slice_wait_instead_of_failing() {
    let hung = Fixture::slow();
    let body = format!(
        "<p id=out>initial</p><script>fetch('{}/hang',{{mode:'no-cors'}}).catch(()=>{{}});\
         fetch('/data.json').then(r=>r.json()).then(d=>{{\
         document.getElementById('out').textContent=d.message}},e=>{{\
         document.getElementById('out').textContent='failed '+e.message}})</script>",
        hung.url
    );
    let page: &'static str = Box::leak(body.into_boxed_str());
    let fixture = Fixture::routes(vec![
        ("/", html("", page)),
        (
            "/data.json",
            reply("application/json", "", "{\"message\":\"fetched\"}"),
        ),
    ]);
    let mut runtime = two_origins(&fixture.url, &hung.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(!texts(&page).contains("failed"), "{page}");
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.run_page_scripts();
    }
    let now = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(texts(&now).contains("fetched"), "{now}");
}

/** A script-built subtree whose markup passes one fragment's bound (a font probe's div of
hundreds of styled spans) is inserted whole, and the page's scripts keep running. */
#[test]
fn large_script_built_subtree_inserts_whole() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>const d=document.createElement('div');\
         for(let i=0;i<900;i++){const s=document.createElement('span');\
         s.setAttribute('style','position:absolute;left:-9999px;font-size:72px;font-style:normal;font-weight:normal;letter-spacing:normal');\
         s.textContent='mmmmmmmmmmlli '+i;d.appendChild(s)}document.body.appendChild(d);\
         document.getElementById('out').textContent='probed '+d.children.length+' '+d.lastChild.isConnected</script>",
        vec![],
    );
    assert!(texts(&page).contains("probed 900 true"), "{page}");
    assert!(!page["degraded"].to_string().contains("scripts"), "{page}");
}

/** A fetched SVG icon, parsed with DOMParser and appended to its placeholder as LinkedIn's
lazy loader does, becomes part of the page. */
#[test]
fn fetched_svg_icon_is_parsed_and_inserted() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><icon id=slot></icon><script>fetch('/icon.svg').then(r=>r.text())\
         .then(t=>{const svg=new DOMParser().parseFromString(t,'image/svg+xml').firstChild;\
         document.getElementById('slot').appendChild(svg);\
         document.getElementById('out').textContent='icon '+svg.tagName+' '+svg.isConnected})</script>",
        vec![(
            "/icon.svg",
            reply(
                "image/svg+xml",
                "",
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path d=\"M0 0h24v24H0z\"/></svg>",
            ),
        )],
    );
    assert!(texts(&page).contains("icon svg true"), "{page}");
    assert!(
        page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["tag"] == "svg"),
        "{page}"
    );
}

/** A stylesheet a script links into the head is fetched and applied, and its `load` event
fires: craigslist keeps its curtain up until its page CSS reports loaded. */
#[test]
fn script_linked_stylesheet_applies_and_fires_load() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><p id=hide>shown</p><script>const l=document.createElement('link');\
         l.rel='stylesheet';l.href='/page.css';l.onload=()=>{\
         document.getElementById('out').textContent='css loaded'};\
         document.head.appendChild(l)</script>",
        vec![("/page.css", reply("text/css", "", "#hide{display:none}"))],
    );
    assert!(texts(&page).contains("css loaded"), "{page}");
    let hidden = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "hide");
    assert!(hidden.is_none_or(|node| node["visible"] == false), "{page}");
}

/** The deferred-sheet idiom: a parsed `<link media=print>` gets its `load` event, and the script
that then sets `media = 'all'` makes the sheet apply. google.com hides its Gmail and Images links
until its header sheet loads this way. */
#[test]
fn parsed_print_sheet_applies_once_its_load_sets_media_all() {
    let (_fixture, _runtime, page) = load(
        "<style>#late{display:none}</style><p id=late>late link</p>\
         <link id=ss rel=stylesheet media=print href=/late.css><script>\
         const l=document.getElementById('ss');l.onload=()=>{l.media='all'};\
         l.removeAttribute('id')</script>",
        vec![("/late.css", reply("text/css", "", "#late{display:block}"))],
    );
    let late = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "late");
    assert!(late.is_some_and(|node| node["visible"] == true), "{page}");
}

/** A sheet a script links that never loads is reported in `degraded`, as a linked one is. */
#[test]
fn script_linked_stylesheet_that_fails_is_degraded() {
    let (_fixture, _runtime, page) = load(
        "<a href=/next id=go>Next</a><script>const l=document.createElement('link');\
         l.rel='stylesheet';l.href='/missing.css';document.head.appendChild(l)</script>",
        vec![],
    );
    assert!(
        page["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "stylesheets"),
        "{page}"
    );
}

/** A script-linked sheet past the style allowance leaves its hiding rules unknown, so no node
 * on the page may be acted on: the same fail-closed rule navigation applies to linked sheets. */
#[test]
fn script_linked_stylesheet_past_the_allowance_fails_closed() {
    // The allowance is larger than one HTML body may be, so linked sheets fill it.
    let sheets = mars::constants::STYLE_TOTAL_BYTES / mars::constants::STYLE_BYTES;
    let filler = mars::constants::STYLE_BYTES - FILLER_SLACK;
    let mut routes: Vec<(&'static str, Vec<u8>)> = (0..sheets)
        .map(|index| {
            let path: &'static str = Box::leak(format!("/fill{index}.css").into_boxed_str());
            let css = format!("/*{}*/", "x".repeat(filler));
            (path, reply("text/css", "", &css))
        })
        .collect();
    let links: String = (0..sheets)
        .map(|index| format!("<link rel=stylesheet href=/fill{index}.css>"))
        .collect();
    let body = format!(
        "{links}<a href=/next id=go>Next</a><script>const l=document.createElement('link');\
         l.rel='stylesheet';l.href='/more.css';document.head.appendChild(l)</script>"
    );
    let past = "y".repeat(sheets * FILLER_SLACK);
    routes.push((
        "/more.css",
        reply("text/css", "", &format!("#go{{color:red}}/*{past}*/")),
    ));
    let (_fixture, mut runtime, page) = load(&body, routes);
    assert!(
        page["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "stylesheets"),
        "{page}"
    );
    let found = agent_call(&mut runtime, "page.find", json!({"text":"Next"})).unwrap();
    let link = &found["matches"][0];
    assert_eq!(link["disabled"], true, "{found}");
}

/** Bytes each filler sheet leaves under STYLE_BYTES; the script-linked sheet is larger than
all of them together, so it lands past the allowance. */
const FILLER_SLACK: usize = 64;

/** The node id of the element whose HTML id is `html_id`. */
fn node_by_html_id(page: &Value, html_id: &str) -> Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == html_id)
        .unwrap()["node_id"]
        .clone()
}

/** A page that changes itself in the background keeps its timers awake only as long as the
changes are progress, and never past AGENT_AWAKE_CAP after the agent's last action (the window
past TICK_WINDOW is proven in tests/agent_progress_window.rs): clicks are not starved for ever. */
#[test]
fn background_change_extends_awake_window_within_its_cap() {
    let (_fixture, mut runtime, _page) = load(
        "<p id=out>0</p><script>let n=0;setInterval(()=>{\
         document.getElementById('out').textContent=String(++n)},50)</script>",
        vec![],
    );
    let awake = runtime.script_awake_until().unwrap();
    let mut published = false;
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        published |= runtime.run_page_scripts();
    }
    assert!(published);
    let extended = runtime.script_awake_until().unwrap();
    assert!(extended >= awake, "progress must not shorten the window");
    assert!(
        extended <= std::time::Instant::now() + std::time::Duration::from_secs(180),
        "progress extended the window past the agent cap"
    );
}

/** A person's or agent's input keeps the page's timers running for another window. */
#[test]
fn input_extends_awake_window() {
    let (_fixture, mut runtime, page) = load(
        "<p id=out>initial</p><div id=go>go</div><script>document.getElementById('go')\
         .addEventListener('click',()=>{setTimeout(()=>{\
         document.getElementById('out').textContent='clicked'},1000)})</script>",
        vec![],
    );
    let awake = runtime.script_awake_until().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    agent_call(
        &mut runtime,
        "page.click",
        json!({"page_id":page["page_id"],"node_id":node_by_html_id(&page,"go"),
               "expected_revision":page["revision"]}),
    )
    .unwrap();
    assert!(runtime.script_awake_until().unwrap() > awake);
}

/** Input that changes nothing yet still keeps the page awake: an agent's evaluation that only
sets a timer commits without a new revision, and that timer must still get to run. */
#[test]
fn quiet_input_extends_awake_window() {
    let (_fixture, mut runtime, _page) = load("<p id=out>initial</p>", vec![]);
    let awake = runtime.script_awake_until().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let result = agent_call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"setTimeout(()=>{},1000);1"}}),
    )
    .unwrap();
    assert_eq!(result["revision"], 1, "{result}");
    assert!(runtime.script_awake_until().unwrap() > awake);
}

/** A background run asks whether input is waiting only after its first step, and stops there
when it is: the page still progresses and commits, and the rest waits for the next run. */
#[test]
fn background_run_yields_to_waiting_input_after_one_step() {
    let (_fixture, mut runtime, _page) = load(
        "<p id=out>initial</p><script>setTimeout(()=>{\
         document.getElementById('out').textContent='timer';\
         fetch('/data.json').then(r=>r.json()).then(d=>{\
         document.getElementById('out').textContent=d.message})},500)</script>",
        vec![(
            "/data.json",
            reply("application/json", "", "{\"message\":\"fetched\"}"),
        )],
    );
    std::thread::sleep(std::time::Duration::from_millis(600));
    let mut asked = 0;
    let published = runtime.run_page_scripts_yielding(&mut || {
        asked += 1;
        true
    });
    assert!(published);
    assert_eq!(asked, 1);
    let now = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(texts(&now).contains("timer"), "{now}");
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.run_page_scripts();
    }
    let now = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(texts(&now).contains("fetched"), "{now}");
}

/** A burst of fetches as large as YouTube's (its icon set plus telemetry and data, over forty
at once) is answered in full: none fails for want of room in the page's request queue. */
#[test]
fn a_large_burst_of_fetches_all_succeed() {
    let (_fixture, mut runtime, _page) = load(
        "<p id=out>waiting</p><script>let ok=0;const n=48;for(let i=0;i<n;i++)\
         fetch('/data.json').then(r=>r.json()).then(()=>{if(++ok===n)\
         document.getElementById('out').textContent='all '+ok}).catch(e=>{\
         document.getElementById('out').textContent='failed '+e.message})</script>",
        vec![(
            "/data.json",
            reply("application/json", "", "{\"message\":\"fetched\"}"),
        )],
    );
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.run_page_scripts();
    }
    let now = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(texts(&now).contains("all 48"), "{now}");
}

/** Native descendants keep their real event target while the ancestor supplies button activation. */
#[test]
fn clicking_a_button_descendant_preserves_the_original_event_target() {
    let (_fixture, mut runtime, page) = load(
        "<p id=out>initial</p><button type=button onclick=\"document.getElementById('out').textContent=event.target.id\"><span id=child>Go</span></button><script>void 0</script>",
        vec![],
    );
    let child = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == "child")
        .unwrap()["node_id"]
        .clone();
    let clicked = agent_call(
        &mut runtime,
        "page.click",
        json!({
            "page_id":page["page_id"], "node_id":child, "expected_revision":page["revision"]
        }),
    )
    .unwrap();
    assert!(texts(&clicked).contains("child"), "{clicked}");
}

/** How long the slow route below holds its answer: well past a navigation's settle, with room for a
debug build on a loaded machine, so the test tells waiting apart from slowness. */
const SLOW_ANSWER: std::time::Duration = std::time::Duration::from_secs(8);

/** A fetch on a slow link does not hold the page: navigation returns before the answer comes,
and the answer still reaches the page on a later background run. */
#[test]
fn a_slow_fetch_does_not_hold_the_page() {
    let page: &'static str = "<p id=out>waiting</p><script>fetch('/slow').then(r=>r.text())\
        .then(t=>document.getElementById('out').textContent=t)</script>";
    let fixture = support::Fixture::start(
        Box::new(move |request| {
            if request.split_whitespace().nth(1) == Some("/slow") {
                std::thread::sleep(SLOW_ANSWER);
                return reply("text/plain", "", "arrived");
            }
            html("", page)
        }),
        false,
    );
    let mut runtime = desktop(&fixture.url);
    let started = std::time::Instant::now();
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(
        started.elapsed() < SLOW_ANSWER,
        "navigation waited for the fetch"
    );
    assert!(texts(&loaded).contains("waiting"), "{loaded}");
    let deadline = std::time::Instant::now() + 4 * SLOW_ANSWER;
    let mut now = loaded;
    while !texts(&now).contains("arrived") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.run_page_scripts();
        now = agent_call(&mut runtime, "page.inspect", json!({})).unwrap();
    }
    assert!(texts(&now).contains("arrived"), "{now}");
}

/** Keeps the runtime's background work running for up to `attempts` short slices, stopping once `done` holds. */
fn run_until(
    runtime: &mut mars::runtime::Runtime,
    attempts: usize,
    done: impl Fn(&Value) -> bool,
) -> Value {
    let mut page = agent_call(runtime, "page.inspect", json!({})).unwrap();
    for _ in 0..attempts {
        if done(&page) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        runtime.run_page_scripts();
        page = agent_call(runtime, "page.inspect", json!({})).unwrap();
    }
    page
}

/** A declarative refresh follows its delay to the address it names, as Chrome does, and replaces the entry it was loaded into. */
#[test]
fn meta_refresh_follows_after_its_delay() {
    let (fixture, mut runtime, page) = load(
        "<meta http-equiv='refresh' content='0; url=/second'><p>first</p>",
        vec![("/second", html("", "<p>second page</p>"))],
    );
    assert_eq!(page["url"], format!("{}/", fixture.url), "{page}");
    let after = run_until(&mut runtime, 40, |page| {
        page["url"] == format!("{}/second", fixture.url)
    });
    assert_eq!(after["url"], format!("{}/second", fixture.url), "{after}");
    assert!(texts(&after).contains("second page"), "{after}");
    let history = agent_call(&mut runtime, "page.history", json!({})).unwrap();
    assert_eq!(history["length"], 1, "{history}");
}

/** A refresh with a delay waits for it: the page stays on its first document until the delay has passed. */
#[test]
fn meta_refresh_waits_for_its_delay() {
    let (fixture, mut runtime, page) = load(
        "<meta http-equiv='refresh' content='30; url=/second'><p>first</p>",
        vec![("/second", html("", "<p>second page</p>"))],
    );
    let after = run_until(&mut runtime, 10, |_| false);
    assert_eq!(after["url"], format!("{}/", fixture.url), "{page}");
    assert!(texts(&after).contains("first"), "{after}");
}

/** A refresh to a non-web address is not followed, so a script URL cannot run through the refresh path. */
#[test]
fn meta_refresh_to_a_script_address_is_not_followed() {
    let (fixture, mut runtime, _page) = load(
        "<meta http-equiv='refresh' content='0; url=javascript:void(0)'><p>first</p>",
        vec![],
    );
    let after = run_until(&mut runtime, 10, |_| false);
    assert_eq!(after["url"], format!("{}/", fixture.url), "{after}");
}

/** A page that refreshes to itself forever is stopped by the chain bound: the refresh navigations stop, and the page stays put. */
#[test]
fn meta_refresh_chain_is_bounded() {
    let body = "<meta http-equiv='refresh' content='0'><p>loop</p>";
    let (fixture, counts) = Fixture::routes_counted(vec![("/", html("", body))]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    run_until(&mut runtime, 80, |_| false);
    let settled = counts.lock().unwrap()["/"];
    run_until(&mut runtime, 10, |_| false);
    let later = counts.lock().unwrap()["/"];
    assert_eq!(settled, later, "the refresh loop kept going");
    assert!(settled <= LOADS_PER_CHAIN, "{settled} page loads");
}

/** deleteRule on a sheet with a nested `@media` block leaves the rules after it in force, as
styled-components' cleanup needs: the page's hidden drawer stays hidden instead of falling inside a
media block whose closing brace was lost. */
#[test]
fn delete_rule_leaves_later_rules_in_force_after_a_media_block() {
    let (_fixture, _runtime, page) = load(
        "<style id=s>@media screen and (max-width: 599px){.m{color:red}}\n.gone{color:blue}\n\
         .target{visibility:hidden}</style><div class=target id=t>x</div><p id=o></p>\
         <script>const s=document.getElementById('s');s.sheet.deleteRule(1);\
         document.getElementById('o').textContent='vis='+\
         getComputedStyle(document.getElementById('t')).visibility;</script>",
        vec![],
    );
    assert!(texts(&page).contains("vis=hidden"), "{page}");
}

/** A script's `<style>` text that closes itself with markup stays text in the agent's document:
the `</style>` in it cannot make an `<a id=evil>` element the script never created. The page keeps
working, so the script's own element still renders. */
#[test]
fn raw_text_closer_from_script_adds_no_element() {
    let (_fixture, _runtime, page) = load(
        "<div id=root></div><script>const s=document.createElement('style');\
         s.textContent='</style><a id=evil href=/x>x</a>';\
         document.getElementById('root').appendChild(s);\
         const p=document.createElement('p');p.textContent='still working';\
         document.getElementById('root').appendChild(p);</script>",
        vec![],
    );
    let evil = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["html_id"] == "evil");
    assert!(!evil, "{page}");
    assert!(texts(&page).contains("still working"), "{page}");
}

/** A closing tag split across two text nodes of a raw-text element, or an element a script
appends inside one, still cannot end the element early when the document is projected: the
element's whole text is guarded at once and its children are written as text. */
#[test]
fn split_or_nested_raw_text_closers_add_no_element() {
    let (_fixture, _runtime, page) = load(
        "<div id=root></div><script>const s=document.createElement('style');\
         s.appendChild(document.createTextNode('</st'));\
         s.appendChild(document.createTextNode('yle><a id=split href=/x>x</a>'));\
         document.getElementById('root').appendChild(s);\
         const t=document.createElement('style');\
         const inner=document.createElement('b');inner.textContent='</style><a id=nested href=/y>y</a>';\
         t.appendChild(inner);document.getElementById('root').appendChild(t);\
         const p=document.createElement('p');p.textContent='still working';\
         document.getElementById('root').appendChild(p);</script>",
        vec![],
    );
    let ids: Vec<&str> = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["html_id"].as_str())
        .collect();
    assert!(
        !ids.contains(&"split") && !ids.contains(&"nested"),
        "{page}"
    );
    assert!(texts(&page).contains("still working"), "{page}");
}

/** A synchronous XMLHttpRequest (Akamai's interstitial does one) gets its answer before the
script's next line: the step replays with the answer, so the text is there after the send. */
#[test]
fn synchronous_xhr_answers_before_the_next_statement() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>const x=new XMLHttpRequest();x.open('GET','/sync.txt',false);\
         x.send();document.getElementById('out').textContent='sync '+x.status+' '+x.responseText</script>",
        vec![("/sync.txt", reply("text/plain", "", "hello"))],
    );
    assert!(texts(&page).contains("sync 200 hello"), "{page}");
    assert!(!page["degraded"].to_string().contains("scripts"), "{page}");
}

/** A timer's synchronous request resumes with its original state and completes the task. */
#[test]
fn timer_synchronous_xhr_answers_before_the_next_statement() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script>setTimeout(()=>{const x=new XMLHttpRequest();\
         x.open('GET','/sync.txt',false);x.send();\
         document.getElementById('out').textContent='sync '+x.status+' '+x.responseText},0)</script>",
        vec![("/sync.txt", reply("text/plain", "", "hello"))],
    );
    assert!(texts(&page).contains("sync 200 hello"), "{page}");
    assert!(!page["degraded"].to_string().contains("scripts"), "{page}");
}

/** A synchronous request a different origin refuses (no CORS grant) throws NetworkError at send(),
which the page's own try/catch takes; the statements after it still run. */
#[test]
fn refused_synchronous_cross_origin_request_is_a_catchable_network_error() {
    let other = Fixture::routes(vec![("/data", reply("text/plain", "", "secret"))]);
    let body = format!(
        "<p id=out>start</p><script>let r='';const x=new XMLHttpRequest();x.open('GET','{}/data',false);\
         try{{x.send()}}catch(e){{r=e.name}}document.getElementById('out').textContent='after '+r;</script>",
        other.url
    );
    let (_fixture, _runtime, page) = load(&body, vec![]);
    assert!(texts(&page).contains("after NetworkError"), "{page}");
}
