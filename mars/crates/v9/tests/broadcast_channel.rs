/** BroadcastChannel bindings publish inert ordered work and receive asynchronous cloned message tasks. */
mod common;
use mars_v9::{BroadcastCommand, Execution, PageEvent};

/** Binding regressions fail on both individual script errors and page-finalization failures. */
fn run(source: &str) -> Execution {
    let execution = common::load(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    execution
}

/** A native coordinator chooses the recipient; this helper exercises only the V9 delivery seam. */
fn receive(source: &str, execution: Execution, id: u64, data: mars_v9::WorkerData) -> Execution {
    let host = common::host();
    let environment = common::environment();
    let sources = [source.to_string()];
    mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        PageEvent::BroadcastMessage {
            id,
            data,
            origin: "https://example.com".into(),
        },
    )
    .unwrap()
}

/** Posted packets are immutable graph copies; no listener executes until a native delivery task. */
#[test]
fn native_delivery_preserves_cycles_snapshot_data_and_message_event_identity() {
    let source = "const a=new BroadcastChannel('room'),b=new BroadcastChannel('room'),NativeMessageEvent=MessageEvent;let result='pending';b.onmessage=e=>{result=[e instanceof NativeMessageEvent,e.target===b,e.currentTarget===b,e.data.self===e.data,e.data.answer,e.origin,e.source===null,e.ports.length,e.isTrusted].join('|')};const data={answer:42};data.self=data;a.postMessage(data);data.answer=7;globalThis.MessageEvent=function(){throw Error('author constructor')};";
    let execution = run(source);
    assert_eq!(common::global(&execution, "result"), "pending");
    let id = execution.effects.broadcast_channels[1].id;
    let data = execution
        .effects
        .broadcast_commands
        .iter()
        .find_map(|command| match command {
            BroadcastCommand::Post { data, .. } => Some(data.clone()),
            _ => None,
        })
        .unwrap();
    let delivered = receive(source, execution, id, data);
    assert_eq!(
        common::global(&delivered, "result"),
        "true|true|true|true|42|https://example.com|true|0|true"
    );
}

/** Constructor and methods are branded; close leaves name readable and rejects before clone getters. */
#[test]
fn close_is_idempotent_and_refuses_post_before_reading_message_properties() {
    let execution = run(
        "const channel=new BroadcastChannel('\\ud800');let result='',reads=0;channel.onmessage=7;const cleared=channel.onmessage===null;channel.close();channel.close();try{channel.postMessage({get value(){reads++;return 1}})}catch(error){result=error.name}let branded=false;try{BroadcastChannel.prototype.close.call({})}catch(error){branded=error instanceof TypeError}result+='|'+reads+'|'+channel.name.charCodeAt(0)+'|'+cleared+'|'+branded;",
    );
    assert_eq!(
        common::global(&execution, "result"),
        "InvalidStateError|0|55296|true|true"
    );
    assert!(execution.effects.broadcast_channels.is_empty());
    assert_eq!(execution.effects.broadcast_commands.len(), 2);
    assert!(
        matches!(&execution.effects.broadcast_commands[0], BroadcastCommand::Open {name,..} if name == &[0xd800])
    );
    assert!(matches!(
        execution.effects.broadcast_commands[1],
        BroadcastCommand::Close { .. }
    ));
}

/** Serialization may close its sender reentrantly, but the already-started post remains ordered work. */
#[test]
fn clone_getter_close_precedes_the_valid_already_started_post() {
    let execution = run(
        "const channel=new BroadcastChannel('room');channel.postMessage({get value(){channel.close();return 1}});",
    );
    assert!(matches!(
        execution.effects.broadcast_commands.as_slice(),
        [
            BroadcastCommand::Open { .. },
            BroadcastCommand::Close { .. },
            BroadcastCommand::Post { .. }
        ]
    ));
    assert!(execution.effects.broadcast_channels.is_empty());
}

/** Native recipient selection never overrides a close that happened before its task could execute. */
#[test]
fn a_closed_recipient_drops_an_already_queued_delivery() {
    let source = "const a=new BroadcastChannel('room'),b=new BroadcastChannel('room');let result='pending';b.onmessage=()=>result='called';a.postMessage(1);b.close();";
    let execution = run(source);
    let id = execution
        .effects
        .broadcast_commands
        .iter()
        .filter_map(|command| match command {
            BroadcastCommand::Open { id, .. } => Some(*id),
            _ => None,
        })
        .nth(1)
        .unwrap();
    let data = execution
        .effects
        .broadcast_commands
        .iter()
        .find_map(|command| match command {
            BroadcastCommand::Post { data, .. } => Some(data.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        common::global(&receive(source, execution, id, data), "result"),
        "pending"
    );
}

/** Unreachable wrappers are weak, except the open message/messageerror listener roots HTML requires. */
#[test]
fn final_snapshot_collects_unobserved_channels_but_keeps_message_listeners() {
    let execution = run(
        "new BroadcastChannel('discard');new BroadcastChannel('handler').onmessage=()=>{};const listened=new BroadcastChannel('listener');listened.addEventListener('messageerror',()=>{});let result='ok';",
    );
    let names: Vec<_> = execution
        .effects
        .broadcast_channels
        .iter()
        .map(|channel| String::from_utf16(&channel.name).unwrap())
        .collect();
    assert_eq!(names, ["handler", "listener"]);
    execution.state.validate_document(&common::page()).unwrap();
    execution
        .state
        .summary()
        .validate_summary_document(&common::page())
        .unwrap();
}

/** Unsupported graph members fail explicitly and leave no misleading successful post effect. */
#[test]
fn noncloneable_values_do_not_emit_post_commands() {
    let execution = run(
        "const channel=new BroadcastChannel('room');let result='';try{channel.postMessage(()=>{})}catch(error){result=error.name}",
    );
    assert_eq!(common::global(&execution, "result"), "DataCloneError");
    assert!(
        execution
            .effects
            .broadcast_commands
            .iter()
            .all(|command| !matches!(command, BroadcastCommand::Post { .. }))
    );
}

/** Dedicated workers expose the same implemented interface through their explicit global allowlist. */
#[test]
fn worker_globals_construct_channels_and_publish_ordered_posts() {
    let environment = mars_v9::PageEnvironment {
        worker_name: Some("worker".into()),
        ..common::environment()
    };
    let host = common::host();
    let sources =
        vec!["const channel=new BroadcastChannel('worker');channel.postMessage({value:1});".into()];
    let mut input = common::input(&sources, &host, &environment);
    input.document.nodes.clear();
    let execution = mars_v9::load_worker(input).unwrap();
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(matches!(
        execution.effects.broadcast_commands.as_slice(),
        [BroadcastCommand::Open { .. }, BroadcastCommand::Post { .. }]
    ));
}
