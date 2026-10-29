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
