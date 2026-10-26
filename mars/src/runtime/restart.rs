/** One crashed or wedged engine is replaced alone, so other tabs keep their committed pages (CB-23).
Wiring, owned by the supervisor: `mod restart;` in runtime/mod.rs; a `pub(super) revival: Revival`
field built with `Revival::new(&programs)` in `start_cancel`; `services`, `Service`, `Service::name`
and `Service::spawn` made `pub(super)`; `healthy()` returns `Err(self.revive(name, None))` instead
of stopping, and `call_until` returns `Err(self.revive(name, result.err()))` instead of stopping.
Then drop the `#[ignore]` on tests/platform_restart.rs. */
use super::supervisor::{EnginePrograms, Runtime, Service};
use crate::protocol::EngineError;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant};

/** A crash loop still ends the session: at most this many restarts inside one window. */
const RESTARTS: usize = 3;
/** Window the restart budget counts over; a stable engine earns its budget back. */
const RESTART_WINDOW: Duration = Duration::from_secs(60);

/** Shown with every restarted-engine failure; nothing is replayed automatically. */
const RESTART_HINT: &str = "Mars restarted the engine. Retry the operation; your tabs were kept, and a page that was running scripts reloads.";

/** Remembers each engine's executable and the recent restarts that spend the budget. */
pub(super) struct Revival {
    /** The same injected paths startup used, never a page-supplied path. */
    programs: EnginePrograms,
    /** Restart instants still inside the window. */
    recent: Vec<Instant>,
}

impl Revival {
    /** Captured at startup, before the supervisor consumes the program paths. */
    pub fn new(programs: &EnginePrograms) -> Self {
        Self {
            programs: programs.clone(),
            recent: Vec::new(),
        }
    }

    /** Spends one restart, or refuses once the window already holds the maximum. */
    fn admit(&mut self, now: Instant) -> bool {
        self.recent
            .retain(|then| now.duration_since(*then) < RESTART_WINDOW);
        if self.recent.len() >= RESTARTS {
            return false;
        }
        self.recent.push(now);
        true
    }

    /** Static service names map to their startup executables; anything else has none. */
    fn program(&self, name: &str) -> Option<PathBuf> {
        let programs = [
            ("network", &self.programs.network),
            ("document", &self.programs.document),
            ("v9", &self.programs.v9),
        ];
        programs
            .into_iter()
            .find(|(service, _)| *service == name)
            .map(|(_, path)| path.clone())
    }
}

impl Runtime {
    /** Replace the named engine and return the error for the operation that was using it.
    Engines keep no page state, so committed tabs, history and cookies survive untouched. A closing
    window, an exhausted budget or a failed respawn falls back to retiring the whole session.
    `cause` is the failed call's error, if any: a deadline keeps its `timeout` code so callers can
    still tell a hung engine from a crashed one. */
    pub(super) fn revive(&mut self, name: &str, cause: Option<EngineError>) -> EngineError {
        let Some(index) = self
            .services
            .iter()
            .position(|service| service.name == name)
        else {
            return self.retire(name);
        };
        let name = self.services[index].name;
        if self.cancel.load(Ordering::Relaxed) || !self.revival.admit(Instant::now()) {
            return self.retire(name);
        }
        drop(self.services.remove(index));
        let Some(program) = self.revival.program(name) else {
            return self.retire(name);
        };
        match Service::spawn(name, &program, Arc::clone(&self.cancel)) {
            Ok(fresh) => self.services.insert(index, fresh),
            Err(_) => return self.retire(name),
        }
        match self.call(name, "service.ready", json!({})) {
            Ok(ready) if ready == json!({"service":name,"ready":true}) => restarted(name, cause),
            _ => self.retire(name),
        }
    }

    /** The pre-CB-23 behavior: stop every engine and report the session as failed. */
    fn retire(&mut self, name: &str) -> EngineError {
        self.stop();
        EngineError::service(name, false)
    }
}

/** The interrupted operation fails visibly and is never replayed; the person or agent retries it. */
fn restarted(name: &str, cause: Option<EngineError>) -> EngineError {
    if let Some(mut timeout) = cause.filter(|error| error.code == "timeout") {
        timeout.hint = RESTART_HINT.into();
        timeout.previous_page_retained = true;
        return timeout;
    }
    let mut error = EngineError::new(
        "engine_restarted",
        format!("The {name} engine stopped and Mars restarted it. This operation did not finish."),
        RESTART_HINT,
    );
    error.retryable = true;
    error.previous_page_retained = true;
    error.service = Some(name.into());
    error
}
