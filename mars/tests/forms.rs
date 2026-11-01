/** Actual engine processes submit to hermetic servers; assertions observe wire behavior. */
#[allow(dead_code)]
mod support;
use mars::{
    protocol::{EngineError, Request},
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit},
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

/** Test traffic is synthetic and isolated to an exact operator-granted origin. */
struct Fixture {
    origin: String,
    records: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Fixture {
    /** Each test owns its socket and response policy. */
    fn start(handler: impl Fn(&str) -> String + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let records = Arc::new(Mutex::new(Vec::new()));
        let observed = records.clone();
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Relaxed) {
                if let Ok((mut socket, _)) = listener.accept() {
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_millis(500)))
                        .unwrap();
                    let mut data = Vec::new();
                    let mut bytes = [0; 1024];
                    loop {
                        match socket.read(&mut bytes) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => data.extend_from_slice(&bytes[..n]),
                        }
                        if let Some(end) = data.windows(4).position(|p| p == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&data[..end]);
                            let length = header
                                .lines()
                                .find_map(|l| {
                                    l.strip_prefix("Content-Length: ")
                                        .and_then(|v| v.parse::<usize>().ok())
                                })
                                .unwrap_or_default();
                            if data.len() >= end + 4 + length {
                                break;
                            }
                        }
                        assert!(data.len() < 32768);
                    }
                    let request = String::from_utf8(data).unwrap();
                    observed.lock().unwrap().push(request.clone());
                    let _ = socket.write_all(handler(&request).as_bytes());
                }
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            origin,
            records,
            stop,
            worker: Some(worker),
        }
    }
    /** Public native operations use the real three child engines. */
    fn runtime(&self) -> Runtime {
        Runtime::desktop(
            EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
            vec![self.origin.clone()],
            Authority::shared(Owner::Agent),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
/** All requests retain the actual current page/revision/epoch contract. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Result<Value, EngineError> {
    if op.starts_with("page.") {
        params["tab_id"] = json!("tab-1");
    }
    if mars::runtime::mutating(op) {
        params["control_epoch"] = json!(1);
    }
    runtime.handle_as(
        Request::new("forms", op, params),
        Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        },
    )
}
/** The test discovers controls by label, just as an agent can. */
fn target(page: &Value, name: &str) -> Value {
    let node = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == name && n["tag"] != "label")
        .unwrap_or_else(|| panic!("Missing label {name}: {page}"));
    json!({"page_id":page["page_id"],"node_id":node["node_id"],"expected_revision":page["revision"]})
}
/** Fixed framing makes response behavior independent of an HTTP library. */
fn reply(status: &str, mime: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}; charset=utf-8\r\nContent-Length: {}\r\n{headers}\r\n{body}",
        body.len()
    )
}
/** The same mixed form is exercised by actual GUI tests and subprocess integration. */
fn site(request: &str) -> String {
    if request.starts_with("GET /forms.css ") {
        return reply(
            "200 OK",
            "text/css",
            "",
            include_str!("../fixtures/forms.css"),
        );
    }
    if request.starts_with("POST /login ") {
        return reply(
            "303 See Other",
            "text/html",
            "Location: /account\r\nSet-Cookie: session=active; Path=/; HttpOnly\r\nSet-Cookie: other=second; Path=/account\r\n",
            "",
        );
    }
    if request.starts_with("GET /account ") {
        return reply("200 OK", "text/html", "", "<h1>Account</h1>");
    }
    reply(
        "200 OK",
        "text/html",
        "",
        include_str!("../fixtures/forms.html"),
    )
}
/** Label entry, exact options, protected values and repeated cookies cross actual wire boundaries. */
#[test]
fn mixed_form_submits_exact_fields_and_keeps_the_session() {
    let fixture = Fixture::start(site);
    let mut runtime = fixture.runtime();
    let mut page = call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/forms",fixture.origin)}),
    )
    .unwrap();
    for (name, text) in [
        ("Username", "Rust 🐙"),
        ("Password", "secret-for-test"),
        ("Notes", "one\ntwo"),
    ] {
        let mut params = target(&page, name);
        params["text"] = json!(text);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    }
    let serialized = page.to_string();
    assert!(!serialized.contains("secret-for-test"));
    assert!(!serialized.contains("synthetic-demo-token"));
    let option = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["tag"] == "option" && n["text"] == "Canada")
        .unwrap()["node_id"]
        .clone();
    let mut params = target(&page, "Region");
    params["option_id"] = option;
    call(&mut runtime, "page.select", params).unwrap();
    page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let mut params = target(&page, "Plus");
    params["checked"] = json!(true);
    call(&mut runtime, "page.set_checked", params).unwrap();
    page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let hidden = page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["html_id"] == "concealed")
        .unwrap();
    assert_eq!(hidden["visible"], false);
    let mut params = target(&page, "Hidden action");
    assert_eq!(
        call(&mut runtime, "page.click", params.clone())
            .unwrap_err()
            .code,
        "invalid_target"
    );
    params["text"] = json!("bad");
    assert!(call(&mut runtime, "page.set_text", params).is_err());
    let result = call(
        &mut runtime,
        "page.click",
        target(&page, "Start demo session"),
    )
    .unwrap();
    assert!(result["url"].as_str().unwrap().ends_with("/account"));
    call(&mut runtime, "page.reload", json!({})).unwrap();
    let records = fixture.records.lock().unwrap();
    let posts = records
        .iter()
        .filter(|r| r.starts_with("POST "))
        .collect::<Vec<_>>();
    assert_eq!(posts.len(), 1);
    let (_, body) = posts[0].split_once("\r\n\r\n").unwrap();
    let fields = url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        [
            ("username", "Rust 🐙"),
            ("password", "secret-for-test"),
            ("region", "ca"),
            ("notes", "one\r\ntwo"),
            ("remember", "yes"),
            ("plan", "plus"),
            ("token", "synthetic-demo-token"),
            ("intent", "login")
        ]
        .map(|(k, v)| (k.into(), v.into()))
        .to_vec()
    );
    assert!(
        records
            .iter()
            .filter(|r| r.starts_with("GET /account "))
            .all(|r| r.contains("Cookie: other=second; session=active\r\n"))
    );
}
/** Direct POST validation documents display their status and cannot be refetched by Reload. */
#[test]
fn validation_html_is_rendered_and_post_history_never_replays() {
    let fixture = Fixture::start(|r| {
        if r.starts_with("POST ") {
            reply(
                "422 Unprocessable Content",
                "text/html",
                "",
                "<h1>Try another name</h1>",
            )
        } else {
            reply(
                "200 OK",
                "text/html",
                "",
                "<form method=post><input name=x value=one><button>Submit</button></form>",
            )
        }
    });
    let mut runtime = fixture.runtime();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let result = call(&mut runtime, "page.click", target(&page, "Submit")).unwrap();
    assert_eq!(result["status"], 422);
    assert_eq!(result["nodes"][0]["text"], "Try another name");
    assert_eq!(
        call(&mut runtime, "page.reload", json!({}))
            .unwrap_err()
            .code,
        "resubmission_required"
    );
    assert_eq!(fixture.records.lock().unwrap().len(), 2);
}
/** A submission the page allowed goes ahead even when the timers its settle runs come due after the submit event. */
#[test]
fn an_allowed_submit_survives_timers_that_run_after_it() {
    let fixture = Fixture::start(|r| {
        if r.starts_with("POST ") {
            reply("303 See Other", "text/html", "Location: /done\r\n", "")
        } else if r.starts_with("GET /done ") {
            reply("200 OK", "text/html", "", "<h1>Done</h1>")
        } else {
            reply(
                "200 OK",
                "text/html",
                "",
                "<form method=post><input name=x value=one><button>Submit</button></form><script>setInterval(function(){ document.title = 'tick' }, 0)</script>",
            )
        }
    });
    let mut runtime = fixture.runtime();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let result = call(&mut runtime, "page.click", target(&page, "Submit")).unwrap();
    assert!(
        result["url"].as_str().unwrap().ends_with("/done"),
        "{result}"
    );
    let records = fixture.records.lock().unwrap();
    assert_eq!(records.iter().filter(|r| r.starts_with("POST ")).count(), 1);
}
/** A protected GET keeps real form semantics while every browser URL projection drops its query. */
#[test]
fn password_get_redirects_redact_urls_and_acknowledgments() {
    let fixture = Fixture::start(|r| {
        if r.starts_with("GET /send?") {
            reply(
                "302 Found",
                "text/html",
                "Location: /done?password=synthetic-secret\r\n",
                "",
            )
        } else if r.starts_with("GET /done?") {
            reply("200 OK", "text/html", "", "<h1>Done</h1>")
        } else {
            reply(
                "200 OK",
                "text/html",
                "",
                "<form action=/send><label for=p>Password</label><input id=p type=password name=password><button>Go</button></form>",
            )
        }
    });
    let mut runtime = fixture.runtime();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let mut params = target(&page, "Password");
    params["text"] = json!("synthetic-secret");
    let ack = call(&mut runtime, "page.type", params).unwrap();
    assert!(!ack.to_string().contains("synthetic-secret"));
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let result = call(&mut runtime, "page.click", target(&page, "Go")).unwrap();
    assert!(result["url"].as_str().unwrap().ends_with("/done"));
    assert!(!result.to_string().contains("synthetic-secret"));
    call(&mut runtime, "page.reload", json!({})).unwrap();
    assert!(
        fixture
            .records
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.starts_with("GET /send?password=synthetic-secret "))
    );
}
/** Required validation never transmits and points to the associated protected field. */
#[test]
fn constraints_prevent_sending_and_hidden_controls_cannot_be_targeted() {
    let fixture = Fixture::start(|_| {
        reply(
            "200 OK",
            "text/html",
            "",
            "<form method=post><label for=p>Password</label><input id=p name=password type=password required><button>Go</button></form>",
        )
    });
    let mut runtime = fixture.runtime();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let error = call(&mut runtime, "page.click", target(&page, "Go")).unwrap_err();
    assert_eq!(error.code, "validation_error");
    assert!(error.invalid_node.is_some());
    assert_eq!(fixture.records.lock().unwrap().len(), 1);
}
/** Incomplete dispatched POSTs preserve the form, invalidate stale handles and never auto-replay. */
#[test]
fn an_uncertain_submission_is_not_replayed_by_a_queued_stale_action() {
    let fixture = Fixture::start(|r| {
        if r.starts_with("POST ") {
            String::new()
        } else {
            reply(
                "200 OK",
                "text/html",
                "",
                "<form method=post><input name=x value=one><button>Send</button></form>",
            )
        }
    });
    let mut runtime = fixture.runtime();
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let params = target(&page, "Send");
    let error = call(&mut runtime, "page.click", params.clone()).unwrap_err();
    assert_eq!(error.code, "submission_uncertain");
    assert!(error.previous_page_retained);
    assert_eq!(
        call(&mut runtime, "page.click", params).unwrap_err().code,
        "revision_conflict"
    );
    assert_eq!(
        fixture
            .records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("POST "))
            .count(),
        1
    );
}
