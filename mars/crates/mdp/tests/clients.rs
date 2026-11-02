/** Real clients (Playwright, and Puppeteer when installed) drive Mars through the front door. */
#[allow(dead_code)]
mod support;
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, atomic::Ordering},
};
use support::Stack;

/** The lab page the client script works on. */
const LAB: &str = r#"<!doctype html><html><head><title>Mars CDP Lab</title></head><body>
<h1 id="title">Lab</h1>
<form id="f" action="/done"><label for="name">Name</label><input id="name" name="name"><button id="go" type="button">Go</button></form>
<p id="greeting"></p><ul id="list"><li>one</li><li>two</li></ul>
<a id="link" href="/done">Done</a>
<script>
document.getElementById('go').addEventListener('click', () => { document.getElementById('greeting').textContent = 'Hello ' + document.getElementById('name').value; });
setTimeout(() => { const p = document.createElement('p'); p.id = 'late'; p.textContent = 'Late'; document.body.appendChild(p); }, 50);
window.data = { answer: 42, list: [1, 2, 3], nested: { ok: true } };
</script></body></html>"#;

/** Where the repo's Playwright lives; the test is skipped without it. */
fn playwright_dir() -> Option<String> {
    let default = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../examples/node_modules/playwright-core"
    );
    let dir = std::env::var("MARS_PLAYWRIGHT").unwrap_or_else(|_| default.into());
    Path::new(&dir).join("index.mjs").is_file().then_some(dir)
}

/** An optional Puppeteer install, named by MARS_PUPPETEER. */
fn puppeteer_dir() -> Option<String> {
    std::env::var("MARS_PUPPETEER").ok().filter(|dir| {
        Path::new(dir)
            .join("lib/puppeteer/puppeteer-core.js")
            .is_file()
    })
}

/** The scripted journey passes against a real runtime; `/handoff` hands the tab to the person. */
#[test]
fn playwright_journey_passes() {
    let Some(playwright) = playwright_dir() else {
        eprintln!("skipped: no playwright-core install (set MARS_PLAYWRIGHT)");
        return;
    };
    let handoff = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = handoff.clone();
    let stack = Stack::start(Arc::new(move |path: &str| {
        if path.starts_with("/done") {
            "<title>Done</title><h1>Done</h1>".to_string()
        } else if path.starts_with("/api/") {
            r#"{"ok":true}"#.to_string()
        } else if path.starts_with("/handoff") {
            signal.store(true, Ordering::Release);
            LAB.to_string()
        } else {
            LAB.to_string()
        }
    }));
    let authority = stack.authority.clone();
    let watcher = handoff.clone();
    std::thread::spawn(move || {
        while !watcher.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The person takes every agent tab back, as the toolbar's takeover does.
        authority.lock().unwrap().revoke_agents();
    });
    let mut command = Command::new("node");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/cdp_playwright.mjs"
        ))
        .arg(stack.port.to_string())
        .arg(&stack.origin)
        .arg(&playwright);
    if let Some(puppeteer) = puppeteer_dir() {
        command.arg(puppeteer);
    }
    // MARS_MDP_DEBUG=<file> (or legacy MARS_CDP_DEBUG=<file>) keeps Playwright's protocol log (DEBUG=pw:protocol) in that file.
    let log = std::env::var("MARS_MDP_DEBUG")
        .or_else(|_| std::env::var("MARS_CDP_DEBUG"))
        .ok();
    if log.is_some() {
        command.env("DEBUG", "pw:protocol,puppeteer:*");
    }
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("node runs");
    let report = String::from_utf8_lossy(&output.stdout);
    match log {
        Some(path) => std::fs::write(path, &output.stderr).expect("log written"),
        None => eprintln!("{report}{}", String::from_utf8_lossy(&output.stderr)),
    }
    eprintln!("{report}");
    assert!(
        output.status.success(),
        "a required client step failed:\n{report}"
    );
}
