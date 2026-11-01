/** Broker fixtures exercise native authority, recipient capture and candidate rollback without engines. */
use super::*;
use crate::v9::WorkerData;

/** Distinct native contexts may share a site partition and an exact tuple origin. */
fn address(context: &str) -> Address {
    Address::new(
        context,
        context,
        "https://app.example.com",
        "https://top.example.com/path",
    )
    .unwrap()
}

/** A minimal valid structured clone keeps these routing tests independent of JavaScript evaluation. */
fn data() -> WorkerData {
    serde_json::from_value(serde_json::json!({"root":{"Value":{"Boolean":true}},"nodes":[]}))
        .unwrap()
}

/** The same UTF-16 name includes an unmatched surrogate to prevent accidental UTF-8 normalization. */
fn channel(id: u64) -> BroadcastEndpoint {
    BroadcastEndpoint {
        id,
        name: vec![0xd800, 65],
    }
}

/** Engine effects are represented as an ordered command list and its final live snapshot. */
fn batch(
    address: Address,
    commands: Vec<BroadcastCommand>,
    channels: Vec<BroadcastEndpoint>,
) -> Batch {
    Batch {
        sequence: 1,
        bytes: batch_bytes(&commands, &channels).unwrap(),
        address,
        commands,
        channels,
    }
}

/** Open one endpoint in a fresh context. */
fn open(broker: &mut Broker, address: Address, id: u64) {
    broker
        .apply(batch(
            address,
            vec![BroadcastCommand::Open {
                id,
                name: channel(id).name,
            }],
            vec![channel(id)],
        ))
        .unwrap();
}

/** Consume tasks in native delivery order without involving a page or granting script authority. */
fn recipients(broker: &mut Broker) -> Vec<String> {
    let mut recipients = Vec::new();
    while let Some(letter) = broker.next() {
        recipients.push(letter.recipient.context);
        broker.acknowledge(&[letter.serial]);
    }
    recipients
}

#[test]
/** Exact origin and schemeful partition both participate, including siblings inside one context. */
fn routes_to_matching_siblings_and_contexts_in_creation_order() {
    let mut broker = Broker::default();
    let a = address("a");
    open(&mut broker, a.clone(), 1);
    open(&mut broker, address("b"), 1);
    let mut foreign = address("foreign");
    foreign.origin = "https://other.example.com".into();
    open(&mut broker, foreign, 1);
    let mut isolated = address("partition");
    isolated.partition = "https://other.test".into();
    open(&mut broker, isolated, 1);
    broker
        .apply(batch(
            a,
            vec![
                BroadcastCommand::Open {
                    id: 2,
                    name: channel(2).name,
                },
                BroadcastCommand::Post {
                    id: 1,
                    data: data(),
                },
            ],
            vec![channel(1), channel(2)],
        ))
        .unwrap();
    assert_eq!(recipients(&mut broker), ["b", "a"]);
}

#[test]
/** Sender close preserves captured tasks while a receiver close cancels only its own task. */
fn closure_and_late_open_preserve_post_time_recipient_set() {
    let mut broker = Broker::default();
    open(&mut broker, address("a"), 1);
    open(&mut broker, address("b"), 1);
    open(&mut broker, address("c"), 1);
    broker
        .apply(batch(
            address("a"),
            vec![
                BroadcastCommand::Post {
                    id: 1,
                    data: data(),
                },
                BroadcastCommand::Close { id: 1 },
            ],
            vec![],
        ))
        .unwrap();
    open(&mut broker, address("late"), 1);
    broker
        .apply(batch(
            address("b"),
            vec![BroadcastCommand::Close { id: 1 }],
            vec![],
        ))
        .unwrap();
    assert_eq!(recipients(&mut broker), ["c"]);
}

#[test]
/** A clone getter may close its source after postMessage starts; the admitted post still sends. */
fn getter_close_before_post_keeps_source_metadata_for_the_batch() {
    let mut broker = Broker::default();
    open(&mut broker, address("a"), 1);
    open(&mut broker, address("b"), 1);
    broker
        .apply(batch(
            address("a"),
            vec![
                BroadcastCommand::Close { id: 1 },
                BroadcastCommand::Post {
                    id: 1,
                    data: data(),
                },
            ],
            vec![],
        ))
        .unwrap();
    let letter = broker.next().unwrap();
    assert_eq!(letter.origin, "https://app.example.com");
    assert_eq!(recipients(&mut broker), ["b"]);
}

#[test]
/** GC and native context retirement cancel recipients without relying on a JavaScript close call. */
fn snapshots_and_tab_teardown_retire_receivers() {
    let mut broker = Broker::default();
    open(&mut broker, address("a"), 1);
    open(&mut broker, address("b"), 1);
    open(&mut broker, address("c"), 1);
    broker
        .apply(batch(
            address("a"),
            vec![BroadcastCommand::Post {
                id: 1,
                data: data(),
            }],
            vec![channel(1)],
        ))
        .unwrap();
    broker.apply(batch(address("b"), vec![], vec![])).unwrap();
    broker.remove_tab("c");
    assert!(broker.next().is_none());
    broker.prune(&[]);
    assert!(!broker.has_context("a"));
}

#[test]
/** Failed candidates can neither publish a partial post nor consume a committed delivery. */
fn abandoned_broker_candidate_leaves_committed_delivery_untouched() {
    let mut committed = Broker::default();
    open(&mut committed, address("a"), 1);
    open(&mut committed, address("b"), 1);
    committed
        .apply(batch(
            address("a"),
            vec![BroadcastCommand::Post {
                id: 1,
                data: data(),
            }],
            vec![channel(1)],
        ))
        .unwrap();
    let letter = committed.next().unwrap();
    let mut candidate = committed.clone();
    candidate.acknowledge(&[letter.serial]);
    let result = candidate.apply(batch(
        address("a"),
        vec![BroadcastCommand::Post {
            id: 1,
            data: data(),
        }],
        vec![channel(99)],
    ));
    assert!(result.is_err());
    assert_eq!(committed.next().unwrap().serial, letter.serial);
    assert_eq!(recipients(&mut committed), ["b"]);
}

#[test]
/** A terminating worker can finish an already-started post without retaining its receiver authority. */
fn retired_native_sender_keeps_captured_post_authority_only() {
    let mut broker = Broker::default();
    open(&mut broker, address("worker"), 1);
    open(&mut broker, address("page"), 1);
    let live = [address("page")];
    let retired = broker.retired_sources(&live);
    broker.prune(&live);
    broker
        .apply_with_sources(
            batch(
                address("worker"),
                vec![BroadcastCommand::Post {
                    id: 1,
                    data: data(),
                }],
                vec![channel(1)],
            ),
            &retired,
        )
        .unwrap();
    broker.prune(&live);
    assert!(!broker.has_context("worker"));
    assert_eq!(recipients(&mut broker), ["page"]);
}

#[test]
/** A full delivery queue rejects a new post before any receiver is partially admitted. */
fn queue_count_admission_is_atomic_for_each_post() {
    let mut broker = Broker::default();
    open(&mut broker, address("a"), 1);
    open(&mut broker, address("b"), 1);
    for _ in 0..TASKS {
        broker
            .apply(batch(
                address("a"),
                vec![BroadcastCommand::Post {
                    id: 1,
                    data: data(),
                }],
                vec![channel(1)],
            ))
            .unwrap();
    }
    assert!(
        broker
            .apply(batch(
                address("a"),
                vec![BroadcastCommand::Post {
                    id: 1,
                    data: data()
                }],
                vec![channel(1)]
            ))
            .is_err()
    );
    assert_eq!(recipients(&mut broker).len(), TASKS);
}
