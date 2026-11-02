/** Real native frame routing retires closed channel endpoints and frees their bounded capacity. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, agent_call, html};

/** Native services use only explicitly granted synthetic loopback origins. */
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

/** Published document text is the public evidence that callbacks completed. */
fn text(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** The parent owns the retained endpoint while an isolated child closes each transferred peer. */
fn fixtures(parent_script: &str, child_script: &str) -> (Fixture, Fixture) {
    let child = Fixture::routes(vec![(
        "/",
        html("", &format!("<script>{child_script}</script>")),
    )]);
    let parent = Fixture::routes(vec![(
        "/",
        html(
            "",
            &format!(
                "<p id=out>pending</p><button id=go>Go</button><iframe src='{}/'></iframe><script>document.getElementById('go').addEventListener('click',()=>{{}});{parent_script}</script>",
                child.url
            ),
        ),
    )]);
    (parent, child)
}

#[test]
fn close_blocks_later_sends_and_queued_messages_in_both_directions() {
    let (parent, child) = fixtures(
        "const c=new MessageChannel();const f=document.querySelector('iframe');c.port1.onmessage=e=>document.getElementById('out').textContent+=' forbidden';addEventListener('message',e=>{if(e.data==='ready')f.contentWindow.postMessage('port','*',[c.port2]);if(e.data==='closed'){c.port1.postMessage('forbidden');document.getElementById('out').textContent='closed'}});",
        "parent.postMessage('ready','*');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>parent.postMessage('forbidden','*');p.close();p.start();p.postMessage('forbidden');parent.postMessage('closed','*')});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("closed"), "{result}");
    assert!(!result.contains("forbidden"), "{result}");
}

#[test]
fn repeated_peer_close_releases_vm_links_and_native_routes_across_turns() {
    let (parent, child) = fixtures(
        "let count=0;let sent=0;const retained=[];const f=document.querySelector('iframe');function batch(){for(let i=0;i<30;i++){const c=new MessageChannel();retained.push(c.port1);f.contentWindow.postMessage('port','*',[c.port2]);sent++}}addEventListener('message',e=>{if(e.data==='ready')batch();else if(e.data==='closed'){count++;document.getElementById('out').textContent='closed:'+count;if(count===sent&&count<300)batch()}});",
        "parent.postMessage('ready','*');addEventListener('message',e=>{e.ports[0].close();parent.postMessage('closed','*')});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    // An ordinary native event settles the next bounded work slice without bypassing routing.
    let mut result = String::new();
    for _ in 0..20 {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        result = text(&page);
        if result.contains("closed:300") {
            break;
        }
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    assert!(result.contains("closed:300"), "{result}");
}

#[test]
fn moving_an_endpoint_after_peer_close_cannot_revive_its_old_route() {
    let (parent, child) = fixtures(
        "let c;let count=0;const f=document.querySelector('iframe');function next(){c=new MessageChannel();f.contentWindow.postMessage('close','*',[c.port2])}addEventListener('message',e=>{if(e.data==='ready')next();else if(e.data==='closed')f.contentWindow.postMessage('orphan','*',[c.port1]);else if(e.data==='orphan'){count++;document.getElementById('out').textContent='orphan:'+count;if(count<80)next()}});",
        "const retained=[];parent.postMessage('ready','*');addEventListener('message',e=>{if(e.data==='close'){e.ports[0].close();parent.postMessage('closed','*')}else{retained.push(e.ports[0]);e.ports[0].postMessage('revived');parent.postMessage('orphan','*')}});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let mut result = String::new();
    for _ in 0..50 {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        result = text(&page);
        if result.contains("orphan:80") {
            break;
        }
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    assert!(result.contains("orphan:80"), "{result}");
    assert!(!result.contains("revived"), "{result}");
}

#[test]
fn returned_endpoint_exchanges_with_its_original_peer_instead_of_itself() {
    let (parent, child) = fixtures(
        "const c=new MessageChannel();const f=document.querySelector('iframe');const out=document.getElementById('out');out.textContent='';c.port1.onmessage=e=>out.textContent+='original:'+e.data+';';addEventListener('message',e=>{if(e.data==='ready')f.contentWindow.postMessage('port','*',[c.port2]);else{const p=e.ports[0];p.onmessage=m=>out.textContent+='returned:'+m.data+';';p.postMessage('from-returned');c.port1.postMessage('from-original')}});",
        "parent.postMessage('ready','*');addEventListener('message',e=>parent.postMessage('return','*',[e.ports[0]]));",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let mut result = String::new();
    for _ in 0..5 {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        result = text(&page);
        if result.contains("original:from-returned;returned:from-original;") {
            break;
        }
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    assert!(
        result.contains("original:from-returned;returned:from-original;"),
        "{result}"
    );
}

#[test]
fn send_before_forward_refuses_without_detaching_the_endpoint_or_losing_its_message() {
    let (parent, child) = fixtures(
        "const c=new MessageChannel();const f=document.querySelector('iframe');const out=document.getElementById('out');out.textContent='';c.port1.onmessage=e=>{out.textContent+='original:'+e.data+';';if(e.data==='before-forward')c.port1.postMessage('still-linked')};addEventListener('message',e=>{if(e.data==='ready')f.contentWindow.postMessage('port','*',[c.port2]);else out.textContent+=e.data+';'});",
        "parent.postMessage('ready','*');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>parent.postMessage('received:'+m.data,'*');p.postMessage('before-forward');try{parent.postMessage('return','*',[p])}catch(error){parent.postMessage(error.name,'*')}});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    for expected in [
        "NotSupportedError",
        "original:before-forward;",
        "received:still-linked;",
    ] {
        assert!(result.contains(expected), "missing {expected}: {result}");
    }
}

#[test]
fn concurrent_original_peer_move_refuses_and_the_returned_channel_remains_usable() {
    let returning = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>parent.postMessage('ready','*');addEventListener('message',e=>{parent.postMessage('move','*');parent.postMessage('return','*',[e.ports[0]])});</script>",
        ),
    )]);
    let last = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>addEventListener('message',e=>parent.postMessage('leaked','*'));</script>",
        ),
    )]);
    let source = format!(
        "<p id=out>log</p><button id=go>Go</button><iframe name=returning src='{}/'></iframe><iframe name=last src='{}/'></iframe><script>document.getElementById('go').addEventListener('click',()=>{{}});const c=new MessageChannel();const out=document.getElementById('out');c.port1.onmessage=e=>out.textContent+=' original:'+e.data;addEventListener('message',e=>{{if(e.data==='ready')frames.returning.postMessage('port','*',[c.port2]);else if(e.data==='move'){{try{{frames.last.postMessage('port','*',[c.port1])}}catch(error){{out.textContent+=' '+error.name}}}}else if(e.data==='return'){{const p=e.ports[0];p.onmessage=m=>out.textContent+=' returned:'+m.data;p.postMessage('from-returned');c.port1.postMessage('from-original')}}else out.textContent+=' '+e.data}});</script>",
        returning.url, last.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &returning.url, &last.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let mut result = String::new();
    for _ in 0..6 {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        result = text(&page);
        if result.contains("returned:from-original") && result.contains("original:from-returned") {
            break;
        }
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    for expected in [
        "NotSupportedError",
        "returned:from-original",
        "original:from-returned",
    ] {
        assert!(result.contains(expected), "{result}");
    }
    assert!(!result.contains("leaked"), "{result}");
}

#[test]
fn forwarding_in_a_later_vm_callback_preserves_already_emitted_port_messages() {
    let middle = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>let p;parent.postMessage('ready','*');addEventListener('message',e=>{if(e.data==='port'){p=e.ports[0];parent.postMessage('request-send','*');parent.postMessage('request-forward','*')}else if(e.data==='send'){p.postMessage('before-forward')}else if(e.data==='forward'){parent.frames.last.postMessage('forward','*',[p])}});</script>",
        ),
    )]);
    let last = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>parent.postMessage('last:'+m.data,'*');p.postMessage('arrived')});</script>",
        ),
    )]);
    let source = format!(
        "<p id=out>log</p><button id=go>Go</button><iframe name=middle src='{}/'></iframe><iframe name=last src='{}/'></iframe><script>document.getElementById('go').addEventListener('click',()=>{{}});const c=new MessageChannel();const out=document.getElementById('out');c.port1.onmessage=e=>{{out.textContent+=' '+e.data;if(e.data==='arrived')c.port1.postMessage('reply')}};addEventListener('message',e=>{{if(e.data==='ready')frames.middle.postMessage('port','*',[c.port2]);else if(e.data==='request-send')frames.middle.postMessage('send','*');else if(e.data==='request-forward')frames.middle.postMessage('forward','*');else out.textContent+=' '+e.data}});</script>",
        middle.url, last.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &middle.url, &last.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let mut result = String::new();
    for _ in 0..6 {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        result = text(&page);
        if result.contains("last:reply") {
            break;
        }
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    for expected in ["before-forward", "arrived", "last:reply"] {
        assert!(result.contains(expected), "missing {expected}: {result}");
    }
}

#[test]
fn external_sender_close_preserves_accepted_messages_but_not_later_posts() {
    let (parent, child) = fixtures(
        "const c=new MessageChannel();const out=document.getElementById('out');out.textContent='';c.port1.onmessage=e=>out.textContent+=e.data+';';addEventListener('message',e=>{if(e.data==='ready')frames[0].postMessage('port','*',[c.port2])});",
        "parent.postMessage('ready','*');addEventListener('message',e=>{const p=e.ports[0];p.postMessage('accepted-one');p.postMessage('accepted-two');p.close();p.postMessage('late')});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("accepted-one;accepted-two;"), "{result}");
    assert!(!result.contains("late"), "{result}");
}

#[test]
fn transfer_send_and_close_before_first_adoption_preserves_the_receiver_queue() {
    let (parent, child) = fixtures(
        "const c=new MessageChannel();const out=document.getElementById('out');out.textContent='';c.port1.onmessage=e=>out.textContent+='forbidden:'+e.data;addEventListener('message',e=>{if(e.data==='ready'){frames[0].postMessage('port','*',[c.port2]);c.port1.postMessage('first');c.port1.postMessage('second');c.port1.close()}else out.textContent+=e.data+';'});",
        "parent.postMessage('ready','*');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>parent.postMessage('received:'+m.data,'*');p.postMessage('closed-receiver')});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(
        result.contains("received:first;received:second;"),
        "{result}"
    );
    assert!(!result.contains("forbidden"), "{result}");
}

#[test]
fn receiver_close_discards_its_later_incoming_messages_but_preserves_its_reply() {
    let (parent, child) = fixtures(
        "const c=new MessageChannel();const out=document.getElementById('out');out.textContent='';c.port1.onmessage=e=>out.textContent+='reply:'+e.data+';';addEventListener('message',e=>{if(e.data==='ready'){frames[0].postMessage('port','*',[c.port2]);c.port1.postMessage('first');c.port1.postMessage('second')}else out.textContent+=e.data+';'});",
        "parent.postMessage('ready','*');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>{parent.postMessage('received:'+m.data,'*');p.postMessage('accepted');p.close()}});",
    );
    let mut browser = runtime(&[&parent.url, &child.url]);
    let page = agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let result = text(&page);
    assert!(result.contains("received:first"), "{result}");
    assert!(result.contains("reply:accepted"), "{result}");
    assert!(!result.contains("second"), "{result}");
}

/** Both queued data and final cleanup must follow the movable endpoint after its sender closes. */
fn close_and_forward(immediate: bool) -> String {
    let script = if immediate {
        "parent.postMessage('ready','*');addEventListener('message',e=>parent.frames.last.postMessage('forward','*',[e.ports[0]]));"
    } else {
        "parent.postMessage('ready','*');addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>{parent.postMessage('middle:'+m.data,'*');parent.frames.last.postMessage('forward','*',[p])}});"
    };
    let middle = Fixture::routes(vec![("/", html("", &format!("<script>{script}</script>")))]);
    let last = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>addEventListener('message',e=>{e.ports[0].onmessage=m=>parent.postMessage('last:'+m.data,'*')});</script>",
        ),
    )]);
    let source = format!(
        "<p id=out>log</p><button id=go>Go</button><iframe name=middle src='{}/'></iframe><iframe name=last src='{}/'></iframe><script>document.getElementById('go').addEventListener('click',()=>{{}});const c=new MessageChannel();const out=document.getElementById('out');addEventListener('message',e=>{{if(e.data==='ready'){{frames.middle.postMessage('port','*',[c.port2]);c.port1.postMessage('first');c.port1.postMessage('second');c.port1.close()}}else out.textContent+=' '+e.data}});</script>",
        middle.url, last.url
    );
    let parent = Fixture::routes(vec![("/", html("", &source))]);
    let mut browser = runtime(&[&parent.url, &middle.url, &last.url]);
    agent_call(&mut browser, "page.navigate", json!({"url":parent.url})).unwrap();
    let mut result = String::new();
    for _ in 0..6 {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        result = text(&page);
        if result.contains("last:second") {
            break;
        }
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    result
}

#[test]
fn accepted_messages_follow_a_port_forwarded_during_its_adoption_after_sender_close() {
    let result = close_and_forward(true);
    assert!(
        result.contains("last:first") && result.contains("last:second"),
        "{result}"
    );
}

#[test]
fn accepted_messages_follow_a_port_forwarded_by_its_first_message_callback_after_sender_close() {
    let result = close_and_forward(false);
    assert!(
        result.contains("middle:first") && result.contains("last:second"),
        "{result}"
    );
}
