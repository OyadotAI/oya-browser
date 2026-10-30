/** An iframe showing another origin's document runs that document's scripts in its own realm,
and the two documents talk by postMessage: the frame's `parent` and the page's `contentWindow`
are proxies, and each message arrives with the sender's origin and source. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, html};

/** A runtime granting both loopback origins. */
fn two_origins(page: &str, frame: &str) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![page.into(), frame.into()],
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** A fixture serving one HTML document at `/`. */
fn serve(body: String) -> Fixture {
    let body: &'static str = Box::leak(body.into_boxed_str());
    Fixture::routes(vec![("/", html("", body))])
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

/** The frame announces itself with its name, then echoes what the page sends it. */
fn frame() -> Fixture {
    serve(
        "<p>frame</p><script>parent.postMessage('ready:'+window.name+':'+(parent!==window),'*');addEventListener('message',e=>parent.postMessage('echo '+e.data+' from '+e.origin.slice(0,7),'*'))</script>".into(),
    )
}

/** The page answers the frame's announcement and records each message it gets. */
fn page_script() -> &'static str {
    "const out=document.getElementById('out');addEventListener('message',e=>{out.textContent+=' '+e.data+(e.source===document.querySelector('iframe').contentWindow?' (frame)':'');if(String(e.data).startsWith('ready'))e.source.postMessage('hello','*')});"
}

/** A frame in the markup loads its own page, and messages cross both ways. */
#[test]
fn a_cross_origin_frame_in_markup_talks_to_its_page() {
    let frame = frame();
    let page = serve(format!(
        "<p id=out>log</p><iframe name=I0 src='{}/'></iframe><script>{}</script>",
        frame.url,
        page_script()
    ));
    let mut runtime = two_origins(&page.url, &frame.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":page.url})).unwrap();
    let text = texts(&loaded);
    assert!(text.contains("ready:I0:true (frame)"), "{text}");
    assert!(text.contains("echo hello from http:// (frame)"), "{text}");
}

/** An iframe a script inserts loads its own page the same way. */
#[test]
fn a_cross_origin_frame_a_script_inserts_talks_to_its_page() {
    let frame = frame();
    let page = serve(format!(
        "<p id=out>log</p><script>{}const f=document.createElement('iframe');f.name='I1';f.src='{}/';document.body.appendChild(f);</script>",
        page_script(),
        frame.url
    ));
    let mut runtime = two_origins(&page.url, &frame.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":page.url})).unwrap();
    let text = texts(&loaded);
    assert!(text.contains("ready:I1:true (frame)"), "{text}");
    assert!(text.contains("echo hello from http:// (frame)"), "{text}");
}

/** A message whose target origin is not the frame's is never delivered. */
#[test]
fn a_message_for_another_origin_is_dropped() {
    let frame = serve(
        "<script>parent.postMessage('ready','*');addEventListener('message',e=>parent.postMessage('leaked','*'))</script>".into(),
    );
    let page = serve(format!(
        "<p id=out>log</p><iframe src='{}/'></iframe><script>addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data;e.source.postMessage('secret','https://elsewhere.test')}})</script>",
        frame.url
    ));
    let mut runtime = two_origins(&page.url, &frame.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":page.url})).unwrap();
    let text = texts(&loaded);
    assert!(text.contains("ready"), "{text}");
    assert!(!text.contains("leaked"), "{text}");
}

/** A MessagePort the frame transfers to the page stays entangled across the two documents. */
#[test]
fn a_transferred_port_carries_messages_both_ways() {
    let frame = serve(
        "<script>const c=new MessageChannel();c.port1.onmessage=e=>c.port1.postMessage('pong '+e.data);parent.postMessage('port','*',[c.port2]);</script>".into(),
    );
    let page = serve(format!(
        "<p id=out>log</p><iframe src='{}/'></iframe><script>addEventListener('message',e=>{{const p=e.ports[0];p.onmessage=m=>{{document.getElementById('out').textContent+=' '+m.data}};p.postMessage('ping')}})</script>",
        frame.url
    ));
    let mut runtime = two_origins(&page.url, &frame.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":page.url})).unwrap();
    let text = texts(&loaded);
    assert!(text.contains("pong ping"), "{text}");
}

/** A document that refuses to be framed (X-Frame-Options, frame-ancestors) runs no scripts as
another origin's frame. */
#[test]
fn a_document_that_refuses_framing_runs_no_scripts() {
    for header in [
        "X-Frame-Options: DENY\r\n",
        "Content-Security-Policy: frame-ancestors 'none'\r\n",
    ] {
        let body = "<script>parent.postMessage('ran','*')</script>";
        let frame = Fixture::routes(vec![("/", html(header, body))]);
        let page = serve(format!(
            "<p id=out>log</p><iframe src='{}/'></iframe><script>addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data}})</script>",
            frame.url
        ));
        let mut runtime = two_origins(&page.url, &frame.url);
        let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":page.url})).unwrap();
        assert!(!texts(&loaded).contains("ran"), "{header}");
    }
}

/** A sandboxed frame without allow-scripts runs no scripts. */
#[test]
fn a_sandboxed_frame_runs_no_scripts() {
    let frame = serve("<script>parent.postMessage('ran','*')</script>".into());
    let page = serve(format!(
        "<p id=out>log</p><iframe sandbox='allow-same-origin' src='{}/'></iframe><script>addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data}})</script>",
        frame.url
    ));
    let mut runtime = two_origins(&page.url, &frame.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":page.url})).unwrap();
    assert!(!texts(&loaded).contains("ran"));
}

/** Static and inserted same-origin sandbox frames must use the same permitted script path. */
fn same_origin_sandbox(inserted: bool) {
    let insert = if inserted {
        "const f=document.createElement('iframe');f.name='widget';f.setAttribute('sandbox','allow-scripts allow-same-origin');f.src='/frame';document.body.appendChild(f);"
    } else {
        ""
    };
    let markup = if inserted {
        ""
    } else {
        "<iframe name='widget' sandbox='allow-scripts allow-same-origin' src='/frame'></iframe>"
    };
    let source = format!(
        "<p id=out>log</p>{markup}<script>{}{insert}</script>",
        page_script()
    );
    let fixture = Fixture::routes(vec![
        ("/", html("", &source)),
        (
            "/frame",
            html(
                "",
                "<p>frame</p><script>parent.postMessage('ready:'+window.name+':'+(parent!==window),'*');addEventListener('message',e=>parent.postMessage('echo '+e.data+' from '+e.origin.slice(0,7),'*'))</script>",
            ),
        ),
    ]);
    let mut runtime = support::desktop(&fixture.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let text = texts(&loaded);
    assert!(text.contains("ready:widget:true (frame)"), "{text}");
    assert!(text.contains("echo hello from http:// (frame)"), "{text}");
}

#[test]
fn same_origin_sandbox_in_markup_executes_its_scripts() {
    same_origin_sandbox(false);
}

#[test]
fn inserted_same_origin_sandbox_executes_its_scripts() {
    same_origin_sandbox(true);
}

/** Inspect script values through the native read-only operation, without altering page state. */
fn peek(runtime: &mut mars::runtime::Runtime, expression: &str) -> Value {
    agent_call(runtime,"page.devtools_peek",json!({"request":{"kind":"evaluate","expression":expression,"options":{"return_by_value":true}}})).unwrap()["result"]["value"].clone()
}

/** One endpoint may serve distinct local-document and scripted browsing contexts. */
#[test]
fn shared_url_keeps_scripted_and_inert_owners_separate() {
    for header in [
        "",
        "X-Frame-Options: DENY\r\n",
        "Content-Security-Policy: frame-ancestors 'none'\r\n",
    ] {
        let source = "<p id=out>log</p><script>addEventListener('message',e=>{document.getElementById('out').textContent+=' '+e.data+':'+(e.source===document.getElementById('active').contentWindow)});for(const id of ['active','inert']){const f=document.createElement('iframe');f.id=id;f.src='/frame';if(id==='active')f.setAttribute('sandbox','allow-scripts allow-same-origin');document.body.appendChild(f)}</script>";
        let child = "<p id=inside>initial</p><script>document.getElementById('inside').textContent='painted';parent.postMessage('ready','*')</script>";
        let fixture = Fixture::routes(vec![
            ("/", html("", source)),
            ("/frame", html(header, child)),
        ]);
        let mut runtime = support::desktop(&fixture.url);
        let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        assert_eq!(texts(&loaded).contains("ready:true"), header.is_empty());
        assert_eq!(
            peek(
                &mut runtime,
                "document.getElementById('active').contentDocument===null"
            ),
            true
        );
        assert_eq!(
            peek(
                &mut runtime,
                "document.getElementById('inert').contentDocument.getElementById('inside').textContent"
            ),
            "initial"
        );
        if header.is_empty() {
            assert!(
                loaded["frame_nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|node| node["text"] == "painted")
            );
        }
    }
}

/** Every navigation gets one child execution and one fetch; inspections never reload it. */
#[test]
fn inserted_frame_runs_once_and_repeat_navigation_starts_fresh() {
    let source = "<p id=out>log</p><script>globalThis.loads=(globalThis.loads||0)+1;let messages=0;addEventListener('message',e=>{messages++;document.getElementById('out').textContent=loads+':'+messages+':'+e.data});const f=document.createElement('iframe');f.setAttribute('sandbox','allow-scripts allow-same-origin');f.src='/frame';document.body.appendChild(f)</script>";
    let (fixture, counts) = Fixture::routes_counted(vec![
        ("/", html("", source)),
        (
            "/frame",
            html(
                "",
                "<script>globalThis.runs=(globalThis.runs||0)+1;parent.postMessage(runs,'*')</script>",
            ),
        ),
    ]);
    let mut runtime = support::desktop(&fixture.url);
    for navigation in 1..=2 {
        let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        assert!(texts(&loaded).contains("1:1:1"), "{}", texts(&loaded));
        for _ in 0..2 {
            assert!(
                texts(&agent_call(&mut runtime, "page.inspect", json!({})).unwrap())
                    .contains("1:1:1")
            );
        }
        assert_eq!(
            counts.lock().unwrap().get("/frame").copied(),
            Some(navigation)
        );
    }
}

/** Refused, failed and removed inserted frames cannot execute or prevent a usable parent. */
#[test]
fn inserted_frame_failures_leave_parent_usable() {
    for (sandbox, remove, child, expected) in [
        (
            "allow-same-origin",
            false,
            Some("<script>parent.postMessage('ran','*')</script>"),
            false,
        ),
        (
            "allow-scripts",
            false,
            Some("<script>parent.postMessage('ran','*')</script>"),
            false,
        ),
        (
            "allow-scripts allow-same-origin",
            true,
            Some("<script>parent.postMessage('ran','*')</script>"),
            false,
        ),
        ("allow-scripts allow-same-origin", false, None, false),
        (
            "allow-scripts allow-same-origin",
            false,
            Some("<script>parent.postMessage('ran','*');throw Error('child failure')</script>"),
            true,
        ),
    ] {
        let source = format!(
            "<p id=out>usable</p><script>addEventListener('message',e=>document.getElementById('out').textContent+=' '+e.data);const f=document.createElement('iframe');f.setAttribute('sandbox','{sandbox}');f.src='/frame';document.body.appendChild(f);if({remove})f.remove();</script>"
        );
        let mut routes = vec![("/", html("", &source))];
        if let Some(child) = child {
            routes.push(("/frame", html("", child)));
        }
        let (fixture, counts) = Fixture::routes_counted(routes);
        let mut runtime = support::desktop(&fixture.url);
        let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        assert!(texts(&loaded).contains("usable"));
        assert_eq!(
            texts(&loaded).contains("ran"),
            expected,
            "{sandbox} remove={remove}"
        );
        if remove {
            assert_eq!(counts.lock().unwrap().get("/frame"), None);
        }
        assert_eq!(
            peek(
                &mut runtime,
                "document.getElementById('out').textContent.includes('usable')"
            ),
            true
        );
    }
}

/** Completing a refused owner must not retry it when a later sibling shares its URL. */
#[test]
fn refused_scripted_owner_is_attempted_once() {
    let source = "<p id=out>pending</p><script>let loads=0;function add(){const f=document.createElement('iframe');f.setAttribute('sandbox','allow-scripts allow-same-origin');f.src='/frame';f.addEventListener('load',()=>{loads++;document.getElementById('out').textContent='loads '+loads;if(loads===1)add()});document.body.appendChild(f)}add()</script>";
    let (fixture, counts) = Fixture::routes_counted(vec![
        ("/", html("", source)),
        (
            "/frame",
            html(
                "X-Frame-Options: DENY\r\nCache-Control: no-store\r\n",
                "<p>refused</p>",
            ),
        ),
    ]);
    let mut runtime = support::desktop(&fixture.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&loaded).contains("loads 2"), "{}", texts(&loaded));
    assert_eq!(counts.lock().unwrap().get("/frame").copied(), Some(2));
}

/** Blank sources retain writable local documents instead of acquiring a scripted route. */
#[test]
fn blank_sandboxed_frames_keep_their_documents() {
    for source in ["", "src=''", "src='  '", "src='about:blank'"] {
        let body =
            format!("<iframe id=f sandbox='allow-scripts allow-same-origin' {source}></iframe>");
        let fixture = Fixture::routes(vec![("/", html("", &body))]);
        let mut runtime = support::desktop(&fixture.url);
        agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
        assert_eq!(
            peek(
                &mut runtime,
                "document.getElementById('f').contentDocument !== null && document.getElementById('f').contentWindow !== null"
            ),
            true,
            "{source}"
        );
    }
}

/** Saturation refuses excess children while parent and admitted children retain working state. */
#[test]
fn inserted_frames_keep_the_existing_admission_limit() {
    let source = "<p id=out>0</p><script>let messages=0;addEventListener('message',e=>{if(e.data==='ready'||e.data===1)e.source.postMessage('ping','*');if(e.data===2)document.getElementById('out').textContent=String(++messages)});for(let i=0;i<17;i++){const f=document.createElement('iframe');f.setAttribute('sandbox','allow-scripts allow-same-origin');f.src='/frame';document.body.appendChild(f)}</script>";
    let (fixture, counts) = Fixture::routes_counted(vec![
        ("/", html("", source)),
        (
            "/frame",
            html(
                "",
                "<script>let count=0;parent.postMessage('ready','*');addEventListener('message',()=>parent.postMessage(++count,'*'))</script>",
            ),
        ),
    ]);
    let mut runtime = support::desktop(&fixture.url);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert_eq!(counts.lock().unwrap().get("/frame").copied(), Some(6));
    assert!(texts(&loaded).contains('6'), "{}", texts(&loaded));
    assert_eq!(peek(&mut runtime, "messages"), 6);
}
