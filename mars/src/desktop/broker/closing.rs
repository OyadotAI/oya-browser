/** Close acknowledgements and owned teardown never wait on the native event thread. */
use super::*;

/** A single close attempt owns its acknowledgement and can retire queued work. */
pub(in crate::desktop) struct SaveTicket {
    /** Dropping an attempt cannot let its late result close a subsequent attempt. */
    result: Receiver<bool>,
    /** A retired queued save is skipped; an already running store operation may finish. */
    active: Arc<AtomicBool>,
}
impl SaveTicket {
    /** Pending, success and failure remain distinct without waiting for the runtime. */
    pub fn poll(&self) -> Option<bool> {
        match self.result.try_recv() {
            Ok(saved) => Some(saved),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(false),
        }
    }
}
impl Drop for SaveTicket {
    /** Retire only this close attempt; do not claim an in-progress write was cancelled. */
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}
impl Broker {
    /** Invalidate every queued permit without waiting for a runtime-held authority lock. */
    pub fn begin_close(&self) -> bool {
        let Ok(mut authority) = self.authority.try_lock() else {
            return false;
        };
        authority.change(Owner::Human);
        true
    }
    /** Queue one save and return its receiver; a full queue is a recoverable close failure. */
    pub fn save_session(&self) -> Option<SaveTicket> {
        let (reply, result) = mpsc::sync_channel(1);
        let active = Arc::new(AtomicBool::new(true));
        self.commands
            .try_send(Command::SaveSession {
                reply,
                active: active.clone(),
            })
            .ok()?;
        Some(SaveTicket { result, active })
    }
    /** Only an explicit native choice can suppress the final session persistence retry. */
    pub fn discard_session(&self) {
        self.discard.store(true, Ordering::Release);
    }
    /** Transfer cleanup to one owned thread while the native event loop remains alive. */
    pub fn begin_shutdown(&mut self) {
        self.connected.store(false, Ordering::Release);
        self.cancel.store(true, Ordering::Release);
        if self.shutdown.is_some()
            || (self.worker.is_none() && self.io.is_empty() && self.attach.is_none())
        {
            return;
        }
        let worker = self.worker.take();
        let io = std::mem::take(&mut self.io);
        let attach = self.attach.take();
        let authority = self.authority.clone();
        self.shutdown = Some(thread::spawn(move || {
            if let Ok(mut authority) = authority.lock() {
                authority.change(Owner::Human);
            }
            reap(worker, io, attach);
        }));
    }
    /** Join only completed cleanup, so engine or secure-store waits cannot block AppKit. */
    pub fn shutdown_finished(&mut self) -> bool {
        if self
            .shutdown
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            return false;
        }
        if let Some(worker) = self.shutdown.take() {
            let _ = worker.join();
        }
        self.worker.is_none() && self.io.is_empty() && self.attach.is_none()
    }
}

/** Runtime retains the actual child handles until it has killed and reaped its services. */
fn reap(
    worker: Option<JoinHandle<()>>,
    io: Vec<JoinHandle<()>>,
    attach: Option<(mars::mcp::attach::Hub, mars::mcp::attach::Endpoint)>,
) {
    if let Some(worker) = worker {
        let _ = worker.join();
    }
    for worker in io {
        let _ = worker.join();
    }
    if let Some((hub, endpoint)) = attach {
        drop(hub);
        endpoint.remove();
    }
}
impl Drop for Broker {
    /** Exceptional non-window owners still reap resources; normal close has already finished. */
    fn drop(&mut self) {
        self.begin_shutdown();
        if let Some(worker) = self.shutdown.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_save_poll_returns_without_waiting_and_late_result_is_retired() {
        let (reply, result) = mpsc::sync_channel(1);
        let active = Arc::new(AtomicBool::new(true));
        let ticket = SaveTicket {
            result,
            active: active.clone(),
        };
        assert_eq!(ticket.poll(), None);
        drop(ticket);
        assert!(!active.load(Ordering::Acquire));
        assert!(reply.try_send(true).is_err());
    }
    #[test]
    fn disconnected_save_is_failure_not_success() {
        let (reply, result) = mpsc::sync_channel(1);
        drop(reply);
        let ticket = SaveTicket {
            result,
            active: Arc::new(AtomicBool::new(true)),
        };
        assert_eq!(ticket.poll(), Some(false));
    }
    /** A fake worker retains ownership without launching services or touching profiles. */
    fn broker(worker: Option<JoinHandle<()>>) -> (Broker, Receiver<Command>) {
        let (commands, input) = mpsc::sync_channel(1);
        let (_, events) = mpsc::sync_channel(1);
        let (replies, _) = mpsc::sync_channel(1);
        (
            Broker {
                commands,
                events,
                replies,
                authority: crate::runtime::authority::Authority::shared(Owner::Human),
                cancel: Arc::new(AtomicBool::new(false)),
                connected: Arc::new(AtomicBool::new(false)),
                transport: Transport::None,
                worker,
                io: Vec::new(),
                attach: None,
                discard: Arc::new(AtomicBool::new(false)),
                shutdown: None,
            },
            input,
        )
    }
    #[test]
    fn cleanup_poll_does_not_join_a_waiting_worker_and_drop_keeps_ownership() {
        let (release, wait) = mpsc::sync_channel(1);
        let completed = Arc::new(AtomicBool::new(false));
        let finished = completed.clone();
        let worker = thread::spawn(move || {
            let _ = wait.recv();
            finished.store(true, Ordering::Release);
        });
        let (mut broker, _input) = broker(Some(worker));
        broker.discard_session();
        broker.begin_shutdown();
        assert!(broker.cancel.load(Ordering::Acquire));
        assert!(broker.discard.load(Ordering::Acquire));
        assert!(!broker.shutdown_finished());
        release.send(()).unwrap();
        drop(broker);
        assert!(completed.load(Ordering::Acquire));
    }
    #[test]
    fn full_save_queue_is_recoverable_and_retired_ticket_disables_queued_save() {
        let (broker, input) = broker(None);
        let ticket = broker.save_session().unwrap();
        assert!(broker.save_session().is_none());
        drop(ticket);
        let Command::SaveSession { active, .. } = input.try_recv().unwrap() else {
            panic!("save command")
        };
        assert!(!active.load(Ordering::Acquire));
        assert!(broker.save_session().is_some());
    }
    #[test]
    fn close_authority_admission_never_waits_for_a_held_lock() {
        let (broker, _input) = broker(None);
        let guard = broker.authority.lock().unwrap();
        assert!(!broker.begin_close());
        drop(guard);
        assert!(broker.begin_close());
    }
}
