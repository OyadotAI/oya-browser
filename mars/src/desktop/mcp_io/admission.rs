/** Actual native admission permits bind cancellation to captured authority rather than client guesses. */
use super::*;
use mars::constants::MCP_PENDING_REQUESTS;
use std::collections::BTreeMap;

/** Only bounded native correlations and their observed permits survive queue admission. */
#[derive(Default)]
pub(super) struct Admissions {
    /// Arguments and page values remain exclusively in the command, never in cancellation metadata.
    permits: Mutex<BTreeMap<String, Permit>>,
}
impl Admissions {
    /** Capture before queueing so a fast UI-acknowledged completion cannot outrun bookkeeping. */
    pub fn capture(&self, native_id: &str, permit: Permit) -> EngineResult<()> {
        let mut permits = self.permits.lock().map_err(|_| invalid())?;
        if permits.len() >= MCP_PENDING_REQUESTS || permits.contains_key(native_id) {
            return Err(invalid());
        }
        permits.insert(native_id.into(), permit);
        Ok(())
    }
    /** Both native completion and rejected admission release the same bounded record. */
    pub fn remove(&self, native_id: Option<&str>) -> EngineResult<()> {
        let mut permits = self.permits.lock().map_err(|_| invalid())?;
        if let Some(native_id) = native_id {
            permits.remove(native_id);
        }
        Ok(())
    }
    /** Supplied preconditions must match the actual admission and the still-current agent grant. */
    pub fn cancel(
        &self,
        authority: &SharedAuthority,
        native_id: &str,
        tab_id: Option<&str>,
        epoch: Option<u64>,
    ) -> EngineResult<()> {
        let captured = self
            .permits
            .lock()
            .map_err(|_| invalid())?
            .get(native_id)
            .copied();
        let Some(captured) = captured else {
            return Ok(());
        };
        let Some((tab, epoch)) = tab_id.and_then(|id| runtime::tab_key(id).ok()).zip(epoch) else {
            return Ok(());
        };
        if captured.owner != Owner::Agent || captured.tab_id != Some(tab) || captured.epoch != epoch
        {
            return Ok(());
        }
        let mut state = authority::lock(authority)?;
        if state.tab_status(tab) == Some((Owner::Agent, captured.epoch)) {
            state.change_tab(tab, Owner::Human)?;
        }
        Ok(())
    }
}

/** Correlation bookkeeping failures terminate the connection without disclosing any request data. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid MCP native admission state.")
}
