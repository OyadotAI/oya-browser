/** Explicit injected executables verify service deadlines and session retirement. */
#[allow(dead_code)]
mod support;
use mars::constants::{RPC_TIMEOUT, WEB_PREPARE_RPC_TIMEOUT};
use mars::protocol::Request;
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner, Permit},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

/** Every temporary directory has one test owner, including parallel test runs. */
static DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);
/** Allows process reap overhead without extending the actual child RPC deadline. */
const REAP_MARGIN: Duration = Duration::from_secs(2);
/// How long the script_blocked fault service (tests/support/service.rs) takes over first.js
/// before it stalls on stall.js.
const FIRST_SCRIPT_DELAY: Duration = Duration::from_secs(6);

/** A copied fault binary is reachable only through an explicit injected path. */
struct Fault {
    /** Unique scratch directory containing the binary and its PID evidence. */
    root: PathBuf,
    /** Executable basename selects the test-only failure. */
    executable: PathBuf,
}
impl Fault {
    /** Never modifies a running or installed Oya executable. */
    fn new(mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oya-native-test-{}-{}",
            std::process::id(),
            DIRECTORY_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let executable = root.join(mode);
        std::fs::copy(env!("CARGO_BIN_EXE_mars-test-service"), &executable).unwrap();
        Self { root, executable }
    }
    /** Normal engines still run the exact production executable. */
    fn programs(&self, service: &str) -> EnginePrograms {
        let mut programs = EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars")));
        match service {
            "network" => programs.network = self.executable.clone(),
            "document" => programs.document = self.executable.clone(),
            "v9" => programs.v9 = self.executable.clone(),
            _ => unreachable!(),
        }
        programs
    }
}
impl Drop for Fault {
    /** The test removes only its uniquely owned scratch artifact. */
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

/** Error and status are inspected through the same public native operation table. */
fn call(runtime: &mut Runtime, op: &str, params: Value) -> mars::protocol::EngineResult<Value> {
    runtime.handle(Request::new("fault-test", op, params))
}

/** Process IDs of every engine the runtime currently owns. */
fn engine_pids(runtime: &Runtime) -> Vec<u32> {
    runtime
        .process_ids()
        .into_iter()
        .map(|(_, pid)| pid)
        .collect()
}

/** The one live engine with this name. */
fn engine_pid(runtime: &Runtime, name: &str) -> u32 {
    runtime
        .process_ids()
        .into_iter()
        .find(|(service, _)| *service == name)
        .map(|(_, pid)| pid)
        .unwrap()
}

/** Broken engine envelopes restart only that engine, never replay the operation, and a crash loop
still retires the entire session. Every fault child exits after the first request it reads, so a
fresh child that is still alive proves the failed operation was not sent to it again. */
#[test]
fn invalid_child_replies_restart_the_engine_without_replay() {
    for mode in [
        "malformed",
        "exit",
        "oversized",
        "wrong_id",
        "wrong_version",
        "bad_result",
    ] {
        let fault = Fault::new(mode);
        let mut runtime = Runtime::start(fault.programs("network")).unwrap();
        let mut owned = engine_pids(&runtime);
        let navigate = json!({"url":"http://127.0.0.1:1/"});
        let crashed = engine_pid(&runtime, "network");
        let error = call(&mut runtime, "page.navigate", navigate.clone()).unwrap_err();
        assert_eq!(error.code, "engine_restarted", "{mode}");
        assert_eq!(error.service.as_deref(), Some("network"));
        assert!(error.retryable, "{mode}");
        let fresh = engine_pid(&runtime, "network");
        assert_ne!(fresh, crashed, "{mode}");
        owned.push(fresh);
        #[cfg(unix)]
        {
            support::assert_gone(&[crashed]);
            assert!(
                support::process_exists(fresh),
                "{mode}: operation was replayed"
            );
        }
        assert_eq!(
            call(&mut runtime, "runtime.capabilities", json!({})).unwrap()["state"],
            "ready"
        );
        let retired = loop {
            let error = call(&mut runtime, "page.navigate", navigate.clone()).unwrap_err();
            if error.code != "engine_restarted" {
                break error;
            }
            owned.push(engine_pid(&runtime, "network"));
        };
        assert_eq!(retired.code, "service_unavailable", "{mode}");
        assert_eq!(
            call(&mut runtime, "page.inspect", json!({}))
                .unwrap_err()
                .code,
            "service_unavailable"
        );
        assert_eq!(
            call(&mut runtime, "runtime.capabilities", json!({})).unwrap()["state"],
            "failed"
        );
        #[cfg(unix)]
        support::assert_gone(&owned);
    }
}

/** The deadline supervisor kills a child even while its worker writes a large request, replaces it,
and the runtime still reaps every engine it owns when it stops. */
#[test]
fn child_that_stops_reading_cannot_hang_or_leak_the_runtime() {
    let fixture = support::Fixture::pages();
    let fault = Fault::new("blocked");
    let mut runtime = Runtime::start(fault.programs("document")).unwrap();
    let stuck = engine_pid(&runtime, "document");
    let start = Instant::now();
    let error = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/large", fixture.url)}),
    )
    .unwrap_err();
    assert_eq!(error.code, "timeout");
    assert_eq!(error.service.as_deref(), Some("document"));
    assert!(start.elapsed() >= RPC_TIMEOUT);
    assert!(start.elapsed() < RPC_TIMEOUT + REAP_MARGIN);
    let fresh = engine_pid(&runtime, "document");
    assert_ne!(fresh, stuck);
    let pids = engine_pids(&runtime);
    #[cfg(unix)]
    support::assert_gone(&[stuck]);
    runtime.stop();
    #[cfg(unix)]
    support::assert_gone(&pids);
}

/** A later script fetch shares the navigation deadline and preserves service failure identity; the
stalled engine is replaced and the committed page stays. */
#[test]
fn stalled_external_script_uses_the_remaining_navigation_deadline() {
    let fixture = support::Fixture::pages();
    let fault = Fault::new("script_blocked");
    let authority = Authority::shared(Owner::Agent);
    let mut runtime = Runtime::desktop(
        fault.programs("network"),
        vec![fixture.url.clone()],
        authority,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let permit = Permit {
        owner: Owner::Agent,
        epoch: 1,
        tab_id: Some(1),
    };
    let old = runtime
        .handle_as(
            Request::new(
                "first",
                "page.navigate",
                json!({"url":format!("{}/healthy", fixture.url),"control_epoch":1,"tab_id":"tab-1"}),
            ),
            permit,
        )
        .unwrap();
    let stalled = engine_pid(&runtime, "network");
    let started = Instant::now();
    let error = runtime
        .handle_as(
            Request::new(
                "stalled",
                "page.navigate",
                json!({"url":format!("{}/stall", fixture.url),"control_epoch":1,"tab_id":"tab-1"}),
            ),
            permit,
        )
        .unwrap_err();
    assert_eq!(error.code, "timeout");
    assert_eq!(error.service.as_deref(), Some("network"));
    assert_eq!(error.location.unwrap().script_index, Some(2));
    assert!(error.previous_page_retained);
    assert!(!error.retryable);
    assert!(!error.message.contains("private-secret"));
    assert!(started.elapsed() < FIRST_SCRIPT_DELAY + WEB_PREPARE_RPC_TIMEOUT + REAP_MARGIN);
    let snapshot = runtime.snapshot();
    assert!(!snapshot.failed);
    assert_eq!(snapshot.inspection["page_id"], old["page_id"]);
    #[cfg(unix)]
    support::assert_gone(&[stalled]);
    assert_ne!(engine_pid(&runtime, "network"), stalled);
}

/** A later startup failure reaps every child whose handshake already succeeded. */
#[test]
fn partial_startup_cleans_up_already_owned_children() {
    let fault = Fault::new("ready");
    let mut programs = fault.programs("network");
    programs.document = fault.root.join("does-not-exist");
    assert!(Runtime::start(programs).is_err());
    let pid: u32 = std::fs::read_to_string(fault.root.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    #[cfg(unix)]
    support::assert_gone(&[pid]);
}

/** An engine exiting before readiness is never reported as a healthy runtime. */
#[test]
fn readiness_requires_a_valid_service_handshake() {
    let fault = Fault::new("startup_exit");
    assert!(Runtime::start(fault.programs("network")).is_err());
    let pid: u32 = std::fs::read_to_string(fault.root.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    #[cfg(unix)]
    support::assert_gone(&[pid]);
}

/** A blocked engine dispatch dies when its parent alone is killed, without any outer deadline. */
#[cfg(unix)]
#[test]
fn independent_owner_watch_retires_a_blocked_orphan() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    let fault = Fault::new("owner_parent");
    std::fs::copy(&fault.executable, fault.root.join("owner_blocked")).unwrap();
    let mut parent = Command::new(&fault.executable)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(parent.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let child: u32 = line.trim().parse().unwrap();
    let started = Instant::now();
    parent.kill().unwrap();
    parent.wait().unwrap();
    support::assert_gone(&[child]);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "blocked orphan survived for {:?}",
        started.elapsed()
    );
}
