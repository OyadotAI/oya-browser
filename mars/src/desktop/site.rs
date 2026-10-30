/** An owned two-page site exercises real HTTP, input, V9 events, links and history. */
#[path = "site/request.rs"]
mod request;
use crate::protocol::{EngineError, EngineResult};
use std::{
    io::Write,
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/** Bounded parking and reads keep demo shutdown predictable. */
const POLL: Duration = Duration::from_millis(10);
/** Per-operation caps and a total header budget prevent slow peers from owning shutdown. */
const SOCKET_WAIT: Duration = Duration::from_millis(250);
/** The fixed local demo never needs an open-ended HTTP request stream. */
const HEADER_DEADLINE: Duration = Duration::from_millis(500);

/// Listener lifetime belongs to the window, separately from the three engine children.
pub(super) struct Site {
    pub origin: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Site {
    /** Stop demo acceptance without making its bounded socket cleanup block native events. */
    pub fn shutdown_finished(&mut self) -> bool {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
            if !worker.is_finished() {
                return false;
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        true
    }

    /// Exact startup origin is the sole exception to public-only desktop egress.
    pub fn start() -> EngineResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(error)?;
        listener.set_nonblocking(true).map_err(error)?;
        let origin = format!("http://{}", listener.local_addr().map_err(error)?);
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let worker = thread::spawn(move || serve(listener, &signal));
        Ok(Self {
            origin,
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for Site {
    /** Join after bounded reads; no self-connect wakeup or detached server survives. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
/** Complete bounded headers precede a fixed in-memory response. */
fn serve(listener: TcpListener, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        if let Ok((mut socket, _)) = listener.accept() {
            let _ = socket.set_nonblocking(false);
            let _ = socket.set_write_timeout(Some(SOCKET_WAIT));
            let deadline = Instant::now() + HEADER_DEADLINE;
            if let Some(input) = request::read(&mut socket, stop, deadline)
                && !stop.load(Ordering::Relaxed)
                && Instant::now() < deadline
            {
                let _ = socket.write_all(request::response(&input).as_bytes());
            }
        }
        thread::park_timeout(POLL);
    }
}
/** Demo failures stay explicit and do not select a different engine. */
fn error(_: std::io::Error) -> EngineError {
    EngineError::invalid("Could not start the owned local demo site.")
}
