/**
 * The control socket's lifetime, as the Electron browser's (browser/src/main/connection/socket.ts):
 * connect, authenticate, hand each message in order to the session, heartbeat, and reconnect with
 * capped, jittered backoff until stopped or the server says a reconnect is pointless. Blocking and
 * single-threaded: the app runs it on a thread of its own, and every command reaches the runtime
 * through `Browser`, one at a time.
 */
use crate::commands::Browser;
use crate::constants::{
    CLOSE_AUTH_TIMEOUT, CLOSE_REJECTED, CLOSE_REPLACED, MAX_MISSED_PONGS, PING_INTERVAL, READ_TICK,
    RECONNECT_BASE_MS, RECONNECT_GROWTH, RECONNECT_JITTER_MS, RECONNECT_MAX_MS,
};
use crate::session::{Event, Session};
use crate::ws::{Incoming, WebSocket, dial, random};
use serde_json::{Value, json};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use url::Url;

/// Why the link stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The app asked it to.
    Stopped,
    /// The server closed with a code after which reconnecting is pointless (replaced, rejected).
    Final(u16),
}

/// Close codes after which the link does not reconnect.
pub fn is_final(code: u16) -> bool {
    matches!(code, CLOSE_REPLACED | CLOSE_AUTH_TIMEOUT | CLOSE_REJECTED)
}

/// How long to wait before reconnect attempt `attempts`: exponential, capped, jittered.
pub fn reconnect_delay(attempts: u32) -> Duration {
    let backoff =
        RECONNECT_BASE_MS * RECONNECT_GROWTH.powi(attempts.try_into().unwrap_or(i32::MAX));
    let jitter = u64::from(u16::from_le_bytes(random::<2>())) % RECONNECT_JITTER_MS;
    Duration::from_millis(backoff.min(RECONNECT_MAX_MS) as u64 + jitter)
}

/// Runs the link until `stop` is set or the server closes finally; events go to `on_event`.
pub fn run(
    server: &Url,
    session: &mut Session,
    browser: &mut dyn Browser,
    on_event: &mut dyn FnMut(Event),
    stop: &AtomicBool,
) -> Outcome {
    let mut attempts = 0;
    while !stop.load(Ordering::Relaxed) {
        let code = connect_once(server, session, browser, on_event, stop).unwrap_or(1006);
        let was_ready = session.ready();
        session.disconnected();
        on_event(Event::Disconnected(code));
        if is_final(code) {
            return Outcome::Final(code);
        }
        attempts = if was_ready { 1 } else { attempts + 1 };
        let wake = Instant::now() + reconnect_delay(attempts);
        while Instant::now() < wake && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(READ_TICK);
        }
    }
    Outcome::Stopped
}

/// One connection: auth, then messages until it closes; the close code it ended with.
fn connect_once(
    server: &Url,
    session: &mut Session,
    browser: &mut dyn Browser,
    on_event: &mut dyn FnMut(Event),
    stop: &AtomicBool,
) -> io::Result<u16> {
    let mut socket = WebSocket::client(dial(server)?, server)?;
    socket.get_ref().set_read_timeout(Some(READ_TICK))?;
    socket.send_text(&session.auth_message().to_string())?;
    let mut heartbeat = Heartbeat::new();
    while !stop.load(Ordering::Relaxed) {
        if !heartbeat.beat(&mut socket, session.ready())? {
            return Ok(1006);
        }
        let text = match socket.read()? {
            None => continue,
            Some(Incoming::Close(code)) => return Ok(code),
            Some(Incoming::Text(text)) => text,
        };
        heartbeat.heard();
        // A frame that is not JSON is skipped, as Oya's socket does.
        let Ok(message) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let step = session.handle(&message, browser);
        for reply in &step.replies {
            socket.send_text(&reply.to_string())?;
        }
        step.events.into_iter().for_each(&mut *on_event);
        if let Some(code) = step.close {
            socket.close(code)?;
            return Ok(code);
        }
    }
    socket.close(1000)?;
    Ok(1000)
}

/// Pings on an interval once signed in, and gives up on a server that stopped answering.
struct Heartbeat {
    /// When the next ping is due.
    next: Instant,
    /// Pings sent since the server was last heard.
    missed: u32,
}

impl Heartbeat {
    /// A heartbeat whose first ping is one interval away.
    fn new() -> Self {
        Self {
            next: Instant::now() + PING_INTERVAL,
            missed: 0,
        }
    }

    /// The server said something: it is alive.
    fn heard(&mut self) {
        self.missed = 0;
    }

    /// Pings when due; false when too many went unanswered and the socket should be dropped.
    fn beat<S: io::Read + io::Write>(
        &mut self,
        socket: &mut WebSocket<S>,
        ready: bool,
    ) -> io::Result<bool> {
        if !ready || Instant::now() < self.next {
            return Ok(true);
        }
        self.next = Instant::now() + PING_INTERVAL;
        self.missed += 1;
        if self.missed > MAX_MISSED_PONGS {
            return Ok(false);
        }
        socket.send_text(&json!({"type": "ping"}).to_string())?;
        Ok(true)
    }
}
