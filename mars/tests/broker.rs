/** The actual bounded desktop broker must terminate visibly rather than hang under saturation. */
use mars::{constants, protocol, runtime};
#[path = "../src/desktop/broker.rs"]
#[allow(dead_code)]
mod broker;
#[path = "../src/desktop/io.rs"]
mod io;
#[path = "../src/desktop/mcp_io.rs"]
mod mcp_io;
#[allow(dead_code)]
mod support;
use protocol::Request;
use runtime::authority::{Owner, Permit};
use serde_json::json;
use std::{path::PathBuf, sync::mpsc::RecvTimeoutError, time::Duration};

/** A bounded undrained event stream fails closed and exposes receiver disconnection. */
#[test]
fn event_saturation_retires_engines_and_disconnects_the_ui_stream() {
    let broker = broker::Broker::start_with_profiles(
        PathBuf::from(env!("CARGO_BIN_EXE_mars")),
        Vec::new(),
        false,
        Owner::Human,
        None,
        broker::ProfileStores {
            cookies: std::sync::Arc::new(mars::profile::test_support::MemoryStore::default()),
            storage: std::sync::Arc::new(
                mars::profile::test_support::MemoryWebStorageStore::default(),
            ),
            sessions: std::sync::Arc::new(
                mars::profile::test_support::MemorySessionStore::default(),
            ),
            records: std::sync::Arc::new(
                mars::profile::test_support::MemoryRecordStore::default(),
            ),
        },
    )
    .unwrap();
    let initial = broker.events.recv_timeout(Duration::from_secs(2)).unwrap();
    let broker::Event::Update { snapshot, .. } = initial else {
        panic!("startup failed");
    };
    assert!(!snapshot.failed);
    for index in 0..broker::QUEUE * 4 {
        if broker
            .commands
            .send(broker::Command::Agent(
                Request::new(&index.to_string(), "runtime.capabilities", json!({})),
                Permit {
                    owner: Owner::Agent,
                    epoch: 1,
                    tab_id: None,
                },
            ))
            .is_err()
        {
            break;
        }
    }
    let mut disconnected = false;
    let mut pids = Vec::new();
    loop {
        match broker.events.recv_timeout(Duration::from_secs(2)) {
            Ok(broker::Event::Update {
                result: Ok(value), ..
            }) => {
                if let Some(engines) = value["engines"].as_array() {
                    pids = engines
                        .iter()
                        .map(|engine| engine["pid"].as_u64().unwrap() as u32)
                        .collect();
                }
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Disconnected) => {
                disconnected = true;
                break;
            }
            Err(_) => break,
        }
    }
    assert!(disconnected, "saturated stream never terminated");
    assert_eq!(pids.len(), 3);
    support::assert_gone(&pids);
}
