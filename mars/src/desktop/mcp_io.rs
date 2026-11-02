/** MCP stdio owns bounded framing, native admission and disconnect cleanup without a second runtime. */
#[path = "mcp_io/admission.rs"]
mod admission;
#[path = "mcp_io/input.rs"]
mod input;
#[path = "mcp_io/output.rs"]
mod output;
use super::{
    broker::{Command, POLL},
    io::Descriptor,
};
use mars::{
    constants::MCP_OUTPUT_QUEUE,
    mcp::{self, Action, Session},
    protocol::{EngineError, EngineResult, Request, Response},
    runtime::{
        self,
        authority::{self, Owner, Permit, SharedAuthority},
    },
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Instant,
};

/** One connection shares pure session state while every blocking operation stays outside its lock. */
struct Connection {
    session: Mutex<Session>,
    /// Actual observed permits prevent cancellation from treating client epochs as authority.
    admissions: admission::Admissions,
    commands: SyncSender<Command>,
    frames: SyncSender<Vec<u8>>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
}

/** Existing broker ownership joins all three pollable tasks on any close path. */
pub(super) fn start(
    commands: SyncSender<Command>,
    responses: Receiver<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) -> EngineResult<Vec<JoinHandle<()>>> {
    let input = Descriptor::new(libc::STDIN_FILENO)?;
    let output = Descriptor::new(libc::STDOUT_FILENO)?;
    Ok(start_descriptors(
        input, output, commands, responses, authority, cancel, connected,
    ))
}

/** Descriptor injection lets transport tests use private pipes without redirecting process stdio. */
pub(super) fn start_descriptors(
    input: Descriptor,
    output: Descriptor,
    commands: SyncSender<Command>,
    responses: Receiver<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) -> Vec<JoinHandle<()>> {
    let (frames, outgoing) = mpsc::sync_channel(MCP_OUTPUT_QUEUE);
    let connection = Arc::new(Connection {
        session: Mutex::new(Session::new(Instant::now())),
        admissions: admission::Admissions::default(),
        commands,
        frames,
        authority,
        cancel,
        connected,
    });
    let reader = connection.clone();
    let writer = connection.clone();
    vec![
        thread::spawn(move || input::read(input, &reader)),
        thread::spawn(move || output::write(output, outgoing, &writer)),
        thread::spawn(move || bridge(responses, &connection)),
    ]
}

impl Connection {
    /** Session methods are pure and their lock never spans native admission or descriptor I/O. */
    fn session(&self, run: impl FnOnce(&mut Session) -> Vec<Action>) -> Vec<Action> {
        match self.session.lock() {
            Ok(mut session) => run(&mut session),
            Err(error) => {
                drop(error);
                self.disconnect("MCP session lock failed.");
                Vec::new()
            }
        }
    }
    /** All terminal paths revoke publication permits before requesting bounded process cleanup. */
    fn disconnect(&self, reason: &'static str) {
        if self.connected.swap(false, Ordering::AcqRel) {
            if let Ok(mut state) = authority::lock(&self.authority) {
                state.revoke_agents();
            }
            self.cancel.store(true, Ordering::Release);
            eprintln!("{reason}");
        }
    }
    /** Cancellation is immediately visible even while the runtime waits on an owned engine. */
    fn active(&self) -> bool {
        self.connected.load(Ordering::Acquire) && !self.cancel.load(Ordering::Acquire)
    }
    /** Output and command admission are bounded and never wait while holding the session lock. */
    fn actions(&self, actions: Vec<Action>) {
        let mut pending = VecDeque::from(actions);
        while self.active() {
            let Some(action) = pending.pop_front() else {
                return;
            };
            match action {
                Action::Send(value) => {
                    let result = mcp::encode(&value).and_then(|bytes| {
                        self.frames
                            .try_send(bytes)
                            .map_err(|_| EngineError::invalid("MCP output queue is full."))
                    });
                    if result.is_err() {
                        self.disconnect("MCP output could not be queued.");
                    }
                }
                Action::Dispatch(request) => {
                    if let Some(response) = self.dispatch(request) {
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
                    if mutating
                        && self
                            .admissions
                            .cancel(
                                &self.authority,
                                &native_id,
                                tab_id.as_deref(),
                                control_epoch,
                            )
                            .is_err()
                    {
                        self.disconnect("MCP native authority failed.");
                    }
                }
                Action::Close(reason) => self.disconnect(reason),
            }
        }
    }
    /** Saturated native admission is a correlated tool error rather than a broken connection. */
    fn dispatch(&self, request: Request) -> Option<Response> {
        let id = request.id.clone();
        let permit = match observe(&self.authority, &request) {
            Ok(permit) => permit,
            Err(_) => {
                self.disconnect("MCP native authority failed.");
                return None;
            }
        };
        if self.admissions.capture(&id, permit).is_err() {
            self.disconnect("MCP native admission state failed.");
            return None;
        }
        match self.commands.try_send(Command::Agent(request, permit)) {
            Ok(()) => None,
            Err(mpsc::TrySendError::Full(_)) => {
                if self.admissions.remove(Some(&id)).is_err() {
                    self.disconnect("MCP native admission state failed.");
                    return None;
                }
                Some(Response::new(
                    Some(id),
                    Err(EngineError::new(
                        "busy",
                        "The native action queue is full.",
                        "Wait for pending work to finish and try again.",
                    )),
                ))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                let _ = self.admissions.remove(Some(&id));
                self.disconnect("MCP native command channel closed.");
                None
            }
        }
    }
}

/** Read observation captures the exact tab epoch without granting mutation ownership. */
fn observe(authority: &SharedAuthority, request: &Request) -> EngineResult<Permit> {
    let state = authority::lock(authority)?;
    let key = request
        .params
        .get("tab_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|id| runtime::tab_key(id).ok());
    Ok(key
        .and_then(|id| state.observe_for(Owner::Agent, id).ok())
        .unwrap_or(Permit {
            owner: Owner::Agent,
            epoch: state.epoch,
            tab_id: key,
        }))
}

/** UI-acknowledged native replies and deadlines share one bounded completion path. */
fn bridge(responses: Receiver<Response>, connection: &Connection) {
    while connection.active() {
        match responses.recv_timeout(POLL) {
            Ok(response) => {
                if connection
                    .admissions
                    .remove(response.id.as_deref())
                    .is_err()
                {
                    connection.disconnect("MCP native admission state failed.");
                    return;
                }
                connection.actions(
                    connection.session(|session| session.complete(response, Instant::now())),
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => {
                connection.disconnect("MCP native response channel closed.");
                return;
            }
        }
        connection.actions(connection.session(|session| session.poll(Instant::now())));
    }
}
