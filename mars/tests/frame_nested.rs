/** Nested executable frames share native identities, routing and ancestor admission fences. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, html};

/** One fixture origin is enough when each child explicitly allows scripts and its origin. */
fn runtime(origins: Vec<String>) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins,
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Public document text records cross-context acknowledgements without inspecting private state. */
fn texts(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** Grandchildren distinguish their immediate parent from top and preserve event.source identities. */
#[test]
fn grandchild_parent_top_and_sources_follow_the_real_tree() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                r#"<p id=out>root</p><iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe><script>
        const out=document.getElementById('out');addEventListener('message',e=>{out.textContent+=' '+e.data;if(String(e.data).startsWith('grand:')){out.textContent+=' source:'+(e.source===frames[0].frames[0]);e.source.postMessage('reply','*')}});
        </script>"#,
            ),
        ),
        (
            "/child",
            html(
                "",
                r#"<p>child-paint</p><iframe sandbox='allow-scripts allow-same-origin' src='/grand'></iframe><script>
        addEventListener('message',e=>{if(e.data==='parent-check')top.postMessage('parent-source:'+(e.source===frames[0])+':'+(parent===top),'*')});
        </script>"#,
            ),
        ),
        (
            "/grand",
            html(
                "",
                r#"<p>grand-paint</p><script>
        top.postMessage('grand:'+(parent!==top)+':'+(parent.parent===top)+':'+(top.frames[0]===parent)+':'+(parent.frames[0]===window),'*');
        parent.postMessage('parent-check','*');addEventListener('message',e=>{if(e.data==='reply')top.postMessage('reply-source:'+(e.source===top),'*')});
        </script>"#,
            ),
        ),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let text = texts(&loaded);
    for expected in [
        "grand:true:true:true:true",
        "source:true",
        "parent-source:true:true",
        "reply-source:true",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(
        loaded["frame_nodes"].to_string().contains("grand-paint"),
        "{loaded}"
    );
}

/** A transferred grandchild port remains entangled across two ancestor boundaries. */
#[test]
fn grandchild_port_roundtrip_reaches_top_without_parent_aliasing() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                r#"<p id=out>root</p><iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe><script>
        const out=document.getElementById('out');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>out.textContent+=' '+m.data;p.postMessage('ping')});
        </script>"#,
            ),
        ),
        (
            "/child",
            html(
                "",
                "<iframe sandbox='allow-scripts allow-same-origin' src='/grand'></iframe>",
            ),
        ),
        (
            "/grand",
            html(
                "",
                r#"<script>const c=new MessageChannel();c.port1.onmessage=e=>c.port1.postMessage('nested-pong:'+e.data);top.postMessage('port','*',[c.port2]);</script>"#,
            ),
        ),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&loaded).contains("nested-pong:ping"), "{loaded}");
}

/** Shared depth admission stops great-grandchild execution instead of recursively resetting a quota. */
#[test]
fn great_grandchildren_are_not_acquired_or_executed() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<p id=out>root</p><iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe><script>const out=document.getElementById('out');addEventListener('message',e=>out.textContent+=' '+e.data)</script>",
            ),
        ),
        (
            "/child",
            html(
                "",
                "<iframe sandbox='allow-scripts allow-same-origin' src='/grand'></iframe>",
            ),
        ),
        (
            "/grand",
            html(
                "",
                "<iframe sandbox='allow-scripts allow-same-origin' src='/great'></iframe><script>top.postMessage('grand-ready','*')</script>",
            ),
        ),
        (
            "/great",
            html("", "<script>top.postMessage('escaped-depth','*')</script>"),
        ),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&loaded).contains("grand-ready"), "{loaded}");
    assert!(!texts(&loaded).contains("escaped-depth"), "{loaded}");
    assert_eq!(
        counts.lock().unwrap().get("/great").copied().unwrap_or(0),
        0
    );
}

/** An allowed immediate parent cannot hide a disallowed root ancestor from the final response policy. */
#[test]
fn grandchild_policy_checks_the_root_ancestor_too() {
    let child = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<iframe sandbox='allow-scripts allow-same-origin' src='/grand'></iframe><script>top.postMessage('child-ready','*')</script>",
            ),
        ),
        (
            "/grand",
            html(
                "Content-Security-Policy: frame-ancestors 'self'\r\n",
                "<script>top.postMessage('forbidden-grand','*')</script>",
            ),
        ),
    ]);
    let root = Fixture::routes(vec![(
        "/",
        html(
            "",
            &format!(
                "<p id=out>root</p><iframe src='{}/'></iframe><script>const out=document.getElementById('out');addEventListener('message',e=>out.textContent+=' '+e.data)</script>",
                child.url
            ),
        ),
    )]);
    let mut runtime = runtime(vec![root.url.clone(), child.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":root.url})).unwrap();
    assert!(texts(&loaded).contains("child-ready"), "{loaded}");
    assert!(!texts(&loaded).contains("forbidden-grand"), "{loaded}");
}

/** Grandchild globals use the iframe content box rather than the root window dimensions. */
#[test]
fn grandchild_executes_with_its_local_viewport() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<p id=out>root</p><iframe style='width:240px;height:180px' sandbox='allow-scripts allow-same-origin' src='/child'></iframe><script>const out=document.getElementById('out');addEventListener('message',e=>out.textContent+=' '+e.data)</script>",
            ),
        ),
        (
            "/child",
            html(
                "",
                "<iframe style='width:123px;height:75px;border:0;padding:0' sandbox='allow-scripts allow-same-origin' src='/grand'></iframe>",
            ),
        ),
        (
            "/grand",
            html(
                "",
                "<script>top.postMessage('viewport:'+innerWidth+'x'+innerHeight,'*')</script>",
            ),
        ),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&loaded).contains("viewport:123x75"), "{loaded}");
}

/** Retiring an ancestor destroys its descendants before queued port traffic can reach them. */
#[test]
fn removing_parent_retires_grandchild_port_receiver() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                r#"<p id=out>root</p><iframe id=child sandbox='allow-scripts allow-same-origin' src='/child'></iframe><script>
        const out=document.getElementById('out');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>out.textContent+=' '+m.data;document.getElementById('child').remove();p.postMessage('retired');out.textContent+=' removed'});
        </script>"#,
            ),
        ),
        (
            "/child",
            html(
                "",
                "<iframe sandbox='allow-scripts allow-same-origin' src='/grand'></iframe>",
            ),
        ),
        (
            "/grand",
            html(
                "",
                "<script>const c=new MessageChannel();c.port1.onmessage=e=>c.port1.postMessage('stale-receiver-ran');top.postMessage('port','*',[c.port2]);</script>",
            ),
        ),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    assert!(texts(&loaded).contains("removed"), "{loaded}");
    assert!(!texts(&loaded).contains("stale-receiver-ran"), "{loaded}");
    assert!(
        loaded["frame_nodes"].as_array().is_some_and(Vec::is_empty),
        "{loaded}"
    );
}

/** Identical local owner numbers in two branches must never select the other branch's receiver. */
#[test]
fn repeated_local_owner_numbers_remain_separate_global_contexts() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                r#"<p id=out>root</p><iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe><iframe sandbox='allow-scripts allow-same-origin' src='/child'></iframe><script>
        const out=document.getElementById('out');addEventListener('message',e=>{out.textContent+=' '+(e.source===frames[0].frames[0]?'first':e.source===frames[1].frames[0]?'second':'unknown')});
        </script>"#,
            ),
        ),
        (
            "/child",
            html(
                "",
                "<iframe sandbox='allow-scripts allow-same-origin' src='/grand'></iframe>",
            ),
        ),
        (
            "/grand",
            html("", "<script>top.postMessage('ready','*')</script>"),
        ),
    ]);
    let mut runtime = runtime(vec![fixture.url.clone()]);
    let loaded = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let text = texts(&loaded);
    assert!(
        text.contains("first") && text.contains("second") && !text.contains("unknown"),
        "{loaded}"
    );
}
