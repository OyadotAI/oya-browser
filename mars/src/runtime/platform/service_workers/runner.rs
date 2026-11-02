/**
 * Running service workers. A version runs in V9's worker runtime (`v9.worker`) with the
 * ServiceWorkerGlobalScope profile, holds one residency lease only while running, and is
 * terminated after SW_IDLE with nothing to do, as browsers stop idle workers. Functional events
 * queue on the worker and are handed over when its parked `sw.next` request is retried; each
 * outcome comes back through `sw.done`. Turns run on the coordinator's idle schedule and, for a
 * fetch the page is waiting on, synchronously until the answer or the deadline.
 */
use super::super::{Ctx, Kind, parse_body};
use crate::protocol::{EngineError, EngineResult};
use crate::runtime::residency::Lease;
use crate::runtime::supervisor::Runtime;
use crate::runtime::worker_loading::Policy;
use crate::v9::{Execution, HostRequest, HostResponse, PageEnvironment};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

/** How long a worker with nothing to do keeps running. */
const SW_IDLE: Duration = Duration::from_secs(30);
/** Turns one idle pass may spend on all workers together. */
const TURNS_PER_PASS: usize = 32;
/** Timer callbacks one timer turn may run. */
const TIMER_BUDGET: usize = 16;
/** Console lines kept from workers until a page settle takes them. */
const CONSOLE_LINES: usize = 64;

/** One running version. */
pub(in crate::runtime) struct Running {
    pub version: u64,
    lease: Arc<Lease>,
    sources: Arc<Vec<String>>,
    environment: PageEnvironment,
    policy: Policy,
    token: Option<u64>,
    requests: Vec<HostRequest>,
    parked: Vec<HostRequest>,
    preflights: crate::network::cors::PreflightCache,
    next_timer: Option<f64>,
    started: Instant,
    last_work: Instant,
    /** Functional events waiting for `sw.next`. */
    pub events: VecDeque<Value>,
    /** Outcomes `sw.done` reported, by event id: the arguments and any response bytes. */
    pub outcomes: BTreeMap<u64, (Value, Vec<u8>)>,
    /** Events dispatched whose outcome has not been taken yet. */
    pub awaiting: usize,
    /** The platform version this worker's parked requests were last retried at. */
    seen: u64,
    /** A module worker's graph: the entry and every module fetched so far. */
    modules: crate::v9::ModuleInput,
    /** Imports the last step asked for, fetched before the next one. */
    module_requests: Vec<crate::v9::ModuleRequest>,
    /** The script's top level has finished (a module graph evaluated), so events may run. */
    ready: bool,
}

impl Running {
    /** The context this worker's platform requests come from. */
    fn ctx(&self, active: bool) -> Ctx {
        let url = self.policy.base.clone();
        Ctx {
            identity: self.lease.identity.clone(),
            kind: Kind::Service(self.version),
            origin: url.origin().ascii_serialization(),
            url,
            top_level: self.policy.partition.clone(),
            active,
            selected: false,
            tab: String::new(),
        }
    }
    /** Whether the worker has a turn to take now. */
    fn has_work(&self, now_ms: f64, platform_version: u64) -> bool {
        self.token.is_none()
            || !self.module_requests.is_empty()
            || !self.requests.is_empty()
            || (!self.events.is_empty() && !self.parked.is_empty())
            || (self.seen != platform_version && !self.parked.is_empty())
            || self.next_timer.is_some_and(|due| due <= now_ms)
    }
    /** Milliseconds on this worker's own clock. */
    fn now_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }
}

/** Facts a worker copies from the browser: locale, user agent, clock and time zone. */
fn environment(base: &PageEnvironment, policy: &Policy, state: Value) -> PageEnvironment {
    PageEnvironment {
        worker_name: Some(String::new()),
        worker_depth: 1,
        service_worker: true,
        service_worker_state: Some(state.to_string()),
        url: policy.base.to_string(),
        locale: base.locale.clone(),
        user_agent: base.user_agent.clone(),
        platform: base.platform.clone(),
        online: true,
        time_origin_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |since| since.as_secs_f64() * 1000.0),
        live_clock: true,
        live_gc: true,
        random_seed: crate::runtime::scripting::random_seed(),
        timezone_offset_minutes: base.timezone_offset_minutes,
        timezone_transitions: base.timezone_transitions.clone(),
        timezone_initial_offset_seconds: base.timezone_initial_offset_seconds,
        timezone_posix_rule: base.timezone_posix_rule.clone(),
        timezone_display_names: base.timezone_display_names.clone(),
        timezone_daylight: base.timezone_daylight.clone(),
        timezone_initial_daylight: base.timezone_initial_daylight,
        handler_policy: crate::v9::HandlerPolicy {
            restrictions: vec![crate::v9::HandlerRestriction::default()],
        },
        ..PageEnvironment::default()
    }
}

impl Runtime {
    /** Start version `version` of registration `registration` unless it is already running. */
    pub(in crate::runtime) fn start_service_worker(
        &mut self,
        registration: u64,
        version: u64,
    ) -> EngineResult<()> {
        if self.platform.workers.live.running.contains_key(&version) {
            return Ok(());
        }
        let registry = &mut self.platform.workers.registry;
        let found = registry
            .by_id(registration)
            .ok_or_else(|| EngineError::invalid("Unknown service worker registration."))?;
        let record = found.record();
        let worker = found
            .versions()
            .find(|v| v.id == version)
            .cloned()
            .ok_or_else(|| EngineError::invalid("Unknown service worker version."))?;
        let partition = if found.top_level.is_empty() {
            found.key.split(' ').nth(1).unwrap_or_default().to_string()
        } else {
            found.top_level.clone()
        };
        let policy = Policy {
            base: url::Url::parse(&worker.script_url)
                .map_err(|_| EngineError::invalid("Invalid service worker URL."))?,
            partition: url::Url::parse(&partition)
                .map_err(|_| EngineError::invalid("Invalid service worker partition."))?,
            csp: Vec::new(),
        };
        let module = found.script_type == "module";
        let lease = self.residency.reserve(false)?;
        let base = self
            .page
            .as_ref()
            .map(|page| page.scripting.environment.clone())
            .unwrap_or_default();
        let state = json!({"registration": record, "worker": worker.record()});
        let mut environment = environment(&base, &policy, state);
        environment.worker_module = module;
        let (sources, modules) = entry(module, &worker.script_url, worker.source.clone());
        let now = Instant::now();
        let running = Running {
            version,
            lease,
            sources: Arc::new(sources),
            environment,
            policy,
            token: None,
            requests: Vec::new(),
            parked: Vec::new(),
            preflights: Default::default(),
            next_timer: None,
            started: now,
            last_work: now,
            events: VecDeque::new(),
            outcomes: BTreeMap::new(),
            awaiting: 0,
            seen: 0,
            modules,
            module_requests: Vec::new(),
            ready: !module,
        };
        self.platform.workers.live.running.insert(version, running);
        Ok(())
    }

    /** Stop a running version, releasing its lease. */
    pub(in crate::runtime) fn stop_service_worker(&mut self, version: u64) {
        self.platform.workers.live.running.remove(&version);
    }

    /** Queue one functional event for a running version; returns its event id. An outcome is
    kept only for a `waited` event, one a lifecycle step or a page's fetch waits for. */
    pub(in crate::runtime) fn queue_service_worker_event(
        &mut self,
        version: u64,
        mut event: Value,
        waited: bool,
    ) -> Option<u64> {
        let live = &mut self.platform.workers.live;
        live.next_event += 1;
        let id = live.next_event;
        event["id"] = json!(id);
        let running = live.running.get_mut(&version)?;
        running.events.push_back(event);
        running.last_work = Instant::now();
        if waited {
            running.awaiting += 1;
            live.waited.insert(id);
        }
        Some(id)
    }

    /** Take the outcome of one event, when the worker reported it. */
    pub(in crate::runtime) fn take_outcome(
        &mut self,
        version: u64,
        event: u64,
    ) -> Option<(Value, Vec<u8>)> {
        let running = self.platform.workers.live.running.get_mut(&version)?;
        let outcome = running.outcomes.remove(&event)?;
        running.awaiting = running.awaiting.saturating_sub(1);
        self.platform.workers.live.waited.remove(&event);
        Some(outcome)
    }

    /** Run one version's turns until `done` says so or `until` passes; false on timeout. */
    pub(in crate::runtime) fn run_service_worker_until(
        &mut self,
        version: u64,
        until: Instant,
        mut done: impl FnMut(&Running) -> bool,
    ) -> EngineResult<bool> {
        loop {
            let Some(running) = self.platform.workers.live.running.get(&version) else {
                return Ok(false);
            };
            if done(running) {
                return Ok(true);
            }
            if Instant::now() >= until || !self.service_worker_turn(version, until)? {
                return Ok(false);
            }
        }
    }

    /** Give every running worker turns while it has work, then stop idle ones. */
    pub(in crate::runtime) fn run_service_workers(&mut self, until: Instant) -> EngineResult<()> {
        let versions: Vec<u64> = self.platform.workers.live.running.keys().copied().collect();
        let mut turns = 0;
        for version in versions {
            while turns < TURNS_PER_PASS
                && Instant::now() < until
                && self.service_worker_turn(version, until)?
            {
                turns += 1;
                self.service_worker_progress()?;
            }
        }
        self.service_worker_progress()?;
        let now = Instant::now();
        self.platform.workers.live.running.retain(|_, running| {
            running.awaiting > 0
                || !running.events.is_empty()
                || now.duration_since(running.last_work) < SW_IDLE
        });
        Ok(())
    }

    /** Whether any running worker has a turn to take. */
    pub(in crate::runtime) fn service_workers_busy(&self) -> bool {
        let version = self.platform.version;
        self.platform
            .workers
            .live
            .running
            .values()
            .any(|running| running.has_work(running.now_ms(), version))
    }

    /** One turn of one worker; false when it had nothing to do. */
    fn service_worker_turn(&mut self, version: u64, until: Instant) -> EngineResult<bool> {
        let Some(mut running) = self.platform.workers.live.running.remove(&version) else {
            return Ok(false);
        };
        let result = self.take_turn(&mut running, until);
        if result.as_ref().is_ok_and(|worked| *worked) {
            running.last_work = Instant::now();
        }
        match result {
            Ok(worked) => {
                self.platform.workers.live.running.insert(version, running);
                Ok(worked)
            }
            Err(error) if crate::runtime::worker_loading::fatal(&error) => Err(error),
            Err(error) => {
                // The worker is stopped; a pending lifecycle event then fails its job.
                let line = format!("[service worker] stopped: {}", error.message);
                self.platform.workers.live.console.push(line);
                Ok(false)
            }
        }
    }

    /** Choose and run the worker's next action. */
    fn take_turn(&mut self, running: &mut Running, until: Instant) -> EngineResult<bool> {
        let action = if running.token.is_none() {
            json!({"kind":"load"})
        } else if !running.module_requests.is_empty() {
            self.service_worker_modules(running, until)?;
            json!({"kind":"modules"})
        } else if let Some(responses) = self.answer_worker_requests(running, until)? {
            json!({"kind":"resume","responses":responses})
        } else if running
            .next_timer
            .is_some_and(|due| due <= running.now_ms())
        {
            json!({"kind":"timers","now_ms":running.now_ms(),"budget":TIMER_BUDGET})
        } else {
            return Ok(false);
        };
        let execution = self.service_worker_call(running, action)?;
        self.service_worker_effects(running, execution)?;
        Ok(true)
    }

    /** Answer the worker's requests and any parked ones that can now be; None when none can. */
    fn answer_worker_requests(
        &mut self,
        running: &mut Running,
        until: Instant,
    ) -> EngineResult<Option<Vec<HostResponse>>> {
        let mut requests = std::mem::take(&mut running.requests);
        let retry = !running.events.is_empty() || running.seen != self.platform.version;
        if retry {
            running.seen = self.platform.version;
            requests.extend(std::mem::take(&mut running.parked));
        }
        let mut responses = Vec::new();
        for request in requests {
            match self.worker_request(running, &request, until)? {
                Some(response) => responses.push(response),
                None => running.parked.push(request),
            }
        }
        Ok((!responses.is_empty()).then_some(responses))
    }

    /** One worker request: a fetch through the network rules, or a platform operation. */
    fn worker_request(
        &mut self,
        running: &mut Running,
        request: &HostRequest,
        until: Instant,
    ) -> EngineResult<Option<HostResponse>> {
        if request.kind != "platform" {
            let deadline =
                until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
            let base = running.policy.base.clone();
            let policy = running.policy.clone();
            return self
                .script_request(
                    &base,
                    request,
                    &mut running.preflights,
                    deadline,
                    Some(&policy),
                )
                .map(Some);
        }
        let (args, active) = parse_body(request.body.as_deref())?;
        match request.method.as_str() {
            "sw.next" => Ok((running.ready && !running.events.is_empty()).then(|| {
                let events: Vec<Value> = running.events.drain(..).collect();
                platform_answer(request.id, json!({"ok": events}))
            })),
            "sw.done" => {
                let event = args["event"].as_u64().unwrap_or_default();
                if self.platform.workers.live.waited.contains(&event) {
                    running
                        .outcomes
                        .insert(event, (args, request.body_bytes.clone()));
                }
                Ok(Some(platform_answer(request.id, json!({"ok": null}))))
            }
            _ => {
                let ctx = running.ctx(active);
                self.dispatch_platform(&ctx, request, &args)
            }
        }
    }

    /** One `v9.worker` call for the worker, importScripts fetched under its policy. */
    fn service_worker_call(
        &mut self,
        running: &mut Running,
        action: Value,
    ) -> EngineResult<Execution> {
        let identity = running.lease.identity.clone();
        let params = json!({"sources":&*running.sources,"modules":running.modules,
            "document":{"identity":identity,"charset":"UTF-8","nodes":[]},
            "environment":running.environment,"languages":self.languages,"resident":true,
            "resident_contexts":self.residency.snapshot(),"base":running.token,"keep":null,"action":action});
        let policy = running.policy.clone();
        self.call_worker(&params, None, |runtime, url, deadline| {
            Ok(runtime
                .worker_source(&policy, url, false, deadline)?
                .map(|source| source.text))
        })
    }

    /** Adopt one execution's state, requests, timers and diagnostics. */
    fn service_worker_effects(
        &mut self,
        running: &mut Running,
        execution: Execution,
    ) -> EngineResult<()> {
        running.token = execution.state_token;
        running.next_timer = execution.effects.next_timer_ms;
        running.module_requests = execution.module_requests;
        running.ready = execution.effects.worker_ready.unwrap_or(running.ready);
        crate::runtime::scripting::queue_requests(
            &mut running.requests,
            execution.effects.requests,
            &execution.effects.cancelled_requests,
        )?;
        let console = &mut self.platform.workers.live.console;
        let lines = execution
            .effects
            .console
            .into_iter()
            .map(|entry| entry.text);
        let failures = execution
            .failed
            .into_iter()
            .map(|failure| failure.error.message);
        for line in lines.chain(failures) {
            if console.len() < CONSOLE_LINES {
                console.push(format!("[service worker] {line}"));
            }
        }
        Ok(())
    }
}

/** The sources and module input a worker starts from: a classic script as its one source, or a
module as the graph's entry with an empty classic source list. */
fn entry(module: bool, url: &str, source: String) -> (Vec<String>, crate::v9::ModuleInput) {
    if !module {
        return (vec![source], crate::v9::ModuleInput::default());
    }
    let input = crate::v9::ModuleInput {
        entries: vec![crate::v9::ModuleEntry {
            index: 0,
            kind: crate::v9::EntryKind::Module,
            url: Some(url.into()),
        }],
        sources: vec![crate::v9::ModuleSource {
            url: url.into(),
            source: Some(source),
            classic: false,
        }],
    };
    (vec![String::new()], input)
}

impl Runtime {
    /** Fetch the imports a module worker asked for (same-origin rules and credentials). */
    fn service_worker_modules(
        &mut self,
        running: &mut Running,
        until: Instant,
    ) -> EngineResult<()> {
        let deadline = until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
        for request in std::mem::take(&mut running.module_requests) {
            if running
                .modules
                .sources
                .iter()
                .any(|known| known.url == request.url)
            {
                continue;
            }
            let credentials = crate::v9::WorkerCredentials::SameOrigin;
            let source = self.worker_module_source(
                &running.policy,
                &request.url,
                false,
                credentials,
                deadline,
            )?;
            running.modules.sources.push(crate::v9::ModuleSource {
                url: request.url,
                source: source.map(|source| source.text),
                classic: false,
            });
        }
        Ok(())
    }
}

/** A platform answer the coordinator gives a worker directly. */
fn platform_answer(id: u32, body: Value) -> HostResponse {
    HostResponse {
        id,
        status: 200,
        body: body.to_string(),
        ..HostResponse::default()
    }
}
