#![cfg(unix)]
/** CB-23: killing one engine fails only the operation using it; Mars restarts it and every tab keeps
its committed page. A crash loop still retires the session instead of restarting forever. */
#[allow(dead_code)]
mod support;
use mars::protocol::{EngineError, Request};
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner, Permit},
};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;
use support::Fixture;

/** Long enough for the slow fixture's request to be in flight inside Relay. */
const IN_FLIGHT: Duration = Duration::from_millis(300);
/** One more crash than the restart budget allows inside its window. */
const CRASH_LOOP: usize = 4;

/** A desktop runtime with the person in control of every tab, each engine in its own process
so killing one is an engine crash, not the test's own death. */
fn runtime(origins: &[&str]) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))).isolated(),
        origins.iter().map(|origin| (*origin).to_owned()).collect(),
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/** One human operation on one tab. */
fn human(
    runtime: &mut Runtime,
    tab: u64,
    op: &str,
    mut params: Value,
) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!(format!("tab-{tab}"));
    }
    let permit = Permit {
        owner: Owner::Human,
        epoch: 1,
        tab_id: Some(tab),
    };
    runtime.handle_as(Request::new("restart", op, params), permit)
}

/** The live process ID of one named engine. */
fn pid(runtime: &Runtime, name: &str) -> u32 {
    runtime
        .process_ids()
        .into_iter()
        .find(|(service, _)| *service == name)
        .map(|(_, pid)| pid)
        .unwrap()
}

/** SIGKILL, like an engine crash; the kernel reaps nothing on the runtime's behalf. */
fn crash(pid: u32) {
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    while support::process_exists(pid) && !reaped(pid) {
        std::thread::sleep(support::POLL);
    }
}

/** A killed child stays a zombie until its parent waits; signal 0 still succeeds on it. */
fn reaped(pid: u32) -> bool {
    let state = std::process::Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&state.stdout)
        .trim_start()
        .starts_with('Z')
}

/** Two tabs with committed pages: tab 1 plain text, tab 2 the scripted page. */
fn two_tabs(runtime: &mut Runtime, origin: &str) {
    human(
        runtime,
        1,
        "page.navigate",
        json!({"url":format!("{origin}/plain")}),
    )
    .unwrap();
    human(runtime, 1, "tab.open", json!({})).unwrap();
    human(
        runtime,
        2,
        "page.navigate",
        json!({"url":format!("{origin}/script")}),
    )
    .unwrap();
}

/** The next operation after a crash reports it; tabs, pages and the other engines survive. */
#[test]
fn a_crashed_engine_is_restarted_and_other_tabs_keep_their_pages() {
    for engine in ["network", "document", "v9"] {
        let fixture = Fixture::pages();
        let mut runtime = runtime(&[&fixture.url]);
        two_tabs(&mut runtime, &fixture.url);
        let before = pid(&runtime, engine);
        let others: Vec<_> = runtime
            .process_ids()
            .into_iter()
            .filter(|(name, _)| *name != engine)
            .map(|(name, pid)| (name.to_owned(), pid))
            .collect();
        crash(before);
        let error = human(
            &mut runtime,
            2,
            "page.navigate",
            json!({"url":format!("{}/plain", fixture.url)}),
        )
        .unwrap_err();
        assert_eq!(error.code, "engine_restarted", "{engine}");
        assert!(error.retryable && error.previous_page_retained, "{engine}");
        assert_ne!(pid(&runtime, engine), before, "{engine}");
        let survivors: Vec<_> = runtime
            .process_ids()
            .into_iter()
            .filter(|(name, _)| *name != engine)
            .map(|(name, pid)| (name.to_owned(), pid))
            .collect();
        assert_eq!(
            survivors, others,
            "{engine}: healthy engines are not restarted"
        );
        assert_eq!(
            runtime.snapshot().inspection["url"],
            format!("{}/script", fixture.url)
        );
        human(&mut runtime, 1, "tab.select", json!({"tab_id":"tab-1"})).unwrap();
        assert_eq!(
            runtime.snapshot().inspection["url"],
            format!("{}/plain", fixture.url)
        );
        let retried = human(
            &mut runtime,
            2,
            "page.navigate",
            json!({"url":format!("{}/script", fixture.url)}),
        )
        .unwrap();
        assert_eq!(retried["nodes"][0]["text"], "Hello from V9", "{engine}");
    }
}

/** A crash in the middle of a navigation fails that navigation and keeps the old page. */
#[test]
fn a_crash_during_navigation_fails_only_that_navigation() {
    let fixture = Fixture::pages();
    let slow = Fixture::slow();
    let mut runtime = runtime(&[&fixture.url, &slow.url]);
    two_tabs(&mut runtime, &fixture.url);
    let network = pid(&runtime, "network");
    let killer = std::thread::spawn(move || {
        std::thread::sleep(IN_FLIGHT);
        unsafe { libc::kill(network as libc::pid_t, libc::SIGKILL) };
    });
    let error = human(&mut runtime, 2, "page.navigate", json!({"url":slow.url})).unwrap_err();
    killer.join().unwrap();
    assert_eq!(error.code, "engine_restarted");
    assert_eq!(error.service.as_deref(), Some("network"));
    assert_eq!(
        runtime.snapshot().inspection["url"],
        format!("{}/script", fixture.url)
    );
    assert!(
        human(
            &mut runtime,
            2,
            "page.navigate",
            json!({"url":format!("{}/plain", fixture.url)})
        )
        .is_ok()
    );
}

/** An engine that keeps dying ends the session with the old fatal error rather than looping. */
#[test]
fn a_crash_loop_retires_the_session() {
    let fixture = Fixture::pages();
    let mut runtime = runtime(&[&fixture.url]);
    let mut last = None;
    for _ in 0..CRASH_LOOP {
        crash(pid(&runtime, "v9"));
        last = human(
            &mut runtime,
            1,
            "page.navigate",
            json!({"url":format!("{}/plain", fixture.url)}),
        )
        .err();
    }
    let error = last.unwrap();
    assert_eq!(error.code, "service_unavailable");
    assert!(runtime.process_ids().is_empty());
}
