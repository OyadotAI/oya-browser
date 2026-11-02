/**
 * Typed in-process calls. An engine running as a thread of this process receives a closure and
 * returns a Rust value, so nothing is encoded to JSON and decoded again on either side: on
 * YouTube that encoding (and the SHA-256 that let scripts skip it) was about a fifth of every
 * step. The job runs on the engine's own thread, so Eye's and V9's thread-local caches stay warm,
 * and the caller waits under the same deadline and cancellation polling as a framed call.
 * Subprocess services (MARS_PROCESS_ISOLATION=1) never receive a job and keep the framed path.
 */
use super::Service;
use crate::constants::RPC_POLL;
use crate::protocol::{EngineError, EngineResult};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender};
use std::time::Instant;

/** Work handed to an in-process engine thread; it sends its own result back. */
pub(in crate::runtime) type Job = Box<dyn FnOnce() + Send>;

/** What an engine's request channel carries. */
pub(in crate::runtime) enum Inbound {
    /** An encoded request, answered by an encoded reply on the reply channel. */
    Frame(Vec<u8>),
    /** A typed call; it answers through the one-shot channel inside it. */
    Job(Job),
}

impl Service {
    /** Whether typed jobs can reach this service: only a thread of this process can take them. */
    pub(in crate::runtime) fn takes_jobs(&self) -> bool {
        self.child.is_none() && self.requests.is_some()
    }

    /**
     * Run `work` on the engine's thread and return its value. The op names the call for its
     * deadline and the perf log. A missed deadline leaves the service wedged exactly as a
     * framed call does, and the caller's revive replaces it.
     */
    pub(in crate::runtime) fn call_job<R: Send + 'static>(
        &mut self,
        op: &str,
        deadline: Option<Instant>,
        cancellation: &super::super::cancellation::Cancellation,
        work: impl FnOnce() -> EngineResult<R> + Send + 'static,
    ) -> EngineResult<R> {
        cancellation
            .check()
            .map_err(|_| super::super::authority::denied())?;
        let started = Instant::now();
        let deadline = super::rpc_deadline(op, started, deadline)?;
        let (answer, result) = mpsc::sync_channel(1);
        let job: Job = Box::new(move || {
            let _ = answer.send(work());
        });
        self.requests
            .as_ref()
            .ok_or_else(|| EngineError::service(self.name, false))?
            .try_send(Inbound::Job(job))
            .map_err(|_| EngineError::service(self.name, false))?;
        let value = self.await_job(op, started, deadline, cancellation, &result);
        super::perf::record("rpc.total", started, || op.to_string());
        value
    }

    /** Poll for a job's result until it arrives, the deadline passes or the caller is cancelled. */
    fn await_job<R>(
        &self,
        op: &str,
        called: Instant,
        deadline: Instant,
        cancellation: &super::super::cancellation::Cancellation,
        result: &mpsc::Receiver<EngineResult<R>>,
    ) -> EngineResult<R> {
        loop {
            cancellation.check()?;
            if self.cancel.load(Ordering::Relaxed) {
                return Err(EngineError::service(self.name, false));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let budget = deadline.saturating_duration_since(called);
                return Err(super::super::restart::deadline(self.name, op, budget));
            }
            match result.recv_timeout(remaining.min(RPC_POLL)) {
                Ok(value) => return value,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(EngineError::service(self.name, false));
                }
            }
        }
    }
}

impl super::Runtime {
    /**
     * A typed call when `name` runs in this process; `None` when it is a subprocess, so the
     * caller takes its framed path. Failures recover as a framed call's do: a cancelled call
     * replaces the engine, a failed or wedged engine is revived.
     */
    pub(in crate::runtime) fn call_job<R: Send + 'static>(
        &mut self,
        name: &str,
        op: &str,
        deadline: Option<Instant>,
        work: impl FnOnce() -> EngineResult<R> + Send + 'static,
    ) -> Option<EngineResult<R>> {
        if let Err(error) = self.healthy() {
            return Some(Err(error));
        }
        let cancellation =
            super::super::cancellation::Cancellation::new(self.authority.clone(), self.permit);
        let service = self
            .services
            .iter_mut()
            .find(|service| service.name == name)?;
        if !service.takes_jobs() {
            return None;
        }
        Some(match service.call_job(op, deadline, &cancellation, work) {
            Err(error) if error.code == "navigation_cancelled" => Err(self.replace_cancelled(name)),
            Err(error) if error.service.is_some() => Err(self.revive(name, Some(op), Some(error))),
            result => result,
        })
    }
}

impl super::Runtime {
    /** A sender for typed jobs to `name`'s engine thread when it runs in this process: what a
    host callback uses to run work there from another thread (V9's layout calls into Eye). */
    pub(in crate::runtime) fn job_sender(&self, name: &str) -> Option<SyncSender<Inbound>> {
        let service = self.services.iter().find(|service| service.name == name)?;
        service
            .takes_jobs()
            .then(|| service.requests.clone())
            .flatten()
    }
}
