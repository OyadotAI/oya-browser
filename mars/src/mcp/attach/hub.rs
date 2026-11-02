/**
 * The hub serves every attached connection against one browser. Each connection is one agent
 * lane with its own MCP session or native correlation table; requests leave through the host's
 * single command queue with an observed permit, and replies return by private correlation ID.
 */
use super::{Protocol, Welcome, admit};
use crate::{
    constants::{
        FRAME_BYTES, MCP_FRAME_BYTES, MCP_OUTPUT_QUEUE, MCP_PENDING_REQUESTS, MCP_WRITE_DEADLINE,
    },
    mcp::{self, Action, Session},
    protocol::{EngineError, EngineResult, Request, Response, decode, encode, valid_id},
    runtime::{
        self,
        authority::{self, Owner, Permit, SharedAuthority},
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    io::{ErrorKind, Read, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Cancellation and session deadlines are checked at least this often.
const POLL: Duration = Duration::from_millis(25);
/// Agents attached at once; each holds a lane, threads and a pending table.
pub const CONNECTIONS: usize = 8;
/// Bytes read from a socket per call.
const CHUNK: usize = 16 * 1024;

/** What happened to a submitted request. */
#[derive(Debug, PartialEq, Eq)]
pub enum Submit {
    /// Accepted; exactly one Response with the same ID will reach the hub's reply channel.
    Queued,
    /// The command queue is full; nothing was queued.
    Busy,
    /// The browser is shutting down.
    Closed,
}

/**
 * The browser side of the hub: the desktop broker's command queue, or a headless runtime. It runs
 * every request through `Runtime::handle_as` with the permit, which re-checks ownership, epoch,
 * revision and lane before anything changes.
 */
pub trait Host: Send + Sync + 'static {
    /// Queue one agent request; its reply must later arrive on the hub's reply channel.
    fn submit(&self, request: Request, permit: Permit) -> Submit;
}

/** State every connection and hub thread shares. */
struct Shared {
    host: Arc<dyn Host>,
    authority: SharedAuthority,
    token: String,
    stop: Arc<AtomicBool>,
    connections: Mutex<HashMap<u64, Arc<Connection>>>,
    next: AtomicU64,
}

/** The running hub; dropping it stops accepting, closes every connection and joins its threads. */
pub struct Hub {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
}

impl Hub {
    /** Start accepting on `listener` and routing `replies` back to their connections. */
    pub fn start(
        listener: UnixListener,
        token: String,
        host: Arc<dyn Host>,
        replies: Receiver<Response>,
        authority: SharedAuthority,
        stop: Arc<AtomicBool>,
    ) -> EngineResult<Self> {
        listener
            .set_nonblocking(true)
            .map_err(|_| EngineError::invalid("The agent endpoint could not be polled."))?;
        let shared = Arc::new(Shared {
            host,
            authority,
            token,
            stop,
            connections: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        });
        let (accepting, routing) = (shared.clone(), shared.clone());
        let threads = vec![
            thread::spawn(move || accept(listener, &accepting)),
            thread::spawn(move || route(replies, &routing)),
        ];
        Ok(Self { shared, threads })
    }
    /** Lanes currently attached, for status displays and tests. */
    pub fn lanes(&self) -> Vec<String> {
        let connections = self
            .shared
            .connections
            .lock()
            .map(|c| c.values().map(|c| c.lane.clone()).collect());
        connections.unwrap_or_default()
    }
}

impl Drop for Hub {
    /** Stopping revokes every attached lane before the threads exit. */
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        let connections = self
            .shared
            .connections
            .lock()
            .map(|c| c.values().cloned().collect::<Vec<_>>());
        for connection in connections.unwrap_or_default() {
            connection.disconnect(&self.shared);
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/** Accept loop: admit, assign a lane, and start the connection's reader and writer. */
fn accept(listener: UnixListener, shared: &Arc<Shared>) {
    while !shared.stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let shared = shared.clone();
                thread::spawn(move || open(stream, &shared));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => thread::sleep(POLL),
            Err(_) => thread::sleep(POLL),
        }
    }
}

/** One admitted connection: the hello, the lane, then reading until either side closes. */
fn open(mut stream: UnixStream, shared: &Arc<Shared>) {
    let _ = stream.set_nonblocking(false);
    let Some(protocol) = admit(&mut stream, &shared.token) else {
        return;
    };
    let full = shared
        .connections
        .lock()
        .map(|c| c.len() >= CONNECTIONS)
        .unwrap_or(true);
    let id = shared.next.fetch_add(1, Ordering::AcqRel);
    let lane = format!("agent-{id}");
    let welcome = if full {
        Welcome {
            ok: false,
            lane: None,
            error: Some("Too many agents are attached.".into()),
        }
    } else {
        Welcome {
            ok: true,
            lane: Some(lane.clone()),
            error: None,
        }
    };
    let line = serde_json::to_vec(&welcome).unwrap_or_default();
    if stream.write_all(&[line, b"\n".to_vec()].concat()).is_err() || full {
        return;
    }
    let (Ok(writer), Ok(closer)) = (stream.try_clone(), stream.try_clone()) else {
        return;
    };
    let (frames, outgoing) = mpsc::sync_channel(MCP_OUTPUT_QUEUE);
    let connection = Arc::new(Connection {
        id,
        lane,
        protocol,
        session: Mutex::new(Session::new(Instant::now())),
        natives: Mutex::new(HashMap::new()),
        admissions: Mutex::new(BTreeMap::new()),
        frames,
        stream: closer,
        open: AtomicBool::new(true),
        sequence: AtomicU64::new(0),
    });
    if let Ok(mut connections) = shared.connections.lock() {
        connections.insert(id, connection.clone());
    }
    let (writing, shared_writer) = (connection.clone(), shared.clone());
    let writer = thread::spawn(move || write(writer, outgoing, &writing, &shared_writer));
    read(stream, &connection, shared);
    connection.disconnect(shared);
    let _ = writer.join();
}

/** Replies come back from the host with the hub's private ID and go to the connection that sent them. */
fn route(replies: Receiver<Response>, shared: &Arc<Shared>) {
    while !shared.stop.load(Ordering::Acquire) {
        let Ok(response) = replies.recv_timeout(POLL) else {
            continue;
        };
        let owner = response
            .id
            .as_deref()
            .and_then(|id| id.strip_prefix('c'))
            .and_then(|id| id.split_once(':'))
            .and_then(|(connection, _)| connection.parse::<u64>().ok());
        let connection = owner.and_then(|id| shared.connections.lock().ok()?.get(&id).cloned());
        if let Some(connection) = connection {
            connection.complete(response, shared);
        }
    }
}

/** Newline-framed reads; idle ticks let the MCP session expire deadlines and re-check waits. */
fn read(mut stream: UnixStream, connection: &Arc<Connection>, shared: &Arc<Shared>) {
    let _ = stream.set_read_timeout(Some(POLL));
    let limit = match connection.protocol {
        Protocol::Mcp => MCP_FRAME_BYTES,
        Protocol::Native => FRAME_BYTES,
    };
    let mut pending = Vec::new();
    let mut bytes = vec![0_u8; CHUNK];
    while connection.active() && !shared.stop.load(Ordering::Acquire) {
        let count = match stream.read(&mut bytes) {
            Ok(0) => return,
            Ok(count) => count,
            Err(error)
                if [
                    ErrorKind::WouldBlock,
                    ErrorKind::TimedOut,
                    ErrorKind::Interrupted,
                ]
                .contains(&error.kind()) =>
            {
                connection.tick(shared);
                continue;
            }
            Err(_) => return,
        };
        for byte in &bytes[..count] {
            if *byte != b'\n' {
                if pending.len() >= limit {
                    return;
                }
                pending.push(*byte);
                continue;
            }
            connection.frame(&pending, shared);
            pending.clear();
            if !connection.active() {
                return;
            }
        }
    }
}

/** One writer per connection, so a stalled client never blocks routing for the others. */
fn write(
    mut stream: UnixStream,
    frames: Receiver<Vec<u8>>,
    connection: &Arc<Connection>,
    shared: &Arc<Shared>,
) {
    let _ = stream.set_write_timeout(Some(MCP_WRITE_DEADLINE));
    while connection.active() {
        match frames.recv_timeout(POLL) {
            Ok(bytes) => {
                if stream.write_all(&bytes).is_err() {
                    connection.disconnect(shared);
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
    }
}

/** One attached agent. Only correlation metadata and observed permits are retained. */
struct Connection {
    id: u64,
    lane: String,
    protocol: Protocol,
    /// MCP lifecycle, tool translation and waits; unused by native connections.
    session: Mutex<Session>,
    /// Native connections: private ID to the client's own request ID.
    natives: Mutex<HashMap<String, String>>,
    /// Permits observed at admission, so cancellation acts on actual authority.
    admissions: Mutex<BTreeMap<String, Permit>>,
    frames: SyncSender<Vec<u8>>,
    stream: UnixStream,
    open: AtomicBool,
    sequence: AtomicU64,
}

impl Connection {
    /** Whether the connection still accepts and sends frames. */
    fn active(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    /**
     * Leaving returns this lane's tabs, and only this lane's, to the person. Work already queued
     * for them fails its permit check because their epochs advance.
     */
    fn disconnect(&self, shared: &Shared) {
        if !self.open.swap(false, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut state) = authority::lock(&shared.authority) {
            state.revoke_lane(&self.lane);
        }
        let _ = self.stream.shutdown(Shutdown::Both);
        if let Ok(mut connections) = shared.connections.lock() {
            connections.remove(&self.id);
        }
    }

    /** The hub's private correlation for one request of this connection. */
    fn private_id(&self, inner: &str) -> String {
        format!("c{}:{inner}", self.id)
    }

    /** One complete inbound frame. */
    fn frame(&self, frame: &[u8], shared: &Shared) {
        match self.protocol {
            Protocol::Mcp => {
                let actions = self.session(|session| session.receive(frame, Instant::now()));
                self.actions(actions, shared);
            }
            Protocol::Native => self.native(frame, shared),
        }
    }

    /** Idle time: MCP deadlines and parked waits advance. */
    fn tick(&self, shared: &Shared) {
        if self.protocol == Protocol::Mcp {
            let actions = self.session(|session| session.poll(Instant::now()));
            self.actions(actions, shared);
        }
    }

    /** Session methods are pure; the lock never spans I/O or host admission. */
    fn session(&self, run: impl FnOnce(&mut Session) -> Vec<Action>) -> Vec<Action> {
        self.session
            .lock()
            .map(|mut session| run(&mut session))
            .unwrap_or_default()
    }

    /** Queue one outbound frame; a full queue means the client stopped reading. */
    fn send(&self, bytes: Vec<u8>, shared: &Shared) {
        if self.frames.try_send(bytes).is_err() {
            self.disconnect(shared);
        }
    }

    /** Carry out session decisions, the same way the stdio MCP transport does. */
    fn actions(&self, actions: Vec<Action>, shared: &Shared) {
        let mut pending = VecDeque::from(actions);
        while let Some(action) = pending.pop_front() {
            if !self.active() {
                return;
            }
            match action {
                Action::Send(value) => match mcp::encode(&value) {
                    Ok(bytes) => self.send(bytes, shared),
                    Err(_) => self.disconnect(shared),
                },
                Action::Dispatch(mut request) => {
                    let inner = request.id.clone();
                    request.id = self.private_id(&inner);
                    if let Err(error) = self.dispatch(request, shared) {
                        let response = Response::new(Some(inner), Err(error));
                        pending.extend(
                            self.session(|session| session.complete(response, Instant::now())),
                        );
                    }
                }
                Action::Cancel {
                    native_id,
                    tab_id,
                    control_epoch,
                    mutating,
                } => {
                    if mutating {
                        self.cancel(
                            &self.private_id(&native_id),
                            tab_id.as_deref(),
                            control_epoch,
                            shared,
                        );
                    }
                }
                Action::Close(_) => self.disconnect(shared),
            }
        }
    }

    /**
     * Admit one request: stamp this connection's lane over anything the client sent, observe the
     * tab's current permit, and queue it. Busy is an error reply, not a dropped connection.
     */
    fn dispatch(&self, mut request: Request, shared: &Shared) -> EngineResult<()> {
        if request.op == "runtime.shutdown" {
            return Err(EngineError::new(
                "control_denied",
                "Attached agents cannot shut down the person's browser.",
                "Disconnect instead; the browser keeps running.",
            ));
        }
        if let Some(params) = request.params.as_object_mut() {
            params.insert("lane".into(), json!(self.lane));
        }
        let permit = observe(&shared.authority, &request)?;
        let id = request.id.clone();
        {
            let mut admissions = self.admissions.lock().map_err(|_| busy())?;
            if admissions.len() >= MCP_PENDING_REQUESTS {
                return Err(busy());
            }
            admissions.insert(id.clone(), permit);
        }
        match shared.host.submit(request, permit) {
            Submit::Queued => Ok(()),
            outcome => {
                if let Ok(mut admissions) = self.admissions.lock() {
                    admissions.remove(&id);
                }
                if outcome == Submit::Closed {
                    self.disconnect(shared);
                }
                Err(busy())
            }
        }
    }

    /** A cancelled mutation hands the tab back to the person, if the permit is still current. */
    fn cancel(&self, id: &str, tab_id: Option<&str>, epoch: Option<u64>, shared: &Shared) {
        let captured = self
            .admissions
            .lock()
            .ok()
            .and_then(|admissions| admissions.get(id).copied());
        let tab = tab_id.and_then(|id| runtime::tab_key(id).ok());
        let (Some(captured), Some(tab), Some(epoch)) = (captured, tab, epoch) else {
            return;
        };
        if captured.owner != Owner::Agent || captured.tab_id != Some(tab) || captured.epoch != epoch
        {
            return;
        }
        if let Ok(mut state) = authority::lock(&shared.authority)
            && state.tab_status(tab) == Some((Owner::Agent, epoch))
        {
            let _ = state.change_tab(tab, Owner::Human);
        }
    }

    /** One native NDJSON request: validated, renamed to a private ID, and admitted. */
    fn native(&self, frame: &[u8], shared: &Shared) {
        let client = decode::<Value>(frame).ok().and_then(|value| {
            value
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| valid_id(id))
                .map(str::to_owned)
        });
        let request =
            decode::<Request>(frame).and_then(|request| request.validate().map(|()| request));
        let result = request.and_then(|mut request| {
            let private = self.private_id(&format!(
                "n{}",
                self.sequence.fetch_add(1, Ordering::AcqRel)
            ));
            {
                let mut natives = self.natives.lock().map_err(|_| busy())?;
                if natives.values().any(|pending| *pending == request.id) {
                    return Err(EngineError::invalid("That request ID is still pending."));
                }
                natives.insert(private.clone(), std::mem::take(&mut request.id));
            }
            request.id = private.clone();
            self.dispatch(request, shared).inspect_err(|_| {
                if let Ok(mut natives) = self.natives.lock() {
                    natives.remove(&private);
                }
            })
        });
        if let Err(error) = result {
            self.reply(Response::new(client, Err(error)), shared);
        }
    }

    /** Write one native response frame. */
    fn reply(&self, response: Response, shared: &Shared) {
        match encode(&response) {
            Ok(bytes) => self.send(bytes, shared),
            Err(_) => self.disconnect(shared),
        }
    }

    /** A host reply for this connection, renamed back to what the client knows. */
    fn complete(&self, mut response: Response, shared: &Shared) {
        let Some(private) = response.id.clone() else {
            return;
        };
        if let Ok(mut admissions) = self.admissions.lock() {
            admissions.remove(&private);
        }
        match self.protocol {
            Protocol::Mcp => {
                response.id = private.split_once(':').map(|(_, inner)| inner.to_owned());
                let actions = self.session(|session| session.complete(response, Instant::now()));
                self.actions(actions, shared);
            }
            Protocol::Native => {
                let client = self
                    .natives
                    .lock()
                    .ok()
                    .and_then(|mut natives| natives.remove(&private));
                if client.is_some() {
                    response.id = client;
                    self.reply(response, shared);
                }
            }
        }
    }
}

/** Reads observe the tab's current epoch without claiming it; the runtime decides mutation. */
fn observe(authority: &SharedAuthority, request: &Request) -> EngineResult<Permit> {
    let state = authority::lock(authority)?;
    let key = request
        .params
        .get("tab_id")
        .and_then(Value::as_str)
        .and_then(|id| runtime::tab_key(id).ok());
    Ok(key
        .and_then(|id| state.observe_for(Owner::Agent, id).ok())
        .unwrap_or(Permit {
            owner: Owner::Agent,
            epoch: state.epoch,
            tab_id: key,
        }))
}

/** The browser's command queue is full. */
fn busy() -> EngineError {
    EngineError::new(
        "busy",
        "The native action queue is full.",
        "Wait for pending work to finish and try again.",
    )
}
