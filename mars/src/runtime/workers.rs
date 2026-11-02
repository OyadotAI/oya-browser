mod broadcast;
/** Candidate-owned dedicated workers share residency, authority and bounded cooperative turns. */
mod execution;
mod lifecycle;
mod messaging;
mod modules;
mod scheduler;

use super::{
    page::Page,
    residency::Lease,
    supervisor::Runtime,
    worker_loading::{Policy, fatal},
};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{
    Execution, PageEnvironment, PageEvent, WORKER_TASKS, WorkerCommand, WorkerCredentials,
    WorkerType,
};
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
    /// Effects can wait until a newly constructed child adopts its channel endpoint.
    port_outbox: Vec<crate::v9::PortMessage>,
    port_closes: Vec<u64>,
    preflights: crate::network::cors::PreflightCache,
    next_timer: Option<f64>,
    /// Parent virtual time at construction maps later ticks into this worker's own clock.
    clock_origin: f64,
    /// Imported cross-origin code must not expose its exception text to the creator page.
    muted_errors: bool,
    /// Module requests and loaded source stay owned by this worker's rollback candidate.
    modules: modules::Graph,
    /// Queued messages wait for module top-level await without preventing fetch/timer progress.
    ready: bool,
}

impl Context {
    /** Owner messages wait for initialization; child replies may resolve the parent's top-level await. */
    fn event_ready(&self, event: &PageEvent) -> bool {
        self.ready
            || matches!(event, PageEvent::WorkerMessage { id, .. } if *id != 0)
            || matches!(
                event,
                PageEvent::WorkerError { .. }
                    | PageEvent::PortMessage { .. }
                    | PageEvent::PortClose { .. }
            )
    }
    /** Readiness does not discard early owner messages while a child completes awaited initialization. */
    fn next_event(&mut self) -> Option<PageEvent> {
        let index = self
            .inbox
            .iter()
            .position(|event| self.event_ready(event))?;
        self.inbox.remove(index)
    }
}

/** Native owner paths qualify each realm's branded handle without granting scripts global IDs. */
type Owner = Vec<u32>;
/** Ordinary residency includes the document itself; pending worker creation shares this ceiling. */
const WORKER_CONTEXTS: usize = super::residency::ORDINARY_CONTEXTS - 1;

/** Commands retain their actual native sender until the coordinator consumes them. */
#[derive(Clone, serde::Serialize)]
struct Task {
    owner: Owner,
    command: WorkerCommand,
}

/** Destination ownership is fixed before another realm receives control. */
#[derive(Clone, serde::Serialize)]
struct Delivery {
    owner: Owner,
    event: PageEvent,
}

/** One page owns the complete bounded worker tree and every asynchronous queue in it. */
#[derive(Clone, Default)]
pub(super) struct Workers {
    contexts: BTreeMap<Owner, Context>,
    pending: VecDeque<Task>,
    deliveries: VecDeque<Delivery>,
    links: BTreeMap<u64, [Owner; 2]>,
    /// Round-robin selection covers descendants and siblings in one bounded scheduler.
    cursor: Owner,
}

impl Workers {
    /** Worker states participate in the same committed/base/result rollback rule as frames. */
    pub fn token(&self, identity: &str) -> Option<u64> {
        self.contexts
            .values()
            .find(|worker| worker.lease.identity == identity)
            .and_then(|worker| worker.token)
    }
    /** Queued readiness, requests and timers keep the shared scheduler active. */
    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
            || !self.deliveries.is_empty()
            || self.contexts.values().any(|worker| {
                worker.token.is_none()
                    || worker.inbox.iter().any(|event| worker.event_ready(event))
                    || !worker.requests.is_empty()
                    || !worker.modules.pending.is_empty()
                    || !worker.port_outbox.is_empty()
                    || !worker.port_closes.is_empty()
                    || worker.next_timer.is_some()
            })
    }
    /** Document effects always originate at the native tree root. */
    pub fn append(&mut self, commands: Vec<WorkerCommand>) -> EngineResult<()> {
        self.append_from(&[], commands)
    }
    /** Pending creations reserve their share before acquisition, including the currently executing parent. */
    fn append_from(&mut self, owner: &[u32], commands: Vec<WorkerCommand>) -> EngineResult<()> {
        for command in commands {
            command.bytes()?;
            if let WorkerCommand::Create { id, .. } = &command {
                let held = self.contexts.len()
                    + usize::from(!owner.is_empty() && !self.contexts.contains_key(owner));
                let reserved = self
                    .pending
                    .iter()
                    .filter(|task| matches!(task.command, WorkerCommand::Create { .. }))
                    .count();
                if held + reserved >= WORKER_CONTEXTS {
                    enqueue_to(
                        self,
                        owner,
                        PageEvent::WorkerError {
                            id: *id,
                            message: "The shared worker context limit was reached.".into(),
                        },
                    )?;
                    continue;
                }
            }
            self.pending.push_back(Task {
                owner: owner.to_vec(),
                command,
            });
        }
        self.validate_queues()
    }
    /** Count and byte limits cover the entire tree, including captured Blob bodies and pending construction. */
    fn validate_queues(&self) -> EngineResult<()> {
        let count = self.pending.len()
            + self.deliveries.len()
            + self
                .contexts
                .values()
                .map(|worker| {
                    worker.inbox.len()
                        + worker.requests.len()
                        + worker.port_outbox.len()
                        + worker.port_closes.len()
                })
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
                    serde_json::to_vec(&(
                        &worker.inbox,
                        &worker.requests,
                        &worker.port_outbox,
                        &worker.port_closes,
                    ))
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
    /** Pending root transfers keep their close effects in the worker domain until admission. */
    pub fn transferring(&self, link: u64) -> bool {
        self.pending.iter().any(|task| task.owner.is_empty()
            && matches!(&task.command, WorkerCommand::Post { ports, .. } if ports.contains(&link)))
    }
    /** Only a registered endpoint can close a route; already accepted peer replies remain ordered. */
    fn close_link(&mut self, link: u64, from: &[u32]) -> Option<Owner> {
        let peer = opposite(self.links.get(&link)?, from)?.clone();
        self.links.remove(&link);
        self.deliveries.retain(|delivery| delivery.owner != from || !matches!(&delivery.event, PageEvent::PortMessage { link: id, .. } if *id == link));
        if let Some(worker) = self.contexts.get_mut(from) {
            worker.inbox.retain(
                |event| !matches!(event, PageEvent::PortMessage { link: id, .. } if *id == link),
            );
        }
        Some(peer)
    }
    /** Retiring a parent discards all descendant candidates, pending creates, timers and port routes. */
    fn remove(&mut self, owner: &[u32], discard_outgoing: bool) -> EngineResult<()> {
        self.contexts.retain(|path, _| !path.starts_with(owner));
        self.pending.retain(|task| !task.owner.starts_with(owner));
        self.deliveries
            .retain(|delivery| !delivery.owner.starts_with(owner));
        if discard_outgoing && let Some((id, parent)) = owner.split_last() {
            self.deliveries.retain(|delivery| delivery.owner != parent || !matches!(&delivery.event, PageEvent::WorkerMessage { id: target, .. } | PageEvent::WorkerError { id: target, .. } if target == id));
            if let Some(worker) = self.contexts.get_mut(parent) {
                worker.inbox.retain(|event| !matches!(event, PageEvent::WorkerMessage { id: target, .. } | PageEvent::WorkerError { id: target, .. } if target == id));
            }
        }
        let links: Vec<_> = self
            .links
            .iter()
            .filter(|(_, ends)| ends.iter().any(|end| end.starts_with(owner)))
            .map(|(link, ends)| (*link, ends.clone()))
            .collect();
        for (link, ends) in links {
            self.links.remove(&link);
            for survivor in ends.iter().filter(|end| !end.starts_with(owner)) {
                enqueue_to(self, survivor, PageEvent::PortClose { link })?;
            }
        }
        self.validate_queues()
    }
}

/** Native endpoints can address their paired peer, never a sibling with an equal local handle. */
fn opposite<'a>(ends: &'a [Owner; 2], from: &[u32]) -> Option<&'a Owner> {
    if ends[0] == from {
        Some(&ends[1])
    } else if ends[1] == from {
        Some(&ends[0])
    } else {
        None
    }
}

/** All worker event loops share the same bounded asynchronous delivery rule. */
fn enqueue_to(workers: &mut Workers, owner: &[u32], event: PageEvent) -> EngineResult<()> {
    if workers.deliveries.len() >= WORKER_TASKS {
        return Err(EngineError::limit("Worker event queue", WORKER_TASKS));
    }
    workers.deliveries.push_back(Delivery {
        owner: owner.to_vec(),
        event,
    });
    Ok(())
}

/** Copy only shared browser facts; DOM projections, cookies and shell capabilities stay absent. */
fn environment(
    parent: &PageEnvironment,
    clock: f64,
    depth: u8,
    policy: &Policy,
    name: String,
) -> PageEnvironment {
    PageEnvironment {
        worker_name: Some(name),
        worker_depth: depth,
        url: policy.base.to_string(),
        locale: parent.locale.clone(),
        user_agent: parent.user_agent.clone(),
        platform: parent.platform.clone(),
        online: parent.online,
        time_origin_ms: parent.time_origin_ms + clock,
        live_clock: parent.live_clock,
        live_gc: parent.live_gc,
        random_seed: super::scripting::random_seed(),
        timezone_offset_minutes: parent.timezone_offset_minutes,
        timezone_initial_offset_seconds: parent.timezone_initial_offset_seconds,
        timezone_transitions: parent.timezone_transitions.clone(),
        timezone_posix_rule: parent.timezone_posix_rule.clone(),
        timezone_display_names: parent.timezone_display_names.clone(),
        timezone_daylight: parent.timezone_daylight.clone(),
        timezone_initial_daylight: parent.timezone_initial_daylight,
        disallow_code_generation_from_strings: !super::csp::allows_eval(&policy.csp),
        handler_policy: crate::v9::HandlerPolicy {
            restrictions: vec![crate::v9::HandlerRestriction::default()],
        },
        ..PageEnvironment::default()
    }
}
