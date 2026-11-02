/** Exact authority checks interrupt service waits without holding a lock across I/O. */
use super::authority::{self, Permit, SharedAuthority};
use crate::protocol::{EngineError, EngineResult};

/** One RPC captures its caller's tab and epoch; later tab changes cannot retarget it. */
pub(super) struct Cancellation(Option<(SharedAuthority, Permit)>);
impl Cancellation {
    /** Internal startup and maintenance requests have no caller permit to revoke. */
    pub fn new(authority: Option<SharedAuthority>, permit: Option<Permit>) -> Self {
        Self(authority.zip(permit))
    }
    /** Poisoned authority and removed tabs fail closed just like an explicit Stop. */
    pub fn check(&self) -> EngineResult<()> {
        let Some((authority, permit)) = &self.0 else {
            return Ok(());
        };
        let valid = authority::lock(authority).and_then(|guard| guard.validate_epoch(*permit));
        valid.map_err(|_| cancelled())
    }
}

/** Cancellation is neither a crash nor a retryable navigation failure. */
pub(super) fn cancelled() -> EngineError {
    let mut error = EngineError::new(
        "navigation_cancelled",
        "Loading stopped.",
        "Enter another address to continue.",
    );
    error.previous_page_retained = true;
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::authority::{Authority, Owner};

    /** Observation has a different owner while retaining the exact tab generation. */
    #[test]
    fn observation_on_human_tab_remains_valid_until_epoch_changes() {
        let authority = Authority::shared(Owner::Human);
        let permit = authority
            .lock()
            .unwrap()
            .observe_for(Owner::Agent, 1)
            .unwrap();
        let guard = Cancellation::new(Some(authority.clone()), Some(permit));
        assert!(guard.check().is_ok());
        authority
            .lock()
            .unwrap()
            .change_tab(1, Owner::Human)
            .unwrap();
        assert_eq!(guard.check().unwrap_err().code, "navigation_cancelled");
    }

    /** Stopping a different tab cannot interrupt this caller's service wait. */
    #[test]
    fn another_tab_epoch_does_not_cancel_this_request() {
        let authority = Authority::shared(Owner::Human);
        authority.lock().unwrap().add_tab(2);
        let permit = authority
            .lock()
            .unwrap()
            .permit_for(Owner::Human, 1)
            .unwrap();
        let guard = Cancellation::new(Some(authority.clone()), Some(permit));
        authority
            .lock()
            .unwrap()
            .change_tab(2, Owner::Agent)
            .unwrap();
        assert!(guard.check().is_ok());
        authority.lock().unwrap().remove_tab(1);
        assert!(guard.check().is_err());
    }
}
