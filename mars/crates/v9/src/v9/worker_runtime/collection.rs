/** A bounded stop-the-world request coordinates actual isolate collections without moving VM pointers. */
use super::*;
use crate::v9::vm::{Res, Vm};

/** A pending generation contains only worker identities and completed acknowledgements. */
#[derive(Default)]
pub(super) struct Collection {
    active: bool,
    expected: BTreeSet<Option<u64>>,
    completed: BTreeSet<Option<u64>>,
    failed: bool,
    generation: u64,
}

/** No participant is parked longer than this bounded interval if another cannot reach a safepoint. */
const COLLECTION_TIMEOUT: Duration = Duration::from_secs(2);

impl ConformanceWorkerHost {
    /** Freeze worker membership and release every participant even if collection times out. */
    pub(in crate::v9) fn coordinate_collection(
        &self,
        requester: Option<u64>,
        collect: impl FnOnce() -> Res<()>,
        finish: impl FnOnce() -> Res<()>,
    ) -> Res<()> {
        let workers = self.workers.lock().unwrap();
        let mut state = self.collection.lock().unwrap();
        if state.active {
            return Err(EngineError::unsupported(
                "Concurrent SharedGC requests are not supported.",
            )
            .into());
        }
        state.active = true;
        state.generation = state.generation.wrapping_add(1);
        state.failed = false;
        state.completed.clear();
        state.expected = workers
            .iter()
            .filter_map(|(&id, worker)| {
                (!worker.done.load(Ordering::Acquire) && Some(id) != requester).then_some(Some(id))
            })
            .collect();
        if requester.is_some() {
            state.expected.insert(None);
        }
        self.collection_requested.store(true, Ordering::Release);
        drop(state);
        let result = collect();
        let deadline = Instant::now() + COLLECTION_TIMEOUT;
        let mut state = self.collection.lock().unwrap();
        while result.is_ok() && !state.failed && !self.closing.load(Ordering::Acquire) {
            let complete = state.expected.iter().all(|id| {
                state.completed.contains(id)
                    || id.is_some_and(|id| {
                        workers
                            .get(&id)
                            .is_none_or(|worker| worker.done.load(Ordering::Acquire))
                    })
            });
            if complete {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
            state = self
                .collection_wake
                .wait_timeout(state, PARK_INTERVAL)
                .unwrap()
                .0;
        }
        let complete = state.expected.iter().all(|id| {
            state.completed.contains(id)
                || id.is_some_and(|id| {
                    workers
                        .get(&id)
                        .is_none_or(|worker| worker.done.load(Ordering::Acquire))
                })
        });
        let failed = state.failed || !complete || self.closing.load(Ordering::Acquire);
        drop(state);
        let result = if result.is_ok() && !failed {
            finish()
        } else {
            result
        };
        let mut state = self.collection.lock().unwrap();
        state.active = false;
        self.collection_requested.store(false, Ordering::Release);
        self.collection_wake.notify_all();
        drop(state);
        drop(workers);
        if failed {
            return Err(EngineError::unsupported(
                "SharedGC could not reach every worker safepoint.",
            )
            .into());
        }
        result
    }

    /** Collect only at a VM instruction or idle boundary, then remain parked until the request ends. */
    pub(in crate::v9) fn collection_safepoint(&self, id: Option<u64>, vm: &mut Vm) -> Res<()> {
        if !self.collection_requested.load(Ordering::Acquire) {
            return Ok(());
        }
        let state = self.collection.lock().unwrap();
        if !state.active || !state.expected.contains(&id) || state.completed.contains(&id) {
            return Ok(());
        }
        let generation = state.generation;
        drop(state);
        let result = vm.collect_now();
        let mut state = self.collection.lock().unwrap();
        if !state.active || state.generation != generation || !state.expected.contains(&id) {
            return result;
        }
        if result.is_err() {
            state.failed = true;
        } else {
            state.completed.insert(id);
        }
        self.collection_wake.notify_all();
        while state.active
            && state.generation == generation
            && !self.closing.load(Ordering::Acquire)
        {
            state = self
                .collection_wake
                .wait_timeout(state, PARK_INTERVAL)
                .unwrap()
                .0;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Missing acknowledgements fail boundedly, release the generation, and allow later collections. */
    #[test]
    fn missing_worker_safepoint_times_out_and_releases_generation() {
        let session = ConformanceWorkerSession::new();
        let host = session.host();
        let (input, _receiver) = mpsc::sync_channel(1);
        let (outgoing, output) = mpsc::sync_channel(1);
        let worker = Arc::new(Worker {
            input,
            outgoing,
            output: Mutex::new(output),
            cancel: Arc::new(AtomicBool::new(false)),
            done: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
            error: Mutex::new(None),
            failure: Mutex::new(None),
        });
        host.workers.lock().unwrap().insert(1, worker.clone());
        let start = Instant::now();
        assert!(
            host.coordinate_collection(None, || Ok(()), || Ok(()))
                .is_err()
        );
        assert!(start.elapsed() < COLLECTION_TIMEOUT + Duration::from_secs(1));
        assert!(!host.collection_requested.load(Ordering::Acquire));
        worker.done.store(true, Ordering::Release);
        assert!(
            host.coordinate_collection(None, || Ok(()), || Ok(()))
                .is_ok()
        );
    }
}
