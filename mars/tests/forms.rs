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

/** A page whose every path serves the same markup, so each pointer test names its own geometry. */
fn serve(page: &'static str) -> Fixture {
    Fixture::start(move |_| reply("200 OK", "text/html", "", page))
}
/** The inspected node with this HTML id, as an agent reads it from page.inspect. */
fn node<'a>(page: &'a Value, html_id: &str) -> &'a Value {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["html_id"] == html_id)
        .unwrap_or_else(|| panic!("no #{html_id} on the page: {page}"))
}
/** The centre of a node's painted box, in viewport points at the top of the page. */
fn centre(page: &Value, html_id: &str) -> (f64, f64) {
    let bounds = &node(page, html_id)["bounds"];
    (
        bounds["x"].as_f64().unwrap() + bounds["width"].as_f64().unwrap() / 2.0,
        bounds["y"].as_f64().unwrap() + bounds["height"].as_f64().unwrap() / 2.0,
    )
}
/** A click at a viewport point on the page as it is now. */
fn click_at(runtime: &mut Runtime, page: &Value, (x, y): (f64, f64)) -> Result<Value, EngineError> {
    call(
        runtime,
        "page.click_at",
        json!({"page_id":page["page_id"],"expected_revision":page["revision"],"x":x,"y":y}),
    )
}

/** A cover with pointer-events:none lets a click reach the field painted beneath it, as in Chrome. */
#[test]
fn pointer_events_none_cover_lets_the_click_reach_the_field() {
    let fixture = serve(
        r#"<html><body style="margin:0"><label id="lab" style="position:absolute;left:10px;top:10px;width:200px;height:40px"><span id="cap">Where</span><input id="where" style="margin-left:60px"></label><div id="cover" style="position:absolute;left:10px;top:10px;width:200px;height:40px;pointer-events:none"></div></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let at = click_at(&mut runtime, &page, centre(&page, "cap")).unwrap();
    assert_eq!(at["focused"], node(&page, "where")["node_id"], "{at}");
}

/** A pointer-events:auto child of a pointer-events:none parent is the click's target. */
#[test]
fn pointer_events_auto_child_is_hit_under_a_pointer_events_none_parent() {
    let fixture = serve(
        r#"<html><body style="margin:0"><div id="outer" style="pointer-events:none;position:absolute;left:0;top:0;width:300px;height:100px"><a id="inner" href="/hit" style="pointer-events:auto;position:absolute;left:10px;top:10px;width:100px;height:30px">Go</a></div></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let result = click_at(&mut runtime, &page, centre(&page, "inner")).unwrap();
    assert!(
        result["url"].as_str().unwrap().ends_with("/hit"),
        "{result}"
    );
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let empty = click_at(&mut runtime, &page, (250.0, 90.0));
    assert_eq!(empty.unwrap_err().code, "invalid_target");
}

/** A click on a label focuses the input it names. */
#[test]
fn a_click_on_a_label_focuses_its_input() {
    let fixture = serve(
        r#"<html><body><label id="lab" for="name">Your name</label> <input id="name"></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let at = click_at(&mut runtime, &page, centre(&page, "lab")).unwrap();
    assert_eq!(at["focused"], node(&page, "name")["node_id"], "{at}");
}

/** A click on a span inside a label activates the label's checkbox. */
#[test]
fn a_click_on_a_span_in_a_label_activates_its_control() {
    let fixture = serve(
        r#"<html><body><label id="lab"><span id="text">Get news</span> <input id="news" type="checkbox"></label></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(node(&page, "news")["checked"], false);
    click_at(&mut runtime, &page, centre(&page, "text")).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(node(&page, "news")["checked"], true);
}

/** A plain div with only a script listener is a click target, and the listener runs. */
#[test]
fn a_div_with_only_a_click_listener_receives_the_click() {
    let fixture = serve(
        r#"<html><body><div id="d" style="width:200px;height:40px;background:#eee">Tap me</div><p id="log">none</p><script>document.getElementById('d').addEventListener('click',function(){document.getElementById('log').textContent='div'})</script></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    click_at(&mut runtime, &page, centre(&page, "d")).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(node(&page, "log")["text"], "div");
}

/** A point on a plain div nothing listens to reports the hit with no effect, rather than refusing. */
#[test]
fn a_click_on_a_plain_div_reports_the_hit_without_effect() {
    let fixture = serve(
        r#"<html><body><div id="plain" style="width:200px;height:40px">Plain</div></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let at = click_at(&mut runtime, &page, centre(&page, "plain")).unwrap();
    assert_eq!(at["activated"], false, "{at}");
    assert_eq!(
        at["hit"]["node_id"],
        node(&page, "plain")["node_id"],
        "{at}"
    );
}

/** A visibility:hidden box is never hit; the click reaches what is visible beneath it. */
#[test]
fn visibility_hidden_boxes_are_not_hit() {
    let fixture = serve(
        r#"<html><body style="margin:0"><a id="under" href="/under" style="position:absolute;left:0;top:0;width:200px;height:100px">Under</a><div id="ghost" style="position:absolute;left:0;top:0;width:200px;height:100px;visibility:hidden"><a href="/ghost">Ghost</a></div></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let result = click_at(&mut runtime, &page, (50.0, 50.0)).unwrap();
    assert!(
        result["url"].as_str().unwrap().ends_with("/under"),
        "{result}"
    );
}

/** An inert subtree is never hit; the click reaches the link beneath it. */
#[test]
fn inert_subtrees_are_not_hit() {
    let fixture = serve(
        r#"<html><body style="margin:0"><a id="under" href="/under" style="position:absolute;left:0;top:0;width:200px;height:100px">Under</a><div id="sealed" inert style="position:absolute;left:0;top:0;width:200px;height:100px"><a href="/sealed">Sealed</a></div></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let result = click_at(&mut runtime, &page, (50.0, 50.0)).unwrap();
    assert!(
        result["url"].as_str().unwrap().ends_with("/under"),
        "{result}"
    );
}

/** A transparent field painted over its visible label takes the click, as in Chrome: it is focused. */
#[test]
fn a_transparent_field_over_its_label_takes_the_click() {
    let fixture = serve(
        r#"<html><body style="margin:0"><label id="lab" style="position:absolute;left:10px;top:10px;width:200px;height:40px"><span id="cap">Where</span></label><input id="where" style="position:absolute;left:10px;top:10px;width:200px;height:40px;opacity:0;border:0"></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let at = click_at(&mut runtime, &page, centre(&page, "cap")).unwrap();
    assert_eq!(at["focused"], node(&page, "where")["node_id"], "{at}");
}

/** An invisible link laid over a visible button does not take an agent's click: the agent acts on
what it is shown, so a transparent overlay is refused and the page is left unchanged. */
#[test]
fn an_invisible_overlay_does_not_take_an_agents_click() {
    let fixture = serve(
        r#"<html><body style="margin:0"><button id="ok" style="position:absolute;left:10px;top:10px;width:200px;height:40px">Cancel</button><a id="trap" href="/trap" style="position:absolute;left:10px;top:10px;width:200px;height:40px;opacity:0">x</a></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    let result = click_at(&mut runtime, &page, centre(&page, "ok"));
    assert_eq!(result.unwrap_err().code, "invalid_target");
    let after = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert!(!after["url"].as_str().unwrap().ends_with("/trap"));
}

/** A label naming a disabled field does not focus it. */
#[test]
fn a_label_for_a_disabled_field_does_not_focus_it() {
    let fixture = serve(
        r#"<html><body><label id="lab" for="name">Your name</label> <input id="name" disabled></body></html>"#,
    );
    let mut runtime = fixture.runtime();
    call(&mut runtime, "page.navigate", json!({"url":fixture.origin})).unwrap();
    let page = call(&mut runtime, "page.inspect", json!({})).unwrap();
    // Refusing the click or answering without a focus both keep the disabled field untouched.
    let focused = click_at(&mut runtime, &page, centre(&page, "lab"))
        .map(|at| at["focused"].clone())
        .unwrap_or(serde_json::Value::Null);
    assert!(focused.is_null(), "{focused}");
}
