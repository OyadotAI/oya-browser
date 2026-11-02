/** One crashed or wedged engine is replaced alone, so other tabs keep their committed pages (CB-23).
Wiring, owned by the supervisor: `mod restart;` in runtime/mod.rs; a `pub(super) revival: Revival`
field built with `Revival::new(&programs)` in `start_cancel`; `services`, `Service`, `Service::name`
and `Service::spawn` made `pub(super)`; `healthy()` returns `Err(self.revive(name, None))` instead
of stopping, and `call_until` returns `Err(self.revive(name, result.err()))` instead of stopping.
Then drop the `#[ignore]` on tests/platform_restart.rs. */
use super::supervisor::{EnginePrograms, Runtime, Service};
use crate::protocol::EngineError;
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
    pub(super) fn revive(
        &mut self,
        name: &str,
        op: Option<&str>,
        cause: Option<EngineError>,
    ) -> EngineError {
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
        let is_in_process = self.services[index].child.is_none();
        drop(self.services.remove(index));
        let fresh = if is_in_process {
            Service::in_process(name, Arc::clone(&self.cancel))
        } else {
            let Some(program) = self.revival.program(name) else {
                return self.retire(name);
            };
            Service::spawn(name, &program, Arc::clone(&self.cancel))
        };
        match fresh {
            Ok(fresh) => self.services.insert(index, fresh),
            Err(_) => return self.retire(name),
        }
        if self.services[index].ready() {
            restarted(name, op, cause)
        } else {
            self.retire(name)
        }
    }

    /** Retire an interrupted pipe before reuse, without spending the crash-loop budget. */
    pub(super) fn replace_cancelled(&mut self, name: &str) -> EngineError {
        let Some(index) = self
            .services
            .iter()
            .position(|service| service.name == name)
        else {
            return self.retire(name);
        };
        let name = self.services[index].name;
        if self.cancel.load(Ordering::Relaxed) {
            return self.retire(name);
        }
        let is_in_process = self.services[index].child.is_none();
        drop(self.services.remove(index));
        if name == "v9" {
            self.v9_sources.clear();
        }
        let fresh = if is_in_process {
            Service::in_process(name, Arc::clone(&self.cancel))
        } else {
            let Some(program) = self.revival.program(name) else {
                return self.retire(name);
            };
            Service::spawn(name, &program, Arc::clone(&self.cancel))
        };
        match fresh {
            Ok(fresh) => self.services.insert(index, fresh),
            Err(_) => return self.retire(name),
        }
        if self.services[index].ready() {
            super::authority::denied()
        } else {
            self.retire(name)
        }
    }

    /** The pre-CB-23 behavior: stop every engine and report the session as failed. */
    fn retire(&mut self, name: &str) -> EngineError {
        self.stop();
        EngineError::service(name, false)
    }
}

/** The interrupted operation fails visibly and is never replayed; the person or agent retries it.
`op` is the call that was running when the engine went away, so the banner can name it. */
fn restarted(name: &str, op: Option<&str>, cause: Option<EngineError>) -> EngineError {
    if let Some(mut timeout) = cause.filter(|error| error.code == "timeout") {
        timeout.hint = RESTART_HINT.into();
        timeout.previous_page_retained = true;
        return timeout;
    }
    let during = op.map(|op| format!(" during {op}")).unwrap_or_default();
    let mut error = EngineError::new(
        "engine_restarted",
        format!(
            "Browser {name} service stopped{during} and Mars restarted it. This operation did not finish."
        ),
        RESTART_HINT,
    );
    error.retryable = true;
    error.previous_page_retained = true;
    error.service = Some(name.into());
    error
}

/** A call that passed its deadline. A hung engine and a crashed one need different reactions from
the person (wait for a slow page versus report a crash), so this names the operation and the
budget instead of saying the service stopped. `budget` is the whole time the call was given. */
pub(super) fn deadline(name: &str, op: &str, budget: Duration) -> EngineError {
    let seconds = budget.as_secs_f64().ceil().max(1.0);
    let unit = if seconds == 1.0 { "second" } else { "seconds" };
    let mut error = EngineError::service(name, true);
    error.message = format!("Browser {name} service did not answer {op} within {seconds} {unit}.");
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A deadline names the service, the operation and the whole seconds it was given. */
    #[test]
    fn a_deadline_names_the_operation_and_its_budget() {
        let error = deadline("network", "network.fetch", Duration::from_secs(10));
        assert_eq!(error.code, "timeout");
        assert_eq!(error.service.as_deref(), Some("network"));
        assert_eq!(
            error.message,
            "Browser network service did not answer network.fetch within 10 seconds."
        );
    }

    /** A partial second rounds up, and one second is singular. */
    #[test]
    fn a_short_deadline_rounds_up_to_one_second() {
        let error = deadline("document", "document.parse", Duration::from_millis(300));
        assert_eq!(
            error.message,
            "Browser document service did not answer document.parse within 1 second."
        );
    }

    /** A crash names the operation that was running and stays retryable. */
    #[test]
    fn a_restart_names_the_interrupted_operation() {
        let error = restarted("v9", Some("v9.run"), None);
        assert_eq!(error.code, "engine_restarted");
        assert!(error.retryable);
        assert_eq!(
            error.message,
            "Browser v9 service stopped during v9.run and Mars restarted it. This operation did not finish."
        );
    }

    /** A timed-out call keeps its deadline message through the restart. */
    #[test]
    fn a_restart_after_a_deadline_keeps_the_deadline_message() {
        let cause = deadline("network", "network.fetch", Duration::from_secs(10));
        let error = restarted("network", Some("network.fetch"), Some(cause));
        assert_eq!(error.code, "timeout");
        assert!(
            error
                .message
                .contains("did not answer network.fetch within 10 seconds")
        );
        assert_eq!(error.hint, RESTART_HINT);
    }
}
