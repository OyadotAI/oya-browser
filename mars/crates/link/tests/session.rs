/**
 * The control-socket session, pairing and reconnect rules through mars-link's public surface,
 * with a recording fake in place of the Mars runtime and loopback sockets in place of the server.
 */
use mars_core::protocol::{EngineError, EngineResult, Request};
use mars_link::client::{is_final, reconnect_delay};
use mars_link::constants::{CLOSE_AUTH_TIMEOUT, CLOSE_REJECTED, CLOSE_REPLACED};
use mars_link::pairing::{claim_url, pairing_server};
use mars_link::profile::{SeedIdentity, generate};
use mars_link::session::Identity;
use mars_link::{Browser, CredentialStore, Event, MemoryCredentials, Session, pair, parse_link};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

/// Records every request and answers from canned results; can refuse writes like a person's tab.
#[derive(Default)]
struct Fake {
    requests: Vec<Request>,
    revoked: bool,
    refuse_writes: bool,
}

impl Browser for Fake {
    fn run(&mut self, request: Request) -> EngineResult<Value> {
        let op = request.op.clone();
        self.requests.push(request);
        match op.as_str() {
            "tab.list" => Ok(
                json!({"selected_tab_id": "tab-1", "tabs": [{"tab_id": "tab-1", "title": "A", "owner": "agent"}, {"tab_id": "tab-2", "title": "B", "owner": "human"}]}),
            ),
            "page.summary" => Ok(
                json!({"page_id": "pg", "revision": 3, "url": "https://a.test/", "title": "A", "headings": [{"level": 1, "text": "Hi"}], "controls": [{"n": 1, "node_id": "n-7", "role": "textbox", "name": "Email"}, {"n": 2, "node_id": "n-9", "role": "button", "name": "Go"}]}),
            ),
            _ if self.refuse_writes => {
                Err(EngineError::new("control_denied", "Control denied.", ""))
            }
            _ => Ok(json!({"url": "https://a.test/", "title": "A"})),
        }
    }
    fn screenshot(&mut self, _: &str) -> EngineResult<Vec<u8>> {
        Ok(vec![0x89, b'P', b'N', b'G'])
    }
    fn revoke_agents(&mut self) {
        self.revoked = true;
    }
}

/// A session for tests.
fn session() -> Session {
    Session::new(Identity {
        api_key: "k".into(),
        browser_name: "Mars".into(),
        persona: None,
        app_version: "1".into(),
        browser_id: Some("oya-1".into()),
    })
}

/// An auth_ok for a persona seeded `seed`.
fn auth_ok(seed: u32) -> Value {
    let fingerprint = generate(&SeedIdentity {
        id: "p".into(),
        seed,
        ..Default::default()
    });
    json!({"type": "auth_ok", "browser_id": "oya-1", "control": {"mode": "agent"}, "fingerprint": fingerprint})
}

/// A signed-in session and its fake browser.
fn signed_in() -> (Session, Fake) {
    let (mut s, mut b) = (session(), Fake::default());
    s.handle(&auth_ok(1), &mut b);
    (s, b)
}

/// The result one command produced.
fn cmd(s: &mut Session, b: &mut Fake, action: &str, params: Value) -> Value {
    let step = s.handle(
        &json!({"type": "cmd", "id": "c", "action": action, "params": params}),
        b,
    );
    step.replies[0].clone()
}

#[test]
fn auth_announces_the_browser_and_only_the_actions_it_answers() {
    let auth = session().auth_message();
    assert_eq!(
        (
            auth["type"].as_str(),
            auth["browser_id"].as_str(),
            auth["cdp"].as_bool()
        ),
        (Some("auth"), Some("oya-1"), Some(false))
    );
    assert_eq!(
        auth["actions"].as_array().unwrap().len(),
        mars_link::commands::ACTIONS.len()
    );
}

#[test]
fn commands_before_auth_are_refused() {
    let (mut s, mut b) = (session(), Fake::default());
    assert_eq!(
        cmd(&mut s, &mut b, "navigate", json!({"url": "https://a.test"}))["error"],
        "Browser not ready"
    );
    assert!(b.requests.is_empty());
}

#[test]
fn a_known_persona_that_comes_back_as_another_device_is_refused() {
    let (mut s, mut b) = signed_in();
    let step = s.handle(&auth_ok(2), &mut b);
    assert_eq!(step.close, Some(CLOSE_REJECTED));
    assert!(matches!(&step.events[0], Event::PersonaRefused(_)));
    assert!(
        s.handle(&auth_ok(1), &mut b).close.is_none(),
        "the same device is welcome again"
    );
}

#[test]
fn element_numbers_map_to_the_analysed_nodes_and_revision() {
    let (mut s, mut b) = signed_in();
    let analysis = cmd(&mut s, &mut b, "analyze", json!({}));
    assert_eq!(
        analysis["data"]["elements"][0],
        json!({"id": 1, "type": "input", "text": "Email", "visible": true})
    );
    assert!(
        analysis["data"]["markdown"]
            .as_str()
            .unwrap()
            .contains("# Hi")
    );
    let typed = cmd(
        &mut s,
        &mut b,
        "type",
        json!({"selector": "[data-ac-id=\"1\"]", "text": "a@b.c"}),
    );
    assert_eq!(typed["ok"], true);
    let sent = &b.requests.last().unwrap();
    assert_eq!(
        (sent.op.as_str(), &sent.params),
        (
            "page.type",
            &json!({"tab_id": "tab-1", "page_id": "pg", "node_id": "n-7", "expected_revision": 3, "text": "a@b.c"})
        )
    );
    assert!(
        cmd(&mut s, &mut b, "click", json!({"element_id": 2}))["error"]
            .as_str()
            .unwrap()
            .contains("analyze_page"),
        "a write forgets the numbers"
    );
}

#[test]
fn an_unknown_element_or_action_is_refused_without_touching_the_page() {
    let (mut s, mut b) = signed_in();
    cmd(&mut s, &mut b, "analyze", json!({}));
    let before = b.requests.len();
    assert_eq!(
        cmd(&mut s, &mut b, "click", json!({"element_id": 9}))["ok"],
        false
    );
    let unsupported = cmd(&mut s, &mut b, "evaluate_raw", json!({}));
    assert_eq!(unsupported["code"], "action_unsupported");
    assert_eq!(b.requests.len(), before);
}

#[test]
fn the_runtimes_ownership_refusal_reaches_the_server() {
    let (mut s, mut b) = signed_in();
    b.refuse_writes = true;
    let result = cmd(&mut s, &mut b, "navigate", json!({"url": "https://a.test"}));
    assert_eq!(
        (result["ok"].as_bool(), result["error"].as_str()),
        (Some(false), Some("Control denied."))
    );
}

#[test]
fn human_mode_revokes_agents_and_refuses_until_agent_mode() {
    let (mut s, mut b) = signed_in();
    let step = s.handle(
        &json!({"type": "control_mode", "state": {"mode": "human"}}),
        &mut b,
    );
    assert!(b.revoked && step.events == [Event::ControlMode("human".into())]);
    assert_eq!(cmd(&mut s, &mut b, "analyze", json!({}))["ok"], false);
    s.handle(&json!({"type": "control_mode", "mode": "agent"}), &mut b);
    assert_eq!(cmd(&mut s, &mut b, "analyze", json!({}))["ok"], true);
}

#[test]
fn an_agent_switches_only_to_a_tab_granted_to_it() {
    let (mut s, mut b) = signed_in();
    assert_eq!(
        cmd(&mut s, &mut b, "switch_tab", json!({"tab_id": 2}))["ok"],
        false
    );
    assert_eq!(
        cmd(&mut s, &mut b, "switch_tab", json!({"tab_id": 1}))["ok"],
        true
    );
    let tabs = cmd(&mut s, &mut b, "list_tabs", json!({}));
    assert_eq!(
        tabs["data"]["tabs"],
        json!([{"id": 1, "title": "A", "url": "", "active": true, "yours": true}, {"id": 2, "title": "B", "url": "", "active": false, "yours": false}])
    );
}

#[test]
fn screenshots_come_back_as_png_data_urls() {
    let (mut s, mut b) = signed_in();
    let shot = cmd(&mut s, &mut b, "screenshot", json!({}));
    assert_eq!(shot["data"]["screenshot"], "data:image/png;base64,iVBORw==");
}

#[test]
fn pings_are_answered_and_unknown_messages_ignored() {
    let (mut s, mut b) = signed_in();
    assert_eq!(
        s.handle(&json!({"type": "ping"}), &mut b).replies,
        [json!({"type": "pong"})]
    );
    let step = s.handle(&json!({"type": "stream_start"}), &mut b);
    assert!(step.replies.is_empty() && step.events.is_empty());
}

#[test]
fn replaced_and_rejected_closes_are_final_and_backoff_is_capped() {
    assert!(is_final(CLOSE_REPLACED) && is_final(CLOSE_AUTH_TIMEOUT) && is_final(CLOSE_REJECTED));
    assert!(!is_final(1006) && !is_final(1001));
    assert!(
        reconnect_delay(1) >= Duration::from_millis(750)
            && reconnect_delay(1) < Duration::from_millis(1250)
    );
    assert!(reconnect_delay(100) <= Duration::from_millis(10_500));
}

#[test]
fn pairing_links_name_a_tls_server_or_this_machine_only() {
    let link = parse_link("oya://connect?code=abc&server=wss%3A%2F%2Foya.test%2Fws").unwrap();
    assert_eq!(
        (link.code.as_str(), link.server.as_str()),
        ("abc", "wss://oya.test/ws")
    );
    assert_eq!(
        claim_url(&link.server).unwrap().as_str(),
        "https://oya.test/api/pairing/claim"
    );
    assert!(
        pairing_server("ws://evil.test/ws").is_none(),
        "plaintext only on loopback"
    );
    assert!(pairing_server("ws://127.0.0.1:8080/ws").is_some());
    assert!(pairing_server("https://oya.test").is_none());
    for bad in [
        "https://x?code=a&server=wss://s",
        "oya://connect?server=wss://s",
        "oya://connect?code=a",
        "oya://connect?code=a&server=ws://evil.test",
    ] {
        assert!(parse_link(bad).is_none(), "{bad}");
    }
}

/// A one-shot loopback claim endpoint answering `status` and `body`; returns its port and the request.
fn claim_server(status: u16, body: &'static str) -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buf = vec![0; 4096];
        let n = socket.read(&mut buf).unwrap();
        let reply = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(reply.as_bytes()).unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    });
    (port, worker)
}

#[test]
fn pairing_asks_first_redeems_the_code_and_stores_the_key() {
    let (port, worker) = claim_server(200, r#"{"apiKey":"oya_k","persona":"work"}"#);
    let store = MemoryCredentials::default();
    let mut asked = String::new();
    let link = format!("oya://connect?code=one&server=ws://127.0.0.1:{port}/ws");
    let paired = pair(
        &link,
        |host| {
            asked = host.into();
            true
        },
        &store,
    )
    .unwrap()
    .unwrap();
    assert_eq!(asked, "127.0.0.1");
    assert_eq!(
        (paired.api_key.as_str(), paired.persona.as_str()),
        ("oya_k", "work")
    );
    assert_eq!(store.load().unwrap(), Some(paired));
    let request = worker.join().unwrap();
    assert!(
        request.starts_with("POST /api/pairing/claim HTTP/1.1")
            && request.ends_with(r#"{"code":"one"}"#)
    );
    store.clear().unwrap();
    assert_eq!(store.load().unwrap(), None);
}

#[test]
fn a_cancelled_or_refused_pairing_stores_nothing() {
    let store = MemoryCredentials::default();
    assert_eq!(
        pair(
            "oya://connect?code=a&server=wss://oya.test/ws",
            |_| false,
            &store
        )
        .unwrap(),
        None
    );
    let (port, worker) = claim_server(400, r#"{"error":"Code expired"}"#);
    let link = format!("oya://connect?code=a&server=ws://localhost:{port}/ws");
    assert_eq!(pair(&link, |_| true, &store).unwrap_err(), "Code expired");
    worker.join().unwrap();
    assert_eq!(store.load().unwrap(), None);
}
