/**
 * The Web Locks lock manager, one per coordinator, shared by every tab, frame and worker. Locks
 * are scoped by storage key (origin and top-level partition); requests queue per name in order
 * and are granted by the spec's rule: an exclusive request when no lock of that name is held and
 * nothing is ahead of it, a shared one when no exclusive lock is held or queued ahead of it.
 */
use serde_json::{Value, json};
use std::collections::BTreeSet;

/** One request: who asked (context identity and its request id), for which key and name. */
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct LockRequest {
    pub owner: String,
    pub id: u64,
    pub key: String,
    pub name: String,
    pub exclusive: bool,
}

impl LockRequest {
    /** Whether this is the same request as `owner`'s request `id`. */
    fn is(&self, owner: &str, id: u64) -> bool {
        self.owner == owner && self.id == id
    }
    /** Whether this request names the same resource as `other`. */
    fn same_resource(&self, other: &Self) -> bool {
        self.key == other.key && self.name == other.name
    }
    /** The LockInfo dictionary `query()` reports. */
    fn info(&self) -> Value {
        json!({"name":self.name,"mode":if self.exclusive {"exclusive"} else {"shared"},"clientId":self.owner})
    }
}

/** Held locks and the queue of waiting requests, in request order. */
#[derive(Default)]
pub(in crate::runtime) struct Locks {
    held: Vec<LockRequest>,
    queue: Vec<LockRequest>,
}

impl Locks {
    /** Queue `request` once; repeated polls of the same request do not queue it again. */
    pub fn enqueue(&mut self, request: LockRequest) {
        let known = |r: &LockRequest| r.is(&request.owner, request.id);
        if !self.held.iter().any(known) && !self.queue.iter().any(known) {
            self.queue.push(request);
        }
    }

    /** Whether `request` could be granted now, ahead of nothing and conflicting with nothing. */
    pub fn available(&self, request: &LockRequest) -> bool {
        let held = self.held.iter().filter(|r| r.same_resource(request));
        let ahead = self.queue.iter().filter(|r| r.same_resource(request));
        if request.exclusive {
            held.count() == 0
                && ahead
                    .take_while(|r| !r.is(&request.owner, request.id))
                    .count()
                    == 0
        } else {
            !held.into_iter().any(|r| r.exclusive)
                && !ahead
                    .take_while(|r| !r.is(&request.owner, request.id))
                    .any(|r| r.exclusive)
        }
    }

    /** Grant every queued request that has become grantable, in order. */
    pub fn process(&mut self) {
        let mut index = 0;
        while index < self.queue.len() {
            if self.available(&self.queue[index].clone()) {
                let granted = self.queue.remove(index);
                self.held.push(granted);
            } else {
                index += 1;
            }
        }
    }

    /** Whether `owner`'s request `id` holds its lock. */
    pub fn holds(&self, owner: &str, id: u64) -> bool {
        self.held.iter().any(|r| r.is(owner, id))
    }

    /** Whether `owner`'s request `id` is still waiting. */
    pub fn waiting(&self, owner: &str, id: u64) -> bool {
        self.queue.iter().any(|r| r.is(owner, id))
    }

    /** Steal: drop every held lock of the request's resource, returning the robbed holders. */
    pub fn steal(&mut self, request: LockRequest) -> Vec<LockRequest> {
        let (robbed, kept) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|r| r.same_resource(&request));
        self.held = kept;
        self.queue.retain(|r| !r.is(&request.owner, request.id));
        self.held.push(request);
        robbed
    }

    /** Release a held lock or abandon a waiting request; true when it was waiting. */
    pub fn release(&mut self, owner: &str, id: u64) -> bool {
        self.held.retain(|r| !r.is(owner, id));
        let before = self.queue.len();
        self.queue.retain(|r| !r.is(owner, id));
        before != self.queue.len()
    }

    /** Drop held locks and requests of contexts that no longer exist; true when any went. */
    pub fn retain(&mut self, live: &BTreeSet<String>) -> bool {
        let before = self.held.len() + self.queue.len();
        self.held.retain(|r| live.contains(&r.owner));
        self.queue.retain(|r| live.contains(&r.owner));
        before != self.held.len() + self.queue.len()
    }

    /** `navigator.locks.query()` for one storage key. */
    pub fn query(&self, key: &str) -> Value {
        let list = |records: &[LockRequest]| {
            records
                .iter()
                .filter(|r| r.key == key)
                .map(LockRequest::info)
                .collect::<Vec<_>>()
        };
        json!({"held":list(&self.held),"pending":list(&self.queue)})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A request for `owner`/`id` on resource `name` in key "k". */
    fn request(owner: &str, id: u64, name: &str, exclusive: bool) -> LockRequest {
        LockRequest {
            owner: owner.into(),
            id,
            key: "k".into(),
            name: name.into(),
            exclusive,
        }
    }

    /** Exclusive requests wait for the holder; release grants the next in order. */
    #[test]
    fn exclusive_locks_grant_in_order() {
        let mut locks = Locks::default();
        locks.enqueue(request("a", 1, "r", true));
        locks.enqueue(request("b", 1, "r", true));
        locks.process();
        assert!(locks.holds("a", 1));
        assert!(locks.waiting("b", 1));
        locks.release("a", 1);
        locks.process();
        assert!(locks.holds("b", 1));
    }

    /** Shared locks coexist; an exclusive request waits, and shared requests behind it wait too. */
    #[test]
    fn shared_locks_coexist_until_an_exclusive_queues() {
        let mut locks = Locks::default();
        locks.enqueue(request("a", 1, "r", false));
        locks.enqueue(request("b", 1, "r", false));
        locks.enqueue(request("c", 1, "r", true));
        locks.enqueue(request("d", 1, "r", false));
        locks.process();
        assert!(locks.holds("a", 1) && locks.holds("b", 1));
        assert!(locks.waiting("c", 1) && locks.waiting("d", 1));
    }

    /** Different names and different storage keys never conflict. */
    #[test]
    fn names_and_keys_are_independent() {
        let mut locks = Locks::default();
        locks.enqueue(request("a", 1, "r", true));
        locks.enqueue(request("b", 1, "s", true));
        let mut other = request("c", 1, "r", true);
        other.key = "other".into();
        locks.enqueue(other);
        locks.process();
        assert!(locks.holds("a", 1) && locks.holds("b", 1) && locks.holds("c", 1));
    }

    /** Stealing takes the lock from its holder at once and reports who lost it. */
    #[test]
    fn steal_robs_current_holders() {
        let mut locks = Locks::default();
        locks.enqueue(request("a", 1, "r", true));
        locks.process();
        let robbed = locks.steal(request("b", 2, "r", true));
        assert_eq!(robbed.len(), 1);
        assert_eq!(robbed[0].owner, "a");
        assert!(locks.holds("b", 2) && !locks.holds("a", 1));
    }

    /** A gone context's locks are released and query reports by key. */
    #[test]
    fn retired_contexts_release_and_query_reports() {
        let mut locks = Locks::default();
        locks.enqueue(request("a", 1, "r", true));
        locks.enqueue(request("b", 1, "r", true));
        locks.process();
        assert_eq!(locks.query("k")["pending"][0]["clientId"], "b");
        let live = BTreeSet::from(["b".to_string()]);
        assert!(locks.retain(&live));
        locks.process();
        assert!(locks.holds("b", 1));
        assert_eq!(locks.query("k")["held"][0]["mode"], "exclusive");
    }
}
