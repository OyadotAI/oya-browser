/**
 * A headless attachable browser (`mars serve`) and the stdio bridge (`mars mcp`). The headless
 * host owns one desktop-mode Runtime on a worker thread and runs each admitted request through
 * `Runtime::handle_as`, exactly as the desktop broker does.
 */
use super::{Endpoint, Host, Hub, Protocol, Submit};
use crate::{
    constants::DESKTOP_QUEUE,
    protocol::{EngineError, EngineResult, Request, Response},
    runtime::{
        EnginePrograms, Runtime,
        authority::{Authority, Owner, Permit, SharedAuthority},
    },
};
use std::{
    io,
    net::Shutdown,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// The worker checks for shutdown this often while idle.
const POLL: Duration = Duration::from_millis(25);

/** The command side of a headless runtime. */
pub struct RuntimeHost {
    commands: SyncSender<(Request, Permit)>,
}

impl Host for RuntimeHost {
    /** A bounded queue: a full one is Busy, a stopped worker is Closed. */
    fn submit(&self, request: Request, permit: Permit) -> Submit {
        match self.commands.try_send((request, permit)) {
            Ok(()) => Submit::Queued,
            Err(TrySendError::Full(_)) => Submit::Busy,
            Err(TrySendError::Disconnected(_)) => Submit::Closed,
        }
    }
}

impl RuntimeHost {
    /**
     * Start a desktop-mode runtime (tabs, ownership and lanes, no window) on its own thread. The
     * returned receiver carries one reply per queued request, for `Hub::start`.
     */
    pub fn start(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
    ) -> EngineResult<(Arc<Self>, Receiver<Response>, JoinHandle<()>)> {
        let (commands, input) = mpsc::sync_channel(DESKTOP_QUEUE);
        let (replies, output) = mpsc::sync_channel(DESKTOP_QUEUE);
        let (started, startup) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let runtime = Runtime::desktop(programs, origins, authority, cancel.clone());
            let mut runtime = match runtime {
                Ok(runtime) => {
                    let _ = started.send(Ok(()));
                    runtime
                }
                Err(error) => {
                    let _ = started.send(Err(error));
                    return;
                }
            };
            work(&mut runtime, &input, &replies, &cancel);
            runtime.stop();
        });
        startup
            .recv()
            .map_err(|_| EngineError::invalid("The headless browser did not start."))??;
        Ok((Arc::new(Self { commands }), output, worker))
    }
}

/** Serve requests one at a time until cancelled or the hub goes away. */
fn work(
    runtime: &mut Runtime,
    input: &Receiver<(Request, Permit)>,
    replies: &SyncSender<Response>,
    cancel: &AtomicBool,
) {
    while !cancel.load(Ordering::Acquire) {
        match input.recv_timeout(POLL) {
            Ok((request, permit)) => {
                let id = request.id.clone();
                let result = runtime.handle_as(request, permit);
                if replies.send(Response::new(Some(id), result)).is_err() {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
    }
}

/**
 * `mars serve`: a windowless browser agents attach to. With `agent_control` the first tab starts
 * granted to agents; otherwise agents work in tabs they open. Runs until the process is stopped.
 */
pub fn serve_headless(executable: &std::path::Path, agent_control: bool) -> EngineResult<()> {
    let endpoint = Endpoint::default_location();
    let owner = if agent_control {
        Owner::Agent
    } else {
        Owner::Human
    };
    let authority = Authority::shared(owner);
    let cancel = Arc::new(AtomicBool::new(false));
    let (listener, token) = endpoint.bind()?;
    let (host, replies, _worker) = RuntimeHost::start(
        EnginePrograms::same(executable),
        Vec::new(),
        authority.clone(),
        cancel.clone(),
    )?;
    let _hub = Hub::start(listener, token, host, replies, authority, cancel)?;
    eprintln!(
        "Mars is listening for agents at {}. Connect with `mars mcp`.",
        endpoint.socket().display()
    );
    loop {
        thread::park();
    }
}

/**
 * `mars mcp` and `mars agent --attach`: join the running browser over its endpoint and copy stdio
 * both ways until either side closes.
 */
pub fn bridge_stdio(protocol: Protocol) -> EngineResult<()> {
    let (stream, _lane) = Endpoint::default_location().connect(protocol)?;
    let _ = stream.set_read_timeout(None);
    let mut upstream = stream
        .try_clone()
        .map_err(|_| EngineError::invalid("The agent connection could not be shared."))?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut upstream);
        let _ = upstream.shutdown(Shutdown::Write);
    });
    io::copy(&mut &stream, &mut io::stdout().lock())
        .map_err(|_| EngineError::invalid("The agent connection closed."))?;
    Ok(())
}
