/** HTTP/2 sessions against a scripted loopback TLS server, one test per protocol rule. */
use super::frame::{self, u32_at};
use super::hpack::{Decoder, encode};
use super::*;
use crate::network::Method;
use rustls::{ServerConnection, StreamOwned};
use std::io::Write;
use std::thread::JoinHandle;
use std::time::Duration;

/** Every test finishes far inside this. */
const DEADLINE: Duration = Duration::from_secs(5);

/** The server's side of one connection. */
struct Peer {
    tls: StreamOwned<ServerConnection, TcpStream>,
    decoder: Decoder,
}

impl Peer {
    /** One frame as (type, flags, stream, payload). */
    fn read(&mut self) -> (u8, u8, u32, Vec<u8>) {
        let mut head = [0; frame::HEADER_BYTES];
        self.tls.read_exact(&mut head).expect("frame header");
        let length = usize::from(head[0]) << 16 | usize::from(head[1]) << 8 | usize::from(head[2]);
        let mut payload = vec![0; length];
        self.tls.read_exact(&mut payload).expect("frame payload");
        (head[3], head[4], u32_at(&head, 5) & 0x7fff_ffff, payload)
    }

    /** Reads until a frame of `kind`, skipping SETTINGS, WINDOW_UPDATE and acknowledgements. */
    fn expect(&mut self, kind: u8) -> (u8, u32, Vec<u8>) {
        loop {
            let (found, flags, stream, payload) = self.read();
            if found == kind {
                return (flags, stream, payload);
            }
        }
    }

    /** The next request's stream and decoded fields. */
    fn request(&mut self) -> (u32, Vec<(String, String)>) {
        let (flags, stream, payload) = self.expect(frame::HEADERS);
        assert!(flags & frame::END_HEADERS != 0);
        let block = &payload[5..];
        (
            stream,
            self.decoder.decode(block, 1 << 20).expect("request block"),
        )
    }

    /**
     * Reads until the client goes quiet, so closing never discards unread client bytes: a
     * close with unread data sends RST, which can destroy responses the client has not read.
     */
    fn drain(&mut self) {
        let _ = self
            .tls
            .sock
            .set_read_timeout(Some(Duration::from_millis(300)));
        let mut bytes = [0; 1024];
        while matches!(self.tls.read(&mut bytes), Ok(count) if count > 0) {}
    }

    /** Writes one frame. */
    fn send(&mut self, kind: u8, flags: u8, stream: u32, payload: &[u8]) {
        let mut bytes = Vec::new();
        frame::write(&mut bytes, kind, flags, stream, payload);
        self.tls.write_all(&bytes).expect("send");
        self.tls.flush().expect("flush");
    }

    /** A response head, optionally ending the stream. */
    fn respond(&mut self, stream: u32, fields: &[(&str, &str)], end: bool) {
        let flags = frame::END_HEADERS | if end { frame::END_STREAM } else { 0 };
        self.send(frame::HEADERS, flags, stream, &encode(fields));
    }
}

/** Starts a one-connection TLS server running `script`, and a client session to it. */
fn connect(script: impl FnOnce(&mut Peer) + Send + 'static) -> (Arc<Session>, JoinHandle<()>) {
    let (tls, server) = crate::network::web::testing::tls(b"h2", |tls| {
        let mut peer = Peer {
            tls,
            decoder: Decoder::new(65_536),
        };
        let mut preface = [0; 24];
        peer.tls.read_exact(&mut preface).expect("preface");
        assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
        peer.send(frame::SETTINGS, 0, 0, &[]);
        script(&mut peer);
        peer.drain();
    });
    let session =
        Session::start(Connection::Tls(Box::new(tls)), Instant::now() + DEADLINE).expect("session");
    (session, server)
}

/** One GET through the session with a `limit`-byte body budget. */
fn get(
    session: &Session,
    path: &str,
    limit: usize,
    deadline: Duration,
) -> Result<body::Response, Failure> {
    let url = Url::parse(&format!("https://localhost{path}")).expect("url");
    let request = WebRequest::get(url.as_str());
    session.exchange(&url, &request, &[], limit, Instant::now() + deadline)
}

/** The error code of a failure, and whether it may be retried. */
fn failure(result: Result<body::Response, Failure>) -> (bool, String) {
    match result {
        Ok(response) => panic!("unexpected response {}", response.status),
        Err(Failure::Retryable(error)) => (true, error.code),
        Err(Failure::Final(error)) => (false, error.code),
    }
}

#[test]
fn requests_lead_with_chromes_pseudo_header_order_and_fields() {
    let (session, server) = connect(|peer| {
        let (stream, fields) = peer.request();
        let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(&names[..4], [":method", ":authority", ":scheme", ":path"]);
        assert_eq!(names[4], "sec-ch-ua");
        assert_eq!(
            fields.last().map(|(n, v)| (n.as_str(), v.as_str())),
            Some(("priority", "u=0, i"))
        );
        peer.respond(
            stream,
            &[(":status", "200"), ("content-type", "text/html")],
            false,
        );
        peer.send(frame::DATA, frame::END_STREAM, stream, b"<p>hi</p>");
    });
    let response = get(&session, "/x?q=1", 1024, DEADLINE)
        .ok()
        .expect("response");
    assert_eq!(
        (response.status, response.body.as_slice()),
        (200, &b"<p>hi</p>"[..])
    );
    server.join().expect("server");
}

#[test]
fn concurrent_requests_share_one_connection_and_complete_out_of_order() {
    let (session, server) = connect(|peer| {
        let (first, _) = peer.request();
        let (second, _) = peer.request();
        peer.respond(second, &[(":status", "200")], false);
        peer.respond(first, &[(":status", "200")], false);
        peer.send(frame::DATA, 0, first, b"one-");
        peer.send(frame::DATA, frame::END_STREAM, second, b"two");
        peer.send(frame::DATA, frame::END_STREAM, first, b"done");
    });
    let other = session.clone();
    let worker = std::thread::spawn(move || get(&other, "/a", 1024, DEADLINE).ok().map(|r| r.body));
    let mine = get(&session, "/b", 1024, DEADLINE).ok().map(|r| r.body);
    let theirs = worker.join().expect("worker");
    let mut bodies = vec![mine.expect("mine"), theirs.expect("theirs")];
    bodies.sort();
    assert_eq!(bodies, [b"one-done".to_vec(), b"two".to_vec()]);
    server.join().expect("server");
}

#[test]
fn goaway_makes_unprocessed_streams_retryable_and_closes_the_session_to_new_ones() {
    let (session, server) = connect(|peer| {
        let _ = peer.request();
        peer.send(frame::GOAWAY, 0, 0, &[0, 0, 0, 0, 0, 0, 0, 0]);
    });
    assert_eq!(
        failure(get(&session, "/", 1024, DEADLINE)),
        (true, "network_error".into())
    );
    assert!(!session.usable());
    server.join().expect("server");
}

#[test]
fn refused_streams_are_retryable_and_other_resets_are_final() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        peer.send(
            frame::RST_STREAM,
            0,
            stream,
            &frame::REFUSED_STREAM.to_be_bytes(),
        );
        let (stream, _) = peer.request();
        peer.send(frame::RST_STREAM, 0, stream, &frame::CANCEL.to_be_bytes());
    });
    assert!(failure(get(&session, "/", 1024, DEADLINE)).0);
    assert!(!failure(get(&session, "/", 1024, DEADLINE)).0);
    server.join().expect("server");
}

#[test]
fn a_body_over_budget_cancels_only_its_stream() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        peer.respond(stream, &[(":status", "200")], false);
        peer.send(frame::DATA, 0, stream, &[b'a'; 16]);
        let (_, reset, code) = peer.expect(frame::RST_STREAM);
        assert_eq!((reset, u32_at(&code, 0)), (stream, frame::CANCEL));
        let (next, _) = peer.request();
        peer.respond(next, &[(":status", "204")], true);
    });
    assert_eq!(
        failure(get(&session, "/", 8, DEADLINE)),
        (false, "limit_exceeded".into())
    );
    assert_eq!(
        get(&session, "/", 8, DEADLINE).ok().map(|r| r.status),
        Some(204)
    );
    server.join().expect("server");
}

/** Large bodies take a while through debug-build TLS; still bounded. */
const BULK_DEADLINE: Duration = Duration::from_secs(60);

/**
 * Sends `total` body bytes on `stream` the way a strict server does: never past either receive
 * window, blocking on WINDOW_UPDATE when one runs dry. Returns the (stream, increment) updates
 * seen, or the RST_STREAM code that stopped it.
 */
fn pour(peer: &mut Peer, stream: u32, total: usize) -> Result<Vec<(u32, u32)>, u32> {
    let mut windows = [
        i64::from(crate::network::constants::H2_STREAM_WINDOW),
        frame::DEFAULT_WINDOW + i64::from(crate::network::constants::H2_CONNECTION_INCREMENT),
    ];
    let mut updates = Vec::new();
    let chunk = [b'a'; crate::network::constants::H2_FRAME_BYTES];
    let mut sent = 0;
    while sent < total {
        let size = chunk.len().min(total - sent);
        while windows.iter().any(|window| *window < size as i64) {
            let (kind, _, id, payload) = peer.read();
            match kind {
                frame::RST_STREAM => return Err(u32_at(&payload, 0)),
                frame::WINDOW_UPDATE => {
                    let increment = u32_at(&payload, 0);
                    windows[usize::from(id == 0)] += i64::from(increment);
                    updates.push((id, increment));
                }
                _ => {}
            }
        }
        windows.iter_mut().for_each(|window| *window -= size as i64);
        sent += size;
        let flags = if sent == total { frame::END_STREAM } else { 0 };
        peer.send(frame::DATA, flags, stream, &chunk[..size]);
    }
    Ok(updates)
}

#[test]
fn a_body_larger_than_the_initial_window_completes_through_window_updates() {
    const SIZE: usize = 7_000_000;
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        let length = SIZE.to_string();
        peer.respond(
            stream,
            &[(":status", "200"), ("content-length", length.as_str())],
            false,
        );
        let updates = pour(peer, stream, SIZE).expect("no reset");
        let half = crate::network::constants::H2_STREAM_WINDOW / 2;
        assert!(!updates.is_empty(), "the stream window was replenished");
        for (id, increment) in updates {
            assert_eq!(id, stream, "7 MB stays inside the connection window's half");
            assert!(increment > half && increment <= half + frame::HEADER_BYTES as u32 + 16_384);
        }
    });
    let response = get(&session, "/", crate::constants::HTML_BYTES, BULK_DEADLINE)
        .ok()
        .expect("response");
    assert_eq!((response.status, response.body.len()), (200, SIZE));
    server.join().expect("server");
}

#[test]
fn a_body_past_the_budget_replenishes_the_connection_then_cancels_with_limit_exceeded() {
    let budget = crate::constants::HTML_BYTES;
    let (session, server) = connect(move |peer| {
        let (stream, _) = peer.request();
        peer.respond(stream, &[(":status", "200")], false);
        let updates = pour(peer, stream, budget + 1).expect("the budget fits the windows");
        let connection = (frame::DEFAULT_WINDOW
            + i64::from(crate::network::constants::H2_CONNECTION_INCREMENT))
            / 2;
        assert!(
            updates
                .iter()
                .any(|(id, increment)| *id == 0 && i64::from(*increment) > connection),
            "the connection window was replenished past half"
        );
        let (_, reset, code) = peer.expect(frame::RST_STREAM);
        assert_eq!((reset, u32_at(&code, 0)), (stream, frame::CANCEL));
        let (next, _) = peer.request();
        peer.respond(next, &[(":status", "204")], true);
    });
    assert_eq!(
        failure(get(&session, "/", budget, BULK_DEADLINE)),
        (false, "limit_exceeded".into())
    );
    assert_eq!(
        get(&session, "/", 8, DEADLINE).ok().map(|r| r.status),
        Some(204),
        "only the stream was cancelled; the connection still serves"
    );
    server.join().expect("server");
}

#[test]
fn continuation_floods_end_the_connection_with_enhance_your_calm() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        peer.send(frame::HEADERS, 0, stream, &encode(&[(":status", "200")]));
        for _ in 0..=crate::network::constants::H2_CONTINUATIONS {
            peer.send(frame::CONTINUATION, 0, stream, &[]);
        }
        let (_, _, payload) = peer.expect(frame::GOAWAY);
        assert_eq!(u32_at(&payload, 4), frame::ENHANCE_YOUR_CALM);
    });
    assert!(!failure(get(&session, "/", 1024, DEADLINE)).0);
    assert!(!session.usable());
    server.join().expect("server");
}

#[test]
fn oversized_frames_end_the_connection_with_frame_size_error() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        peer.respond(stream, &[(":status", "200")], false);
        peer.send(
            frame::DATA,
            0,
            stream,
            &vec![0; crate::network::constants::H2_FRAME_BYTES + 1],
        );
        let (_, _, payload) = peer.expect(frame::GOAWAY);
        assert_eq!(u32_at(&payload, 4), frame::FRAME_SIZE_ERROR);
    });
    assert!(!failure(get(&session, "/", 1 << 20, DEADLINE)).0);
    server.join().expect("server");
}

#[test]
fn push_promises_are_refused_since_push_is_disabled() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        peer.send(
            frame::PUSH_PROMISE,
            frame::END_HEADERS,
            stream,
            &[0, 0, 0, 2],
        );
        let (_, _, payload) = peer.expect(frame::GOAWAY);
        assert_eq!(u32_at(&payload, 4), frame::PROTOCOL_ERROR);
    });
    assert!(!failure(get(&session, "/", 1024, DEADLINE)).0);
    server.join().expect("server");
}

#[test]
fn pings_are_answered_and_settings_acknowledged() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        peer.send(frame::PING, 0, 0, b"12345678");
        loop {
            let (kind, flags, _, payload) = peer.read();
            if kind == frame::PING {
                assert_eq!((flags, payload.as_slice()), (frame::ACK, &b"12345678"[..]));
                break;
            }
        }
        peer.respond(stream, &[(":status", "200")], true);
    });
    assert_eq!(
        get(&session, "/", 1024, DEADLINE).ok().map(|r| r.status),
        Some(200)
    );
    server.join().expect("server");
}

#[test]
fn a_silent_server_times_out_at_the_deadline_without_hanging() {
    let (session, server) = connect(|peer| {
        let _ = peer.request();
        std::thread::sleep(Duration::from_millis(400));
    });
    let start = Instant::now();
    assert_eq!(
        failure(get(&session, "/", 1024, Duration::from_millis(200))),
        (false, "timeout".into())
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    server.join().expect("server");
}

#[test]
fn request_bodies_wait_for_the_peers_flow_control_window() {
    let (session, server) = connect(|peer| {
        let mut settings = frame::SETTINGS_INITIAL_WINDOW_SIZE.to_be_bytes().to_vec();
        settings.extend_from_slice(&2u32.to_be_bytes());
        peer.send(frame::SETTINGS, 0, 0, &settings);
        let (warmup, _) = peer.request();
        peer.respond(warmup, &[(":status", "204")], true);
        let (stream, fields) = peer.request();
        assert!(fields.contains(&(":method".into(), "POST".into())));
        let (_, _, first) = peer.expect(frame::DATA);
        assert_eq!(first, b"a=");
        peer.send(frame::WINDOW_UPDATE, 0, stream, &100u32.to_be_bytes());
        let (flags, _, rest) = peer.expect(frame::DATA);
        assert_eq!(
            (flags & frame::END_STREAM, rest.as_slice()),
            (frame::END_STREAM, &b"bc"[..])
        );
        peer.respond(stream, &[(":status", "303"), ("location", "/next")], true);
    });
    assert!(
        get(&session, "/", 1024, DEADLINE).is_ok(),
        "the settings arrive with a first response"
    );
    let url = Url::parse("https://localhost/form").expect("url");
    let request = WebRequest {
        method: Method::Post,
        body: "a=bc".into(),
        ..WebRequest::get(url.as_str())
    };
    let response = session
        .exchange(&url, &request, &[], 1024, Instant::now() + DEADLINE)
        .ok()
        .expect("response");
    assert_eq!(response.status, 303);
    server.join().expect("server");
}

#[test]
fn header_lists_over_the_advertised_limit_end_the_connection() {
    let (session, server) = connect(|peer| {
        let (stream, _) = peer.request();
        let big = "v".repeat(crate::network::constants::H2_FRAME_BYTES - 64);
        let mut block = Vec::new();
        for _ in 0..(crate::network::constants::H2_HEADER_LIST_BYTES / big.len() + 2) {
            block.extend(encode(&[("x-big", big.as_str())]));
        }
        let mut chunks = block.chunks(crate::network::constants::H2_FRAME_BYTES);
        peer.send(frame::HEADERS, 0, stream, chunks.next().expect("first"));
        for chunk in chunks {
            peer.send(frame::CONTINUATION, 0, stream, chunk);
        }
        let (_, _, payload) = peer.expect(frame::GOAWAY);
        assert_eq!(u32_at(&payload, 4), frame::ENHANCE_YOUR_CALM);
    });
    assert!(!failure(get(&session, "/", 1024, DEADLINE)).0);
    server.join().expect("server");
}

#[test]
fn the_registry_hands_one_session_per_origin_and_waits_for_a_pending_connect() {
    let registry = registry::Registry::new();
    let url = Url::parse("https://localhost/").expect("url");
    let deadline = Instant::now() + Duration::from_millis(100);
    let Acquired::Claim(claim) = registry.acquire(&url, true, deadline) else {
        panic!("first request claims the connect");
    };
    assert!(
        matches!(registry.acquire(&url, true, deadline), Acquired::Http1),
        "waiter gives up at its deadline"
    );
    claim.finish(None);
    assert!(
        matches!(registry.acquire(&url, true, deadline), Acquired::Http1),
        "HTTP/1.1 origins are remembered"
    );
    assert_eq!(registry.len(), 0);
}
