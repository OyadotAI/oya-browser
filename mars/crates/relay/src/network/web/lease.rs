/** Prepared connections close at their deadline even while the service waits for another frame. */
use crate::protocol::{EngineError, EngineResult};
use std::{
    net::{Shutdown, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Instant,
};
/** The timer is joined promptly when a request completes or is replaced. */
pub(super) struct Lease {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Lease {
    /** A cloned descriptor can close I/O without owning TLS or any application data. */
    pub fn start(socket: &TcpStream, deadline: Instant) -> EngineResult<Self> {
        let socket = socket
            .try_clone()
            .map_err(|_| EngineError::service("network", false))?;
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::spawn(move || {
            loop {
                if signal.load(Ordering::Relaxed) {
                    return;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    let _ = socket.shutdown(Shutdown::Both);
                    return;
                }
                thread::park_timeout(remaining);
            }
        });
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for Lease {
    /** Replacement, EOF and normal completion release the timer without waiting out its deadline. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
