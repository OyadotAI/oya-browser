/**
 * Network hops off the runtime thread. A page's `fetch` and `XMLHttpRequest` hops (Relay's
 * prepare and transmit) run here on a small pool of worker threads, each with its own network
 * service process, so the runtime keeps answering clicks, typing and other tabs while a slow
 * link delivers a response; results come back on a channel the runtime drains on idle ticks.
 * Everything that needs the runtime (cookies, CORS, authority, redirects) stays on the runtime
 * thread (resources/in_flight.rs); a worker only moves bytes. Browsers load the same way: the
 * network runs beside the page, never inside its event loop.
 */
use super::supervisor::Service;
use crate::network::{WebRequest, WebResponse};
use crate::protocol::{EngineError, EngineResult};
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Worker threads (and network service processes) in the pool: enough for a page's parallel
/// fetches to overlap, as Chrome keeps six connections per host, without a process per request.
const WORKERS: usize = 6;
/// Time past a hop's own allowance that its RPCs may take before the worker gives up on them.
const RPC_MARGIN: Duration = Duration::from_secs(5);

/// One job for a worker.
enum Job {
    /// Prepare and transmit one request.
    Hop(Hop),
    /// Empty the worker's HTTP cache ("Clear browsing data").
    ClearCache,
    /// Drop the worker's cached copies of these URLs: another process sent them an unsafe request.
    Forget(Vec<String>),
}

/// One request's hop: Relay's request, the egress grants, and its time allowance.
struct Hop {
    id: u64,
    request: WebRequest,
    grants: Vec<String>,
    remaining_ms: u64,
}

/// A finished hop, by the identity `submit` returned.
pub(super) struct Done {
    pub id: u64,
    pub result: EngineResult<WebResponse>,
}

/// One worker's queue and how many hops it has not finished.
struct Worker {
    /// Its own queue, so a broadcast reaches each worker exactly once.
    jobs: Sender<Job>,
    /// Hops queued or running; zero means it would start a new one at once.
    busy: Arc<AtomicUsize>,
}

/**
 * The pool. Each network process keeps its own memory cache, so a URL goes to the same worker
 * whenever that worker is free: a repeated GET is a cache hit and an unsafe request invalidates
 * the copy that would answer it. A busy preferred worker hands the hop to an idle one instead,
 * so one slow response never queues the requests behind it; unsafe requests are then announced
 * to every cache (`forget`).
 */
pub(super) struct Loader {
    /// The workers, by index.
    workers: Vec<Worker>,
    /// Finished hops, drained by `poll` and `wait`.
    results: Receiver<Done>,
    /// Set on drop, so a hop on a slow link stops at its next poll instead of running on.
    stop: Arc<AtomicBool>,
    /// The last identity handed out.
    next: u64,
}

impl Loader {
    /** Start the pool over the network service program at `executable`. Workers spawn their
    service processes on their first hop, so an idle pool costs only parked threads. */
    pub fn start(executable: &Path) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (done, results) = mpsc::channel();
        let workers = (0..WORKERS)
            .filter_map(|index| spawn(index, executable, &stop, done.clone()))
            .collect();
        Self {
            workers,
            results,
            stop,
            next: 0,
        }
    }

    /** Queue one hop; its result comes back from `poll` under the returned identity. */
    pub fn submit(&mut self, request: WebRequest, grants: Vec<String>, remaining_ms: u64) -> u64 {
        self.next += 1;
        let id = self.next;
        let Some(worker) = self.pick(&request.url) else {
            return id;
        };
        worker.busy.fetch_add(1, Ordering::Relaxed);
        let hop = Hop {
            id,
            request,
            grants,
            remaining_ms,
        };
        if worker.jobs.send(Job::Hop(hop)).is_err() {
            worker.busy.fetch_sub(1, Ordering::Relaxed);
        }
        id
    }

    /** The URL's own worker when it is free, else any free worker, else the URL's own. */
    fn pick(&self, url: &str) -> Option<&Worker> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.split('#').next().unwrap_or(url).hash(&mut hasher);
        let count = u64::try_from(self.workers.len())
            .ok()
            .filter(|count| *count > 0)?;
        let preferred = &self.workers[usize::try_from(hasher.finish() % count).ok()?];
        let idle = |worker: &&Worker| worker.busy.load(Ordering::Relaxed) == 0;
        Some(if idle(&preferred) {
            preferred
        } else {
            self.workers.iter().find(idle).unwrap_or(preferred)
        })
    }

    /** Every hop finished since the last call, without waiting. */
    pub fn poll(&self) -> Vec<Done> {
        self.results.try_iter().collect()
    }

    /** Hops finished by `until`: returns as soon as one is, so a caller that has nothing else
    to do (a page settling after load) waits for the network without spinning. */
    pub fn wait(&self, until: Instant) -> Vec<Done> {
        let remaining = until.saturating_duration_since(Instant::now());
        let Ok(first) = self.results.recv_timeout(remaining) else {
            return Vec::new();
        };
        std::iter::once(first)
            .chain(self.results.try_iter())
            .collect()
    }

    /** Ask every worker to empty its HTTP cache. */
    pub fn clear_cache(&self) {
        self.broadcast(|| Job::ClearCache);
    }

    /** Ask every worker to drop its copies of these URLs. */
    pub fn forget(&self, urls: &[String]) {
        self.broadcast(|| Job::Forget(urls.to_vec()));
    }

    /** One job to each worker, after whatever it already has queued. */
    fn broadcast(&self, job: impl Fn() -> Job) {
        for worker in &self.workers {
            let _ = worker.jobs.send(job());
        }
    }
}

impl Drop for Loader {
    /** Workers are not joined: a hop on a slow link could hold the closing runtime for minutes.
    The stop flag ends their current RPC, the closed queues end their loops, and each worker's
    service process is killed when its `Service` drops. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/** Start one worker thread with its own queue; None when the system refuses a thread. */
fn spawn(
    index: usize,
    executable: &Path,
    stop: &Arc<AtomicBool>,
    done: Sender<Done>,
) -> Option<Worker> {
    let (jobs, queue) = mpsc::channel::<Job>();
    let busy = Arc::new(AtomicUsize::new(0));
    let (executable, cancel, count) = (
        executable.to_path_buf(),
        Arc::clone(stop),
        Arc::clone(&busy),
    );
    thread::Builder::new()
        .name(format!("oya-loader-{index}"))
        .spawn(move || work(&executable, &cancel, &queue, &done, &count))
        .ok()?;
    Some(Worker { jobs, busy })
}

/** Starts or replaces the worker network service respecting process isolation settings. */
fn ensure_service(service: &mut Option<Service>, exe: &Path, cancel: &Arc<AtomicBool>) {
    if service.is_none() {
        *service = if super::supervisor::process_isolation_enabled() {
            Service::spawn("network", exe, Arc::clone(cancel)).ok()
        } else {
            Service::in_process("network", Arc::clone(cancel)).ok()
        };
    }
}

/** Dispatches a single worker job to the network service. */
fn dispatch_job(service: &mut Option<Service>, job: Job) -> Option<Done> {
    match job {
        Job::Hop(hop) => Some(run(service, hop)),
        Job::ClearCache => maintain(service, "network.cache_clear", json!({})),
        Job::Forget(urls) => maintain(service, "network.cache_forget", json!({"urls": urls})),
    }
}

/** One worker: run jobs until its queue closes, starting a network service on first use. */
fn work(
    executable: &Path,
    cancel: &Arc<AtomicBool>,
    queue: &Receiver<Job>,
    done: &Sender<Done>,
    busy: &AtomicUsize,
) {
    let mut service: Option<Service> = None;
    while let Ok(job) = queue.recv() {
        ensure_service(&mut service, executable, cancel);
        if let Some(finished) = dispatch_job(&mut service, job) {
            busy.fetch_sub(1, Ordering::Relaxed);
            if done.send(finished).is_err() {
                return;
            }
        }
    }
}

/** Run one hop on the worker's service, dropping a service that failed so the next job starts a
fresh one. */
fn run(service: &mut Option<Service>, hop: Hop) -> Done {
    let id = hop.id;
    let Some(running) = service.as_mut() else {
        return Done {
            id,
            result: Err(EngineError::service("network", false)),
        };
    };
    let result = transfer(running, hop);
    if result.as_ref().is_err_and(|error| error.service.is_some()) {
        *service = None;
    }
    Done { id, result }
}

/** A cache maintenance call; nothing to report, and a service without a cache has none to fix. */
fn maintain(service: &mut Option<Service>, op: &str, params: Value) -> Option<Done> {
    if let Some(running) = service.as_mut() {
        let _ = running.request::<Value>(op, &params, None);
    }
    None
}

/** Relay's prepare then transmit for one hop, within its allowance plus a margin. */
fn transfer(service: &mut Service, hop: Hop) -> EngineResult<WebResponse> {
    let deadline = Instant::now() + Duration::from_millis(hop.remaining_ms) + RPC_MARGIN;
    let prepare = json!({
        "request": hop.request,
        "fixture_origins": hop.grants,
        "remaining_ms": hop.remaining_ms,
    });
    let token: Value = service.request("network.prepare", &prepare, Some(deadline))?;
    let transmit = json!({"token": token["token"]});
    service.request("network.transmit", &transmit, Some(deadline))
}
