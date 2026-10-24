/** A page's cross-origin fetch and XMLHttpRequest follow the Fetch standard's CORS rules through
the real runtime and owned services: two granted loopback origins, the page's and an API's. */
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

/** A redirect to `location` with extra header lines. */
fn redirect(location: &str, headers: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\n{headers}Content-Length: 0\r\nConnection: close\r\n\r\n"
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

/** A runtime granting the page's origin and the API's. */
fn two_origins(page: &str, api: &str) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![page.into(), api.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** A page fixture whose script is `script`; `'API` in it reads the API origin from the
page URL's fragment, so the API fixture may start after the page and still name its origin. The
page sets a session cookie so credentials are visible to the API. */
fn page(script: &str) -> Fixture {
    let script = script.replace("'API", "location.hash.slice(1)+'");
    let body = format!("<p id=out>initial</p><script>{script}</script>");
    let page: &'static str = Box::leak(body.into_boxed_str());
    Fixture::routes(vec![(
        "/",
        html("Set-Cookie: session=abc; Path=/; SameSite=Lax\r\n", page),
    )])
}

/** Navigate to `page` naming `api` in the fragment; returns the page's text after it settles. */
fn visit(page: &Fixture, api: &Fixture) -> String {
    let mut runtime = two_origins(&page.url, &api.url);
    let url = format!("{}/#{}", page.url, api.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":url})).unwrap();
    texts(&page)
}

/** Load `script` against `api` in one step. */
fn run(api: &Fixture, script: &str) -> String {
    visit(&page(script), api)
}

/** The page's origin as the API sees it, from the one page fixture a run starts. */
fn origin_of(last: &str) -> Option<String> {
    last.lines()
        .find_map(|line| line.strip_prefix("Origin: "))
        .map(str::to_owned)
}

/** Reads the body as text into #out, or the error message on failure. */
const SHOW: &str = ".then(r=>r.text()).then(t=>{document.getElementById('out').textContent='read '+t},\
                    e=>{document.getElementById('out').textContent='failed '+e.message})";

/** Requests the API fixture received. */
fn requests(api: &Fixture) -> usize {
    api.requests.load(Ordering::Relaxed)
}

/** The API's last raw request. */
fn last(api: &Fixture) -> String {
    api.last_request.lock().unwrap().clone()
}

#[test]
fn an_allowed_cross_origin_fetch_is_read_and_sends_origin_without_cookies() {
    let api = Fixture::routes(vec![(
        "/data",
        reply("Access-Control-Allow-Origin: *\r\n", "shared"),
    )]);
    let out = run(&api, &format!("fetch('API/data'){SHOW}"));
    assert!(out.contains("read shared"), "{out}");
    let last = last(&api);
    assert!(last.starts_with("GET /data"), "{last}");
    assert!(origin_of(&last).is_some_and(|origin| origin.starts_with("http://127.0.0.1:")));
    assert!(!last.to_ascii_lowercase().contains("cookie:"), "{last}");
}

#[test]
fn a_cross_origin_response_without_allow_origin_fails_with_no_details() {
    let api = Fixture::routes(vec![("/data", reply("", "secret"))]);
    let out = run(&api, &format!("fetch('API/data'){SHOW}"));
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert!(!out.contains("secret"), "{out}");
    assert_eq!(requests(&api), 1);
}

#[test]
fn a_wildcard_does_not_admit_a_credentialed_request() {
    let api = Fixture::routes(vec![(
        "/data",
        reply("Access-Control-Allow-Origin: *\r\n", "secret"),
    )]);
    let out = run(
        &api,
        &format!("fetch('API/data',{{credentials:'include'}}){SHOW}"),
    );
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert!(last(&api).contains("Cookie: session=abc"), "{}", last(&api));
}

#[test]
fn an_exact_origin_with_allow_credentials_admits_xhr_with_credentials() {
    let page = page(
        "const x=new XMLHttpRequest();x.open('GET','API/data');x.withCredentials=true;\
         x.onload=()=>{document.getElementById('out').textContent='read '+x.responseText};\
         x.onerror=()=>{document.getElementById('out').textContent='failed'};x.send();",
    );
    let allow: &'static str = Box::leak(
        format!(
            "Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\n",
            page.url
        )
        .into_boxed_str(),
    );
    let api = Fixture::routes(vec![("/data", reply(allow, "credentialed"))]);
    let out = visit(&page, &api);
    assert!(out.contains("read credentialed"), "{out}");
    let last = last(&api);
    assert!(last.contains("Cookie: session=abc"), "{last}");
    assert_eq!(origin_of(&last).as_deref(), Some(page.url.as_str()));
}

#[test]
fn an_exact_origin_without_allow_credentials_refuses_a_credentialed_request() {
    let page = page(&format!(
        "fetch('API/data',{{credentials:'include'}}){SHOW}"
    ));
    let allow: &'static str =
        Box::leak(format!("Access-Control-Allow-Origin: {}\r\n", page.url).into_boxed_str());
    let api = Fixture::routes(vec![("/data", reply(allow, "secret"))]);
    let out = visit(&page, &api);
    assert!(out.contains("failed Failed to fetch"), "{out}");
}

#[test]
fn a_non_simple_request_is_preflighted_once_and_sent_when_allowed() {
    let allow = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: X-Token, Content-Type\r\nAccess-Control-Max-Age: 60\r\n";
    let api = Fixture::routes(vec![("/api", reply(allow, "posted"))]);
    let fetch = "fetch('API/api',{method:'POST',headers:{'X-Token':'1','Content-Type':'application/json'},body:'{}'})";
    let script = format!("{fetch}.then(()=>{fetch}){SHOW}");
    let out = run(&api, &script);
    assert!(out.contains("read posted"), "{out}");
    // One preflight, then two real requests: the second reuses the cached preflight.
    assert_eq!(requests(&api), 3);
    let last = last(&api);
    assert!(last.starts_with("POST /api"), "{last}");
    assert!(last.contains("X-Token: 1"), "{last}");
    assert!(last.contains("Content-Type: application/json"), "{last}");
}

#[test]
fn a_failed_preflight_stops_the_real_request() {
    let api = Fixture::routes(vec![(
        "/api",
        reply("Access-Control-Allow-Origin: *\r\n", "never"),
    )]);
    let out = run(
        &api,
        &format!("fetch('API/api',{{headers:{{'X-Token':'1'}}}}){SHOW}"),
    );
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert_eq!(requests(&api), 1);
    let last = last(&api);
    assert!(last.starts_with("OPTIONS /api"), "{last}");
    assert!(
        last.contains("Access-Control-Request-Method: GET"),
        "{last}"
    );
    assert!(
        last.contains("Access-Control-Request-Headers: x-token"),
        "{last}"
    );
    assert!(!last.to_ascii_lowercase().contains("cookie:"), "{last}");
}

#[test]
fn a_cross_origin_redirect_response_must_pass_the_cors_check_too() {
    let api = Fixture::routes(vec![
        ("/hop", redirect("/data", "")),
        (
            "/data",
            reply("Access-Control-Allow-Origin: *\r\n", "secret"),
        ),
    ]);
    let out = run(&api, &format!("fetch('API/hop'){SHOW}"));
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert_eq!(requests(&api), 1);
}

#[test]
fn an_allowed_redirect_within_the_other_origin_keeps_the_page_origin() {
    let api = Fixture::routes(vec![
        (
            "/hop",
            redirect("/data", "Access-Control-Allow-Origin: *\r\n"),
        ),
        (
            "/data",
            reply("Access-Control-Allow-Origin: *\r\n", "followed"),
        ),
    ]);
    let out = run(&api, &format!("fetch('API/hop'){SHOW}"));
    assert!(out.contains("read followed"), "{out}");
    let origin = origin_of(&last(&api)).unwrap_or_default();
    assert!(origin.starts_with("http://127.0.0.1:"), "{origin}");
}

#[test]
fn a_redirect_from_the_other_origin_back_to_a_private_origin_stays_refused() {
    // Egress refuses a cross-origin redirect between private fixture origins, so the tainted
    // `Origin: null` hop is never sent; the relay unit tests pin the taint rule itself.
    let page = Fixture::routes(vec![]);
    let back: &'static str = Box::leak(format!("{}/data", page.url).into_boxed_str());
    let api = Fixture::routes(vec![(
        "/hop",
        redirect(back, "Access-Control-Allow-Origin: *\r\n"),
    )]);
    let out = run(&api, &format!("fetch('API/hop'){SHOW}"));
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert_eq!(page.requests.load(Ordering::Relaxed), 0);
}

#[test]
fn no_cors_responses_are_opaque() {
    let api = Fixture::routes(vec![("/data", reply("X-Secret: 1\r\n", "secret"))]);
    let out = run(
        &api,
        "fetch('API/data',{mode:'no-cors'}).then(r=>r.text().then(t=>{\
         document.getElementById('out').textContent=r.type+' '+r.status+' ['+t+'] '+r.headers.get('x-secret')}))",
    );
    assert!(out.contains("opaque 0 [] null"), "{out}");
    assert_eq!(requests(&api), 1);
}

#[test]
fn same_origin_mode_never_sends_a_cross_origin_request() {
    let api = Fixture::routes(vec![(
        "/data",
        reply("Access-Control-Allow-Origin: *\r\n", "secret"),
    )]);
    let out = run(
        &api,
        &format!("fetch('API/data',{{mode:'same-origin'}}){SHOW}"),
    );
    assert!(out.contains("failed Failed to fetch"), "{out}");
    assert_eq!(requests(&api), 0);
}

#[test]
fn only_safelisted_and_exposed_headers_reach_the_script_and_never_set_cookie() {
    let api = Fixture::routes(vec![(
        "/data",
        reply(
            "Access-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: X-Shared, Set-Cookie\r\nX-Shared: yes\r\nX-Secret: no\r\nSet-Cookie: tracker=1\r\n",
            "body",
        ),
    )]);
    let out = run(
        &api,
        "fetch('API/data').then(r=>{const h=r.headers;document.getElementById('out').textContent=\
         [r.type,h.get('content-type'),h.get('x-shared'),h.get('x-secret'),h.get('set-cookie')].join('|')})",
    );
    assert!(out.contains("cors|text/plain|yes||"), "{out}");
}
