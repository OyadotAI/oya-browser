/** Waiting network hops belong to the pool, so a slow worker cannot hold unrelated requests. */
use super::{Done, EngineError, Hop, Job, Sender, Worker};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/** A short lock protects admission only; workers perform every network operation outside it. */
pub(super) struct Queue {
    pub(super) workers: Vec<Worker>,
    pending: VecDeque<Hop>,
    done: Sender<Done>,
}

impl Queue {
    /** Keep the result channel available even if every worker fails to start. */
    pub(super) fn new(done: Sender<Done>) -> Self {
        Self {
            workers: Vec::new(),
            pending: VecDeque::new(),
            done,
        }
    }

    /** Accepted hops wait in submission order until any worker has capacity. */
    pub(super) fn submit(&mut self, hop: Hop) {
        self.pending.push_back(hop);
        self.dispatch();
    }

    /** Completion starts queued work immediately, independently of the page's JavaScript thread. */
    pub(super) fn complete(&mut self, index: usize) {
        self.workers[index].busy = false;
        self.dispatch();
    }

    /** Preserve cache affinity for an idle worker, then use any other idle worker. */
    fn pick(&self, url: &str) -> Option<usize> {
        if self.workers.is_empty() {
            return None;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.split('#').next().unwrap_or(url).hash(&mut hasher);
        let preferred = (hasher.finish() % self.workers.len() as u64) as usize;
        let idle = |worker: &Worker| !worker.busy && !worker.closed;
        if idle(&self.workers[preferred]) {
            Some(preferred)
        } else {
            self.workers.iter().position(idle)
        }
    }

    /** Never enqueue a hop behind active I/O; failed worker channels produce explicit results. */
    fn dispatch(&mut self) {
        while let Some(hop) = self.pending.front() {
            let Some(index) = self.pick(&hop.request.url) else {
                break;
            };
            let hop = self.pending.pop_front().expect("front checked");
            self.workers[index].busy = true;
            if let Err(error) = self.workers[index].jobs.send(Job::Hop(Box::new(hop))) {
                self.workers[index].busy = false;
                self.workers[index].closed = true;
                if let Job::Hop(hop) = error.0 {
                    self.failed(hop.id);
                }
            }
        }
        if self.workers.iter().all(|worker| worker.closed) {
            while let Some(hop) = self.pending.pop_front() {
                self.failed(hop.id);
            }
        }
    }

    /** A caller must receive a terminal answer when no worker can own its admitted hop. */
    fn failed(&self, id: u64) {
        let _ = self.done.send(Done {
            id,
            result: Err(EngineError::service("network", false)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::WebRequest;
    use std::sync::mpsc;

    /** One deterministic hop whose URL always prefers the same worker. */
    fn hop(id: u64) -> Hop {
        Hop {
            id,
            request: WebRequest::get("https://example.com/resource"),
            grants: Vec::new(),
            remaining_ms: 1000,
        }
    }

    /** A fast worker takes waiting requests even while their cache-affinity worker remains busy. */
    #[test]
    fn saturated_pool_releases_queued_hops_to_the_next_free_worker() {
        let (done, _) = mpsc::channel();
        let mut queue = Queue::new(done);
        let mut receivers = Vec::new();
        for _ in 0..super::super::WORKERS {
            let (jobs, receiver) = mpsc::channel();
            queue.workers.push(Worker {
                jobs,
                busy: false,
                closed: false,
            });
            receivers.push(receiver);
        }
        let preferred = queue.pick(&hop(1).request.url).unwrap();
        for id in 0..=super::super::WORKERS {
            queue.submit(hop(id as u64));
        }
        for receiver in &receivers {
            assert!(matches!(receiver.try_recv(), Ok(Job::Hop(_))));
            assert!(
                receiver.try_recv().is_err(),
                "an active worker must not own waiting hops"
            );
        }
        let free = (preferred + 1) % receivers.len();
        queue.complete(free);
        let Job::Hop(next) = receivers[free].try_recv().unwrap() else {
            panic!("expected hop")
        };
        assert_eq!(next.id, super::super::WORKERS as u64);
        assert!(queue.workers[preferred].busy);
        assert!(queue.pending.is_empty());
    }

    /** Pool creation or channel failures cannot leave accepted request identities unanswered. */
    #[test]
    fn unavailable_workers_report_request_failures() {
        let (done, results) = mpsc::channel();
        let mut queue = Queue::new(done);
        queue.submit(hop(1));
        assert!(results.recv().unwrap().result.is_err());
        let (jobs, receiver) = mpsc::channel();
        drop(receiver);
        queue.workers.push(Worker {
            jobs,
            busy: false,
            closed: false,
        });
        queue.submit(hop(2));
        let failure = results.recv().unwrap();
        assert_eq!(failure.id, 2);
        assert!(failure.result.is_err());
    }
}
