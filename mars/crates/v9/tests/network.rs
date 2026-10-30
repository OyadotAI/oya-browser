/** fetch and XMLHttpRequest: requests are recorded for the app; responses resume the page. */
mod common;
use common::{console, input};
use mars_v9::HostResponse;

/** Load sources and return the execution. */
fn start(sources: &[String]) -> mars_v9::Execution {
    let environment = common::environment();
    let host = common::host();
    mars_v9::load_page(input(sources, &host, &environment)).unwrap()
}

/** Resume a state with responses. */
fn answer(
    sources: &[String],
    state: mars_v9::VmState,
    responses: Vec<HostResponse>,
) -> mars_v9::Execution {
    let environment = common::environment();
    let host = common::host();
    mars_v9::resume_requests(
        input(sources, &host, &environment),
        common::round_trip(state),
        responses,
    )
    .unwrap()
}

/** fetch records a resolved request and V9 opens no connection; the response resumes the page. */
#[test]
fn fetch_records_and_resumes() {
    let sources = vec!["fetch('/api/items?x=1',{method:'post',headers:{'X-Test':'1'},body:'a=b'}).then(r=>{console.log(r.ok,r.status,r.headers.get('content-type'));return r.json()}).then(j=>console.log(j.items.length,j.items[1].name));".to_string()];
    let loaded = start(&sources);
    let request = &loaded.effects.requests[0];
    assert_eq!(request.kind, "fetch");
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://example.com/api/items?x=1");
    // Fetch's "extract a body" labels a string body text/plain in the request's own headers.
    assert_eq!(
        request.headers,
        [
            ("x-test".to_string(), "1".to_string()),
            (
                "content-type".to_string(),
                "text/plain;charset=UTF-8".to_string()
            )
        ]
    );
    assert_eq!(request.body.as_deref(), Some("a=b"));
    let response = HostResponse {
        id: request.id,
        status: 200,
        status_text: "OK".into(),
        url: request.url.clone(),
        headers: vec![("content-type".into(), "application/json".into())],
        body: "{\"items\":[{\"name\":\"a\"},{\"name\":\"b\"}]}".into(),
        error: None,
        ..HostResponse::default()
    };
    let resumed = answer(&sources, loaded.state, vec![response]);
    assert_eq!(console(&resumed), ["true 200 application/json", "2 b"]);
    assert!(resumed.effects.requests.is_empty());
}

/** A network error rejects fetch with a TypeError. */
#[test]
fn fetch_network_errors_reject() {
    let sources = vec![
        "fetch('https://other.example/x').catch(e=>console.log(e instanceof TypeError,e.message));"
            .to_string(),
    ];
    let loaded = start(&sources);
    let id = loaded.effects.requests[0].id;
    let resumed = answer(
        &sources,
        loaded.state,
        vec![HostResponse {
            id,
            error: Some("refused".into()),
            ..HostResponse::default()
        }],
    );
    assert_eq!(console(&resumed), ["true Failed to fetch"]);
}

/** async/await across the request boundary: the suspended function resumes in a later request. */
#[test]
fn await_survives_the_request_boundary() {
    let sources = vec!["(async()=>{try{const r=await fetch('/t');const t=await r.text();console.log('got',t)}finally{console.log('finally')}})();console.log('loaded');".to_string()];
    let loaded = start(&sources);
    assert_eq!(console(&loaded), ["loaded"]);
    let id = loaded.effects.requests[0].id;
    let resumed = answer(
        &sources,
        loaded.state,
        vec![HostResponse {
            id,
            status: 200,
            body: "hello".into(),
            ..HostResponse::default()
        }],
    );
    assert_eq!(console(&resumed), ["got hello", "finally"]);
}

/** XMLHttpRequest goes through open, send and the load events. */
#[test]
fn xhr_fires_readystate_and_load() {
    let sources = vec!["const x=new XMLHttpRequest();x.open('GET','data.txt');x.setRequestHeader('Accept','text/plain');\
        x.onreadystatechange=()=>console.log('state',x.readyState);x.addEventListener('load',()=>console.log('load',x.status,x.responseText,x.getResponseHeader('X-A')));x.send();".to_string()];
    let loaded = start(&sources);
    // The handler was assigned after open(), so the OPENED change was not observed.
    assert!(console(&loaded).is_empty());
    let request = &loaded.effects.requests[0];
    assert_eq!(
        (request.kind.as_str(), request.url.as_str()),
        ("xhr", "https://example.com/app/data.txt")
    );
    let response = HostResponse {
        id: request.id,
        status: 200,
        body: "text".into(),
        headers: vec![("x-a".into(), "1".into())],
        ..HostResponse::default()
    };
    let resumed = answer(&sources, loaded.state, vec![response]);
    // HEADERS_RECEIVED, LOADING and DONE each fire readystatechange (XHR standard).
    assert_eq!(
        console(&resumed),
        ["state 2", "state 3", "state 4", "load 200 text 1"]
    );
}

/** Synchronous XMLHttpRequest is refused as unsupported. */
#[test]
fn synchronous_xhr_is_unsupported() {
    let execution = common::load("const x=new XMLHttpRequest();x.open('GET','/a',false);");
    assert!(
        common::failure(&execution)
            .unwrap()
            .contains("synchronous XMLHttpRequest")
    );
}

/** sendBeacon is fire-and-forget. */
#[test]
fn send_beacon_records_without_waiting() {
    let execution = common::load("const r=navigator.sendBeacon('/log','x=1');");
    assert_eq!(common::global(&execution, "r"), "true");
    assert_eq!(execution.effects.requests[0].kind, "beacon");
    assert!(
        serde_json::to_string(&execution.state)
            .unwrap()
            .contains("\"pending\":[]")
    );
}

/** URL and URLSearchParams parse and serialize with the WHATWG rules. */
#[test]
fn url_and_search_params() {
    let source = "const u=new URL('../b?x=1&y=2#h','https://a.example/p/q');u.searchParams;const p=new URLSearchParams('a=1&b=2&a=3');p.append('c','x y');\
        const r=[u.href,u.pathname,u.searchParams.get('y'),p.getAll('a').join(),p.toString()].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "https://a.example/b?x=1&y=2#h|/b|2|1,3|a=1&b=2&a=3&c=x+y"
    );
}

/** A typed array body is sent as bytes, as browsers send a BufferSource: text when the bytes
are UTF-8, and `body_bytes` otherwise (YouTube posts its guide request gzipped this way). */
#[test]
fn buffer_bodies_are_sent_as_bytes() {
    let loaded = common::load(
        "fetch('/text', {method: 'POST', body: new TextEncoder().encode('hi')});
         fetch('/gzip', {method: 'POST', headers: {'Content-Encoding': 'gzip'}, body: new Uint8Array([31, 139, 8, 0, 255])});
         const x = new XMLHttpRequest(); x.open('POST', '/xhr'); x.send(new Uint8Array([200, 1]).buffer);",
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let requests = &loaded.effects.requests;
    assert_eq!(requests[0].body.as_deref(), Some("hi"));
    assert!(requests[0].body_bytes.is_empty());
    assert_eq!(requests[1].body, None);
    assert_eq!(requests[1].body_bytes, vec![31, 139, 8, 0, 255]);
    assert_eq!(requests[2].body_bytes, vec![200, 1]);
}
