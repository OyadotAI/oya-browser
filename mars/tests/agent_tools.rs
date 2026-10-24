/** Agent tools (summary, act, wait, diff, find, compact inspect) run through the real Runtime and engines. */
#[allow(dead_code)]
mod support;
use mars::{
    protocol::{EngineError, Request},
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit, SharedAuthority},
    },
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/** A typed secret that must never appear in any agent-facing output. */
const SECRET: &str = "hunter2-secret-value";

/** The signup page every test starts from. */
const SIGNUP: &str = r#"<html><head><title>Signup</title></head><body>
<header aria-label="Site"><h1>Join</h1></header>
<nav><a href="/takeover">Help</a></nav>
<main>
<h2>Account</h2>
<form action="/done" method="get">
<label for="name">Full name</label><input id="name" name="name">
<label for="pw">Password</label><input id="pw" name="pw" type="password">
<label for="news">Newsletter</label><input id="news" name="news" type="checkbox">
<label for="plan">Plan</label><select id="plan" name="plan"><option>Free</option><option>Pro</option></select>
<button type="submit">Create account</button>
</form>
<h3>Details</h3>
</main></body></html>"#;

/** One hermetic loopback server; the handler may act on the test's authority. */
struct Site {
    origin: String,
    authority: SharedAuthority,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Site {
    /** Serve the signup flow; GET /takeover hands the tab to the person before it answers. */
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let authority = Authority::shared(Owner::Agent);
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (signal, seen, owner) = (stop.clone(), requests.clone(), authority.clone());
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Relaxed) {
                if let Ok((mut socket, _)) = listener.accept() {
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_millis(500)))
                        .unwrap();
                    let mut data = Vec::new();
                    let mut bytes = [0; 1024];
                    while !data.windows(4).any(|part| part == b"\r\n\r\n") {
                        match socket.read(&mut bytes) {
                            Ok(0) | Err(_) => break,
                            Ok(count) => data.extend_from_slice(&bytes[..count]),
                        }
                    }
                    let request = String::from_utf8_lossy(&data).to_string();
                    seen.lock().unwrap().push(request.clone());
                    let body = if request.starts_with("GET /takeover") {
                        owner.lock().unwrap().change_tab(1, Owner::Human).unwrap();
                        "<h1>Help</h1>"
                    } else if request.starts_with("GET /done") {
                        "<title>Welcome</title><h1>Welcome</h1>"
                    } else {
                        SIGNUP
                    };
                    let _ = socket.write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    );
                }
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            origin,
            authority,
            requests,
            stop,
            worker: Some(worker),
        }
    }
    /** A desktop runtime granted only this origin, sharing the site's authority. */
    fn runtime(&self) -> Runtime {
        let mut runtime = Runtime::desktop(
            EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
            vec![self.origin.clone()],
            self.authority.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        call(
            &mut runtime,
            "page.navigate",
            json!({"url":format!("{}/", self.origin)}),
        )
        .unwrap();
        runtime
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

/** Agent calls carry tab-1 and epoch 1, the agent's grant at startup. */
fn call(runtime: &mut Runtime, op: &str, params: Value) -> Result<Value, EngineError> {
    call_epoch(runtime, op, params, 1)
}

/** Callers can present a stale epoch to prove it is refused. */
fn call_epoch(
    runtime: &mut Runtime,
    op: &str,
    mut params: Value,
    epoch: u64,
) -> Result<Value, EngineError> {
    params["tab_id"] = json!("tab-1");
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(epoch);
    }
    runtime.handle_as(
        Request::new("agent-tools", op, params),
        Permit {
            owner: Owner::Agent,
            epoch,
            tab_id: Some(1),
        },
    )
}

/** Current page and revision, as an agent would read them once. */
fn head(runtime: &mut Runtime) -> (Value, Value) {
    let summary = call(runtime, "page.summary", json!({})).unwrap();
    (summary["page_id"].clone(), summary["revision"].clone())
}

/** Run a batch planned against the current revision. */
fn act(runtime: &mut Runtime, steps: Value, result: &str) -> Value {
    let (page_id, revision) = head(runtime);
    call(
        runtime,
        "page.act",
        json!({"page_id":page_id,"expected_revision":revision,"steps":steps,"result":result}),
    )
    .unwrap()
}

/** The control in a summary with this role. */
fn control<'a>(summary: &'a Value, role: &str) -> &'a Value {
    summary["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|control| control["role"] == role)
        .unwrap_or_else(|| panic!("no {role} in {summary}"))
}

/** Summaries outline headings, landmarks and numbered controls, stable for one revision. */
#[test]
fn summary_outlines_headings_landmarks_and_numbered_controls() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let summary = call(&mut runtime, "page.summary", json!({})).unwrap();
    assert_eq!(summary["title"], "Signup");
    assert_eq!(summary["headings"][0]["text"], "Join");
    assert_eq!(summary["headings"][0]["children"][0]["text"], "Account");
    assert_eq!(
        summary["headings"][0]["children"][0]["children"][0]["text"],
        "Details"
    );
    let landmarks = summary["landmarks"].as_array().unwrap();
    for role in ["banner", "navigation", "main", "form"] {
        assert!(
            landmarks.iter().any(|landmark| landmark["role"] == role),
            "{role}"
        );
    }
    let roles = summary["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|control| control["role"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        [
            "link", "textbox", "password", "checkbox", "combobox", "button"
        ]
    );
    for (index, control) in summary["controls"].as_array().unwrap().iter().enumerate() {
        assert_eq!(control["n"], index + 1);
    }
    assert_eq!(control(&summary, "combobox")["value"], "Free");
    assert_eq!(control(&summary, "combobox")["options"][1]["text"], "Pro");
    assert_eq!(control(&summary, "checkbox")["checked"], false);
    assert_eq!(
        call(&mut runtime, "page.summary", json!({})).unwrap(),
        summary
    );
    let inspect = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(summary.to_string().len() * 3 < inspect.to_string().len());
}

/** One batch fills, selects, checks and submits a form; the server sees every value. */
#[test]
fn act_fills_and_submits_a_form_in_one_call() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let result = act(
        &mut runtime,
        json!([
            {"action":"type","find":{"role":"textbox","label":"Full name"},"text":"Ada"},
            {"action":"type","find":{"role":"password"},"text":SECRET},
            {"action":"set_checked","find":{"role":"checkbox","name":"Newsletter"},"checked":true},
            {"action":"select","find":{"role":"combobox"},"option":"pro"},
            {"action":"click","find":{"role":"button","name":"Create account"}}
        ]),
        "summary",
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["completed"], 5);
    let revisions = result["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["revision"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert!(revisions[..4].windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(result["summary"]["title"], "Welcome");
    let sent = site.requests.lock().unwrap().last().unwrap().clone();
    assert!(sent.contains("name=Ada") && sent.contains("plan=Pro") && sent.contains("news="));
    assert!(!result.to_string().contains(SECRET));
}

/** The first failing step stops the batch and earlier steps stay committed. */
#[test]
fn act_stops_at_the_first_failing_step() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let result = act(
        &mut runtime,
        json!([
            {"action":"type","find":{"role":"textbox"},"text":"Ada"},
            {"action":"click","find":{"role":"button","name":"No such button"}},
            {"action":"type","find":{"role":"textbox"},"text":"Never"}
        ]),
        "summary",
    );
    assert_eq!(result["ok"], false);
    assert_eq!(result["completed"], 1);
    assert_eq!(result["steps"].as_array().unwrap().len(), 2);
    assert_eq!(result["steps"][1]["error"]["code"], "invalid_target");
    assert_eq!(control(&result["summary"], "textbox")["value"], "Ada");
}

/** A person taking the tab back mid-batch stops it before the next step runs. */
#[test]
fn human_takeover_mid_batch_stops_the_batch() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let result = act(
        &mut runtime,
        json!([
            {"action":"type","find":{"role":"textbox"},"text":"Ada"},
            {"action":"click","find":{"role":"link","name":"Help"}},
            {"action":"type","find":{"role":"textbox"},"text":"After takeover"}
        ]),
        "summary",
    );
    assert_eq!(result["ok"], false, "{result}");
    assert_eq!(result["completed"], 1);
    assert_eq!(result["steps"][1]["error"]["code"], "control_denied");
    assert_eq!(result["steps"].as_array().unwrap().len(), 2);
    assert_eq!(result["summary"]["title"], "Signup");
    assert_eq!(control(&result["summary"], "textbox")["value"], "Ada");
    let (page_id, revision) = head(&mut runtime);
    let again = call(
        &mut runtime,
        "page.act",
        json!({"page_id":page_id,"expected_revision":revision,"steps":[{"action":"type","find":{"role":"textbox"},"text":"x"}]}),
    );
    assert_eq!(again.unwrap_err().code, "control_denied");
}

/** A stale control epoch is refused before any step runs. */
#[test]
fn act_with_a_stale_epoch_is_refused() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let (page_id, revision) = head(&mut runtime);
    let error = call_epoch(
        &mut runtime,
        "page.act",
        json!({"page_id":page_id,"expected_revision":revision,"steps":[{"action":"type","find":{"role":"textbox"},"text":"x"}]}),
        2,
    )
    .unwrap_err();
    assert_eq!(error.code, "control_denied");
    let summary = call(&mut runtime, "page.summary", json!({})).unwrap();
    assert_eq!(summary["revision"], revision);
}

/** Batches are bounded, planned against the current revision and target one node per step. */
#[test]
fn act_bounds_stale_revisions_and_ambiguous_targets() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let (page_id, revision) = head(&mut runtime);
    let step = json!({"action":"scroll","y":0});
    let too_many = call(
        &mut runtime,
        "page.act",
        json!({"page_id":page_id,"expected_revision":revision,"steps":vec![step;17]}),
    );
    assert_eq!(too_many.unwrap_err().code, "limit_exceeded");
    let stale = call(
        &mut runtime,
        "page.act",
        json!({"page_id":page_id,"expected_revision":revision.as_u64().unwrap()+1,"steps":[{"action":"scroll","y":0}]}),
    );
    assert_eq!(stale.unwrap_err().code, "invalid_target");
    let result = act(
        &mut runtime,
        json!([{"action":"click","find":{"label":"a"}}]),
        "none",
    );
    assert_eq!(result["steps"][0]["error"]["code"], "ambiguous_target");
    assert!(result.get("summary").is_none());
}

/** Diffs report semantic changes since a retained revision and never a secret value. */
#[test]
fn diff_reports_changes_without_secrets() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let (_, start) = head(&mut runtime);
    let result = act(
        &mut runtime,
        json!([
            {"action":"type","find":{"role":"textbox"},"text":"Ada"},
            {"action":"type","find":{"role":"password"},"text":SECRET},
            {"action":"set_checked","find":{"role":"checkbox"},"checked":true}
        ]),
        "diff",
    );
    let changed = result["diff"]["changed"].as_array().unwrap();
    assert!(changed.iter().any(|node| node["changes"]["value"] == "Ada"));
    assert!(
        changed
            .iter()
            .any(|node| node["changes"]["checked"] == true)
    );
    assert!(!result.to_string().contains(SECRET));
    let diff = call(&mut runtime, "page.diff", json!({"since_revision":start})).unwrap();
    assert_eq!(diff["from_revision"], start);
    assert_eq!(diff["changed"], result["diff"]["changed"]);
    assert!(!diff.to_string().contains(SECRET));
    let missing = call(&mut runtime, "page.diff", json!({"since_revision":9999}));
    assert_eq!(missing.unwrap_err().code, "revision_unavailable");
}

/** Find returns node IDs and bounds by role, name, label or text, within its bound. */
#[test]
fn find_returns_identities_and_bounds() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let found = call(
        &mut runtime,
        "page.find",
        json!({"role":"button","name":"create ACCOUNT"}),
    )
    .unwrap();
    assert_eq!(found["total"], 1);
    assert!(
        found["matches"][0]["node_id"]
            .as_str()
            .unwrap()
            .contains(":node-")
    );
    assert!(found["matches"][0]["bounds"]["width"].as_f64().unwrap() > 0.0);
    let by_text = call(&mut runtime, "page.find", json!({"text":"deta"})).unwrap();
    assert!(by_text["matches"][0].to_string().contains("Details"));
    let bounded = call(&mut runtime, "page.find", json!({"label":"a","limit":1})).unwrap();
    assert_eq!(bounded["matches"].as_array().unwrap().len(), 1);
    assert!(bounded["total"].as_u64().unwrap() > 1);
    assert_eq!(
        call(&mut runtime, "page.find", json!({})).unwrap_err().code,
        "invalid_request"
    );
}

/** Wait conditions are evaluated against committed state, exactly one at a time. */
#[test]
fn wait_evaluates_each_condition_kind() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let (page_id, revision) = head(&mut runtime);
    let satisfied = |runtime: &mut Runtime, condition: Value| {
        call(runtime, "page.wait", condition).unwrap()["satisfied"].clone()
    };
    assert_eq!(satisfied(&mut runtime, json!({"text":"details"})), true);
    assert_eq!(satisfied(&mut runtime, json!({"text":"Welcome"})), false);
    assert_eq!(
        satisfied(
            &mut runtime,
            json!({"role":"button","name":"Create account"})
        ),
        true
    );
    assert_eq!(
        satisfied(
            &mut runtime,
            json!({"role":"button","name":"Create account","absent":true})
        ),
        false
    );
    assert_eq!(
        satisfied(&mut runtime, json!({"url":site.origin.as_str()})),
        true
    );
    assert_eq!(
        satisfied(&mut runtime, json!({"after_revision":revision})),
        false
    );
    assert_eq!(
        satisfied(&mut runtime, json!({"navigated_from":page_id})),
        false
    );
    act(
        &mut runtime,
        json!([{"action":"click","find":{"role":"button"}}]),
        "none",
    );
    assert_eq!(
        satisfied(&mut runtime, json!({"navigated_from":page_id})),
        true
    );
    assert_eq!(satisfied(&mut runtime, json!({"text":"Welcome"})), true);
    for invalid in [
        json!({}),
        json!({"text":"a","url":"b"}),
        json!({"absent":true}),
        json!({"text":"a","timeout_ms":20001}),
    ] {
        assert!(call(&mut runtime, "page.wait", invalid).is_err());
    }
}

/** Inspect options project fields, filter to the viewport and omit defaults; plain inspect is unchanged. */
#[test]
fn inspect_options_make_output_cheaper() {
    let site = Site::start();
    let mut runtime = site.runtime();
    let full = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(full["nodes"][0].get("checked").is_some());
    let projected = call(
        &mut runtime,
        "page.inspect",
        json!({"fields":["role","name"],"visible_only":true}),
    )
    .unwrap();
    for node in projected["nodes"].as_array().unwrap() {
        let keys = node
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            keys.iter()
                .all(|key| ["node_id", "role", "name"].contains(&key.as_str()))
        );
    }
    let compact = call(&mut runtime, "page.inspect", json!({"compact":true})).unwrap();
    for node in compact["nodes"].as_array().unwrap() {
        assert!(
            node.as_object()
                .unwrap()
                .values()
                .all(|value| value != &json!(false)
                    || node["enabled"] == json!(false)
                    || node["visible"] == json!(false))
        );
    }
    assert!(compact.to_string().len() < full.to_string().len());
    assert!(projected.to_string().len() < compact.to_string().len());
}

/** The headless NDJSON runtime answers the same tools; its pages are settled after every command. */
#[test]
fn headless_runtime_serves_summary_find_and_wait() {
    let fixture = support::Fixture::routes(vec![(
        "/",
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{SIGNUP}",
            SIGNUP.len()
        )
        .into_bytes(),
    )]);
    let mut runtime =
        Runtime::start(EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars")))).unwrap();
    let mut call = |op: &str, params: Value| runtime.handle(Request::new("headless", op, params));
    call("page.navigate", json!({"url":format!("{}/", fixture.url)})).unwrap();
    let summary = call("page.summary", json!({})).unwrap();
    assert_eq!(summary["controls"].as_array().unwrap().len(), 6);
    let (page_id, revision) = (summary["page_id"].clone(), summary["revision"].clone());
    let result = call(
        "page.act",
        json!({"page_id":page_id,"expected_revision":revision,"steps":[{"action":"type","find":{"role":"textbox"},"text":"Ada"}],"result":"diff"}),
    )
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(
        call("page.wait", json!({"after_revision":revision})).unwrap()["satisfied"],
        true
    );
    assert!(call("runtime.capabilities", json!({})).unwrap()["operations"]["page.act"].is_object());
}
