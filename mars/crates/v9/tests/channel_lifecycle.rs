/** Port closure and route-domain refusals exercise public VM turns and serialized retained state. */
mod common;
use mars_v9::{Execution, MessageData, PageEnvironment, PageEvent};

/** External posting uses a real parent proxy while keeping the fixture independent of a browser. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        embedder_origin: Some("https://parent.test".into()),
        ..common::environment()
    }
}

/** A later native event reopens exactly the retained state a service would serialize. */
fn event(source: &str, loaded: Execution, event: PageEvent) -> Execution {
    let host = common::host();
    let environment = environment();
    let sources = vec![source.to_owned()];
    let result = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        event,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    result
}

#[test]
fn close_is_idempotent_and_removes_both_send_and_receive_authority() {
    let source = "let out='';const c=new MessageChannel();c.port1.onmessage=e=>out+=e.data;parent.postMessage('port','*',[c.port2]);c.port1.close();c.port1.close();c.port1.start();c.port1.postMessage('forbidden');";
    let loaded = common::load_with(source, &environment());
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let link = loaded.effects.messages[0].ports[0];
    assert_eq!(loaded.effects.port_closes, vec![link]);
    assert!(loaded.effects.port_messages.is_empty());
    let result = event(
        source,
        loaded,
        PageEvent::PortMessage {
            link,
            data: MessageData::Text("forbidden".into()),
        },
    );
    assert_eq!(common::global(&result, "out"), "");
}

#[test]
fn queued_local_delivery_does_not_fire_on_a_closed_port() {
    let source = "let out='';const c=new MessageChannel();c.port2.onmessage=e=>out+=e.data;c.port1.postMessage('forbidden');c.port2.close();c.port2.start();c.port1.postMessage('also-forbidden');";
    let loaded = common::load(source);
    let host = common::host();
    let environment = common::environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &environment),
        common::round_trip(loaded.state),
        10.0,
        16,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(common::global(&result, "out"), "");
}

#[test]
fn frame_to_worker_refusal_preserves_every_port_in_the_batch() {
    let source = "let out='';const c=new MessageChannel();const local=new MessageChannel();const w=new Worker('/worker.js');parent.postMessage('port','*',[c.port2]);try{w.postMessage('move',[local.port2,c.port1])}catch(e){out=e.name}local.port1.onmessage=e=>out+=e.data;local.port2.postMessage('local');c.port1.postMessage('still-frame');";
    let loaded = common::load_with(source, &environment());
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert_eq!(common::global(&loaded, "out"), "NotSupportedError");
    assert_eq!(loaded.effects.port_messages.len(), 1);
    assert_eq!(loaded.effects.worker_commands.len(), 1);
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(loaded.state),
        10.0,
        16,
    )
    .unwrap();
    assert_eq!(common::global(&result, "out"), "NotSupportedErrorlocal");
}

#[test]
fn worker_to_frame_refusal_keeps_the_original_worker_link() {
    let source = "let out='';const c=new MessageChannel();const w=new Worker('/worker.js');w.postMessage('port',[c.port2]);try{parent.postMessage('move','*',[c.port1])}catch(e){out=e.name}c.port1.postMessage('still-worker');";
    let loaded = common::load_with(source, &environment());
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert_eq!(common::global(&loaded, "out"), "NotSupportedError");
    assert!(loaded.effects.messages.is_empty());
    assert_eq!(loaded.effects.port_messages.len(), 1);
}

#[test]
fn received_frame_ports_keep_their_route_domain_across_serialization() {
    let source = "let out='';const w=new Worker('/worker.js');addEventListener('message',e=>{try{w.postMessage('move',[e.ports[0]])}catch(error){out=error.name}e.ports[0].postMessage('still-frame')});";
    let loaded = common::load_with(source, &environment());
    let result = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("port".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![7],
        },
    );
    assert_eq!(common::global(&result, "out"), "NotSupportedError");
    assert_eq!(result.effects.port_messages[0].link, 7);
}

#[test]
fn retransfer_marks_existing_links_without_changing_their_identity() {
    let source = "addEventListener('message',e=>parent.postMessage('return','*',[e.ports[0]]));";
    let loaded = common::load_with(source, &environment());
    let result = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("port".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![9],
        },
    );
    assert_eq!(result.effects.messages[0].ports, vec![9]);
    assert_eq!(result.effects.messages[0].moved_ports, vec![9]);
}

#[test]
fn worker_link_state_can_be_reopened_for_a_port_message() {
    let source = "const c=new MessageChannel();c.port1.onmessage=e=>c.port1.postMessage(e.data);postMessage('port',[c.port2]);";
    let host = common::host();
    let env = PageEnvironment {
        worker_name: Some("test".into()),
        ..common::environment()
    };
    let sources = vec![source.into()];
    let mut input = common::input(&sources, &host, &env);
    input.document.nodes.clear();
    let loaded = mars_v9::load_worker(input).unwrap();
    let link = match &loaded.effects.worker_commands[0] {
        mars_v9::WorkerCommand::Post { ports, .. } => ports[0],
        _ => panic!("post"),
    };
    let mut input = common::input(&sources, &host, &env);
    input.document.nodes.clear();
    let result = mars_v9::page_event(
        input,
        common::round_trip(loaded.state),
        PageEvent::PortMessage {
            link,
            data: MessageData::Text("echo".into()),
        },
    )
    .unwrap();
    assert_eq!(result.effects.port_messages[0].link, link);
}

#[test]
fn remote_close_releases_the_surviving_link_without_a_callback_or_echo() {
    let source = "let out='';const c=new MessageChannel();c.port1.onmessage=e=>out=e.data;parent.postMessage('port','*',[c.port2]);addEventListener('message',()=>c.port1.postMessage('forbidden'));";
    let loaded = common::load_with(source, &environment());
    let link = loaded.effects.messages[0].ports[0];
    let closed = event(source, loaded, PageEvent::PortClose { link });
    assert!(closed.effects.port_closes.is_empty());
    assert_eq!(common::global(&closed, "out"), "");
    let result = event(
        source,
        closed,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("send".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![],
        },
    );
    assert!(result.effects.port_messages.is_empty());
}

/** Locate a private slot in the documented inert snapshot representation, without private Rust APIs. */
fn replace_route(value: &mut serde_json::Value) -> bool {
    let key = serde_json::json!({"p":"messageRoute".encode_utf16().collect::<Vec<_>>()});
    match value {
        serde_json::Value::Array(entries) => {
            if entries.first() == Some(&key) {
                entries[1] = serde_json::to_value(mars_v9::JsValue::Number(-1.0)).unwrap();
                true
            } else {
                entries.iter_mut().any(replace_route)
            }
        }
        serde_json::Value::Object(object) => object.values_mut().any(replace_route),
        _ => false,
    }
}

#[test]
fn malformed_route_provenance_is_rejected_before_a_retained_turn_executes() {
    let source = "const c=new MessageChannel();parent.postMessage('port','*',[c.port2]);";
    let loaded = common::load_with(source, &environment());
    let mut encoded = serde_json::to_value(loaded.state).unwrap();
    assert!(replace_route(&mut encoded));
    let state = serde_json::from_value(encoded).unwrap();
    let host = common::host();
    let env = environment();
    let error = mars_v9::run_timers(common::input(&[source.into()], &host, &env), state, 0.0, 1)
        .err()
        .unwrap();
    assert_eq!(error.message, "Invalid MessagePort route state.");
}

#[test]
fn returning_a_transferred_endpoint_reentangles_with_its_original_local_peer() {
    let source = "let out='';const c=new MessageChannel();c.port1.onmessage=e=>out+='original:'+e.data+';';parent.postMessage('port','*',[c.port2]);addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>out+='returned:'+m.data+';';p.postMessage('from-returned');c.port1.postMessage('from-original')});";
    let loaded = common::load_with(source, &environment());
    let link = loaded.effects.messages[0].ports[0];
    let returned = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("return".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![link],
        },
    );
    assert!(returned.effects.port_messages.is_empty());
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(returned.state),
        10.0,
        16,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(
        common::global(&result, "out"),
        "original:from-returned;returned:from-original;"
    );
}

#[test]
fn reunited_queues_keep_both_directions_after_retained_state_round_trip() {
    let source = "let out='';const c=new MessageChannel();c.port1.onmessage=e=>out+='original:'+e.data+';';parent.postMessage('port','*',[c.port2]);addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>out+='returned:'+m.data+';';p.postMessage('local-returned');c.port1.postMessage('local-original')});";
    let loaded = common::load_with(source, &environment());
    let link = loaded.effects.messages[0].ports[0];
    let queued = vec![
        mars_v9::QueuedPortMessage {
            link,
            data: MessageData::Text("queued-original".into()),
            to_transferred: true,
        },
        mars_v9::QueuedPortMessage {
            link,
            data: MessageData::Text("queued-returning".into()),
            to_transferred: false,
        },
    ];
    let returned = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: queued,
            data: MessageData::Text("return".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![link],
        },
    );
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(returned.state),
        10.0,
        16,
    )
    .unwrap();
    assert_eq!(
        common::global(&result, "out"),
        "returned:queued-original;original:queued-returning;original:local-returned;returned:local-original;"
    );
}

#[test]
fn immediate_retransfer_with_queued_tasks_refuses_atomically_without_losing_the_queue() {
    let source = "let out='';const c=new MessageChannel();const other=new MessageChannel();parent.postMessage('port','*',[c.port2]);addEventListener('message',e=>{const p=e.ports[0];p.onmessage=m=>out+=m.data;try{parent.postMessage('again','*',[other.port2,p])}catch(error){out=error.name+';'}other.port1.onmessage=m=>out+=m.data;other.port2.postMessage(';local')});";
    let loaded = common::load_with(source, &environment());
    let link = loaded.effects.messages[0].ports[0];
    let returned = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![mars_v9::QueuedPortMessage {
                link,
                data: MessageData::Text("queued".into()),
                to_transferred: true,
            }],
            data: MessageData::Text("return".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![link],
        },
    );
    assert!(returned.effects.messages.is_empty());
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(returned.state),
        10.0,
        16,
    )
    .unwrap();
    assert_eq!(
        common::global(&result, "out"),
        "NotSupportedError;queued;local"
    );
}

#[test]
fn linked_worker_retransfer_refuses_before_detaching_any_port() {
    let source = "let out='';const c=new MessageChannel();const other=new MessageChannel();const w=new Worker('/worker.js');w.postMessage('port',[c.port2]);try{w.postMessage('return',[other.port2,c.port1])}catch(error){out=error.name}c.port1.postMessage('still-linked');other.port1.onmessage=e=>out+=e.data;other.port2.postMessage(';local');";
    let loaded = common::load_with(source, &environment());
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert_eq!(loaded.effects.worker_commands.len(), 2);
    assert_eq!(loaded.effects.port_messages.len(), 1);
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(loaded.state),
        10.0,
        16,
    )
    .unwrap();
    assert_eq!(common::global(&result, "out"), "NotSupportedError;local");
}

#[test]
fn queued_transfer_metadata_cannot_target_a_port_outside_the_transfer() {
    let source = "const c=new MessageChannel();parent.postMessage('port','*',[c.port2]);";
    let loaded = common::load_with(source, &environment());
    let host = common::host();
    let env = environment();
    let result = mars_v9::page_event(
        common::input(&[source.into()], &host, &env),
        common::round_trip(loaded.state),
        PageEvent::Message {
            queued_port_messages: vec![mars_v9::QueuedPortMessage {
                link: 123,
                data: MessageData::Text("forged".into()),
                to_transferred: false,
            }],
            data: MessageData::Text("return".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![],
        },
    );
    assert_eq!(
        result.err().unwrap().message,
        "Invalid reunited MessagePort queue."
    );
}

#[test]
fn original_external_peer_is_immovable_across_state_round_trip_without_partial_detach() {
    let source = "let out='';const c=new MessageChannel();const other=new MessageChannel();parent.postMessage('port','*',[c.port2]);addEventListener('message',()=>{try{parent.postMessage('move','*',[other.port2,c.port1])}catch(error){out=error.name}c.port1.postMessage('still-linked');other.port1.onmessage=e=>out+=e.data;other.port2.postMessage(';local')});";
    let loaded = common::load_with(source, &environment());
    let result = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("move".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![],
        },
    );
    assert!(result.effects.messages.is_empty());
    assert_eq!(result.effects.port_messages.len(), 1);
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(result.state),
        10.0,
        16,
    )
    .unwrap();
    assert_eq!(common::global(&result, "out"), "NotSupportedError;local");
}

#[test]
fn send_before_forward_preserves_the_sent_message_and_every_transfer_entry() {
    let source = "let out='';const other=new MessageChannel();addEventListener('message',e=>{const p=e.ports[0];p.postMessage('sent');try{parent.postMessage('forward','*',[other.port2,p])}catch(error){out=error.name}other.port1.onmessage=m=>out+=m.data;other.port2.postMessage(';local');p.postMessage('still-linked')});";
    let loaded = common::load_with(source, &environment());
    let result = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("port".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![42],
        },
    );
    assert!(result.effects.messages.is_empty());
    assert_eq!(result.effects.port_messages.len(), 2);
    let host = common::host();
    let env = environment();
    let result = mars_v9::run_timers(
        common::input(&[source.into()], &host, &env),
        common::round_trip(result.state),
        10.0,
        16,
    )
    .unwrap();
    assert_eq!(common::global(&result, "out"), "NotSupportedError;local");
}

#[test]
fn local_reunion_clears_anchor_restrictions_for_a_fresh_transfer() {
    let source = "const c=new MessageChannel();parent.postMessage('port','*',[c.port2]);addEventListener('message',()=>parent.postMessage('fresh','*',[c.port1]));";
    let loaded = common::load_with(source, &environment());
    let link = loaded.effects.messages[0].ports[0];
    let result = event(
        source,
        loaded,
        PageEvent::Message {
            queued_port_messages: vec![],
            data: MessageData::Text("return".into()),
            origin: "https://parent.test".into(),
            source: Some(-1),
            ports: vec![link],
        },
    );
    assert_eq!(result.effects.messages.len(), 1);
    assert!(result.effects.messages[0].moved_ports.is_empty());
    assert_ne!(result.effects.messages[0].ports[0], link);
}
