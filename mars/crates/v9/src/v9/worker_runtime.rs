/** Explicitly scoped d8 worker sessions own isolated V9 threads, queues and cancellation capabilities. */
mod engine;
mod profiles;
mod shared_gc;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::builtins::WorkerGraph;
use crate::v9::platform::PageEnvironment;
use crate::v9::vm::data::{SharedBytes, WeakSharedBytes};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/** Concurrent workers and queued messages have explicit host bounds independent of VM heap quotas. */
const WORKER_LIMIT: usize = 32;
/** Completed JavaScript handles have a separate bound from concurrently active Rust threads. */
const WORKER_RECORD_LIMIT: usize = 4096;
const QUEUE_CAPACITY: usize = 16;
/** Grants retain real stores across snapshots, with an explicit bound on total native storage. */
const SHARED_STORE_LIMIT: usize = 4096;
const SHARED_STORAGE_BYTES: usize = 32 * 1_048_576;
/** Periodic weak-grant cleanup bounds registry metadata without rescanning it for every byte write. */
const SHARED_PURGE_INTERVAL: usize = 64;
/** Cancellation is checked while parked as well as at interpreter instruction boundaries. */
const PARK_INTERVAL: Duration = Duration::from_millis(10);
/** Session identities distinguish capabilities even if a restored snapshot reuses an object id. */
static NEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);

/** The owning runner keeps this guard across all requests and drops it before scoring a case. */
pub struct ConformanceWorkerSession {
    host: Arc<ConformanceWorkerHost>,
}
impl ConformanceWorkerSession {
    /** Start a fresh capability namespace with no workers or shared authority from previous cases. */
    pub fn new() -> Self {
        Self {
            host: Arc::new(ConformanceWorkerHost {
                nonce: NEXT_IDENTITY.fetch_add(1, Ordering::Relaxed),
                workers: Mutex::new(BTreeMap::new()),
                closing: AtomicBool::new(false),
                shared: Mutex::new(SharedRegistry::default()),
                shared_gc: Mutex::new(shared_gc::Registry::default()),
                available: Condvar::new(),
                profiles: Mutex::new(Vec::new()),
            }),
        }
    }
    /** Supply this handle only to environments belonging to this one explicit test session. */
    pub fn host(&self) -> Arc<ConformanceWorkerHost> {
        self.host.clone()
    }
    /** Join child isolates and surface native limits or engine failures separately from ordinary child throws. */
    pub fn finish(&self) -> EngineResult<()> {
        let workers = self.host.shutdown();
        for worker in workers {
            if let Some(error) = worker.failure.lock().unwrap().clone() {
                return Err(error);
            }
        }
        Ok(())
    }
}
impl Default for ConformanceWorkerSession {
    /** Default sessions are fresh scopes with the same deterministic ownership rules as explicit construction. */
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for ConformanceWorkerSession {
    /** Shutdown is explicit because worker environments themselves hold references to the broker. */
    fn drop(&mut self) {
        let _ = self.host.shutdown();
    }
}

/** Non-serializable native host authority; JavaScript sees only branded local Worker objects. */
pub struct ConformanceWorkerHost {
    nonce: u64,
    workers: Mutex<BTreeMap<u64, Arc<Worker>>>,
    closing: AtomicBool,
    shared: Mutex<SharedRegistry>,
    shared_gc: Mutex<shared_gc::Registry>,
    available: Condvar,
    /** Completed inert artifacts survive child shutdown until the owning runner consumes them. */
    profiles: Mutex<Vec<(Option<u64>, String)>>,
}
/** An opaque snapshot identity has authority only in endpoints which were granted its actual Arc. */
struct SharedGrant {
    buffer: WeakSharedBytes,
    endpoints: BTreeSet<Option<u64>>,
    allocated: usize,
}
/** A capability grant does not keep otherwise unreachable shared bytes alive. */
#[derive(Default)]
struct SharedRegistry {
    stores: BTreeMap<u64, SharedGrant>,
    allocated: usize,
    updates: usize,
}
impl SharedRegistry {
    /** Cached allocation is immutable per backing; dead grants release its entire reservation. */
    fn purge(&mut self) {
        self.stores
            .retain(|_, grant| grant.buffer.upgrade().is_some());
        self.allocated = self.stores.values().map(|grant| grant.allocated).sum();
        self.updates = 0;
    }
}
impl std::fmt::Debug for ConformanceWorkerHost {
    /** Diagnostics disclose only the session label, never queue contents, pointers or worker messages. */
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConformanceWorkerHost")
            .field("session", &self.nonce)
            .finish_non_exhaustive()
    }
}
/** Queues exchange owned graph packets; no source heap object or callback crosses a thread. */
struct Worker {
    input: SyncSender<WorkerGraph>,
    outgoing: SyncSender<WorkerGraph>,
    output: Mutex<Receiver<WorkerGraph>>,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    error: Mutex<Option<String>>,
    failure: Mutex<Option<EngineError>>,
}
/** Worker-side endpoints retain only their own inbox and output sender. */
pub(super) struct Endpoint {
    input: Receiver<WorkerGraph>,
    worker: Arc<Worker>,
}

impl ConformanceWorkerHost {
    /** Native allocation or graph transfer grants one endpoint an actual shared backing handle. */
    pub(in crate::v9) fn shared(
        &self,
        endpoint: Option<u64>,
        buffer: SharedBytes,
    ) -> EngineResult<SharedBytes> {
        if self.closing.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        if endpoint.is_some_and(|id| !self.workers.lock().unwrap().contains_key(&id)) {
            return Err(EngineError::invalid(
                "Shared backing endpoint is outside its worker session.",
            ));
        }
        let mut registry = self.shared.lock().unwrap();
        let id = buffer.identity();
        if let Some(grant) = registry.stores.get_mut(&id) {
            if let Some(live) = grant.buffer.upgrade() {
                if !live.same_store(&buffer) {
                    return Err(EngineError::invalid("Shared backing identity collision."));
                }
                grant.endpoints.insert(endpoint);
                return Ok(buffer);
            }
            registry.purge();
        }
        if registry.updates >= SHARED_PURGE_INTERVAL
            || registry.stores.len() >= SHARED_STORE_LIMIT
            || buffer.allocated() > SHARED_STORAGE_BYTES.saturating_sub(registry.allocated)
        {
            registry.purge();
        }
        if registry.stores.len() >= SHARED_STORE_LIMIT
            || buffer.allocated() > SHARED_STORAGE_BYTES.saturating_sub(registry.allocated)
        {
            return Err(EngineError::limit(
                "Shared backing session storage",
                SHARED_STORAGE_BYTES,
            ));
        }
        registry.allocated += buffer.allocated();
        registry.updates += 1;
        registry.stores.insert(
            id,
            SharedGrant {
                buffer: buffer.downgrade(),
                endpoints: BTreeSet::from([endpoint]),
                allocated: buffer.allocated(),
            },
        );
        Ok(buffer)
    }
    /** An inert saved ID cannot discover handles previously granted only to other endpoints. */
    pub(in crate::v9) fn restore_shared(
        &self,
        endpoint: Option<u64>,
        id: u64,
    ) -> Option<SharedBytes> {
        let stores = self.shared.lock().unwrap();
        let grant = stores.stores.get(&id)?;
        if !grant.endpoints.contains(&endpoint) {
            return None;
        }
        grant.buffer.upgrade()
    }
    /** Native brands include the session nonce so old serialized identities grant no new capability. */
    pub(in crate::v9) fn nonce(&self) -> u64 {
        self.nonce
    }
    /** Root and child agents remain distinct even when two explicit sessions share an imported backing. */
    pub(in crate::v9) fn agent(&self, endpoint: Option<u64>) -> u64 {
        endpoint.unwrap_or(self.nonce)
    }
    /** Futex ticket cancellation changes native queue state only and never executes a JavaScript callback. */
    fn cancel_agent(&self, agent: u64) {
        let stores: Vec<_> = self
            .shared
            .lock()
            .unwrap()
            .stores
            .values()
            .filter_map(|grant| grant.buffer.upgrade())
            .collect();
        for buffer in stores {
            buffer.cancel_agent(agent);
        }
    }
    /** Thread creation completes before a handle is published, and every published handle is joined. */
    pub(in crate::v9) fn spawn(
        self: &Arc<Self>,
        mut environment: PageEnvironment,
        source: String,
        arguments: Option<WorkerGraph>,
        deadline: Instant,
    ) -> EngineResult<u64> {
        let mut workers = self.workers.lock().unwrap();
        if self.closing.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        for worker in workers
            .values()
            .filter(|worker| worker.done.load(Ordering::Acquire))
        {
            Self::join(worker);
        }
        if workers.len() >= WORKER_RECORD_LIMIT {
            return Err(EngineError::limit(
                "d8 Worker handles per session",
                WORKER_RECORD_LIMIT,
            ));
        }
        while workers
            .values()
            .filter(|worker| !worker.done.load(Ordering::Acquire))
            .count()
            >= WORKER_LIMIT
        {
            if self.closing.load(Ordering::Acquire)
                || environment.conformance_worker_id.is_some_and(|id| {
                    workers
                        .get(&id)
                        .is_none_or(|worker| worker.cancel.load(Ordering::Acquire))
                })
            {
                return Err(cancelled());
            }
            if Instant::now() >= deadline {
                return Err(EngineError::limit(
                    "d8 concurrently active worker threads",
                    WORKER_LIMIT,
                ));
            }
            workers = self
                .available
                .wait_timeout(
                    workers,
                    PARK_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                )
                .unwrap()
                .0;
        }
        if self.closing.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        let id = NEXT_IDENTITY.fetch_add(1, Ordering::Relaxed);
        let (input, received) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (sent, output) = mpsc::sync_channel(QUEUE_CAPACITY);
        let worker = Arc::new(Worker {
            input,
            outgoing: sent,
            output: Mutex::new(output),
            cancel: Arc::new(AtomicBool::new(false)),
            done: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
            error: Mutex::new(None),
            failure: Mutex::new(None),
        });
        environment.conformance_workers = Some(self.clone());
        environment.conformance_worker_id = Some(id);
        environment.conformance_worker_cancel = Some(worker.cancel.clone());
        environment.conformance_d8 = true;
        let endpoint = Endpoint {
            input: received,
            worker: worker.clone(),
        };
        let thread = std::thread::Builder::new()
            .name(format!("v9-worker-{id}"))
            .stack_size(crate::constants::V9_STACK_BYTES)
            .spawn(move || engine::run(environment, source, arguments, endpoint))
            .map_err(|_| EngineError::unsupported("Could not start a d8 worker thread."))?;
        *worker.thread.lock().unwrap() = Some(thread);
        workers.insert(id, worker);
        Ok(id)
    }
    /** Invalid capabilities fail rather than accessing a worker from another test case. */
    fn worker(&self, id: u64) -> EngineResult<Arc<Worker>> {
        self.workers
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| EngineError::invalid("Unknown worker in this conformance session."))
    }
    /** Bounded sends never park the parent behind a worker that is waiting on the parent itself. */
    pub(in crate::v9) fn post(&self, id: u64, message: WorkerGraph) -> EngineResult<()> {
        let worker = self.worker(id)?;
        if worker.done.load(Ordering::Acquire) {
            return Ok(());
        }
        match worker.input.try_send(message) {
            Ok(()) | Err(mpsc::TrySendError::Disconnected(_)) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                Err(EngineError::limit("Worker message queue", QUEUE_CAPACITY))
            }
        }
    }
    /** D8 returns undefined after a worker exits and its outbound queue has drained. */
    pub(in crate::v9) fn receive(
        &self,
        id: u64,
        requester: Option<u64>,
        deadline: Instant,
    ) -> EngineResult<Option<WorkerGraph>> {
        let worker = self.worker(id)?;
        let output = worker.output.lock().unwrap();
        loop {
            match output.try_recv() {
                Ok(message) => return Ok(Some(message)),
                Err(mpsc::TryRecvError::Disconnected) => return Ok(None),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if worker.done.load(Ordering::Acquire) {
                return Ok(output.try_recv().ok());
            }
            if self.cancelled(requester) || Instant::now() >= deadline {
                return Err(crate::v9::platform::timeout());
            }
            match output.recv_timeout(PARK_INTERVAL) {
                Ok(message) => return Ok(Some(message)),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
    /** Worker-global postMessage publishes only to its creator, preserving FIFO order. */
    pub(in crate::v9) fn post_parent(&self, id: u64, message: WorkerGraph) -> EngineResult<()> {
        let worker = self.worker(id)?;
        worker
            .outgoing
            .try_send(message)
            .map_err(|_| EngineError::limit("Worker outgoing message queue", QUEUE_CAPACITY))
    }
    /** Cancellation applies only to the endpoint named by a worker environment, never its parent. */
    pub(in crate::v9) fn cancelled(&self, id: Option<u64>) -> bool {
        id.is_some_and(|id| {
            self.workers
                .lock()
                .unwrap()
                .get(&id)
                .is_none_or(|worker| worker.cancel.load(Ordering::Acquire))
        })
    }
    /** terminate requests cancellation; terminateAndWait additionally proves the thread has stopped. */
    pub(in crate::v9) fn terminate(&self, id: u64, wait: bool) -> EngineResult<()> {
        let worker = self.worker(id)?;
        worker.cancel.store(true, Ordering::Release);
        self.cancel_agent(id);
        if wait {
            Self::join(&worker);
        }
        Ok(())
    }
    /** A completed thread has released every isolate-owned closure, realm and message handler. */
    fn join(worker: &Worker) {
        if let Some(thread) = worker.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
    /** Parked constructors admit the next real isolate after a worker has actually completed. */
    pub(super) fn finished(&self, id: Option<u64>) {
        if let Some(id) = id {
            self.cancel_agent(id);
        }
        self.available.notify_all();
    }
    /** Cancel all workers before joining any, including workers parked waiting for child workers. */
    fn shutdown(&self) -> Vec<Arc<Worker>> {
        self.closing.store(true, Ordering::Release);
        self.available.notify_all();
        let workers: Vec<_> = self.workers.lock().unwrap().values().cloned().collect();
        for worker in &workers {
            worker.cancel.store(true, Ordering::Release);
        }
        for worker in &workers {
            Self::join(worker);
        }
        for id in self
            .workers
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            self.cancel_agent(id);
        }
        self.cancel_agent(self.nonce);
        self.workers.lock().unwrap().clear();
        *self.shared.lock().unwrap() = SharedRegistry::default();
        workers
    }
}

/** Shutdown refusal is an uncatchable cancellation outcome, distinct from an engine defect or quota. */
fn cancelled() -> EngineError {
    EngineError::new(
        "conformance_worker_cancelled",
        "Worker session is shutting down.",
        "No new child authority is created after session teardown begins.",
    )
}
