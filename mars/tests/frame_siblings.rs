/** Cross-origin siblings discover one another and exchange messages through native frame contexts. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, html};

/** Each fixture uses its own explicit origin grant and never touches a user profile. */
fn runtime(origins: &[&str]) -> mars::runtime::Runtime {
    use mars::runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner},
    };
    Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        origins.iter().map(|origin| (*origin).into()).collect(),
        Authority::shared(Owner::Agent),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** Collect published visible text, so successful script execution must reach the real document. */
fn text(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** Separate origins prevent an accidental same-realm implementation from passing. */
fn siblings(first: &str, second: &str) -> (Fixture, Fixture, Fixture) {
    let first = Fixture::routes(vec![("/", html("", first))]);
    let second = Fixture::routes(vec![("/", html("", second))]);
    let parent = Fixture::routes(vec![(
        "/",
        html(
            "",
            &format!(
                "<p id=out>log</p><iframe name=alpha src='{}/'></iframe><iframe name=beta src='{}/'></iframe><script>addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data}})</script>",
                first.url, second.url
            ),
        ),
    )]);
    (parent, first, second)
}

#[test]
fn cross_origin_siblings_resolve_identity_and_reply_directly() {
    let (parent, first, second) = siblings(
        "<script>const peer=parent.frames.beta;parent.postMessage('lookup:'+(peer===parent.frames[1])+':'+(parent.frames[0]===window)+':'+parent.frames.length,'*');addEventListener('message',e=>parent.postMessage('reply:'+e.data+':'+(e.source===peer)+':'+(e.origin!==location.origin),'*'));peer.postMessage('ping','*')</script>",
        "<script>addEventListener('message',e=>{parent.postMessage('received:'+e.data+':'+(e.source===parent.frames.alpha),'*');e.source.postMessage('pong','*')})</script>",
    );
    let mut runtime = runtime(&[&parent.url, &first.url, &second.url]);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("lookup:true:true:2"), "{result}");
    assert!(result.contains("received:ping:true"), "{result}");
    assert!(result.contains("reply:pong:true:true"), "{result}");
}

#[test]
fn sibling_transfer_creates_a_bidirectional_message_port() {
    let (parent, first, second) = siblings(
        "<script>const c=new MessageChannel();c.port1.onmessage=e=>{parent.postMessage('port:'+e.data,'*');if(e.data==='ready')c.port1.postMessage('ping')};parent.frames.beta.postMessage('port','*',[c.port2])</script>",
        "<script>addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>p.postMessage('echo:'+m.data);p.postMessage('ready')})</script>",
    );
    let mut runtime = runtime(&[&parent.url, &first.url, &second.url]);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("port:ready"), "{result}");
    assert!(result.contains("port:echo:ping"), "{result}");
}

/** Ports posted in the same callback as their transfer wait until the destination adopts them. */
#[test]
fn callback_transfer_preserves_immediately_posted_port_messages() {
    let (parent, first, second) = siblings(
        "<script>addEventListener('message',()=>{const c=new MessageChannel();parent.frames.beta.postMessage('port','*',[c.port2]);c.port1.postMessage('early')});parent.frames.beta.postMessage('start','*')</script>",
        "<script>addEventListener('message',e=>{if(e.data==='start'){e.source.postMessage('go','*')}else{e.ports[0].onmessage=m=>parent.postMessage('delivered:'+m.data,'*')}})</script>",
    );
    let mut runtime = runtime(&[&parent.url, &first.url, &second.url]);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":parent.url})).unwrap();
    assert!(text(&page).contains("delivered:early"), "{}", text(&page));
}

#[test]
fn sibling_messages_for_the_wrong_origin_are_not_delivered() {
    let (parent, first, second) = siblings(
        "<script>parent.frames.beta.postMessage('secret','https://elsewhere.invalid');parent.postMessage('sent','*')</script>",
        "<script>addEventListener('message',e=>parent.postMessage('leaked','*'))</script>",
    );
    let mut runtime = runtime(&[&parent.url, &first.url, &second.url]);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("sent"), "{result}");
    assert!(!result.contains("leaked"), "{result}");
}

/** Refused destinations must not retain a detached channel peer in the sender's bounded VM map. */
#[test]
fn refused_transfers_release_peer_roots_across_native_turns() {
    for admitted in [true, false] {
        let response = if admitted {
            html(
                "",
                "<script>addEventListener('message',()=>parent.postMessage('leaked','*'))</script>",
            )
        } else {
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        };
        let child = Fixture::raw(response);
        let origin = if admitted {
            "https://wrong.invalid"
        } else {
            "*"
        };
        let parent = Fixture::raw(html(
            "",
            &format!(
                "<p id=out>sent:0</p><button id=go>Go</button><iframe src='{}/'></iframe><script>let sent=0;const retained=[];const out=document.getElementById('out');addEventListener('message',()=>out.textContent='leaked');document.getElementById('go').addEventListener('click',()=>{{for(let i=0;i<10;i++){{const c=new MessageChannel();retained.push(c.port1);document.querySelector('iframe').contentWindow.postMessage('discard','{origin}',[c.port2]);sent++}}out.textContent='sent:'+sent}})</script>",
                child.url
            ),
        ));
        let mut browser = runtime(&[&parent.url, &child.url]);
        agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
        let mut page = Value::Null;
        for _ in 0..10 {
            page = click(&mut browser, "go");
        }
        assert!(
            text(&page).contains("sent:100"),
            "admitted={admitted}: {}",
            text(&page)
        );
        assert!(!text(&page).contains("leaked"));
    }
}

/** Later callbacks removing a destination must not erase earlier transfers before peer retirement. */
#[test]
fn removal_after_a_callback_transfer_releases_its_pending_peers() {
    let child = Fixture::raw(html(
        "",
        "<script>parent.postMessage('transfer','*');parent.postMessage('remove','*')</script>",
    ));
    let parent = Fixture::raw(html(
        "",
        &format!(
            "<p id=out>sent:0</p><button id=go>Go</button><iframe src='{0}/'></iframe><script>let sent=0;const retained=[];addEventListener('message',e=>{{const f=document.querySelector('iframe');if(e.data==='transfer'){{for(let i=0;i<8;i++){{const c=new MessageChannel();retained.push(c.port1);f.contentWindow.postMessage('discard','*',[c.port2]);sent++}}document.getElementById('out').textContent='sent:'+sent}}else if(f)f.remove()}});document.getElementById('go').addEventListener('click',()=>{{const f=document.createElement('iframe');f.src='{0}/';document.body.append(f)}})</script>",
            child.url
        ),
    ));
    let mut browser = runtime(&[&parent.url, &child.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let mut page = Value::Null;
    for _ in 0..9 {
        page = click(&mut browser, "go");
    }
    assert!(text(&page).contains("sent:80"), "{}", text(&page));
}

/** Native input executes the same transaction and lifecycle as a person's click. */
fn click(browser: &mut mars::runtime::Runtime, id: &str) -> Value {
    let page = agent_call(browser, "page.inspect", json!({})).unwrap();
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["html_id"] == id)
        .unwrap();
    agent_call(browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":node["node_id"]})).unwrap();
    agent_call(browser, "page.inspect", json!({})).unwrap()
}

#[test]
fn source_navigation_keeps_window_identity_and_retires_the_old_document() {
    let child = Fixture::routes(vec![
        (
            "/old",
            html(
                "",
                "<script>parent.postMessage('old-ready','*');addEventListener('message',e=>parent.postMessage('old:'+e.data,'*'))</script>",
            ),
        ),
        (
            "/new",
            html(
                "",
                "<script>parent.postMessage('new-ready','*');addEventListener('message',e=>parent.postMessage('new:'+e.data,'*'))</script>",
            ),
        ),
    ]);
    let source = format!(
        "<p id=out>log</p><button id=go>Go</button><iframe src='{0}/old'></iframe><script>const f=document.querySelector('iframe');const saved=f.contentWindow;addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data;if(e.data==='new-ready'){{document.getElementById('out').textContent+=' stable:'+(saved===f.contentWindow);saved.postMessage('ping','*')}}}});document.getElementById('go').addEventListener('click',()=>{{f.src='{0}/new'}});</script>",
        child.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &child.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&click(&mut browser, "go"));
    assert!(result.contains("new-ready"), "{result}");
    assert!(result.contains("stable:true"), "{result}");
    assert!(result.contains("new:ping"), "{result}");
    assert!(!result.contains("old:ping"), "{result}");
}

#[test]
fn detaching_and_reinserting_the_same_element_retires_retained_proxies() {
    let child = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>parent.postMessage('ready','*');addEventListener('message',e=>parent.postMessage('received:'+e.data,'*'))</script>",
        ),
    )]);
    let source = format!(
        "<p id=out>log</p><button id=go>Go</button><iframe src='{}/'></iframe><script>const f=document.querySelector('iframe');const old=f.contentWindow;let turns=0;addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data;if(e.data==='ready'&&++turns===2){{old.postMessage('stale','*');f.contentWindow.postMessage('fresh','*')}}}});document.getElementById('go').addEventListener('click',()=>{{f.remove();document.body.appendChild(f)}});</script>",
        child.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &child.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&click(&mut browser, "go"));
    assert!(result.contains("received:fresh"), "{result}");
    assert!(!result.contains("received:stale"), "{result}");
}

#[test]
fn hidden_iframes_execute_and_remain_in_the_parent_directory() {
    let child = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>parent.postMessage('hidden:'+parent.frames.length+':'+(parent.frames.hiddenFrame===window),'*')</script>",
        ),
    )]);
    let source = format!(
        "<p id=out>log</p><iframe hidden name=hiddenFrame src='{}/'></iframe><script>addEventListener('message',e=>document.getElementById('out').textContent+=' '+e.data)</script>",
        child.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &child.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    assert!(text(&page).contains("hidden:1:true"), "{}", text(&page));
}

#[test]
fn queued_sibling_mail_cannot_enter_a_replacement_document() {
    let sender = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>parent.postMessage('navigate','*');parent.frames.beta.postMessage('stale','*')</script>",
        ),
    )]);
    let recipient = Fixture::routes(vec![
        (
            "/old",
            html(
                "",
                "<script>addEventListener('message',e=>parent.postMessage('old:'+e.data,'*'))</script>",
            ),
        ),
        (
            "/new",
            html(
                "",
                "<script>parent.postMessage('new-ready','*');addEventListener('message',e=>parent.postMessage('new:'+e.data,'*'))</script>",
            ),
        ),
    ]);
    let source = format!(
        "<p id=out>log</p><iframe src='{}/'></iframe><iframe name=beta src='{1}/old'></iframe><script>addEventListener('message',e=>{{document.getElementById('out').textContent+=' '+e.data;if(e.data==='navigate')document.querySelectorAll('iframe')[1].src='{1}/new'}})</script>",
        sender.url, recipient.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &sender.url, &recipient.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("new-ready"), "{result}");
    assert!(!result.contains(":stale"), "{result}");
}

#[test]
fn a_received_port_can_move_to_a_sibling_without_retaining_the_middle_endpoint() {
    let first = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>const c=new MessageChannel();c.port1.onmessage=e=>{parent.postMessage(e.data,'*');if(e.data==='arrived')c.port1.postMessage('reply')};parent.frames.middle.postMessage('port','*',[c.port2])</script>",
        ),
    )]);
    let middle = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>addEventListener('message',e=>parent.frames.last.postMessage('forward','*',[e.ports[0]]))</script>",
        ),
    )]);
    let last = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>parent.postMessage('last:'+m.data,'*');p.postMessage('arrived')})</script>",
        ),
    )]);
    let source = format!(
        "<p id=out>log</p><iframe src='{}/'></iframe><iframe name=middle src='{}/'></iframe><iframe name=last src='{}/'></iframe><script>addEventListener('message',e=>document.getElementById('out').textContent+=' '+e.data)</script>",
        first.url, middle.url, last.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &first.url, &middle.url, &last.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("arrived"), "{result}");
    assert!(result.contains("last:reply"), "{result}");
}
