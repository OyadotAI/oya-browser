/*! An opt-in official SDK exchange controls only its newly launched Mars window. */
#![cfg(all(target_os = "macos", feature = "test-support"))]
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/** Allows the full toolbar and form journey while bounding a failed SDK client. */
const DEADLINE: Duration = Duration::from_secs(40);
/** Process observation does not compete with the native event loop. */
const POLL: Duration = Duration::from_millis(20);

/** Installed official client interoperability includes actual human-toolbar actions and process cleanup. */
#[test]
#[ignore = "opens the actual native macOS MCP window using the installed official Node SDK"]
fn official_sdk_exercises_toolbar_ownership_and_native_forms() {
    let mut child = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/mcp-sdk.mjs"
        ))
        .arg(env!("CARGO_BIN_EXE_mars"))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "official MCP SDK journey failed");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("official MCP SDK journey exceeded its deadline");
        }
        std::thread::sleep(POLL);
    }
}
