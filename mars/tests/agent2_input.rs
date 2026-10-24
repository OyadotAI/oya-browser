/** Agent lanes, own tabs, pointer and keyboard input, screenshots and traces, through the real Runtime. */
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
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/** A value the site sets as a cookie; no agent view may contain it. */
const COOKIE: &str = "session-cookie-value-7f3a";
/** A typed password; no agent view may contain it. */
const SECRET: &str = "hunter2-agent2-secret";

/** A form with a text field, a password, a checkbox, a link and a submit button. */
const FORM: &str = r#"<html><head><title>Form</title></head><body>
<h1>Sign up</h1>
<form action="/done" method="get">
<label for="name">Name</label><input id="name" name="name">
<label for="pw">Password</label><input id="pw" name="pw" type="password">
<label for="news">News</label><input id="news" name="news" type="checkbox">
<button type="submit">Join</button>
</form>
<a href="/done">Skip</a>
<p>One</p><p>Two</p><p>Three</p><p>Four</p><p>Five</p><p>Six</p><p>Seven</p><p>Eight</p>
<p>Nine</p><p>Ten</p><p>Eleven</p><p>Twelve</p><p>Thirteen</p><p>Fourteen</p><p>Fifteen</p>
<p>Sixteen</p><p>Seventeen</p><p>Eighteen</p><p>Nineteen</p><p>Twenty</p><p>End</p>
</body></html>"#;

/** One hermetic loopback site. */
struct Site {
    origin: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Site {
    /** Serve the form at every path except /done. */
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::spawn(move || {
            while !signal.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
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
                let request = String::from_utf8_lossy(&data);
                let body = if request.starts_with("GET /done") {
                    "<title>Welcome</title><h1>Welcome</h1>"
                } else {
                    FORM
                };
                let _ = socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nSet-Cookie: sid={COOKIE}; Path=/\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        Self {
            origin,
            stop,
            worker: Some(worker),
        }
    }
    /** A desktop runtime granted this origin; `owner` holds tab-1 at startup. */
    fn runtime(&self, owner: Owner) -> (Runtime, SharedAuthority) {
        let authority = Authority::shared(owner);
        let runtime = Runtime::desktop(
            EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
            vec![self.origin.clone()],
            authority.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        (runtime, authority)
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

/** An agent call in `lane` on `tab`, observing the tab's current epoch as the front door does. */
fn agent(
    runtime: &mut Runtime,
    authority: &SharedAuthority,
    lane: &str,
    tab: Option<u64>,
    op: &str,
    mut params: Value,
) -> Result<Value, EngineError> {
    let permit = {
        let state = authority.lock().unwrap();
        match tab {
            Some(id) => state.observe_for(Owner::Agent, id).unwrap_or(Permit {
                owner: Owner::Agent,
                epoch: state.epoch,
                tab_id: Some(id),
            }),
            None => Permit {
                owner: Owner::Agent,
                epoch: state.epoch,
                tab_id: None,
            },
        }
    };
    params["lane"] = json!(lane);
    if let Some(id) = tab {
        params["tab_id"] = json!(format!("tab-{id}"));
        if mars::runtime::mutating(op) && params.get("control_epoch").is_none() {
            params["control_epoch"] = json!(permit.epoch);
        }
    }
    runtime.handle_as(Request::new("agent2", op, params), permit)
}

/** Open an agent tab on the form; returns its numeric id. */
fn open_form(runtime: &mut Runtime, authority: &SharedAuthority, site: &Site, lane: &str) -> u64 {
    let opened = agent(
        runtime,
        authority,
        lane,
        None,
        "tab.open",
        json!({"url":format!("{}/", site.origin)}),
    )
    .unwrap();
    assert!(opened.get("navigation_error").is_none(), "{opened}");
    opened["tab_id"]
        .as_str()
        .unwrap()
        .trim_start_matches("tab-")
        .parse()
        .unwrap()
}

/** The summary control with this role. */
fn control(summary: &Value, role: &str) -> Value {
    summary["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["role"] == role)
        .cloned()
        .unwrap_or_else(|| panic!("no {role}"))
}

/** Page identity and revision for a mutation. */
fn head(runtime: &mut Runtime, authority: &SharedAuthority, lane: &str, tab: u64) -> Value {
    let summary = agent(
        runtime,
        authority,
        lane,
        Some(tab),
        "page.summary",
        json!({}),
    )
    .unwrap();
    json!({"page_id":summary["page_id"],"expected_revision":summary["revision"],"summary":summary})
}

/** The viewport centre of a node, from page.find bounds and the current scroll. */
fn centre(
    runtime: &mut Runtime,
    authority: &SharedAuthority,
    lane: &str,
    tab: u64,
    query: Value,
) -> (f64, f64) {
    let found = agent(runtime, authority, lane, Some(tab), "page.find", query).unwrap();
    let bounds = &found["matches"][0]["bounds"];
    let layout = agent(
        runtime,
        authority,
        lane,
        Some(tab),
        "page.inspect",
        json!({}),
    )
    .unwrap();
    let scroll = layout["viewport"]["scroll_y"].as_f64().unwrap_or_default();
    (
        bounds["x"].as_f64().unwrap() + 4.0,
        bounds["y"].as_f64().unwrap() - scroll + bounds["height"].as_f64().unwrap() / 2.0,
    )
}

/** An agent-opened tab starts under the agent's lane and leaves the person's selection alone. */
#[test]
fn agent_tab_opens_in_background_under_its_lane() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let tabs = agent(&mut runtime, &authority, "a", None, "tab.list", json!({})).unwrap();
    assert_eq!(tabs["selected_tab_id"], "tab-1");
    assert_eq!(
        authority.lock().unwrap().tab_status(tab).unwrap().0,
        Owner::Agent
    );
    assert_eq!(
        authority.lock().unwrap().tab_status(1).unwrap().0,
        Owner::Human
    );
    let summary = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.summary",
        json!({}),
    )
    .unwrap();
    assert_eq!(summary["title"], "Form");
}

/** A second agent can neither act on nor close the first agent's tab. */
#[test]
fn lanes_never_act_on_each_others_tabs() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let at = head(&mut runtime, &authority, "a", tab);
    let name = control(&at["summary"], "textbox")["node_id"].clone();
    let typed = agent(
        &mut runtime,
        &authority,
        "b",
        Some(tab),
        "page.type",
        json!({"page_id":at["page_id"],"expected_revision":at["expected_revision"],"node_id":name,"text":"intruder"}),
    );
    assert_eq!(typed.unwrap_err().code, "control_denied");
    let closed = agent(
        &mut runtime,
        &authority,
        "b",
        Some(tab),
        "tab.close",
        json!({}),
    );
    assert_eq!(closed.unwrap_err().code, "control_denied");
    agent(&mut runtime, &authority, "a", Some(tab), "page.type", json!({"page_id":at["page_id"],"expected_revision":at["expected_revision"],"node_id":name,"text":"owner"})).unwrap();
    let closed = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "tab.close",
        json!({}),
    )
    .unwrap();
    assert_eq!(closed["closed"], format!("tab-{tab}"));
}

/** An agent closes only tabs it opened, never a tab the person granted it. */
#[test]
fn agents_close_only_tabs_they_opened() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Agent);
    let closed = agent(
        &mut runtime,
        &authority,
        "a",
        Some(1),
        "tab.close",
        json!({}),
    );
    assert_eq!(closed.unwrap_err().code, "control_denied");
}

/** The first lane to act on a granted tab holds it; leaving hands it back to the person. */
#[test]
fn granted_tab_belongs_to_first_lane_until_it_leaves() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Agent);
    let url = json!({"url":format!("{}/", site.origin)});
    agent(
        &mut runtime,
        &authority,
        "a",
        Some(1),
        "page.navigate",
        url.clone(),
    )
    .unwrap();
    let denied = agent(
        &mut runtime,
        &authority,
        "b",
        Some(1),
        "page.navigate",
        url.clone(),
    );
    assert_eq!(denied.unwrap_err().code, "control_denied");
    authority.lock().unwrap().revoke_lane("a");
    assert_eq!(
        authority.lock().unwrap().tab_status(1).unwrap().0,
        Owner::Human
    );
    let after = agent(&mut runtime, &authority, "a", Some(1), "page.navigate", url);
    assert_eq!(after.unwrap_err().code, "control_denied");
}

/** The person taking over an agent tab stops that agent at once. */
#[test]
fn human_takeover_of_agent_tab_denies_the_agent() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let stale = authority.lock().unwrap().tab_status(tab).unwrap().1;
    authority
        .lock()
        .unwrap()
        .change_tab(tab, Owner::Human)
        .unwrap();
    let denied = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.navigate",
        json!({"url":format!("{}/", site.origin),"control_epoch":stale}),
    );
    assert_eq!(denied.unwrap_err().code, "control_denied");
}

/** One lane cannot fill the tab strip. */
#[test]
fn lane_tab_count_is_bounded() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let mut last = Ok(json!({}));
    for _ in 0..9 {
        last = agent(&mut runtime, &authority, "a", None, "tab.open", json!({}));
    }
    assert_eq!(last.unwrap_err().code, "limit_exceeded");
    agent(&mut runtime, &authority, "b", None, "tab.open", json!({})).unwrap();
}

/** Clicking at a point activates what is painted there and reports the hit. */
#[test]
fn click_at_activates_the_node_under_the_point() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let (x, y) = centre(
        &mut runtime,
        &authority,
        "a",
        tab,
        json!({"role":"link","name":"Skip"}),
    );
    let hover = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.hover",
        json!({"x":x,"y":y}),
    )
    .unwrap();
    assert!(
        hover["clickable"].as_str().unwrap().contains("node-"),
        "{hover}"
    );
    let at = head(&mut runtime, &authority, "a", tab);
    let clicked = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.click_at",
        json!({"page_id":at["page_id"],"expected_revision":at["expected_revision"],"x":x,"y":y}),
    )
    .unwrap();
    assert_eq!(clicked["title"], "Welcome");
    let outside = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.hover",
        json!({"x":-1.0,"y":5.0}),
    );
    assert_eq!(outside.unwrap_err().code, "invalid_target");
}

/** Keys map to native operations: text, Backspace, Space toggles, Enter submits; secrets stay out. */
#[test]
fn keys_type_toggle_erase_and_submit() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let press = |runtime: &mut Runtime, role: &str, extra: Value| {
        let at = head(runtime, &authority, "a", tab);
        let mut params = json!({"page_id":at["page_id"],"expected_revision":at["expected_revision"],"node_id":control(&at["summary"], role)["node_id"]});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        agent(runtime, &authority, "a", Some(tab), "page.press", params)
    };
    press(&mut runtime, "textbox", json!({"text":"Ada"})).unwrap();
    press(&mut runtime, "textbox", json!({"key":"!"})).unwrap();
    press(&mut runtime, "textbox", json!({"key":"Backspace"})).unwrap();
    let typed = press(&mut runtime, "password", json!({"text":SECRET})).unwrap();
    assert!(!typed.to_string().contains(SECRET));
    press(&mut runtime, "checkbox", json!({"key":"Space"})).unwrap();
    let at = head(&mut runtime, &authority, "a", tab);
    assert_eq!(control(&at["summary"], "textbox")["value"], "Ada");
    assert_eq!(control(&at["summary"], "checkbox")["checked"], true);
    assert!(!at.to_string().contains(SECRET));
    let unsupported = press(&mut runtime, "textbox", json!({"key":"Escape"}));
    assert_eq!(unsupported.unwrap_err().code, "unsupported_operation");
    let submitted = press(&mut runtime, "textbox", json!({"key":"Enter"})).unwrap();
    assert_eq!(submitted["title"], "Welcome");
}

/** PageDown and the wheel scroll the document; a stale revision is refused. */
#[test]
fn scrolling_keys_and_wheel_move_the_document() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let at = head(&mut runtime, &authority, "a", tab);
    let scrolled = agent(&mut runtime, &authority, "a", Some(tab), "page.press", json!({"page_id":at["page_id"],"expected_revision":at["expected_revision"],"key":"PageDown"})).unwrap();
    let down = scrolled["viewport"]["scroll_y"].as_f64().unwrap();
    assert!(down > 0.0, "{}", scrolled["viewport"]);
    let at = head(&mut runtime, &authority, "a", tab);
    let wheel = agent(&mut runtime, &authority, "a", Some(tab), "page.scroll_at", json!({"page_id":at["page_id"],"expected_revision":at["expected_revision"],"x":10.0,"y":10.0,"delta_y":-down})).unwrap();
    assert_eq!(wheel["viewport"]["scroll_y"].as_f64().unwrap(), 0.0);
    let stale = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.scroll_at",
        json!({"page_id":at["page_id"],"expected_revision":999,"x":10.0,"y":10.0,"delta_y":5.0}),
    );
    assert_eq!(stale.unwrap_err().code, "invalid_target");
}

/** Screenshots return a PNG of the viewport or one element, without granting control. */
#[test]
fn screenshot_returns_png_of_viewport_and_element() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let shot = agent(
        &mut runtime,
        &authority,
        "b",
        Some(tab),
        "page.screenshot",
        json!({}),
    )
    .unwrap();
    assert_eq!(shot["mime"], "image/png");
    assert!(
        shot["png_base64"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );
    assert!(shot["width"].as_u64().unwrap() >= 280);
    let found = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.find",
        json!({"role":"button"}),
    )
    .unwrap();
    let node = &found["matches"][0];
    let clip = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.screenshot",
        json!({"node_id":node["node_id"]}),
    )
    .unwrap();
    assert_eq!(
        clip["width"].as_f64().unwrap(),
        node["bounds"]["width"]
            .as_f64()
            .unwrap()
            .clamp(1.0, 2048.0)
            .trunc()
    );
}

/** Console and network views list the page's loads and messages, never cookies or values. */
#[test]
fn traces_list_loads_without_cookies_or_values() {
    let site = Site::start();
    let (mut runtime, authority) = site.runtime(Owner::Human);
    let tab = open_form(&mut runtime, &authority, &site, "a");
    let network = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.network",
        json!({}),
    )
    .unwrap();
    assert_eq!(network["entries"][0]["kind"], "document");
    assert_eq!(network["entries"][0]["method"], "GET");
    assert_eq!(network["entries"][0]["status"], 200);
    let console = agent(
        &mut runtime,
        &authority,
        "a",
        Some(tab),
        "page.console",
        json!({}),
    )
    .unwrap();
    assert!(console["entries"].is_array());
    for view in [network, console] {
        assert!(!view.to_string().contains(COOKIE), "{view}");
    }
}
