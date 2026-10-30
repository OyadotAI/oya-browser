/** Candidate-owned dedicated workers share residency, authority and bounded cooperative turns. */
mod execution;
mod lifecycle;
mod messaging;
mod scheduler;

use super::{
    page::Page,
    residency::Lease,
    supervisor::Runtime,
    worker_loading::{Policy, fatal},
};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{Execution, PageEnvironment, PageEvent, WORKER_TASKS, WorkerCommand};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

/** A worker never allocates a native thread or owns network authority between turns. */
#[derive(Clone)]
struct Context {
    lease: Arc<Lease>,
    sources: Arc<Vec<String>>,
    environment: PageEnvironment,
    policy: Policy,
    token: Option<u64>,
    inbox: VecDeque<PageEvent>,
    requests: Vec<crate::v9::HostRequest>,
    preflights: crate::network::cors::PreflightCache,
    next_timer: Option<f64>,
    /// Parent virtual time at construction maps later ticks into this worker's own clock.
    clock_origin: f64,
    /// Imported cross-origin code must not expose its exception text to the creator page.
    muted_errors: bool,
}

/** Cloning a page preserves rollback ownership and copies queues without delivering any work. */
#[derive(Clone, Default)]
pub(super) struct Workers {
    contexts: BTreeMap<u32, Context>,
    pending: VecDeque<WorkerCommand>,
    deliveries: VecDeque<PageEvent>,
    links: BTreeMap<u64, u32>,
    /// Round-robin selection prevents one zero-delay worker timer from starving its siblings.
    cursor: u32,
}

impl Workers {
    /** Worker states participate in the same committed/base/result rollback rule as frames. */
    pub fn token(&self, identity: &str) -> Option<u64> {
        self.contexts
            .values()
            .find(|worker| worker.lease.identity == identity)
            .and_then(|worker| worker.token)
    }
    /** Background scheduling includes queued messages, unfinished loading and worker timers. */
    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
            || !self.deliveries.is_empty()
            || self.contexts.values().any(|worker| {
                worker.token.is_none()
                    || !worker.inbox.is_empty()
                    || !worker.requests.is_empty()
                    || worker.next_timer.is_some()
            })
    }
    /** Page effects cannot grow a hidden queue across many script steps. */
    pub fn append(&mut self, commands: Vec<WorkerCommand>) -> EngineResult<()> {
        if self.pending.len() + commands.len() > WORKER_TASKS {
            return Err(EngineError::limit("Worker queued tasks", WORKER_TASKS));
        }
        let bytes = self
            .pending
            .iter()
            .chain(&commands)
            .try_fold(0usize, |sum, command| {
                Ok::<_, EngineError>(sum.saturating_add(match command {
                    WorkerCommand::Post { data, .. } => data.bytes()?,
                    _ => 0,
                }))
            })?;
        if bytes > crate::v9::WORKER_MESSAGE_BYTES {
            return Err(EngineError::limit(
                "Worker queued bytes",
                crate::v9::WORKER_MESSAGE_BYTES,
            ));
        }
        self.pending.extend(commands);
        self.validate_queues()
    }
    /** Bound the combined retained queues, including messages waiting across browser ticks. */
    fn validate_queues(&self) -> EngineResult<()> {
        let count = self.pending.len()
            + self.deliveries.len()
            + self
                .contexts
                .values()
                .map(|worker| worker.inbox.len() + worker.requests.len())
                .sum::<usize>();
        let bytes = serde_json::to_vec(&self.pending)
            .map_err(|_| EngineError::invalid("Invalid Worker queue."))?
            .len()
            + serde_json::to_vec(&self.deliveries)
                .map_err(|_| EngineError::invalid("Invalid Worker deliveries."))?
                .len()
            + self
                .contexts
                .values()
                .map(|worker| {
                    serde_json::to_vec(&(&worker.inbox, &worker.requests))
                        .map(|bytes| bytes.len())
                        .unwrap_or(crate::v9::WORKER_MESSAGE_BYTES)
                })
                .sum::<usize>();
        if count > WORKER_TASKS || bytes > crate::v9::WORKER_MESSAGE_BYTES {
            return Err(EngineError::limit(
                "Worker retained queue",
                crate::v9::WORKER_MESSAGE_BYTES,
            ));
        }
        Ok(())
    }
    /** Pending transfers keep their close effects until the route admission has been processed. */
    pub fn transferring(&self, link: u64) -> bool {
        self.pending.iter().any(
            |command| matches!(command, WorkerCommand::Post { ports, .. } if ports.contains(&link)),
        )
    }

    /** Retire the matching route and discard only messages addressed to the endpoint that closed. */
    fn close_link(&mut self, link: u64, expected: Option<u32>) -> bool {
        let Some(owner) = self.links.get(&link).copied() else {
            return false;
        };
        if expected.is_some_and(|id| id != owner) {
            return false;
        }
        self.links.remove(&link);
        if expected.is_none() {
            self.deliveries.retain(
                |event| !matches!(event, PageEvent::PortMessage { link: id, .. } if *id == link),
            );
        }
        if expected.is_some()
            && let Some(worker) = self.contexts.get_mut(&owner)
        {
            worker.inbox.retain(
                |event| !matches!(event, PageEvent::PortMessage { link: id, .. } if *id == link),
            );
        }
        true
    }

    /** Retire routes after preserving replies already queued on the surviving owner's event loop. */
    fn close_links(&mut self, id: u32) -> EngineResult<()> {
        let links: Vec<_> = self
            .links
            .iter()
            .filter_map(|(link, owner)| (*owner == id).then_some(*link))
            .collect();
        for link in links {
            self.links.remove(&link);
            enqueue(self, PageEvent::PortClose { link })?;
        }
        self.validate_queues()
    }

    /** Releasing candidate ownership queues peer closes; committed state still owns rollback. */
    fn remove(&mut self, id: u32) -> EngineResult<()> {
        self.contexts.remove(&id);
        self.deliveries.retain(|event| match event {
            PageEvent::WorkerMessage { id: owner, .. }
            | PageEvent::WorkerError { id: owner, .. } => *owner != id,
            PageEvent::PortMessage { link, .. } => self.links.get(link) != Some(&id),
            _ => true,
        });
        self.close_links(id)
    }
}

/** Owner delivery has the same count bound as worker input and pending construction. */
fn enqueue(workers: &mut Workers, event: PageEvent) -> EngineResult<()> {
    if workers.deliveries.len() >= WORKER_TASKS {
        return Err(EngineError::limit("Worker owner deliveries", WORKER_TASKS));
    }
    workers.deliveries.push_back(event);
    Ok(())
}

/** Copy only shared browser facts; DOM projections, cookies and shell capabilities stay absent. */
fn environment(page: &Page, policy: &Policy, name: String) -> PageEnvironment {
    let parent = &page.scripting.environment;
    PageEnvironment {
        worker_name: Some(name),
        url: policy.base.to_string(),
        locale: parent.locale.clone(),
        user_agent: parent.user_agent.clone(),
        platform: parent.platform.clone(),
        online: parent.online,
        time_origin_ms: parent.time_origin_ms + page.scripting.clock_ms,
        live_clock: parent.live_clock,
        random_seed: super::scripting::random_seed(),
        timezone_offset_minutes: parent.timezone_offset_minutes,
        timezone_initial_offset_seconds: parent.timezone_initial_offset_seconds,
        timezone_transitions: parent.timezone_transitions.clone(),
        timezone_posix_rule: parent.timezone_posix_rule.clone(),
        timezone_display_names: parent.timezone_display_names.clone(),
        timezone_daylight: parent.timezone_daylight.clone(),
        timezone_initial_daylight: parent.timezone_initial_daylight,
        disallow_code_generation_from_strings: !super::csp::allows_eval(&policy.csp),
        ..PageEnvironment::default()
    }
}
