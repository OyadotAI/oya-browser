/** Dedicated worker execution preserves candidate ownership and bounded execution. */
use super::*;

impl Runtime {
    /** A retained worker executes one load, message, response batch or bounded timer task. */
    pub(super) fn worker_turn(
        &mut self,
        worker: &mut Context,
        now: f64,
        until: Instant,
    ) -> EngineResult<Execution> {
        let action = if worker.token.is_none() {
            json!({"kind":"load"})
        } else if !worker.modules.pending.is_empty() {
            self.worker_modules(worker, until)?;
            json!({"kind":"modules"})
        } else if let Some(event) = worker.next_event() {
            json!({"kind":"event","event":event})
        } else if !worker.requests.is_empty() {
            let mut responses = Vec::new();
            for request in std::mem::take(&mut worker.requests) {
                let deadline =
                    until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
                responses.push(self.script_request(
                    &worker.policy.base,
                    &request,
                    &mut worker.preflights,
                    deadline,
                    Some(&worker.policy),
                )?);
            }
            json!({"kind":"resume","responses":responses})
        } else {
            json!({"kind":"timers","now_ms":(now-worker.clock_origin).max(0.0),"budget":1})
        };
        let identity = &worker.lease.identity;
        let keep = self
            .page
            .as_ref()
            .and_then(|page| page.context_token(identity));
        let params = json!({"sources":&*worker.sources,"modules":worker.modules.input,"document":{"identity":identity,"charset":"UTF-8","nodes":[]},
            "environment":worker.environment,"languages":self.languages,"resident":true,"resident_contexts":self.residency.snapshot(),
            "base":worker.token,"keep":keep,"action":action});
        let started = Instant::now();
        self.call_worker(&params, None, |runtime, url, deadline| {
            let source = runtime.worker_source(&worker.policy, url, false, deadline)?;
            if let Some(source) = &source {
                worker.muted_errors |= source.policy.base.origin() != worker.policy.base.origin();
            }
            Ok(source.map(|source| source.text))
        })
        .inspect_err(|error| {
            super::super::worker_loading::execution_failure(
                started,
                super::super::worker_loading::Failure::execution(error),
            );
        })
    }

    /** Worker effects are accepted only on their candidate and translated into later owner tasks. */
    pub(super) fn worker_effects(
        &mut self,
        page: &mut Page,
        owner: &[u32],
        worker: &mut Context,
        execution: Execution,
    ) -> EngineResult<bool> {
        worker.token = execution.state_token;
        worker.ready = execution.effects.worker_ready.ok_or_else(|| {
            EngineError::invalid("Worker execution omitted initialization state.")
        })?;
        worker.modules.pending = execution.module_requests;
        worker.next_timer = execution.effects.next_timer_ms;
        crate::runtime::scripting::queue_requests(
            &mut worker.requests,
            execution.effects.requests,
            &execution.effects.cancelled_requests,
        )?;
        crate::runtime::scripting::record_console(
            &mut page.scripting,
            execution.effects.console,
            execution.effects.console_dropped,
        );
        let mut closed = worker.environment.worker_module
            && !worker.ready
            && !execution.effects.worker_errors.is_empty();
        let (id, parent) = owner.split_last().expect("worker has a native owner path");
        let mut commands = Vec::new();
        for command in execution.effects.worker_commands {
            match command {
                WorkerCommand::Post {
                    id: 0,
                    data,
                    ports,
                    moved_ports,
                } => {
                    page.scripting
                        .workers
                        .post(owner, parent, *id, data, ports, moved_ports)?;
                }
                WorkerCommand::Terminate { id: 0 } => closed = true,
                command => commands.push(command),
            }
        }
        if !closed {
            page.scripting.workers.append_from(owner, commands)?;
        }
        for message in execution.effects.port_messages {
            if !page.scripting.workers.port(owner, message.clone())? {
                worker.port_outbox.push(message);
            }
        }
        for link in execution.effects.port_closes {
            if !page.scripting.workers.close_port(owner, link)? {
                worker.port_closes.push(link);
            }
            worker.inbox.retain(|event| !matches!(event, PageEvent::PortMessage { link: value, .. } if *value == link));
        }
        if !execution.effects.worker_errors.is_empty() || !execution.failed.is_empty() {
            super::super::worker_loading::execution_failure(
                Instant::now(),
                super::super::worker_loading::Failure::Execution,
            );
        }
        for error in execution.effects.worker_errors {
            let message = if worker.muted_errors {
                "Script error."
            } else {
                &error
            };
            self.worker_error(page, parent, *id, message)?;
        }
        Ok(closed)
    }
}
