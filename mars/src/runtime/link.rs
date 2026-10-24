/**
 * The Oya link's way into the runtime: every server command is admitted as the connected agent
 * with the epoch the tab's owner record holds right now, then validated by the same authority
 * check the local agent surfaces get. The person holding a tab, or taking it back mid-command,
 * refuses the write; nothing here grants control, and Human Mode from the server only revokes.
 */
use crate::protocol::{EngineError, EngineResult, Request};
use crate::runtime::authority::{self, Owner, Permit, SharedAuthority};
use crate::runtime::{mutating, tab_key};
use mars_link::Browser;
use serde_json::Value;

/// Runs a request under a permit: the runtime's `handle_as`, or the desktop broker's agent submission.
pub type Submit<'a> = dyn FnMut(Request, Permit) -> EngineResult<Value> + 'a;
/// Captures one tab as PNG bytes under an agent permit; the desktop window owns the display list.
pub type Capture<'a> = dyn FnMut(&str, Permit) -> EngineResult<Vec<u8>> + 'a;

/// The runtime as the link's `Browser`.
pub struct RuntimeBrowser<'a> {
    /// Shared with the window and every other surface; read for epochs, written only to revoke.
    authority: SharedAuthority,
    /// Where requests go.
    submit: Box<Submit<'a>>,
    /// Where screenshots come from, when a window exists.
    capture: Option<Box<Capture<'a>>>,
}

impl<'a> RuntimeBrowser<'a> {
    /// A browser over `submit`, checked against `authority`; `capture` is None when headless.
    pub fn new(
        authority: SharedAuthority,
        submit: Box<Submit<'a>>,
        capture: Option<Box<Capture<'a>>>,
    ) -> Self {
        Self {
            authority,
            submit,
            capture,
        }
    }

    /// The agent permit for `tab` (or the window when None). A write needs the tab granted to the
    /// agent (`permit_for` refuses a person's tab); a read observes at the tab's current epoch.
    fn permit(&self, tab: Option<u64>, write: bool) -> EngineResult<Permit> {
        let state = authority::lock(&self.authority)?;
        match tab {
            Some(id) if write => state.permit_for(Owner::Agent, id),
            Some(id) => state.observe_for(Owner::Agent, id),
            None => Ok(Permit {
                owner: Owner::Agent,
                epoch: state.epoch,
                tab_id: None,
            }),
        }
    }
}

/// The numeric tab a request names, if any.
fn tab_of(request: &Request) -> EngineResult<Option<u64>> {
    request
        .params
        .get("tab_id")
        .and_then(Value::as_str)
        .map(tab_key)
        .transpose()
}

impl Browser for RuntimeBrowser<'_> {
    fn run(&mut self, mut request: Request) -> EngineResult<Value> {
        let write = mutating(&request.op);
        let permit = self.permit(tab_of(&request)?, write)?;
        if write && let Some(params) = request.params.as_object_mut() {
            params.insert("control_epoch".into(), permit.epoch.into());
        }
        (self.submit)(request, permit)
    }

    fn screenshot(&mut self, tab_id: &str) -> EngineResult<Vec<u8>> {
        // Like Oya's recording capture: pixels only from a tab the agent holds.
        let permit = self.permit(Some(tab_key(tab_id)?), true)?;
        match self.capture.as_mut() {
            Some(capture) => capture(tab_id, permit),
            None => Err(EngineError::new(
                "unsupported_operation",
                "Screenshots need the desktop window.",
                "Run Mars desktop to capture tabs.",
            )),
        }
    }

    fn revoke_agents(&mut self) {
        if let Ok(mut state) = authority::lock(&self.authority) {
            state.revoke_agents();
        }
    }
}
