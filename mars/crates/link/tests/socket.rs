/**
 * The control socket over a real loopback WebSocket: handshake, masked client frames, large and
 * fragmented server messages, protocol pings, reconnect after an ordinary close, and stopping on
 * the codes Oya's server uses to say "do not come back".
 */
use mars_core::protocol::{EngineResult, Request};
use mars_link::profile::{SeedIdentity, generate};
use mars_link::session::Identity;
use mars_link::ws::accept_key;
use mars_link::{Browser, Event, Outcome, Session, run};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Answers every request as a page that loaded the URL it was asked for.
struct Echo;

impl Browser for Echo {
    fn run(&mut self, request: Request) -> EngineResult<Value> {
        Ok(json!({"selected_tab_id": "tab-1", "url": request.params["url"], "title": "T"}))
    }
    fn screenshot(&mut self, _: &str) -> EngineResult<Vec<u8>> {
        Ok(Vec::new())
    }
    fn revoke_agents(&mut self) {}
}

/// The server end of one accepted WebSocket.
struct Peer(TcpStream);

impl Peer {
    /// Accepts one upgrade.
    fn accept(listener: &TcpListener) -> Self {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut head = Vec::new();
        let mut byte = [0; 1];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8(head).unwrap();
        assert!(head.starts_with("GET /ws HTTP/1.1"));
        let key = head
            .lines()
            .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap();
        write!(socket, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n", accept_key(key.trim())).unwrap();
        Self(socket)
    }

    /// One unmasked frame.
    fn frame(&mut self, fin: bool, opcode: u8, payload: &[u8]) {
        let mut frame = vec![if fin { 0x80 } else { 0 } | opcode];
        match payload.len() {
            n @ 0..=125 => frame.push(n as u8),
            n @ 126..=0xffff => frame.extend([126, (n >> 8) as u8, n as u8]),
            n => frame.extend([127].into_iter().chain((n as u64).to_be_bytes())),
        }
        frame.extend_from_slice(payload);
        self.0.write_all(&frame).unwrap();
    }

    /// One client frame, unmasked: (opcode, payload).
    fn read(&mut self) -> (u8, Vec<u8>) {
        let mut head = [0; 2];
        self.0.read_exact(&mut head).unwrap();
        assert_ne!(head[1] & 0x80, 0, "client frames are masked");
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
        (head[0] & 0x0f, payload)
    }

    /// One client text message as JSON.
    fn json(&mut self) -> Value {
        let (opcode, payload) = self.read();
        assert_eq!(opcode, 1);
        serde_json::from_slice(&payload).unwrap()
    }

    /// Signs the client in.
    fn welcome(&mut self) {
        let fingerprint = generate(&SeedIdentity {
            id: "p".into(),
            seed: 5,
            ..Default::default()
        });
        let message = json!({"type": "auth_ok", "browser_id": "oya-9", "control": {"mode": "agent"}, "fingerprint": fingerprint, "cookies": []});
        self.frame(true, 1, message.to_string().as_bytes());
    }
}

/// Runs the link against `script`, which plays the server over any number of connections.
fn drive(script: impl FnOnce(&TcpListener) + Send + 'static) -> (Outcome, Vec<Event>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = url::Url::parse(&format!("ws://{}/ws", listener.local_addr().unwrap())).unwrap();
    let server = std::thread::spawn(move || script(&listener));
    let mut session = Session::new(Identity {
        api_key: "k".into(),
        browser_name: "Mars".into(),
        persona: None,
        app_version: "1".into(),
        browser_id: None,
    });
    let mut events = Vec::new();
    let outcome = run(
        &url,
        &mut session,
        &mut Echo,
        &mut |e| events.push(e),
        &AtomicBool::new(false),
    );
    server.join().unwrap();
    (outcome, events)
}

#[test]
fn large_and_fragmented_commands_are_answered_and_pings_ponged() {
    let (outcome, events) = drive(|listener| {
        let mut peer = Peer::accept(listener);
        assert_eq!(peer.json()["type"], "auth");
        peer.welcome();
        peer.frame(true, 9, b"hi");
        assert_eq!(
            peer.read(),
            (10, b"hi".to_vec()),
            "a protocol ping is ponged with its payload"
        );
        let url = format!("https://a.test/{}", "x".repeat(70_000));
        let message =
            json!({"type": "cmd", "id": "big", "action": "navigate", "params": {"url": url}})
                .to_string();
        let (first, rest) = message.as_bytes().split_at(100);
        peer.frame(false, 1, first);
        peer.frame(true, 0, rest);
        let result = peer.json();
        assert_eq!(
            (result["id"].as_str(), result["data"]["url"].as_str()),
            (Some("big"), Some(url.as_str()))
        );
        peer.frame(true, 8, &4000u16.to_be_bytes());
    });
    assert_eq!(outcome, Outcome::Final(4000));
    assert!(matches!(&events[0], Event::Ready { browser_id, .. } if browser_id == "oya-9"));
    assert_eq!(events.last(), Some(&Event::Disconnected(4000)));
}

#[test]
fn an_ordinary_close_reconnects_with_the_browser_id_the_server_gave() {
    let (outcome, events) = drive(|listener| {
        let mut first = Peer::accept(listener);
        assert_eq!(first.json()["browser_id"], Value::Null);
        first.welcome();
        first.frame(true, 8, &1001u16.to_be_bytes());
        let mut second = Peer::accept(listener);
        assert_eq!(
            second.json()["browser_id"],
            "oya-9",
            "the reconnect keeps the server's id"
        );
        second.frame(true, 8, &4003u16.to_be_bytes());
    });
    assert_eq!(outcome, Outcome::Final(4003));
    assert!(events.contains(&Event::Disconnected(1001)));
}

#[test]
fn a_refused_upgrade_is_retried_not_trusted() {
    let (outcome, _) = drive(|listener| {
        let (mut socket, _) = listener.accept().unwrap();
        let mut buf = [0; 1024];
        let _ = socket.read(&mut buf).unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        drop(socket);
        let mut peer = Peer::accept(listener);
        peer.json();
        peer.frame(true, 8, &4001u16.to_be_bytes());
    });
    assert_eq!(outcome, Outcome::Final(4001));
}
