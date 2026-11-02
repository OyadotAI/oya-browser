/** Window messaging between a page and its frames: a message to the page's own window arrives
from a task, a frame's `parent` is a proxy whose postMessage leaves as an effect, and a message
the app hands back fires `message` with the data, origin and source proxy. */
use crate::common;

use common::{failure, global, host, input, load_with, round_trip};
use mars_v9::{MESSAGE_PARENT, MessageData, PageEnvironment, PageEvent};

/** The environment of a page running in a frame of https://embedder.test. */
fn framed() -> PageEnvironment {
    PageEnvironment {
        embedder_origin: Some("https://embedder.test".into()),
        window_name: "I0_1".into(),
        ..common::environment()
    }
}

/** A message to the page's own window is delivered from a task, never synchronously. */
#[test]
fn a_message_to_the_own_window_arrives_from_a_task() {
    let environment = common::environment();
    let source = "let out='';addEventListener('message',e=>{out+='got:'+e.data+':'+e.origin+':'+(e.source===window)});postMessage('x','*');out+='sync;';";
    let loaded = load_with(source, &environment);
    assert_eq!(failure(&loaded), None);
    assert_eq!(global(&loaded, "out"), "sync;");
    let sources = vec![source.to_string()];
    let host = host();
    let after = mars_v9::run_timers(
        input(&sources, &host, &environment),
        round_trip(loaded.state),
        10.0,
        16,
    )
    .expect("timers");
    assert_eq!(global(&after, "out"), "sync;got:x:https://example.com:true");
}

/** In a frame, `parent` is a proxy and posting to it leaves the realm as an effect. */
#[test]
fn a_frame_posts_to_its_parent_through_an_effect() {
    let loaded = load_with(
        "let out=[parent!==window,top===parent,parent.closed,window.name].join('|');parent.postMessage({ready:1},'https://embedder.test');parent.postMessage('hi','*');",
        &framed(),
    );
    assert_eq!(failure(&loaded), None);
    assert_eq!(global(&loaded, "out"), "true|true|false|I0_1");
    let messages = &loaded.effects.messages;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].target, MESSAGE_PARENT);
    assert_eq!(messages[0].data, MessageData::Json("{\"ready\":1}".into()));
    assert_eq!(messages[0].target_origin, "https://embedder.test");
    assert_eq!(messages[1].data, MessageData::Text("hi".into()));
    assert_eq!(messages[1].target_origin, "*");
}

/** A top-level page is its own parent and posts nothing to it. */
#[test]
fn a_top_level_page_is_its_own_parent() {
    let loaded = load_with(
        "let out=String(parent===window&&top===window);",
        &common::environment(),
    );
    assert_eq!(global(&loaded, "out"), "true");
    assert!(loaded.effects.messages.is_empty());
}

/** A message from the embedding page fires `message` with parsed data and `parent` as source. */
#[test]
fn a_message_from_the_parent_fires_with_its_proxy_as_source() {
    let environment = framed();
    let source = "let out='';addEventListener('message',e=>{out=[e.data.n,e.origin,e.source===parent].join('|')});";
    let loaded = load_with(source, &environment);
    let sources = vec![source.to_string()];
    let host = host();
    let after = mars_v9::page_event(
        input(&sources, &host, &environment),
        round_trip(loaded.state),
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Json("{\"n\":7}".into()),
            origin: "https://embedder.test".into(),
            source: Some(MESSAGE_PARENT),
            ports: Vec::new(),
        },
    )
    .expect("message");
    assert_eq!(failure(&after), None);
    assert_eq!(global(&after, "out"), "7|https://embedder.test|true");
}

/** An iframe that loads its own document gives a proxy as contentWindow, posting to that frame. */
#[test]
fn an_iframe_content_window_posts_to_its_frame() {
    let loaded = load_with(
        "let out='';const f=document.createElement('iframe');f.src='https://other.test/w';document.getElementById('main').appendChild(f);out=String(f.contentWindow===f.contentWindow&&f.contentDocument===null);f.contentWindow.postMessage('ping','https://other.test');",
        &common::environment(),
    );
    assert_eq!(failure(&loaded), None);
    assert_eq!(global(&loaded, "out"), "true");
    let message = &loaded.effects.messages[0];
    assert!(message.target >= 0);
    assert_eq!(message.data, MessageData::Text("ping".into()));
    assert_eq!(message.target_origin, "https://other.test");
}
