/** Native worker traces distinguish loading stages without retaining page-controlled text. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use std::process::{Command, Stdio};
use support::{Fixture, agent_call, desktop, finish, html};

/** Only this child test initializes the process-global opt-in trace sink. */
const CHILD_ENV: &str = "MARS_WORKER_DIAGNOSTIC_CHILD";
/** Deliberately present in URLs, source and exception text, but forbidden in worker records. */
const SECRET: &str = "worker-private-detail";

/** Synthetic script responses pass through the ordinary source loader. */
fn script(source: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{source}", source.len()).into_bytes()
}

/** One isolated fixture tests a real startup outcome, including imports and exceptions. */
fn scenario(policy: &str, response: Vec<u8>, imported: Vec<u8>, expected: &str) {
    let page = format!(
        "<script>let result='waiting';const w=new Worker('/worker.js?{SECRET}');w.onerror=e=>result='failed';w.onmessage=e=>result=e.data;</script>"
    );
    let fixture = Fixture::routes(vec![
        ("/", html(policy, &page)),
        ("/worker.js?worker-private-detail", response),
        ("/import.js", imported),
    ]);
    let mut runtime = desktop(&fixture.url);
    agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let observed = agent_call(&mut runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"result","options":{"return_by_value":true}}})).unwrap();
    assert_eq!(observed["result"]["value"], expected);
}

/** The subprocess owns its environment from startup, avoiding unsafe parallel env mutation. */
#[test]
fn worker_diagnostic_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    scenario(
        "Content-Security-Policy: worker-src 'none'\r\n",
        script("postMessage('wrong')"),
        vec![],
        "failed",
    );
    scenario(
        "",
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
        vec![],
        "failed",
    );
    scenario("", html("", SECRET), vec![], "failed");
    scenario("", b"malformed response\r\n\r\n".to_vec(), vec![], "failed");
    scenario(
        "",
        script("importScripts('/import.js')"),
        html("", SECRET),
        "failed",
    );
    scenario(
        "",
        script("importScripts('file:///worker-private-detail')"),
        vec![],
        "failed",
    );
    scenario(
        "",
        script("throw new Error('worker-private-detail')"),
        vec![],
        "failed",
    );
    scenario(
        "",
        script("importScripts('/import.js')"),
        script("postMessage('ready')"),
        "ready",
    );
}

/** Logging must identify the refused stage while never copying URL, MIME, source or exception text. */
#[test]
fn native_worker_trace_reports_fixed_failure_categories_without_source_details() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "mars-worker-diagnostics-{}-{nonce}.log",
        std::process::id()
    ));
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "worker_diagnostic_child", "--nocapture"])
        .env(CHILD_ENV, "1")
        .env("MARS_PERF", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = finish(child);
    let log = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(path);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let records: Vec<_> = log
        .lines()
        .filter(|line| line.starts_with("worker."))
        .collect();
    for (event, reason) in [
        ("worker.load", "policy"),
        ("worker.load", "status:404"),
        ("worker.load", "mime"),
        ("worker.load", "transport"),
        ("worker.import", "mime"),
        ("worker.import", "address"),
        ("worker.execute", "execution"),
        ("worker.import", "loaded"),
    ] {
        assert!(
            records
                .iter()
                .any(|line| line.starts_with(event) && line.ends_with(reason)),
            "missing {event}/{reason}: {records:?}"
        );
    }
    assert!(
        !records
            .iter()
            .any(|line| line.contains(SECRET) || line.contains("http") || line.contains("file:")),
        "{records:?}"
    );
}
