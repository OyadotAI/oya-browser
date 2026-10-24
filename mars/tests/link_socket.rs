/**
 * The Oya link end to end: a loopback server speaking Oya's control-socket protocol drives a real
 * Mars runtime through mars-link and the runtime adapter (src/runtime/link.rs). It checks the
 * `auth` Mars sends, the persona it loads from `auth_ok`, `cmd`s answered as `cmd_result`, and
 * that ownership only tightens: a tab the person takes back refuses the server's writes, and the
 * server's Human Mode revokes the agent.
 */
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner, SharedAuthority},
};
use mars_link::profile::{SeedIdentity, generate};
use mars_link::ws::accept_key;
use mars_link::{Event, Outcome, Session, session::Identity};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use mars::runtime::{link as runtime_link, persona as runtime_persona};

/// The form every scenario starts from.
const FORM: &str = r#"<html><head><title>Signup</title></head><body><main><h1>Join</h1>
<form action="/done" method="get"><label for="name">Full name</label><input id="name" name="name">
<button type="submit">Create account</button></form></main></body></html>"#;

/// Serves FORM, and a welcome page for /done.
fn site() -> (String, Arc<AtomicBool>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let worker = thread::spawn(move || {
        while !signal.load(Ordering::Relaxed) {
            if let Ok((mut socket, _)) = listener.accept() {
                socket.set_nonblocking(false).unwrap();
                let head = read_head(&mut socket);
                let body = if head.starts_with("GET /done") {
                    "<title>Welcome</title><h1>Welcome</h1>"
                } else {
                    FORM
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            }
            thread::sleep(Duration::from_millis(2));
        }
    });
    (origin, stop, worker)
}

/// An HTTP request head.
fn read_head(socket: &mut TcpStream) -> String {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut data = Vec::new();
    let mut byte = [0; 1];
    while !data.ends_with(b"\r\n\r\n") && socket.read(&mut byte).unwrap_or(0) == 1 {
        data.push(byte[0]);
    }
    String::from_utf8_lossy(&data).into_owned()
}

/// The server side of one WebSocket: unmasked writes, masked reads.
struct Peer(TcpStream);

impl Peer {
    /// Accepts the upgrade the link sends.
    fn accept(listener: &TcpListener) -> Self {
        let (mut socket, _) = listener.accept().unwrap();
        let head = read_head(&mut socket);
        let key = head
            .lines()
            .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap()
            .trim()
            .to_owned();
        let reply = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept_key(&key)
        );
        socket.write_all(reply.as_bytes()).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        Self(socket)
    }

    /// Sends one frame.
    fn frame(&mut self, opcode: u8, payload: &[u8]) {
        let mut frame = vec![0x80 | opcode];
        match payload.len() {
            n @ 0..=125 => frame.push(n as u8),
            n @ 126..=0xffff => frame.extend([126, (n >> 8) as u8, n as u8]),
            n => frame.extend([127].into_iter().chain((n as u64).to_be_bytes())),
        }
        frame.extend_from_slice(payload);
        self.0.write_all(&frame).unwrap();
    }

    /// Sends a JSON message.
    fn send(&mut self, message: Value) {
        self.frame(0x1, message.to_string().as_bytes());
    }

    /// Reads one masked text message as JSON, skipping pings.
    fn recv(&mut self) -> Value {
        loop {
            let mut head = [0; 2];
            self.0.read_exact(&mut head).unwrap();
            assert!(head[1] & 0x80 != 0, "client frames are masked");
            let len = match head[1] & 0x7f {
                126 => {
                    let mut b = [0; 2];
                    self.0.read_exact(&mut b).unwrap();
                    u64::from(u16::from_be_bytes(b))
                }
                127 => {
                    let mut b = [0; 8];
                    self.0.read_exact(&mut b).unwrap();
                    u64::from_be_bytes(b)
                }
                n => u64::from(n),
            };
            let mut mask = [0; 4];
            self.0.read_exact(&mut mask).unwrap();
            let mut payload = vec![0; len as usize];
            self.0.read_exact(&mut payload).unwrap();
            payload
                .iter_mut()
                .enumerate()
                .for_each(|(i, b)| *b ^= mask[i % 4]);
            let message: Value = serde_json::from_slice(&payload).unwrap();
            if message["type"] != "ping" {
                return message;
            }
        }
    }

    /// Sends a command and returns its result.
    fn command(&mut self, id: &str, action: &str, params: Value) -> Value {
        self.send(json!({"type": "cmd", "id": id, "action": action, "params": params}));
        let result = self.recv();
        assert_eq!(
            (result["type"].as_str(), result["id"].as_str()),
            (Some("cmd_result"), Some(id))
        );
        result
    }

    /// Closes with an application code.
    fn close(&mut self, code: u16) {
        self.frame(0x8, &code.to_be_bytes());
    }
}

/// The persona the fake server signs Mars in as.
fn fingerprint() -> Value {
    serde_json::to_value(generate(&SeedIdentity {
        id: "p-work".into(),
        seed: 42,
        ..Default::default()
    }))
    .unwrap()
}

/// A desktop runtime granted `origin`, its first tab granted to the agent at startup.
fn runtime(origin: &str, authority: &SharedAuthority) -> Runtime {
    Runtime::desktop(
        EnginePrograms::same(Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![origin.into()],
        authority.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

/// What the scripted server saw.
struct Seen {
    auth: Value,
    results: Vec<Value>,
}

/// Runs `script` as the server against a real runtime driven through the link.
fn scenario(
    script: impl FnOnce(&mut Peer, &str, &SharedAuthority) -> Vec<Value> + Send + 'static,
) -> (Seen, Vec<Event>, Outcome, SharedAuthority) {
    let (origin, stop_site, site_worker) = site();
    let authority = Authority::shared(Owner::Agent);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server = url::Url::parse(&format!("ws://{}/ws", listener.local_addr().unwrap())).unwrap();
    let (shared, page) = (authority.clone(), origin.clone());
    let peer = thread::spawn(move || {
        let mut peer = Peer::accept(&listener);
        let auth = peer.recv();
        peer.send(json!({"type": "auth_ok", "browser_id": "oya-test", "control": {"mode": "agent"}, "persona": {"id": "p-work", "name": "Work"}, "fingerprint": fingerprint(), "cookies": [], "now": 0}));
        let results = script(&mut peer, &page, &shared);
        peer.close(mars_link::constants::CLOSE_REJECTED);
        Seen { auth, results }
    });
    let mut runtime = runtime(&origin, &authority);
    let mut submit = |request, permit| runtime.handle_as(request, permit);
    let mut browser =
        runtime_link::RuntimeBrowser::new(authority.clone(), Box::new(&mut submit), None);
    let identity = Identity {
        api_key: "oya_test".into(),
        browser_name: "Mars".into(),
        persona: Some("p-work".into()),
        app_version: "0.1.0".into(),
        browser_id: None,
    };
    let mut session = Session::new(identity);
    let mut events = Vec::new();
    let outcome = mars_link::run(
        &server,
        &mut session,
        &mut browser,
        &mut |e| events.push(e),
        &AtomicBool::new(false),
    );
    drop(browser);
    let seen = peer.join().unwrap();
    stop_site.store(true, Ordering::Relaxed);
    site_worker.join().unwrap();
    (seen, events, outcome, authority)
}

#[test]
fn the_server_sees_mars_as_an_oya_browser_and_drives_it_natively() {
    let (seen, events, outcome, _) = scenario(|peer, origin, _| {
        let navigated = peer.command("1", "navigate", json!({"url": format!("{origin}/")}));
        let analyzed = peer.command("2", "analyze", json!({}));
        let field = analyzed["data"]["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == "input")
            .unwrap()["id"]
            .clone();
        let button = analyzed["data"]["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == "button")
            .unwrap()["id"]
            .clone();
        let typed = peer.command(
            "3",
            "type",
            json!({"selector": format!("[data-ac-id=\"{field}\"]"), "text": "Ada"}),
        );
        let stale = peer.command("4", "click", json!({"element_id": button}));
        let fresh = peer.command("5", "analyze", json!({}));
        let clicked = peer.command("6", "click", json!({"element_id": button}));
        let landed = peer.command("7", "analyze", json!({}));
        let tabs = peer.command("8", "list_tabs", json!({}));
        let unsupported = peer.command("9", "drag", json!({}));
        vec![
            navigated,
            analyzed,
            typed,
            stale,
            fresh,
            clicked,
            landed,
            tabs,
            unsupported,
        ]
    });
    assert_eq!(seen.auth["type"], "auth");
    assert_eq!(
        (
            seen.auth["api_key"].as_str(),
            seen.auth["provider"].as_str()
        ),
        (Some("oya_test"), Some("oya-desktop"))
    );
    assert!(
        seen.auth["actions"]
            .as_array()
            .unwrap()
            .contains(&json!("navigate"))
    );
    let r = &seen.results;
    assert_eq!(r[0]["ok"], true, "{}", r[0]);
    assert_eq!(r[1]["data"]["title"], "Signup");
    assert_eq!(r[2]["ok"], true, "{}", r[2]);
    assert_eq!(
        r[3]["ok"], false,
        "element numbers die with the revision that typed into the page: {}",
        r[3]
    );
    assert_eq!(r[5]["ok"], true, "{}", r[5]);
    assert_eq!(r[6]["data"]["title"], "Welcome");
    assert_eq!(r[7]["data"]["tabs"][0]["id"], 1);
    assert_eq!(
        (r[8]["ok"].as_bool(), r[8]["code"].as_str()),
        (Some(false), Some("action_unsupported"))
    );
    assert!(
        matches!(&events[0], Event::Ready { browser_id, persona } if browser_id == "oya-test" && persona.id() == "p-work")
    );
    assert_eq!(
        outcome,
        Outcome::Final(mars_link::constants::CLOSE_REJECTED)
    );
}

#[test]
fn a_tab_the_person_takes_back_refuses_the_servers_writes() {
    let (seen, _, _, _) = scenario(|peer, origin, authority| {
        let first = peer.command("1", "navigate", json!({"url": format!("{origin}/")}));
        authority
            .lock()
            .unwrap()
            .change_tab(1, Owner::Human)
            .unwrap();
        let refused = peer.command("2", "navigate", json!({"url": format!("{origin}/done")}));
        let read = peer.command("3", "analyze", json!({}));
        vec![first, refused, read]
    });
    assert_eq!(seen.results[0]["ok"], true);
    assert_eq!(seen.results[1]["ok"], false, "{}", seen.results[1]);
    assert_eq!(
        seen.results[2]["data"]["title"], "Signup",
        "the refused navigation never happened"
    );
}

#[test]
fn server_human_mode_revokes_the_agent_and_refuses_commands() {
    let (seen, events, _, authority) = scenario(|peer, origin, _| {
        peer.send(json!({"type": "control_mode", "mode": "human", "state": {"mode": "human"}}));
        vec![peer.command("1", "navigate", json!({"url": format!("{origin}/")}))]
    });
    assert_eq!(seen.results[0]["ok"], false);
    assert!(
        seen.results[0]["error"]
            .as_str()
            .unwrap()
            .contains("Human Mode")
    );
    assert!(events.contains(&Event::ControlMode("human".into())));
    assert_eq!(
        authority.lock().unwrap().tab_status(1).unwrap().0,
        Owner::Human,
        "the agent's tab went back to the person"
    );
}

#[test]
fn server_agent_mode_never_grants_a_tab_the_person_holds() {
    let (seen, _, _, _) = scenario(|peer, origin, authority| {
        authority
            .lock()
            .unwrap()
            .change_tab(1, Owner::Human)
            .unwrap();
        peer.send(json!({"type": "control_mode", "mode": "agent", "state": {"mode": "agent"}}));
        vec![peer.command("1", "navigate", json!({"url": format!("{origin}/")}))]
    });
    assert_eq!(seen.results[0]["ok"], false, "{}", seen.results[0]);
}

#[test]
fn the_persona_runtime_presents_the_personas_languages() {
    let unit = mars_link::Persona::from_welcome(&json!({"fingerprint": fingerprint()})).unwrap();
    let identity = unit.identity(runtime_persona::PRESENTED_ENGINE).unwrap();
    assert_eq!(
        identity.policy.languages,
        unit.profile().navigator.languages
    );
    assert!(
        identity
            .policy
            .user_agent
            .contains(runtime_persona::PRESENTED_ENGINE)
    );
}

/// `mars link` refuses a plain-text server off this machine and a missing key before connecting.
#[test]
fn the_link_command_refuses_unsafe_servers_and_a_missing_key() {
    let run = |server: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_mars"))
            .args(["link", "--server", server])
            .env_remove("OYA_API_KEY")
            .output()
            .unwrap()
    };
    let remote = run("ws://example.com/ws");
    assert!(!remote.status.success());
    assert!(String::from_utf8_lossy(&remote.stderr).contains("wss://"));
    let keyless = run("ws://127.0.0.1:1/ws");
    assert!(!keyless.status.success());
    assert!(String::from_utf8_lossy(&keyless.stderr).contains("OYA_API_KEY"));
}
