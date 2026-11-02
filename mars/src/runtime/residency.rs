/** Page-owned leases bound live script contexts without evicting committed or candidate state. */
use crate::protocol::{EngineError, EngineResult};
use std::sync::{Arc, Weak};

/** One slot remains available for replacing a committed root at saturation. */
pub(super) const CONTEXTS: usize = 8;
/** Ordinary contexts include all tabs, frames and their unpublished candidates. */
pub(super) const ORDINARY_CONTEXTS: usize = CONTEXTS - 1;

/** Cloned pages share this lifetime; moving a frame out of its parent does not release it. */
pub(super) struct Lease {
    /** Unique across navigation failures and document generation reuse. */
    pub identity: String,
}

/** Weak entries observe ownership without keeping abandoned candidates alive. */
#[derive(Default)]
pub(super) struct Residency {
    /** Monotonic identities are independent of successful navigation generations. */
    next: u64,
    /** At most CONTEXTS live entries; dead entries are pruned before admission. */
    held: Vec<Weak<Lease>>,
}

impl Residency {
    /** Reserve before acquisition; only a replacement of an existing root may use headroom. */
    pub fn reserve(&mut self, replacement: bool) -> EngineResult<Arc<Lease>> {
        let limit = if replacement {
            CONTEXTS
        } else {
            ORDINARY_CONTEXTS
        };
        if self.snapshot().len() >= limit {
            return Err(EngineError::limit("resident script contexts", limit));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("script context identities", usize::MAX))?;
        let lease = Arc::new(Lease {
            identity: format!("context-{}", self.next),
        });
        self.held.push(Arc::downgrade(&lease));
        Ok(lease)
    }

    /** The service reconciles this bounded ownership snapshot before executing a resident call. */
    pub fn snapshot(&mut self) -> Vec<String> {
        self.held.retain(|lease| lease.strong_count() != 0);
        self.held
            .iter()
            .filter_map(Weak::upgrade)
            .map(|lease| lease.identity.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Candidate clones keep reservations alive; ordinary work cannot consume replacement headroom. */
    #[test]
    fn clones_and_replacement_respect_one_shared_budget() {
        let mut pool = Residency::default();
        let mut pages: Vec<_> = (0..ORDINARY_CONTEXTS)
            .map(|_| pool.reserve(false).unwrap())
            .collect();
        let moved = pages.pop().unwrap();
        assert!(pool.reserve(false).is_err());
        let replacement = pool.reserve(true).unwrap();
        assert!(pool.reserve(true).is_err());
        let rollback = moved.clone();
        drop(moved);
        assert!(pool.reserve(false).is_err());
        drop(rollback);
        drop(replacement);
        assert!(pool.reserve(false).is_ok());
    }

    /** Abandoned candidates release capacity and never reuse an earlier identity. */
    #[test]
    fn failed_candidates_leave_no_unbounded_registry() {
        let mut pool = Residency::default();
        let first = pool.reserve(false).unwrap().identity.clone();
        for _ in 0..100 {
            assert_ne!(pool.reserve(false).unwrap().identity, first);
        }
        assert!(pool.snapshot().is_empty());
        assert!(pool.held.is_empty());
    }
}
