/**
 * In-process engine verification test.
 * Confirms that Relay, Eye, and V9 execute within the coordinator process space
 * sharing memory, thread pool, and channels without spawning child processes.
 */
use mars::protocol::Request;
use mars::runtime::{EnginePrograms, Runtime};
use serde_json::{Value, json};
use std::path::Path;

/** Starts the unified in-process runtime where all engines share the coordinator process. */
fn in_process_runtime() -> Runtime {
    unsafe {
        std::env::remove_var("MARS_PROCESS_ISOLATION");
    }
    Runtime::start(EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars")))).unwrap()
}

/** Sends an engine request through the public runtime request handler. */
fn call(runtime: &mut Runtime, op: &str, params: Value) -> mars::protocol::EngineResult<Value> {
    runtime.handle(Request::new("test", op, params))
}

/** Verifies that all three services share the coordinator PID when running in-process. */
#[test]
fn unified_engines_share_coordinator_process() {
    let runtime = in_process_runtime();
    let current_pid = std::process::id();
    let pids = runtime.process_ids();
    assert_eq!(pids.len(), 3);
    for (name, pid) in pids {
        assert_eq!(pid, current_pid, "Service {name} should run in-process");
    }
}

/** Confirms in-process runtime executes operations and handles requests correctly. */
#[test]
fn in_process_runtime_executes_capabilities_and_ready() {
    let mut runtime = in_process_runtime();
    let caps = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    assert!(caps.get("subsets").is_some());
}
