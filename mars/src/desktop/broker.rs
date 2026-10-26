/** One bounded worker serializes native UI and agent operations against one Runtime. */
use crate::{
    protocol::{EngineError, EngineResult, Request, Response},
    runtime::{
        BrowserSnapshot, EnginePrograms, Runtime,
        authority::{self, Owner, Permit, SharedAuthority},
    },
};
use mars::document::RenderContext;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Bounded admission prevents stalled consumers from growing a work backlog.
pub(super) const QUEUE: usize = crate::constants::DESKTOP_QUEUE;
/// Cancellation is checked even while no commands arrive.
pub(super) const POLL: Duration = Duration::from_millis(25);

/** Transport selection never creates a separate runtime or changes startup ownership. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Transport {
    /** Human-only launch owns no agent descriptors. */
    None,
    /** Existing versioned NDJSON retains its window-on-EOF behavior. */
    Native,
    /** MCP owns its launched window for the client's stdio lifetime. */
    Mcp,
    /** A person's window that also accepts `mars mcp` and `mars agent --attach` on the per-user
    attach socket; each connection is its own lane and starts without any tab grant. */
    Attach,
}
impl From<bool> for Transport {
    /** Existing broker callers keep their native-agent boolean seam. */
    fn from(agent: bool) -> Self {
        if agent { Self::Native } else { Self::None }
    }
}

/** Secure profiles remain independent while the composition root passes them together. */
pub struct ProfileStores {
    /// Persistent cookies and their recovery marker.
    pub cookies: std::sync::Arc<dyn mars::profile::ProfileStore>,
    /// Origin-scoped Web Storage and its recovery marker.
    pub storage: std::sync::Arc<dyn mars::profile::WebStorageStore>,
    /// Ordered saved tab URLs.
    pub sessions: std::sync::Arc<dyn mars::profile::SessionStore>,
}

/// Native UI actions are admitted with a local human epoch.
pub(super) enum Command {
    /** Locally ordered edits rebase only within the same visible page generation. */
    Human {
        op: String,
        params: Value,
        page: Option<String>,
        permit: Permit,
        /** A click on page content nothing listens to is not an error the person sees. */
        quiet: bool,
    },
    /** Agent revision preconditions are never rewritten. */
    Agent(Request, Permit),
    /** OS presentation events do not grant document mutation. */
    Viewport(f64, f64),
    /** Complete OS observations reflow from one viewport measured below the Mars shell and preference. */
    Context(RenderContext),
    /** Explicit appearance replies wait for both runtime publication and native display. */
    Appearance {
        context: RenderContext,
        permit: Permit,
        id: String,
        response: Value,
    },
    /** Scrolling remains an observation action for the person. */
    Scroll(f64),
    /** Human-confirmed profile recovery remains serialized with navigation. */
    ProfileClear,
    /** A recovery retry reads secure storage without reloading the current page. */
    ProfileRetry,
    /** Explicit site-storage recovery never clears the independent cookie item. */
    ProfileStorageClear,
    /** "Clear browsing data" empties the network engine's HTTP cache. */
    CacheClear,
    /** A close decision saves session state on the runtime-owning worker. */
    SaveSession(mpsc::SyncSender<bool>),
    /** Explicit close-without-saving prevents the worker's final retry. */
    DiscardSession(mpsc::SyncSender<()>),
    /** Save Link fetches one address as the person, through the governed network path, for saving. */
    Download {
        id: u64,
        url: String,
        permit: Permit,
    },
}

/// Replies are acknowledged only after the UI has consumed their immutable snapshot.
pub(super) enum Event {
    /** A long agent action leaves the committed page visible while work continues. */
    Loading(String),
    /** Successful startup and every completed engine action feed the same view. */
    Update {
        snapshot: Box<BrowserSnapshot>,
        result: EngineResult<Value>,
        agent_id: Option<String>,
        /** Internal correlation prevents reused client IDs from acknowledging appearance early. */
        appearance_update: bool,
        clear_error: bool,
        destination: Option<String>,
        rejected_edit: Option<(String, usize, String)>,
    },
    /** OS-only observations and captures execute on the owning main thread. */
    Window(Request, Permit),
    /** Profile recovery is reported without inventing a new page snapshot. */
    Profile {
        result: EngineResult<&'static str>,
        snapshot: Box<BrowserSnapshot>,
    },
    /** Startup cannot leave a misleading live-looking shell. */
    Failed(EngineError),
    /** A fetched download; the UI thread saves it to the folder the person chose. */
    Download {
        id: u64,
        result: EngineResult<crate::runtime::downloads::Fetched>,
    },
    /** A navigation answered with a file instead of a page; the UI thread lists and saves it. */
    NavigationDownload(crate::runtime::downloads::Fetched),
}

/// All tasks are cancellable and joined before the desktop owner exits.
pub(super) struct Broker {
    pub commands: SyncSender<Command>,
    pub events: Receiver<Event>,
    pub replies: SyncSender<Response>,
    pub authority: SharedAuthority,
    pub cancel: Arc<AtomicBool>,
    pub connected: Arc<AtomicBool>,
    /** MCP close semantics are distinct from the existing NDJSON compatibility path. */
    pub transport: Transport,
    worker: Option<JoinHandle<()>>,
    io: Vec<JoinHandle<()>>,
    /** The attach hub and the endpoint it bound; the socket is removed when the window closes. */
    attach: Option<(mars::mcp::attach::Hub, mars::mcp::attach::Endpoint)>,
}

/** Attached agents enter through the same bounded command queue as every other agent. */
struct AttachHost {
    commands: SyncSender<Command>,
}
impl mars::mcp::attach::Host for AttachHost {
    /** A full queue is Busy and a stopped worker is Closed; nothing waits on the UI thread. */
    fn submit(&self, request: Request, permit: Permit) -> mars::mcp::attach::Submit {
        use mars::mcp::attach::Submit;
        match self.commands.try_send(Command::Agent(request, permit)) {
            Ok(()) => Submit::Queued,
            Err(mpsc::TrySendError::Full(_)) => Submit::Busy,
            Err(mpsc::TrySendError::Disconnected(_)) => Submit::Closed,
        }
    }
}

/** Listen on the per-user attach socket. A socket another browser already owns, or a directory
that is not private, leaves the window without agents instead of failing startup. */
fn attach(
    commands: SyncSender<Command>,
    replies: Receiver<Response>,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
) -> Option<(mars::mcp::attach::Hub, mars::mcp::attach::Endpoint)> {
    let endpoint = mars::mcp::attach::Endpoint::default_location();
    let started = endpoint.bind().and_then(|(listener, token)| {
        let host = Arc::new(AttachHost { commands });
        mars::mcp::attach::Hub::start(listener, token, host, replies, authority, cancel)
    });
    match started {
        Ok(hub) => Some((hub, endpoint)),
        Err(error) => {
            eprintln!("Agents cannot attach to this window: {}", error.message);
            None
        }
    }
}

impl Broker {
    /** Only the executable composition root supplies service paths and fixture grants. */
    pub fn start(
        program: PathBuf,
        origins: Vec<String>,
        agent: impl Into<Transport>,
        owner: Owner,
        initial: Option<String>,
    ) -> EngineResult<Self> {
        Self::start_with_profiles(
            program,
            origins,
            agent,
            owner,
            initial,
            ProfileStores {
                cookies: mars::profile::system_store(),
                storage: mars::profile::system_web_storage_store(),
                sessions: mars::profile::system_session_store(),
            },
        )
    }
    /** Isolated profile injection keeps broker tests hermetic and failure-testable. */
    pub fn start_with_profiles(
        program: PathBuf,
        origins: Vec<String>,
        agent: impl Into<Transport>,
        owner: Owner,
        initial: Option<String>,
        stores: ProfileStores,
    ) -> EngineResult<Self> {
        let transport = agent.into();
        let (commands, input) = mpsc::sync_channel(QUEUE);
        let (updates, events) = mpsc::sync_channel(QUEUE);
        let (replies, output) = mpsc::sync_channel(QUEUE);
        let authority = crate::runtime::authority::Authority::shared(owner);
        let cancel = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(matches!(
            transport,
            Transport::Native | Transport::Mcp
        )));
        let guard = Arc::clone(&authority);
        let stop = Arc::clone(&cancel);
        let mut hub = None;
        let io = match transport {
            Transport::Native => super::io::start(
                commands.clone(),
                output,
                authority.clone(),
                cancel.clone(),
                connected.clone(),
            )?,
            Transport::Mcp => super::mcp_io::start(
                commands.clone(),
                output,
                authority.clone(),
                cancel.clone(),
                connected.clone(),
            )?,
            Transport::Attach => {
                hub = attach(commands.clone(), output, authority.clone(), cancel.clone());
                Vec::new()
            }
            Transport::None => Vec::new(),
        };
        let start = WorkerStart {
            program,
            origins,
            initial,
            mcp: transport == Transport::Mcp,
            profile: stores.cookies,
            storage: stores.storage,
            sessions: stores.sessions,
        };
        let worker = thread::spawn(move || worker(start, guard, stop, input, updates));
        Ok(Self {
            commands,
            events,
            replies,
            authority,
            cancel,
            connected,
            transport,
            worker: Some(worker),
            io,
            attach: hub,
        })
    }
    /** Permission is captured before queue admission, never inferred when work finishes. */
    pub fn permit(&self, owner: Owner, tab_id: &str) -> EngineResult<Permit> {
        let state = authority::lock(&self.authority)?;
        state.permit_for(owner, crate::runtime::tab_key(tab_id)?)
    }
    /// UI-only tab actions do not inherit or depend on the selected tab's agent grant.
    pub fn local_tab_permit(&self) -> EngineResult<Permit> {
        let state = authority::lock(&self.authority)?;
        Ok(Permit {
            owner: Owner::Human,
            epoch: state.epoch,
            tab_id: None,
        })
    }
    /** The native toolbar grants or revokes one exact tab's agent access. */
    pub fn change(&self, owner: Owner, tab_id: &str) -> EngineResult<()> {
        authority::lock(&self.authority)?.change_tab(crate::runtime::tab_key(tab_id)?, owner)?;
        Ok(())
    }
    /// Agent disconnect and transport failure revoke every tab immediately.
    pub fn revoke_agents(&self) {
        if let Ok(mut authority) = self.authority.lock() {
            authority.revoke_agents();
        }
    }
    /** MCP failure and local close revoke authority before cancelling all pending publication. */
    pub fn close_mcp(&self) {
        self.revoke_agents();
        self.connected.store(false, Ordering::Release);
        self.cancel.store(true, Ordering::Release);
    }
    /** Invalidate queued human and agent permits before a close-save decision. */
    pub fn begin_close(&self) {
        if let Ok(mut authority) = self.authority.lock() {
            authority.change(Owner::Human);
        }
    }
    /** Ask the runtime owner to write the latest session and return only success state. */
    pub fn save_session(&self) -> bool {
        self.session_command(Command::SaveSession)
    }
    /** Disable the final retry after the person explicitly chose to close without saving. */
    pub fn discard_session(&self) -> bool {
        self.session_command_ack(Command::DiscardSession)
    }
    /** Bridge one bounded close decision through the runtime-owning worker. */
    fn session_command(&self, command: fn(mpsc::SyncSender<bool>) -> Command) -> bool {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands.try_send(command(reply)).is_ok() && result.recv().unwrap_or(false)
    }
    /** Confirm an explicit discard was applied before allowing the window to close. */
    fn session_command_ack(&self, command: fn(mpsc::SyncSender<()>) -> Command) -> bool {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands.try_send(command(reply)).is_ok() && result.recv().is_ok()
    }
}
impl Drop for Broker {
    /** Polling I/O and child waits observe cancellation before tasks are joined. */
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        for worker in self.io.drain(..) {
            let _ = worker.join();
        }
        if let Some((hub, endpoint)) = self.attach.take() {
            drop(hub);
            endpoint.remove();
        }
    }
}

/// Startup-only inputs are grouped so worker lifetime has one coherent owner.
struct WorkerStart {
    /// The executable shared by native network/document/V9 services.
    program: PathBuf,
    /// Fixture grants never come from page or agent input.
    origins: Vec<String>,
    /// Optional navigation occurs only after profile restoration.
    initial: Option<String>,
    /// Session-owned windows report bounded final save failures without a modal dialog.
    mcp: bool,
    /// Desktop cookies are persisted only through secure storage.
    profile: std::sync::Arc<dyn mars::profile::ProfileStore>,
    /// Storage values remain independent from cookies and the tab-session item.
    storage: std::sync::Arc<dyn mars::profile::WebStorageStore>,
    /// Session URLs remain in a third independent encrypted record.
    sessions: std::sync::Arc<dyn mars::profile::SessionStore>,
}

/** Failed startup or a closed UI receiver cannot strand supervised children. */
fn worker(
    start: WorkerStart,
    authority: SharedAuthority,
    cancel: Arc<AtomicBool>,
    input: Receiver<Command>,
    events: SyncSender<Event>,
) {
    let WorkerStart {
        program,
        origins,
        initial,
        mcp,
        profile,
        storage,
        sessions,
    } = start;
    let mut runtime = match Runtime::desktop_with_session_profiles(
        EnginePrograms::same(&program),
        origins,
        authority.clone(),
        cancel.clone(),
        profile,
        storage,
        sessions,
    ) {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = events.try_send(Event::Failed(error));
            return;
        }
    };
    let first_paint = events.clone();
    runtime.show_first_paint(Box::new(move |snapshot| {
        let _ = first_paint.try_send(Event::Update {
            snapshot: Box::new(snapshot),
            result: Ok(json!({})),
            agent_id: None,
            appearance_update: false,
            clear_error: false,
            destination: None,
            rejected_edit: None,
        });
    }));
    let destination = initial.clone();
    let result = if let Some(url) = initial {
        let state = authority::lock(&authority).expect("new authority");
        let permit = Permit {
            owner: state.owner,
            epoch: state.epoch,
            tab_id: Some(1),
        };
        drop(state);
        let mut params = json!({"url":url,"tab_id":runtime.selected_tab_id()});
        if permit.owner == Owner::Agent {
            params["control_epoch"] = json!(permit.epoch);
        }
        runtime.handle_as(Request::new("startup", "page.navigate", params), permit)
    } else {
        runtime.restore_session().map(|_| json!({}))
    };
    let _ = events.try_send(Event::Update {
        snapshot: Box::new(runtime.snapshot()),
        result,
        agent_id: None,
        appearance_update: false,
        clear_error: true,
        destination,
        rejected_edit: None,
    });
    // A command that arrived while a background run was going; it runs next, ahead of the queue.
    let mut stashed: Option<Command> = None;
    while !cancel.load(Ordering::Relaxed) {
        let next = match stashed.take() {
            Some(command) => Ok(command),
            None => input.recv_timeout(POLL),
        };
        match next {
            Ok(command) => {
                if execute(&mut runtime, command, &events).is_err() {
                    break;
                }
                if let Some(fetched) = runtime.take_download()
                    && events.try_send(Event::NavigationDownload(fetched)).is_err()
                {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Idle time runs the visible page's timers and requests; a change repaints.
                let mut waiting = || {
                    if stashed.is_none() {
                        stashed = input.try_recv().ok();
                    }
                    stashed.is_some()
                };
                if runtime.run_page_scripts_yielding(&mut waiting)
                    && events
                        .try_send(Event::Update {
                            snapshot: Box::new(runtime.snapshot()),
                            result: Ok(json!({})),
                            agent_id: None,
                            appearance_update: false,
                            clear_error: false,
                            destination: None,
                            rejected_edit: None,
                        })
                        .is_err()
                {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    if !runtime.stop() && mcp {
        eprintln!("MCP window closed; the latest tab session could not be saved.");
    }
}

/** UI-only requests never gain a second document model or an agent self-grant path. */
fn execute(runtime: &mut Runtime, command: Command, events: &SyncSender<Event>) -> Result<(), ()> {
    if let Command::SaveSession(reply) = command {
        let _ = reply.send(runtime.save_session_now());
        return Ok(());
    }
    if let Command::DiscardSession(reply) = command {
        runtime.discard_pending_session();
        let _ = reply.send(());
        return Ok(());
    }
    if let Command::Download { id, url, permit } = command {
        let result = runtime.download(&url, permit);
        return events
            .try_send(Event::Download { id, result })
            .map_err(|_| ());
    }
    if matches!(
        &command,
        Command::ProfileClear
            | Command::ProfileRetry
            | Command::ProfileStorageClear
            | Command::CacheClear
    ) {
        let result = match command {
            Command::ProfileClear => runtime.clear_profile().map(|had_cookies| {
                if had_cookies {
                    "Saved cookies cleared."
                } else {
                    "There are no saved cookies to clear."
                }
            }),
            Command::ProfileRetry => runtime.retry_profile().map(|()| "Saved cookies restored."),
            Command::ProfileStorageClear => runtime
                .clear_saved_web_storage()
                .map(|()| "Saved site storage cleared. You can continue browsing."),
            Command::CacheClear => runtime.clear_cache().map(|()| "Cached files cleared."),
            _ => unreachable!(),
        };
        return events
            .try_send(Event::Profile {
                result,
                snapshot: Box::new(runtime.snapshot()),
            })
            .map_err(|_| ());
    }
    if let Command::Appearance {
        context,
        permit,
        id,
        response,
    } = command
    {
        let result = runtime
            .set_render_context_as(context, permit)
            .map(|()| response);
        return events
            .try_send(Event::Update {
                snapshot: Box::new(runtime.snapshot()),
                result,
                agent_id: Some(id),
                appearance_update: true,
                clear_error: false,
                destination: None,
                rejected_edit: None,
            })
            .map_err(|_| ());
    }
    if let Command::Agent(request, permit) = command {
        if request.op.starts_with("window.") || request.op == "page.screenshot" {
            return events
                .try_send(Event::Window(request, permit))
                .map_err(|_| ());
        }
        let id = request.id.clone();
        let clear_error = crate::runtime::mutating(&request.op);
        let destination = request
            .params
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if clear_error {
            events
                .try_send(Event::Loading(request.op.clone()))
                .map_err(|_| ())?;
        }
        let timed = std::time::Instant::now();
        let result = runtime.handle_as(request, permit);
        crate::runtime::perf::record("broker.handle", timed, String::new);
        let timed = std::time::Instant::now();
        let snapshot = Box::new(runtime.snapshot());
        crate::runtime::perf::record("broker.snapshot", timed, String::new);
        return events
            .try_send(Event::Update {
                snapshot,
                result,
                agent_id: Some(id),
                appearance_update: false,
                clear_error,
                destination,
                rejected_edit: None,
            })
            .map_err(|_| ());
    }
    let clear_error = matches!(command, Command::Human { .. });
    let edited = if let Command::Human { op, params, .. } = &command {
        if op == "page.type"
            && !runtime.snapshot().inspection["nodes"]
                .as_array()
                .is_some_and(|nodes| {
                    nodes
                        .iter()
                        .any(|n| n["node_id"] == params["node_id"] && n["protected"] == true)
                })
        {
            params["page_id"]
                .as_str()
                .zip(params["node_id"].as_str())
                .zip(params["text"].as_str())
                .and_then(|((page, node), text)| {
                    node.rsplit_once("node-")
                        .and_then(|(_, id)| id.parse::<usize>().ok())
                        .map(|id| (page.to_owned(), id, text.to_owned()))
                })
        } else {
            None
        }
    } else {
        None
    };
    let destination = if let Command::Human { params, .. } = &command {
        params.get("url").and_then(Value::as_str).map(str::to_owned)
    } else {
        None
    };
    let result = match command {
        Command::Human {
            op,
            mut params,
            page,
            permit,
            quiet,
        } => {
            let current = runtime.snapshot().inspection;
            let result = if page.is_some() && page.as_deref() != current["page_id"].as_str() {
                Err(EngineError::new(
                    "invalid_target",
                    "The visible page changed before this action.",
                    "Try again on the current page.",
                ))
            } else {
                if params.get("expected_revision").is_some() {
                    params["expected_revision"] = current["revision"].clone();
                }
                if op.starts_with("page.") {
                    params["tab_id"] = current["tab_id"].clone();
                }
                runtime.handle_as(Request::new("human", &op, params), permit)
            };
            match result {
                Err(error) if quiet && error.code == "invalid_target" => Ok(json!({})),
                other => other,
            }
        }
        Command::Viewport(width, height) => runtime.set_viewport(width, height).map(|()| json!({})),
        Command::Context(context) => runtime.set_render_context(context).map(|()| json!({})),
        Command::Scroll(y) => {
            runtime.observe_scroll(y);
            Ok(json!({}))
        }
        Command::Agent(_, _)
        | Command::Appearance { .. }
        | Command::ProfileClear
        | Command::ProfileRetry
        | Command::ProfileStorageClear
        | Command::CacheClear
        | Command::SaveSession(_)
        | Command::DiscardSession(_)
        | Command::Download { .. } => unreachable!(),
    };
    let rejected_edit = if result.is_err() { edited } else { None };
    events
        .try_send(Event::Update {
            snapshot: Box::new(runtime.snapshot()),
            result,
            agent_id: None,
            appearance_update: false,
            clear_error,
            destination,
            rejected_edit,
        })
        .map_err(|_| ())
}
