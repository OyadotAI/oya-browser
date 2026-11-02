#![cfg(target_os = "macos")]
/** PL-09: every real engine child runs under its Seatbelt profile and pages still load through it. */
#[allow(dead_code)]
mod support;
use mars::protocol::Request;
use mars::runtime::{EnginePrograms, Runtime};
use serde_json::json;
use std::ffi::{CString, c_char};
use std::path::{Path, PathBuf};
use support::Fixture;

/** sandbox_check filter: the operation has no argument. */
const FILTER_NONE: i32 = 0;
/** sandbox_check filter: the argument is a file path. */
const FILTER_PATH: i32 = 1;
/** Asks the kernel without logging a violation report for a probe. */
const NO_REPORT: i32 = 0x4000_0000;
/** sandbox_check answer for an operation the process's policy denies. */
const DENIED: i32 = 1;
/** sandbox_check answer for an operation the process's policy allows. */
const ALLOWED: i32 = 0;

unsafe extern "C" {
    /** Evaluates another process's live sandbox policy, exactly as the kernel would. */
    fn sandbox_check(pid: libc::pid_t, operation: *const c_char, filter: i32, ...) -> i32;
}

/** Real engine processes from the production executable. The sandbox applies only to
subprocess engines, which `MARS_PROCESS_ISOLATION` selects; in-process engines share the
coordinator's sandbox. */
fn runtime() -> Runtime {
    // SAFETY: every test in this binary sets the same value before starting a runtime.
    unsafe { std::env::set_var("MARS_PROCESS_ISOLATION", "1") };
    Runtime::start(EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars")))).unwrap()
}

/** One path-filtered policy question about a live child. */
fn check_path(pid: u32, operation: &str, path: &Path) -> i32 {
    let operation = CString::new(operation).unwrap();
    let path = CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        sandbox_check(
            pid as libc::pid_t,
            operation.as_ptr(),
            FILTER_PATH | NO_REPORT,
            path.as_ptr(),
        )
    }
}

/** One unfiltered policy question about a live child. */
fn check(pid: u32, operation: &str) -> i32 {
    let operation = CString::new(operation).unwrap();
    unsafe {
        sandbox_check(
            pid as libc::pid_t,
            operation.as_ptr(),
            FILTER_NONE | NO_REPORT,
        )
    }
}

/** The person's home folder, which no engine may write or read. */
fn home() -> PathBuf {
    std::fs::canonicalize(std::env::var("HOME").unwrap()).unwrap()
}

/** The one writable scratch folder, resolved the same way the engines resolve it: from the
per-user temp folder, not $TMPDIR, which a developer shell may point elsewhere. */
fn scratch() -> PathBuf {
    let mut buffer = vec![0_u8; libc::PATH_MAX as usize];
    let length = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    buffer.truncate(length - 1);
    std::fs::canonicalize(String::from_utf8(buffer).unwrap())
        .unwrap()
        .join("mars-engine")
}

/** A child cannot write into or read the home folder, or write to the shared /tmp. Seatbelt only
answers path probes for paths that exist, so the probes use existing folders, never new files. */
#[test]
fn engines_cannot_write_or_read_the_home_folder() {
    let runtime = runtime();
    let library = home().join("Library");
    for (name, pid) in runtime.process_ids() {
        for path in [home(), library.clone(), PathBuf::from("/private/tmp")] {
            assert_eq!(
                check_path(pid, "file-write-create", &path),
                DENIED,
                "{name} {path:?}"
            );
        }
        assert_eq!(check_path(pid, "file-read-data", &home()), DENIED, "{name}");
        assert_eq!(
            check_path(pid, "file-read-data", &library),
            DENIED,
            "{name}"
        );
    }
}

/** The scratch folder stays writable, which also proves the probes above are not blanket denials. */
#[test]
fn engines_may_write_only_their_scratch_folder() {
    let runtime = runtime();
    let file = scratch().join(format!("probe-{}", std::process::id()));
    std::fs::write(&file, b"probe").unwrap();
    for (name, pid) in runtime.process_ids() {
        assert_eq!(
            check_path(pid, "file-write-create", &scratch()),
            ALLOWED,
            "{name}"
        );
        assert_eq!(check_path(pid, "file-write-data", &file), ALLOWED, "{name}");
    }
    std::fs::remove_file(file).unwrap();
}

/** No engine may start another program or fork. */
#[test]
fn engines_cannot_start_processes() {
    let runtime = runtime();
    for (name, pid) in runtime.process_ids() {
        assert_eq!(check(pid, "process-exec*"), DENIED, "{name}");
        assert_eq!(check(pid, "process-fork"), DENIED, "{name}");
    }
}

/** Only Relay keeps outbound network; Eye and V9 have no sockets, and nothing listens. */
#[test]
fn only_the_network_engine_reaches_the_network() {
    let runtime = runtime();
    for (name, pid) in runtime.process_ids() {
        let expected = if name == "network" { ALLOWED } else { DENIED };
        assert_eq!(check(pid, "network-outbound"), expected, "{name}");
        assert_eq!(check(pid, "network-inbound"), DENIED, "{name}");
    }
}

/** HTTP through Relay, parsing and layout in Eye and scripts in V9 all still work confined. */
#[test]
fn pages_still_load_inside_the_sandbox() {
    let fixture = Fixture::pages();
    let mut runtime = runtime();
    let page = runtime
        .handle(Request::new(
            "sandbox",
            "page.navigate",
            json!({"url":format!("{}/script", fixture.url)}),
        ))
        .unwrap();
    assert_eq!(page["nodes"][0]["text"], "Hello from V9");
    let layout = runtime
        .handle(Request::new("sandbox", "page.layout", json!({})))
        .unwrap();
    assert!(!layout["rows"].as_array().unwrap().is_empty());
}
