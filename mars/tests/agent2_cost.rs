/**
 * Round trips and response bytes for three scripted agent tasks, done two ways through the real
 * Runtime: the inspect loop (inspect, act, inspect again) and the agent tools (summary, find,
 * act, press). Run with --nocapture to print the table; the test pins the advantage.
 */
use mars::{
    protocol::Request,
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
    sync::{Arc, atomic::AtomicBool},
    thread,
};

/** A sign-up page with a realistic amount of surrounding content. */
fn page() -> String {
    let filler: String = (1..=40)
        .map(|n| format!("<p>Paragraph {n} explains a plan feature in a sentence or two of ordinary marketing copy.</p>"))
        .collect();
    let links: String = (1..=30)
        .map(|n| format!("<li><a href=\"/doc/{n}\">Guide {n}</a></li>"))
        .collect();
    format!(
        r#"<title>Signup</title><header><h1>Join</h1><nav><ul>{links}</ul></nav></header><main><h2>Account</h2>
<form action="/done"><label for="name">Full name</label><input id="name" name="name">
<label for="pw">Password</label><input id="pw" name="pw" type="password">
<label for="news">Newsletter</label><input id="news" name="news" type="checkbox">
<button type="submit">Create account</button></form>
<form action="/search"><input id="q" name="q" aria-label="Search"><button>Search</button></form>
<a href="/pricing">Pricing</a>{filler}</main>"#
    )
}

/** Serve the page, /done, /search and /pricing on loopback. */
fn site() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let body = page();
    thread::spawn(move || {
        for mut socket in listener.incoming().flatten() {
            let mut data = [0; 2048];
            let count = socket.read(&mut data).unwrap_or(0);
            let request = String::from_utf8_lossy(&data[..count]).to_string();
            let reply = if request.starts_with("GET /done") {
                "<title>Welcome</title><h1>Welcome</h1>".to_owned()
            } else if request.starts_with("GET /search") {
                "<title>Results</title><h1>Results</h1>".to_owned()
            } else if request.starts_with("GET /pricing") {
                "<title>Pricing</title><h1>Pricing</h1>".to_owned()
            } else {
                body.clone()
            };
            let _ = socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    origin
}

/** Counts calls and response bytes for one approach. */
struct Meter {
    runtime: Runtime,
    calls: usize,
    bytes: usize,
}

impl Meter {
    /** A desktop runtime with tab-1 granted, already on the page (not counted). */
    fn start(origin: &str) -> Self {
        let runtime = Runtime::desktop(
            EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
            vec![origin.to_owned()],
            Authority::shared(Owner::Agent),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let mut meter = Self {
            runtime,
            calls: 0,
            bytes: 0,
        };
        meter.call("page.navigate", json!({"url":format!("{origin}/")}));
        (meter.calls, meter.bytes) = (0, 0);
        meter
    }
    /** One agent call; its serialized result is what the model would read. */
    fn call(&mut self, op: &str, mut params: Value) -> Value {
        params["tab_id"] = json!("tab-1");
        if mars::runtime::mutating(op) {
            params["control_epoch"] = json!(1);
        }
        let permit = Permit {
            owner: Owner::Agent,
            epoch: 1,
            tab_id: Some(1),
        };
        let result = self
            .runtime
            .handle_as(Request::new("cost", op, params), permit)
            .unwrap();
        self.calls += 1;
        self.bytes += result.to_string().len();
        result
    }
    /** The inspect loop's node lookup by accessible name. */
    fn node(inspection: &Value, name: &str) -> Value {
        inspection["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| {
                node["name"] == name
                    && ["input", "button", "a"].contains(&node["tag"].as_str().unwrap_or_default())
            })
            .unwrap()["node_id"]
            .clone()
    }
    /** Page and revision from an inspection or summary. */
    fn at(value: &Value) -> Value {
        json!({"page_id":value["page_id"],"expected_revision":value["revision"]})
    }
    /** Merge target fields into a mutation. */
    fn with(mut base: Value, extra: Value) -> Value {
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        base
    }
}

/** Sign-up, inspect loop: inspect, then act and re-inspect for each of four controls. */
fn signup_inspect(meter: &mut Meter) {
    let mut page = meter.call("page.inspect", json!({}));
    for (op, name, extra) in [
        ("page.type", "Full name", json!({"text":"Ada Lovelace"})),
        ("page.type", "Password", json!({"text":"s3cret-pass"})),
        ("page.set_checked", "Newsletter", json!({"checked":true})),
        ("page.submit", "Create account", json!({})),
    ] {
        let node = Meter::node(&page, name);
        meter.call(
            op,
            Meter::with(
                Meter::at(&page),
                Meter::with(json!({"node_id":node}), extra),
            ),
        );
        page = meter.call("page.inspect", json!({}));
    }
    assert_eq!(page["title"], "Welcome");
}

/** Sign-up, agent tools: one summary, one batch that returns the next page's summary. */
fn signup_tools(meter: &mut Meter) {
    let summary = meter.call("page.summary", json!({}));
    let done = meter.call(
        "page.act",
        Meter::with(
            Meter::at(&summary),
            json!({"steps":[
        {"action":"type","find":{"role":"textbox","name":"Full name"},"text":"Ada Lovelace"},
        {"action":"type","find":{"role":"password","name":"Password"},"text":"s3cret-pass"},
        {"action":"set_checked","find":{"role":"checkbox","name":"Newsletter"},"checked":true},
        {"action":"submit","find":{"role":"button","name":"Create account"}}]}),
        ),
    );
    assert_eq!(done["summary"]["title"], "Welcome");
}

/** Search, inspect loop: inspect, type, inspect, submit, inspect. */
fn search_inspect(meter: &mut Meter) {
    let page = meter.call("page.inspect", json!({}));
    let node = Meter::node(&page, "Search");
    meter.call(
        "page.type",
        Meter::with(Meter::at(&page), json!({"node_id":node,"text":"pricing"})),
    );
    let page = meter.call("page.inspect", json!({}));
    meter.call(
        "page.submit",
        Meter::with(Meter::at(&page), json!({"node_id":node})),
    );
    let page = meter.call("page.inspect", json!({}));
    assert_eq!(page["title"], "Results");
}

/** Search, agent tools: find the box, type with a key press (its reply carries the new revision), press Enter. */
fn search_tools(meter: &mut Meter) {
    let found = meter.call(
        "page.find",
        json!({"name":"Search","role":"textbox","limit":1}),
    );
    let node = found["matches"][0]["node_id"].clone();
    let at = Meter::at(&found);
    let typed = meter.call(
        "page.press",
        Meter::with(at, json!({"node_id":node,"text":"pricing"})),
    );
    let page = json!({"page_id":found["page_id"],"revision":typed["revision"]});
    let done = meter.call(
        "page.press",
        Meter::with(Meter::at(&page), json!({"node_id":node,"key":"Enter"})),
    );
    assert_eq!(done["title"], "Results");
}

/** Follow a link, inspect loop: inspect, click, inspect. */
fn link_inspect(meter: &mut Meter) {
    let page = meter.call("page.inspect", json!({}));
    let node = Meter::node(&page, "Pricing");
    meter.call(
        "page.click",
        Meter::with(Meter::at(&page), json!({"node_id":node})),
    );
    let page = meter.call("page.inspect", json!({}));
    assert_eq!(page["title"], "Pricing");
}

/** Follow a link, agent tools: one batch with a find target, summary of the result. */
fn link_tools(meter: &mut Meter) {
    let page = meter.call("page.wait", json!({"after_revision":0}));
    let done = meter.call(
        "page.act",
        Meter::with(
            Meter::at(&page),
            json!({"steps":[{"action":"click","find":{"role":"link","name":"Pricing"}}]}),
        ),
    );
    assert_eq!(done["summary"]["title"], "Pricing");
}

/** One way of doing a task against a fresh runtime. */
type Task = fn(&mut Meter);

/** Agent tools use about half the calls (or fewer) and under a tenth of the bytes on every task. */
#[test]
fn agent_tools_cost_a_fraction_of_the_inspect_loop() {
    let origin = site();
    let tasks: [(&str, Task, Task); 3] = [
        ("sign-up form", signup_inspect, signup_tools),
        ("search and submit", search_inspect, search_tools),
        ("follow a link", link_inspect, link_tools),
    ];
    eprintln!("| task | inspect loop calls | bytes | agent tools calls | bytes | byte ratio |");
    for (name, inspect, tools) in tasks {
        let mut slow = Meter::start(&origin);
        inspect(&mut slow);
        let mut fast = Meter::start(&origin);
        tools(&mut fast);
        let ratio = slow.bytes as f64 / fast.bytes as f64;
        eprintln!(
            "| {name} | {} | {} | {} | {} | {ratio:.1}x |",
            slow.calls, slow.bytes, fast.calls, fast.bytes
        );
        assert!(
            fast.calls * 2 <= slow.calls + 1,
            "{name}: {} vs {} calls",
            fast.calls,
            slow.calls
        );
        assert!(ratio >= 10.0, "{name}: {ratio:.1}x");
    }
}
